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
    /// Providers that just failed are skipped for a while (spec §99 fallback).
    cooldown: Mutex<HashMap<String, Instant>>,
    /// Whether the privacy mode allows cloud at all; until known, assume Local Only (the default).
    cloud_permitted: std::sync::atomic::AtomicBool,
    /// Recent first-token latency per model (exponential average, ms), measured on this Mac.
    first_token_ms: Mutex<HashMap<(String, String), f64>>,
}

/// A model whose recent first-token average exceeds this is treated as slow by the router.
const SLOW_FIRST_TOKEN_MS: f64 = 4000.0;

const COOLDOWN: Duration = Duration::from_secs(120);

pub struct Prepared {
    pub route: RouteInfo,
    pub stream: ChunkStream,
}

impl AiServices {
    /// Record how long a model took to start answering (feeds routing).
    pub fn record_first_token(&self, model: &ModelRef, ms: u64) {
        if let Ok(mut m) = self.first_token_ms.lock() {
            let e = m
                .entry((model.provider_id.clone(), model.model.clone()))
                .or_insert(ms as f64);
            *e = *e * 0.6 + ms as f64 * 0.4;
        }
    }

    fn slow_models(&self) -> Vec<ModelRef> {
        self.first_token_ms
            .lock()
            .map(|m| {
                m.iter()
                    .filter(|(_, ms)| **ms > SLOW_FIRST_TOKEN_MS)
                    .map(|((p, id), _)| ModelRef {
                        provider_id: p.clone(),
                        model: id.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Remember the privacy mode so Local Only never contacts cloud endpoints.
    pub fn set_privacy(&self, privacy: PrivacyMode) {
        self.cloud_permitted.store(
            privacy != PrivacyMode::LocalOnly,
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    pub fn host(&self) -> Option<Arc<dyn HostBridge>> {
        self.host.lock().ok().and_then(|h| h.clone())
    }

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
                // In Local Only, cloud endpoints aren't even contacted (not even to list models).
                if !self
                    .cloud_permitted
                    .load(std::sync::atomic::Ordering::Relaxed)
                    && wisp_providers::openai::locality_of(&e.base_url)
                        == wisp_ipc::types::models::Locality::Cloud
                {
                    continue;
                }
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
            let mut info = p.info().await;
            if self.cooling_down(&info.id)
                && info.state == wisp_ipc::types::models::ProviderState::Available
            {
                info.state = wisp_ipc::types::models::ProviderState::Unavailable;
                info.detail =
                    Some("Failed a moment ago; Wisp is using another model for now.".into());
            }
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
        self.set_privacy(privacy);
        let catalog = self.catalog(settings).await;
        let preference = override_model.or(if needs_vision {
            settings.vision.as_ref()
        } else {
            settings.general.as_ref()
        });
        let slow = self.slow_models();
        let route = wisp_ai::route(&RouteInput {
            providers: &catalog,
            privacy,
            needs_vision,
            allow_cloud,
            preference,
            estimated_prompt_tokens,
            slow: &slow,
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

    fn cooling_down(&self, provider_id: &str) -> bool {
        self.cooldown
            .lock()
            .ok()
            .and_then(|c| c.get(provider_id).copied())
            .is_some_and(|t| t.elapsed() < COOLDOWN)
    }

    fn mark_failed(&self, provider_id: &str) {
        if let Ok(mut c) = self.cooldown.lock() {
            c.insert(provider_id.to_owned(), Instant::now());
        }
    }

    /// Process the start of a prompt so a request that begins the same way is answered sooner:
    /// local models keep that work and reuse it. Returns whether anything was prepared. Never used
    /// with cloud models (it would cost money and they cache differently) or Apple's model.
    pub async fn prefill(
        &self,
        settings: &ModelSettings,
        privacy: PrivacyMode,
        system: Option<String>,
        messages: Vec<ChatMessage>,
        cancel: CancellationToken,
    ) -> Result<bool, IpcError> {
        let needs_vision = messages.iter().any(|m| !m.images.is_empty());
        let tokens = estimate_tokens(system.as_deref(), &messages);
        let (mut route, provider) = self
            .route(settings, privacy, needs_vision, false, tokens, None)
            .await?;
        if route.locality != wisp_ipc::types::models::Locality::Local
            || provider.kind() == wisp_ipc::types::models::ProviderKind::Apple
        {
            return Ok(false);
        }
        swap_for_twin(settings, &mut route, provider.as_ref()).await;
        let req = GenerateRequest {
            model: route.model.model.clone(),
            system,
            messages,
            temperature: settings.temperature,
            max_tokens: 1,
            context_length: settings.context_length,
            think: settings.deep_thinking,
        };
        let mut stream = provider.generate(req, cancel).await?;
        while let Some(item) = stream.next().await {
            item?;
        }
        Ok(true)
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
        on_route: &(dyn Fn(&RouteInfo, Option<bool>) + Send + Sync),
    ) -> Result<Prepared, IpcError> {
        let needs_vision = messages.iter().any(|m| !m.images.is_empty());
        let tokens = estimate_tokens(system.as_deref(), &messages);
        // Try the routed model; if it fails before producing anything, fall back to the next
        // eligible one (never crossing the privacy boundary — routing enforces that each time).
        let mut last_err: Option<IpcError> = None;
        for _attempt in 0..3 {
            let (route, provider) = match self
                .route(
                    settings,
                    privacy,
                    needs_vision,
                    allow_cloud,
                    tokens,
                    override_model,
                )
                .await
            {
                Ok(r) => r,
                Err(e) => return Err(last_err.unwrap_or(e)),
            };
            let mut route = route;
            swap_for_twin(settings, &mut route, provider.as_ref()).await;
            // Report the choice before the (possibly long) model load so the UI can explain the wait.
            on_route(&route, provider.is_loaded(&route.model.model).await);
            let req = GenerateRequest {
                model: route.model.model.clone(),
                system: system.clone(),
                messages: messages.clone(),
                temperature: settings.temperature,
                max_tokens: settings.max_tokens,
                context_length: settings.context_length,
                think: settings.deep_thinking,
            };
            let mut stream = match provider.generate(req, cancel.clone()).await {
                Ok(s) => s,
                Err(AiError::Cancelled) => return Err(AiError::Cancelled.into()),
                Err(e) => {
                    tracing::warn!(provider = %route.model.provider_id, error = %e, "provider failed; trying another");
                    self.mark_failed(&route.model.provider_id);
                    last_err = Some(e.into());
                    if override_model.is_some() {
                        break;
                    }
                    continue;
                }
            };
            // Peek the first item: a provider that errors before any text gets replaced.
            match stream.next().await {
                Some(Err(AiError::Cancelled)) => return Err(AiError::Cancelled.into()),
                Some(Err(e)) => {
                    tracing::warn!(provider = %route.model.provider_id, error = %e, "provider failed before answering; trying another");
                    self.mark_failed(&route.model.provider_id);
                    last_err = Some(e.into());
                    if override_model.is_some() {
                        break;
                    }
                    continue;
                }
                Some(Ok(first)) => {
                    let stream: ChunkStream =
                        Box::pin(futures::stream::once(async move { Ok(first) }).chain(stream));
                    return Ok(Prepared { route, stream });
                }
                None => {
                    let stream: ChunkStream = Box::pin(futures::stream::empty());
                    return Ok(Prepared { route, stream });
                }
            }
        }
        Err(last_err.unwrap_or_else(|| {
            AiError::NoModel("No AI model could answer right now.".into()).into()
        }))
    }

    /// Whether an image can be sent: some allowed model can see, and if the user picked a model,
    /// it's that one.
    pub async fn can_see(
        &self,
        settings: &ModelSettings,
        privacy: PrivacyMode,
        allow_cloud: bool,
        override_model: Option<&ModelRef>,
    ) -> bool {
        match self
            .route(settings, privacy, true, allow_cloud, 0, override_model)
            .await
        {
            Ok((r, _)) => override_model.is_none_or(|o| *o == r.model),
            Err(_) => false,
        }
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
                &|_, _| {},
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
                Chunk::Reasoning(_) => {}
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

/// With deep thinking off, use a model's non-thinking twin when it's installed: some models
/// (e.g. `qwen3-vl:8b`) think even when asked not to, and their instruct twin answers directly.
async fn swap_for_twin(settings: &ModelSettings, route: &mut RouteInfo, provider: &dyn AiProvider) {
    if settings.deep_thinking {
        return;
    }
    let Some(twin) = non_thinking_twin(&route.model.model) else {
        return;
    };
    if provider
        .models()
        .await
        .is_ok_and(|ms| ms.iter().any(|m| m.id == twin))
    {
        route.model_name = twin.clone();
        route.model.model = twin;
    }
}

/// The non-thinking sibling of a model that reasons by default, as Ollama names them:
/// `qwen3-vl:8b` / `qwen3-vl:8b-thinking` → `qwen3-vl:8b-instruct`.
fn non_thinking_twin(model: &str) -> Option<String> {
    let (name, tag) = model.split_once(':')?;
    if tag.contains("instruct") {
        return None;
    }
    let base = tag.strip_suffix("-thinking").unwrap_or(tag);
    Some(format!("{name}:{base}-instruct"))
}

#[cfg(test)]
mod twin_tests {
    use super::non_thinking_twin;

    #[test]
    fn finds_the_instruct_twin() {
        assert_eq!(
            non_thinking_twin("qwen3-vl:8b").as_deref(),
            Some("qwen3-vl:8b-instruct")
        );
        assert_eq!(
            non_thinking_twin("qwen3-vl:8b-thinking").as_deref(),
            Some("qwen3-vl:8b-instruct")
        );
        assert_eq!(non_thinking_twin("qwen3-vl:8b-instruct"), None);
        assert_eq!(non_thinking_twin("llama3"), None);
    }
}
