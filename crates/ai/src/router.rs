//! Model router (spec §13, §99): pick the model for a request from what's actually available,
//! the privacy mode, the user's preference and what the task needs.
//!
//! Pure function over data so every privacy rule is unit-tested. Local Only can never select a
//! cloud model — not as a preference, not as a fallback.

use wisp_ipc::types::{
    models::{
        Locality, ModelCapability, ModelInfo, ModelRef, ProviderInfo, ProviderKind, ProviderState,
        RouteInfo,
    },
    settings::PrivacyMode,
};

use crate::AiError;

pub struct RouteInput<'a> {
    pub providers: &'a [(ProviderInfo, Vec<ModelInfo>)],
    pub privacy: PrivacyMode,
    pub needs_vision: bool,
    /// The user approved cloud for this one request (Local Preferred).
    pub allow_cloud: bool,
    pub preference: Option<&'a ModelRef>,
    /// Rough prompt size, to keep small-context models out of long conversations.
    pub estimated_prompt_tokens: u32,
    /// Models measured as slow to start answering on this Mac (recent first-token average).
    pub slow: &'a [ModelRef],
}

fn locality_allowed(l: Locality, privacy: PrivacyMode, allow_cloud: bool) -> bool {
    match (l, privacy) {
        (Locality::Local, _) => true,
        (Locality::Cloud, PrivacyMode::LocalOnly) => false,
        (Locality::Cloud, PrivacyMode::LocalPreferred) => allow_cloud,
        (Locality::Cloud, PrivacyMode::CloudAllowed) => true,
    }
}

fn capable(m: &ModelInfo, needs_vision: bool) -> bool {
    m.capabilities.contains(&ModelCapability::Text)
        && (!needs_vision || m.capabilities.contains(&ModelCapability::Vision))
}

fn fits(m: &ModelInfo, tokens: u32) -> bool {
    m.context_length.is_none_or(|c| tokens + 512 <= c)
}

pub fn route(input: &RouteInput<'_>) -> Result<RouteInfo, AiError> {
    let mut eligible: Vec<(&ProviderInfo, &ModelInfo)> = vec![];
    let mut blocked_by_privacy = false;
    for (p, models) in input.providers {
        if p.state != ProviderState::Available {
            continue;
        }
        for m in models {
            if !capable(m, input.needs_vision) || !fits(m, input.estimated_prompt_tokens) {
                continue;
            }
            if locality_allowed(m.locality, input.privacy, input.allow_cloud) {
                eligible.push((p, m));
            } else {
                blocked_by_privacy = true;
            }
        }
    }

    let info = |p: &ProviderInfo, m: &ModelInfo, reason: String| RouteInfo {
        model: ModelRef {
            provider_id: p.id.clone(),
            model: m.id.clone(),
        },
        model_name: m.name.clone(),
        provider_name: p.name.clone(),
        locality: m.locality,
        reason,
    };

    if let Some((p, m)) = input.preference.and_then(|pref| {
        eligible
            .iter()
            .find(|(p, m)| p.id == pref.provider_id && m.id == pref.model)
    }) {
        return Ok(info(p, m, "Your chosen model".into()));
    }

    // Automatic choice. Vision: the most capable local vision model. Text: Apple's on-device model
    // for short prompts (fast, private), otherwise the largest-context local model.
    let rank = |(p, m): &(&ProviderInfo, &ModelInfo)| {
        let local = (m.locality == Locality::Local) as u8;
        // Reasoning ("thinking") models deliberate silently before answering; for interactive
        // questions prefer models that answer straight away.
        let is_slow = input
            .slow
            .iter()
            .any(|s| s.provider_id == p.id && s.model == m.id);
        let answers_immediately =
            (!m.capabilities.contains(&ModelCapability::Reasoning) && !is_slow) as u8;
        // Apple's on-device model is preferred for short questions — unless it has proven slow here.
        let apple_short = (p.kind == ProviderKind::Apple
            && !is_slow
            && !input.needs_vision
            && input.estimated_prompt_tokens < 2500) as u8;
        let vision_bonus = (input.needs_vision && p.kind == ProviderKind::Ollama) as u8;
        (
            local,
            apple_short,
            vision_bonus,
            answers_immediately,
            m.context_length.unwrap_or(0),
            m.size_bytes.unwrap_or(0),
        )
    };
    let best = eligible.iter().max_by_key(|c| rank(c));
    if let Some((p, m)) = best {
        let reason = match (input.preference.is_some(), m.locality) {
            (true, _) => {
                "Your chosen model isn't available right now, so Wisp picked the closest match"
                    .into()
            }
            (false, Locality::Local) if input.needs_vision => {
                "Best local model that can see images".into()
            }
            (false, Locality::Local) => "Runs on this Mac".into(),
            (false, Locality::Cloud) => "Cloud model (allowed by your privacy setting)".into(),
        };
        return Ok(info(p, m, reason));
    }

    if blocked_by_privacy {
        return Err(AiError::PrivacyBlocked(match input.privacy {
            PrivacyMode::LocalOnly => "Local Only is on, and no model on this Mac can handle this. Download a local model in Models.".into(),
            _ => "This needs a cloud model. Approve cloud use for this request, or download a local model.".into(),
        }));
    }
    Err(AiError::NoModel(if input.needs_vision {
        "No model that can see images is ready. Download one in Models (for example qwen3-vl) or start Ollama.".into()
    } else {
        "No AI model is ready. Start Ollama or download a model in Models.".into()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(
        id: &str,
        kind: ProviderKind,
        locality: Locality,
        state: ProviderState,
    ) -> ProviderInfo {
        ProviderInfo {
            id: id.into(),
            kind,
            name: id.into(),
            locality,
            state,
            detail: None,
            version: None,
        }
    }

    fn model(provider: &str, id: &str, locality: Locality, vision: bool, ctx: u32) -> ModelInfo {
        let mut caps = vec![ModelCapability::Text, ModelCapability::Streaming];
        if vision {
            caps.push(ModelCapability::Vision);
        }
        ModelInfo {
            provider_id: provider.into(),
            id: id.into(),
            name: id.into(),
            locality,
            capabilities: caps,
            size_bytes: Some(1),
            context_length: Some(ctx),
            parameter_size: None,
            family: None,
        }
    }

    fn world() -> Vec<(ProviderInfo, Vec<ModelInfo>)> {
        vec![
            (
                provider(
                    "apple",
                    ProviderKind::Apple,
                    Locality::Local,
                    ProviderState::Available,
                ),
                vec![model("apple", "system", Locality::Local, false, 4096)],
            ),
            (
                provider(
                    "ollama",
                    ProviderKind::Ollama,
                    Locality::Local,
                    ProviderState::Available,
                ),
                vec![
                    model("ollama", "qwen3-vl:8b", Locality::Local, true, 32768),
                    model("ollama", "llama3.2:3b", Locality::Local, false, 8192),
                ],
            ),
            (
                provider(
                    "cloud",
                    ProviderKind::OpenaiCompatible,
                    Locality::Cloud,
                    ProviderState::Available,
                ),
                vec![model("cloud", "gpt-x", Locality::Cloud, true, 128000)],
            ),
        ]
    }

    fn input<'a>(
        w: &'a [(ProviderInfo, Vec<ModelInfo>)],
        privacy: PrivacyMode,
        vision: bool,
    ) -> RouteInput<'a> {
        RouteInput {
            providers: w,
            privacy,
            needs_vision: vision,
            allow_cloud: false,
            preference: None,
            estimated_prompt_tokens: 200,
            slow: &[],
        }
    }

    #[test]
    fn short_text_goes_to_apple_vision_goes_to_local_vision_model() {
        let w = world();
        assert_eq!(
            route(&input(&w, PrivacyMode::LocalOnly, false))
                .expect("route")
                .model
                .provider_id,
            "apple"
        );
        let r = route(&input(&w, PrivacyMode::LocalOnly, true)).expect("route");
        assert_eq!(r.model.model, "qwen3-vl:8b");
        assert_eq!(r.locality, Locality::Local);
    }

    #[test]
    fn long_prompts_skip_small_context_models() {
        let w = world();
        let mut i = input(&w, PrivacyMode::LocalOnly, false);
        i.estimated_prompt_tokens = 6000;
        assert_eq!(route(&i).expect("route").model.model, "qwen3-vl:8b");
    }

    #[test]
    fn local_only_never_selects_cloud_even_when_preferred_or_as_fallback() {
        let w = world();
        let cloud = ModelRef {
            provider_id: "cloud".into(),
            model: "gpt-x".into(),
        };
        let mut i = input(&w, PrivacyMode::LocalOnly, true);
        i.preference = Some(&cloud);
        i.allow_cloud = true; // approval is irrelevant in Local Only
        assert_eq!(route(&i).expect("route").locality, Locality::Local);

        // Only a cloud model could do it → refuse rather than upload.
        let only_cloud = vec![w[2].clone()];
        let err = route(&input(&only_cloud, PrivacyMode::LocalOnly, true)).expect_err("blocked");
        assert!(matches!(err, AiError::PrivacyBlocked(_)));
    }

    #[test]
    fn local_preferred_needs_per_request_approval_for_cloud() {
        let only_cloud = vec![world()[2].clone()];
        let mut i = input(&only_cloud, PrivacyMode::LocalPreferred, false);
        assert!(matches!(route(&i), Err(AiError::PrivacyBlocked(_))));
        i.allow_cloud = true;
        assert_eq!(route(&i).expect("route").locality, Locality::Cloud);
    }

    #[test]
    fn cloud_allowed_still_prefers_local() {
        let w = world();
        assert_eq!(
            route(&input(&w, PrivacyMode::CloudAllowed, true))
                .expect("route")
                .locality,
            Locality::Local
        );
    }

    #[test]
    fn honors_preference_and_falls_back_when_unavailable() {
        let mut w = world();
        let pref = ModelRef {
            provider_id: "ollama".into(),
            model: "llama3.2:3b".into(),
        };
        let mut i = input(&w, PrivacyMode::LocalOnly, false);
        i.preference = Some(&pref);
        assert_eq!(route(&i).expect("route").model, pref);

        w[1].0.state = ProviderState::NotRunning;
        let mut i = input(&w, PrivacyMode::LocalOnly, false);
        i.preference = Some(&pref);
        let r = route(&i).expect("fallback");
        assert_eq!(r.model.provider_id, "apple");
        assert!(r.reason.contains("isn't available"));
    }

    #[test]
    fn prefers_models_that_answer_immediately() {
        let mut w = world();
        let mut thinking = model(
            "ollama",
            "qwen3-vl:8b-thinking",
            Locality::Local,
            true,
            262144,
        );
        thinking.capabilities.push(ModelCapability::Reasoning);
        thinking.size_bytes = Some(10);
        w[1].1.push(thinking);
        assert_eq!(
            route(&input(&w, PrivacyMode::LocalOnly, true))
                .expect("route")
                .model
                .model,
            "qwen3-vl:8b"
        );
    }

    #[test]
    fn measured_slowness_moves_answers_to_a_faster_local_model() {
        let w = world();
        let slow = [ModelRef {
            provider_id: "apple".into(),
            model: "system".into(),
        }];
        let mut i = input(&w, PrivacyMode::LocalOnly, false);
        i.slow = &slow;
        let r = route(&i).expect("route");
        assert_eq!(r.model.provider_id, "ollama");
        assert_eq!(r.locality, Locality::Local, "still local");
    }

    #[test]
    fn explains_when_nothing_can_see_images() {
        let w = vec![world()[0].clone()];
        assert!(
            matches!(route(&input(&w, PrivacyMode::LocalOnly, true)), Err(AiError::NoModel(m)) if m.contains("see images"))
        );
    }
}
