//! Voice service (spec §39–48): push-to-talk audio → VAD → whisper.cpp → question;
//! streamed answers → sentences → Kokoro → speech chunks the host plays in order.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use base64::Engine;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wisp_audio::{models, stt::WhisperStt, text::SentenceSplitter, tts::KokoroTts, vad};
use wisp_ipc::{
    Envelope, IpcError,
    methods::{SpeechChunkEvent, SpeechModelProgressEvent},
    types::{
        conversations::{MessageStatus, SendParams},
        voice::{
            NoSpeechReason, SpeechChunk, SpeechModelInfo, SpeechModelProgress, VoiceFinishParams,
            VoiceFinishResult, VoiceSettings, VoiceStatus,
        },
    },
};

use crate::Core;

const MAX_SESSION_SAMPLES: usize = vad::SAMPLE_RATE * 60;

struct Session {
    pcm: Vec<i16>,
    started: Instant,
}

enum SpeakJob {
    Sentence {
        utterance: String,
        seq: u32,
        text: String,
        last: bool,
    },
}

#[derive(Default)]
pub struct VoiceService {
    sessions: Mutex<HashMap<String, Session>>,
    stt: Mutex<Option<(String, Arc<WhisperStt>)>>,
    tts: Mutex<Option<Arc<KokoroTts>>>,
    downloads: Mutex<HashMap<String, CancellationToken>>,
    /// Assistant message ids whose answers should be spoken, with their sentence splitters.
    speaking: Mutex<HashMap<String, (SentenceSplitter, u32)>>,
    queue: Mutex<Option<mpsc::UnboundedSender<SpeakJob>>>,
    /// Bumped by "stop speaking" so queued jobs from before are dropped.
    generation: Mutex<u64>,
}

impl VoiceService {
    pub fn append(&self, session_id: &str, pcm_b64: &str) {
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(pcm_b64) else {
            return;
        };
        let samples = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b));
        if let Ok(mut s) = self.sessions.lock() {
            // Bound the number of concurrent sessions and their size.
            if !s.contains_key(session_id) && s.len() >= 4 {
                return;
            }
            let session = s.entry(session_id.to_owned()).or_insert_with(|| Session {
                pcm: vec![],
                started: Instant::now(),
            });
            if session.pcm.len() < MAX_SESSION_SAMPLES {
                session
                    .pcm
                    .extend(samples.take(MAX_SESSION_SAMPLES - session.pcm.len()));
            }
            // Drop stale sessions (host crashed mid-recording).
            s.retain(|_, v| v.started.elapsed() < Duration::from_secs(120));
        }
    }

    fn take(&self, session_id: &str) -> Vec<i16> {
        self.sessions
            .lock()
            .ok()
            .and_then(|mut s| s.remove(session_id))
            .map(|s| s.pcm)
            .unwrap_or_default()
    }

    pub fn cancel(&self, session_id: &str) {
        self.take(session_id);
    }
}

fn models_root(core: &Core) -> PathBuf {
    core.data_dir().join("models")
}

impl Core {
    fn voice(&self) -> &VoiceService {
        self.inner_voice()
    }

    fn whisper(&self, settings: &VoiceSettings) -> Result<Arc<WhisperStt>, IpcError> {
        if let Some((_, s)) = self
            .voice()
            .stt
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .filter(|(id, _)| *id == settings.stt_model)
        {
            return Ok(s);
        }
        let m = models::find(&settings.stt_model)
            .filter(|m| m.kind == "stt")
            .ok_or_else(|| IpcError::invalid_params("unknown speech model"))?;
        let root = models_root(self);
        if !models::installed(&root, &m) {
            return Err(IpcError::new(
                wisp_ipc::ErrorCode::NotFound,
                format!("{} isn't downloaded yet. Get it in Voice settings.", m.name),
            ));
        }
        let path = models::model_dir(&root, &m.id).join(&m.files[0].path);
        let stt = Arc::new(WhisperStt::load(&path).map_err(|e| {
            IpcError::new(
                wisp_ipc::ErrorCode::Unavailable,
                "The speech model couldn't be loaded.",
            )
            .with_details(e.to_string())
        })?);
        if let Ok(mut g) = self.voice().stt.lock() {
            *g = Some((settings.stt_model.clone(), stt.clone()));
        }
        Ok(stt)
    }

    fn kokoro(&self) -> Result<Arc<KokoroTts>, IpcError> {
        if let Some(t) = self.voice().tts.lock().ok().and_then(|g| g.clone()) {
            return Ok(t);
        }
        let m = models::find("kokoro-82m")
            .ok_or_else(|| IpcError::internal("kokoro missing from manifest"))?;
        let root = models_root(self);
        if !models::installed(&root, &m) {
            return Err(IpcError::new(
                wisp_ipc::ErrorCode::NotFound,
                "The Kokoro voice isn't downloaded yet. Get it in Voice settings.",
            ));
        }
        let t = Arc::new(
            KokoroTts::load(&models::model_dir(&root, &m.id))
                .map_err(|e| IpcError::new(wisp_ipc::ErrorCode::Unavailable, e.to_string()))?,
        );
        if let Ok(mut g) = self.voice().tts.lock() {
            *g = Some(t.clone());
        }
        Ok(t)
    }

    /// Load the speech models in the background so the first push-to-talk is instant.
    pub fn warm_up_voice(&self) {
        let core = self.clone();
        tokio::spawn(async move {
            let Ok(settings) = core.blocking(|db| db.load_settings()).await else {
                return;
            };
            if !settings.voice.enabled {
                return;
            }
            let c = core.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let started = Instant::now();
                if c.whisper(&settings.voice).is_ok() {
                    tracing::info!(
                        ms = started.elapsed().as_millis() as u64,
                        "speech recognition ready"
                    );
                }
                if settings.voice.speak_answers != wisp_ipc::types::voice::SpeakAnswers::Never
                    && c.kokoro().is_ok()
                {
                    tracing::info!("voice ready");
                }
            })
            .await;
        });
    }

    pub(crate) async fn voice_status(&self) -> Result<VoiceStatus, IpcError> {
        let settings = self.blocking(|db| db.load_settings()).await?.voice;
        let root = models_root(self);
        let list: Vec<SpeechModelInfo> = models::catalog()
            .into_iter()
            .map(|m| SpeechModelInfo {
                installed: models::installed(&root, &m),
                size_bytes: models::total_size(&m),
                id: m.id,
                name: m.name,
                kind: m.kind,
                preset: m.preset,
                languages: m.languages,
                license: m.license,
                voices: m.voices,
            })
            .collect();
        let stt_ready = list
            .iter()
            .any(|m| m.id == settings.stt_model && m.installed);
        let phonemizer = wisp_audio::tts::find_espeak().is_some();
        let tts_ready = phonemizer && list.iter().any(|m| m.kind == "tts" && m.installed);
        let detail = if !phonemizer {
            Some("Kokoro needs espeak-ng for pronunciation. Install it with Homebrew: brew install espeak-ng".into())
        } else if !stt_ready {
            Some("Download a speech recognition model to talk to Wisp.".into())
        } else {
            None
        };
        Ok(VoiceStatus {
            models: list,
            stt_ready,
            tts_ready,
            phonemizer,
            detail,
        })
    }

    pub(crate) async fn voice_finish(
        &self,
        p: VoiceFinishParams,
    ) -> Result<VoiceFinishResult, IpcError> {
        let pcm = self.voice().take(&p.session_id);
        let audio_ms = (pcm.len() * 1000 / vad::SAMPLE_RATE) as u32;
        let settings = self.blocking(|db| db.load_settings()).await?;
        let cfg = vad::VadConfig {
            sensitivity: settings.voice.vad_sensitivity as f32,
            ..Default::default()
        };
        let detected = vad::detect(&pcm, cfg);
        let Some(range) = detected.range else {
            let reason = if detected.speech_ms > 0 {
                NoSpeechReason::TooShort
            } else {
                NoSpeechReason::Silence
            };
            return Ok(VoiceFinishResult {
                transcript: None,
                no_speech: Some(reason),
                sent: None,
                audio_ms,
                stt_ms: 0,
            });
        };
        let stt = self.whisper(&settings.voice)?;
        let lang = settings.voice.language.clone();
        let started = Instant::now();
        let speech = pcm[range].to_vec();
        let text = tokio::task::spawn_blocking(move || stt.transcribe(&speech, &lang))
            .await
            .map_err(|e| IpcError::internal(e.to_string()))?
            .map_err(|e| {
                IpcError::new(
                    wisp_ipc::ErrorCode::Internal,
                    "I couldn't understand that recording.",
                )
                .with_details(e.to_string())
            })?;
        let stt_ms = started.elapsed().as_millis() as u32;
        let _ = self
            .blocking(move |db| {
                db.record_diagnostic(
                    "audio",
                    "transcribe",
                    None,
                    None,
                    Some(stt_ms as i64),
                    (None, None),
                    serde_json::json!({ "audioMs": audio_ms }),
                )
            })
            .await;
        if text.trim().is_empty() {
            return Ok(VoiceFinishResult {
                transcript: None,
                no_speech: Some(NoSpeechReason::Silence),
                sent: None,
                audio_ms,
                stt_ms,
            });
        }
        if p.transcribe_only {
            return Ok(VoiceFinishResult {
                transcript: Some(text),
                no_speech: None,
                sent: None,
                audio_ms,
                stt_ms,
            });
        }
        let sent = self
            .send(SendParams {
                teach: false,
                conversation_id: p.conversation_id,
                text: text.clone(),
                include_focus: p.include_focus,
                allow_cloud: false,
                model: None,
            })
            .await?;
        if settings.voice.speak_answers != wisp_ipc::types::voice::SpeakAnswers::Never {
            self.speak_message(&sent.assistant_message.id);
        }
        Ok(VoiceFinishResult {
            transcript: Some(text),
            no_speech: None,
            sent: Some(sent),
            audio_ms,
            stt_ms,
        })
    }

    /// Speak an assistant message as it streams in.
    pub(crate) fn speak_message(&self, message_id: &str) {
        if let Ok(mut s) = self.voice().speaking.lock() {
            s.insert(message_id.to_owned(), (SentenceSplitter::default(), 0));
        }
    }

    pub(crate) fn speak_text(&self, text: &str) -> Result<(), IpcError> {
        let text = wisp_audio::text::speakable(text);
        if text.is_empty() || text.chars().count() > 2000 {
            return Err(IpcError::invalid_params("text must be 1–2000 characters"));
        }
        self.kokoro()?;
        self.enqueue(SpeakJob::Sentence {
            utterance: wisp_common::ids::new_id(),
            seq: 0,
            text,
            last: true,
        });
        Ok(())
    }

    pub(crate) fn stop_speaking(&self) {
        if let Ok(mut g) = self.voice().generation.lock() {
            *g += 1;
        }
        if let Ok(mut s) = self.voice().speaking.lock() {
            s.clear();
        }
    }

    fn enqueue(&self, job: SpeakJob) {
        let mut q = match self.voice().queue.lock() {
            Ok(q) => q,
            Err(p) => p.into_inner(),
        };
        if q.is_none() {
            let (tx, rx) = mpsc::unbounded_channel();
            *q = Some(tx);
            let core = self.clone();
            tokio::spawn(async move { core.speech_worker(rx).await });
        }
        if let Some(tx) = q.as_ref() {
            let _ = tx.send(job);
        }
    }

    /// Single worker: synthesize sentences in order so playback order is guaranteed.
    async fn speech_worker(&self, mut rx: mpsc::UnboundedReceiver<SpeakJob>) {
        let dir = self.data_dir().join("cache/tts");
        let _ = std::fs::create_dir_all(&dir);
        while let Some(SpeakJob::Sentence {
            utterance,
            seq,
            text,
            last,
        }) = rx.recv().await
        {
            let generation = self.voice().generation.lock().map(|g| *g).unwrap_or(0);
            let settings = match self.blocking(|db| db.load_settings()).await {
                Ok(s) => s.voice,
                Err(_) => continue,
            };
            let Ok(tts) = self.kokoro() else { continue };
            let path = dir.join(format!("{utterance}-{seq}.wav"));
            let p2 = path.clone();
            let speak = text.clone();
            let ok = tokio::task::spawn_blocking(move || {
                let audio = tts.synthesize(&speak, &settings.voice, settings.speed as f32)?;
                wisp_audio::tts::write_wav(&p2, &audio, wisp_audio::tts::SAMPLE_RATE)
            })
            .await;
            // Stopped while synthesizing → drop it.
            if self.voice().generation.lock().map(|g| *g).unwrap_or(0) != generation {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            match ok {
                Ok(Ok(())) => self.events().publish::<SpeechChunkEvent>(&SpeechChunk {
                    utterance_id: utterance,
                    seq,
                    path: path.display().to_string(),
                    text,
                    last,
                }),
                other => {
                    tracing::warn!(result = ?other.map(|r| r.map_err(|e| e.to_string())), "speech synthesis failed")
                }
            }
            // Keep the cache small: remove speech older than ten minutes.
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    if e.metadata()
                        .and_then(|m| m.modified())
                        .map(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(600))
                        .unwrap_or(false)
                    {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
        }
    }

    /// Feed streamed answers into the speech queue (called for every core event).
    pub(crate) fn on_event_for_speech(&self, env: &Envelope) {
        let Envelope::Event { event, payload, .. } = env else {
            return;
        };
        match event.as_str() {
            "conversation.delta" => {
                let (Some(id), Some(delta)) =
                    (payload["messageId"].as_str(), payload["delta"].as_str())
                else {
                    return;
                };
                let sentences = {
                    let Ok(mut s) = self.voice().speaking.lock() else {
                        return;
                    };
                    let Some((splitter, seq)) = s.get_mut(id) else {
                        return;
                    };
                    splitter
                        .push(delta)
                        .into_iter()
                        .map(|t| {
                            let n = *seq;
                            *seq += 1;
                            (n, t)
                        })
                        .collect::<Vec<_>>()
                };
                for (seq, t) in sentences {
                    let t = wisp_audio::text::speakable(&t);
                    if !t.is_empty() {
                        self.enqueue(SpeakJob::Sentence {
                            utterance: id.to_owned(),
                            seq,
                            text: t,
                            last: false,
                        });
                    }
                }
            }
            "conversation.messageUpdated" => {
                let Some(id) = payload["id"].as_str() else {
                    return;
                };
                if payload["status"] == "streaming" {
                    return;
                }
                let Some((mut splitter, seq)) = self
                    .voice()
                    .speaking
                    .lock()
                    .ok()
                    .and_then(|mut s| s.remove(id))
                else {
                    return;
                };
                let complete = serde_json::from_value::<MessageStatus>(payload["status"].clone())
                    .ok()
                    == Some(MessageStatus::Complete);
                let rest = splitter
                    .finish()
                    .map(|t| wisp_audio::text::speakable(&t))
                    .filter(|t| !t.is_empty() && complete);
                // Always send a final chunk so the host knows the utterance ended.
                self.enqueue(SpeakJob::Sentence {
                    utterance: id.to_owned(),
                    seq,
                    text: rest.unwrap_or_default(),
                    last: true,
                });
            }
            _ => {}
        }
    }

    pub(crate) fn start_model_download(&self, id: String) -> Result<(), IpcError> {
        let m = models::find(&id).ok_or_else(|| IpcError::invalid_params("unknown model"))?;
        let token = CancellationToken::new();
        {
            let mut d = self
                .voice()
                .downloads
                .lock()
                .map_err(|_| IpcError::internal("lock"))?;
            if d.contains_key(&id) {
                return Ok(());
            }
            d.insert(id.clone(), token.clone());
        }
        let core = self.clone();
        let root = models_root(self);
        tokio::spawn(async move {
            let total = models::total_size(&m);
            let mut last = Instant::now() - Duration::from_secs(1);
            let ev = core.events().clone();
            let id2 = id.clone();
            let result = models::download(&root, &id, token, |done, total| {
                if last.elapsed() > Duration::from_millis(250) {
                    last = Instant::now();
                    ev.publish::<SpeechModelProgressEvent>(&SpeechModelProgress {
                        id: id2.clone(),
                        completed: done,
                        total,
                        done: false,
                        error: None,
                    });
                }
            })
            .await;
            if let Ok(mut d) = core.voice().downloads.lock() {
                d.remove(&id);
            }
            let error = match result {
                Ok(()) => None,
                Err(models::DownloadError::Cancelled) => Some("Paused".into()),
                Err(e) => Some(e.to_string()),
            };
            core.events()
                .publish::<SpeechModelProgressEvent>(&SpeechModelProgress {
                    id,
                    completed: total,
                    total,
                    done: true,
                    error,
                });
        });
        Ok(())
    }

    pub(crate) fn cancel_model_download(&self, id: &str) {
        if let Some(t) = self
            .voice()
            .downloads
            .lock()
            .ok()
            .and_then(|d| d.get(id).cloned())
        {
            t.cancel();
        }
    }

    pub(crate) fn remove_model(&self, id: &str) -> Result<(), IpcError> {
        if let Ok(mut s) = self.voice().stt.lock()
            && s.as_ref().is_some_and(|(m, _)| m == id)
        {
            *s = None;
        }
        if id == "kokoro-82m"
            && let Ok(mut t) = self.voice().tts.lock()
        {
            *t = None;
        }
        models::remove(&models_root(self), id).map_err(|e| IpcError::internal(e.to_string()))
    }
}
