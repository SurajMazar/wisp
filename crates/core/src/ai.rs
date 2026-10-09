//! AI services on the core: provider registry, model management, routing and generation.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures::StreamExt;
use tokio_util::sync::CancellationToken;
use wisp_ai::{
    AiError, AiProvider, ChatMessage, Chunk, ChunkStream, GenerateRequest, Role, RouteInput,
};
use wisp_ipc::{
    IpcError,
    methods::ModelsPullProgress,
    types::{
        models::{
            ModelInfo, ModelRef, ModelSettings, ModelTestResult, ModelsList, ProviderInfo,
            PullProgress, RouteInfo,
        },
        settings::PrivacyMode,
    },
};
use wisp_providers::{
    HostBridge,
    apple::{AppleProvider, AppleStreams},
    ollama::OllamaProvider,
    openai::OpenAiCompatibleProvider,
};

use crate::events::EventBus;

/// Owns provider instances (rebuilt when model settings change) and in-flight model downloads.
#[derive(Default)]
pub struct AiServices {
    host: Mutex<Option<Arc<dyn HostBridge>>>,
    pub apple_streams: AppleStreams,
    ollama: Mutex<Option<(String, Arc<OllamaProvider>)>>,
    pulls: Mutex<HashMap<String, CancellationToken>>,
    /// Test-only providers that replace the real ones.
    overrides: Mutex<Option<Vec<Arc<dyn AiProvider>>>>,
}

pub struct Prepared {
    pub route: RouteInfo,
    pub stream: ChunkStream,
}

impl AiServices {
    pub fn attach_host(&self, host: Arc<dyn HostBridge>) {
        if let Ok(mut h) = self.host.lock() {
            *h = Some(host);
        }
    }

    pub fn detach_host(&self) {
        if let Ok(mut h) = self.host.lock() {
            *h = None;
        }
    }

    pub fn set_overrides(&self, providers: Vec<Arc<dyn AiProvider>>) {
        if let Ok(mut o) = self.overrides.lock() {
            *o = Some(providers);
        }
    }

    pub fn ollama(&self, settings: &ModelSettings) -> Arc<OllamaProvider> {
        let mut slot = match self.ollama.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some((_, p)) = slot.as_ref().filter(|(url, _)| *url == settings.ollama_url) {
            return p.clone();
        }
        let p = Arc::new(OllamaProvider::new(settings.ollama_url.clone()));
        *slot = Some((settings.ollama_url.clone(), p.clone()));
        p
    }

    pub fn providers(&self, settings: &ModelSettings) -> Vec<Arc<dyn AiProvider>> {
        if let Some(o) = self.overrides.lock().ok().and_then(|o| o.clone()) {
            return o;
        }
        let mut out: Vec<Arc<dyn AiProvider>> = vec![];
        let host = self.host.lock().ok().and_then(|h| h.clone());
        if let Some(h) = &host {
            out.push(Arc::new(AppleProvider::new(
                h.clone(),
                self.apple_streams.clone(),
            )));
        }
        out.push(self.ollama(settings));
        if let Some(h) = host {
            for e in &settings.endpoints {
                out.push(Arc::new(OpenAiCompatibleProvider::new(
                    e.clone(),
                    h.clone(),
                )));
            }
        }
        out
    }

    /// Live status and models of every provider, probed concurrently with a time budget.
    pub async fn catalog(&self, settings: &ModelSettings) -> Vec<(ProviderInfo, Vec<ModelInfo>)> {
        let providers = self.providers(settings);
        let probes = providers.iter().map(|p| async move {
            let info = p.info().await;
            let models = if info.state == wisp_ipc::types::models::ProviderState::Available {
                tokio::time::timeout(Duration::from_secs(6), p.models())
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or_default()
            } else {
                vec![]
            };
            (info, models)
        });
        futures::future::join_all(probes).await
    }

    pub async fn list(&self, settings: &ModelSettings) -> ModelsList {
        let catalog = self.catalog(settings).await;
        let mut providers = vec![];
        let mut models = vec![];
        for (p, m) in catalog {
            providers.push(p);
            models.extend(m);
        }
        ModelsList { providers, models }
    }

    pub async fn route(
        &self,
        settings: &ModelSettings,
        privacy: PrivacyMode,
        needs_vision: bool,
        allow_cloud: bool,
        estimated_prompt_tokens: u32,
        override_model: Option<&ModelRef>,
    ) -> Result<(RouteInfo, Arc<dyn AiProvider>), IpcError> {
        let catalog = self.catalog(settings).await;
        let preference = override_model.or(if needs_vision {
            settings.vision.as_ref()
        } else {
            settings.general.as_ref()
        });
        let route = wisp_ai::route(&RouteInput {
            providers: &catalog,
            privacy,
            needs_vision,
            allow_cloud,
            preference,
            estimated_prompt_tokens,
        })?;
        let provider = self
            .providers(settings)
            .into_iter()
            .find(|p| p.id() == route.model.provider_id)
            .ok_or_else(|| {
                IpcError::from(AiError::NoModel("That model's provider went away.".into()))
            })?;
        Ok((route, provider))
    }

    /// Route and start generating.
    #[allow(clippy::too_many_arguments)]
    pub async fn generate(
        &self,
        settings: &ModelSettings,
        privacy: PrivacyMode,
        allow_cloud: bool,
        override_model: Option<&ModelRef>,
        system: Option<String>,
        messages: Vec<ChatMessage>,
        cancel: CancellationToken,
    ) -> Result<Prepared, IpcError> {
        let needs_vision = messages.iter().any(|m| !m.images.is_empty());
        let tokens = estimate_tokens(system.as_deref(), &messages);
        let (route, provider) = self
            .route(
                settings,
                privacy,
                needs_vision,
                allow_cloud,
                tokens,
                override_model,
            )
            .await?;
        let req = GenerateRequest {
            model: route.model.model.clone(),
            system,
            messages,
            temperature: settings.temperature,
            max_tokens: settings.max_tokens,
            context_length: settings.context_length,
        };
        let stream = provider.generate(req, cancel).await?;
        Ok(Prepared { route, stream })
    }

    pub async fn test(
        &self,
        settings: &ModelSettings,
        privacy: PrivacyMode,
        model: Option<ModelRef>,
        prompt: Option<String>,
    ) -> Result<ModelTestResult, IpcError> {
        let prompt = prompt.unwrap_or_else(|| "What is 2 + 2? Answer with just the number.".into());
        let started = Instant::now();
        let prepared = self
            .generate(
                settings,
                privacy,
                false,
                model.as_ref(),
                Some("You are a concise assistant.".into()),
                vec![ChatMessage::text(Role::User, prompt)],
                CancellationToken::new(),
            )
            .await?;
        let mut first: Option<Duration> = None;
        let mut text = String::new();
        let mut stream = prepared.stream;
        while let Some(c) = stream.next().await {
            match c? {
                Chunk::Text(t) => {
                    first.get_or_insert(started.elapsed());
                    text.push_str(&t);
                }
                Chunk::Done(_) => break,
            }
        }
        let ms = |d: Duration| d.as_millis().min(u32::MAX as u128) as u32;
        Ok(ModelTestResult {
            model: prepared.route.model,
            locality: prepared.route.locality,
            text: text.trim().to_owned(),
            first_token_ms: ms(first.unwrap_or_else(|| started.elapsed())),
            total_ms: ms(started.elapsed()),
        })
    }

    pub fn start_pull(
        self: &Arc<Self>,
        settings: &ModelSettings,
        name: String,
        events: EventBus,
    ) -> Result<(), IpcError> {
        wisp_providers::ollama::validate_model_name(&name)?;
        let token = CancellationToken::new();
        {
            let mut pulls = self.pulls.lock().map_err(|_| IpcError::internal("lock"))?;
            if pulls.contains_key(&name) {
                return Ok(()); // already downloading
            }
            pulls.insert(name.clone(), token.clone());
        }
        let ollama = self.ollama(settings);
        let me = self.clone();
        tokio::spawn(async move {
            let ev = events.clone();
            let mut last = Instant::now() - Duration::from_secs(1);
            let result = ollama
                .pull(&name, token, |p| {
                    // Throttle progress events to ~4/s.
                    if p.done || last.elapsed() >= Duration::from_millis(250) {
                        last = Instant::now();
                        ev.publish::<ModelsPullProgress>(&p);
                    }
                })
                .await;
            if let Ok(mut pulls) = me.pulls.lock() {
                pulls.remove(&name);
            }
            let final_event = match result {
                Ok(()) => PullProgress {
                    name: name.clone(),
                    status: "success".into(),
                    completed: None,
                    total: None,
                    done: true,
                    error: None,
                },
                Err(e) => PullProgress {
                    name: name.clone(),
                    status: if e == AiError::Cancelled {
                        "cancelled".into()
                    } else {
                        "error".into()
                    },
                    completed: None,
                    total: None,
                    done: true,
                    error: (e != AiError::Cancelled).then(|| e.to_string()),
                },
            };
            events.publish::<ModelsPullProgress>(&final_event);
        });
        Ok(())
    }

    pub fn cancel_pull(&self, name: &str) {
        if let Some(t) = self.pulls.lock().ok().and_then(|p| p.get(name).cloned()) {
            t.cancel();
        }
    }
}

/// ~4 characters per token, plus a fixed cost per image.
pub fn estimate_tokens(system: Option<&str>, messages: &[ChatMessage]) -> u32 {
    let chars: usize =
        system.map(str::len).unwrap_or(0) + messages.iter().map(|m| m.content.len()).sum::<usize>();
    let images: usize = messages.iter().map(|m| m.images.len()).sum();
    (chars / 4 + images * 1000) as u32
}
