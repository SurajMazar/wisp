//! Method dispatch: decode typed params, run the handler, encode the typed result.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use wisp_common::paths::DataPaths;
use wisp_database::Database;
use wisp_ipc::methods::{
    VoiceCancel, VoiceFinish, VoiceModelCancel, VoiceModelDownload, VoiceModelRemove, VoiceSpeak,
    VoiceStatusMethod, VoiceStopSpeaking,
};
use wisp_ipc::{
    IpcError, Method,
    methods::{
        AccessibilityRank, AiRoute, AppInfoMethod, ConversationCancel, ConversationDelete,
        ConversationGet, ConversationListMethod, ConversationPin, ConversationRename,
        ConversationSend, FocusLatest, ModelsCancelPull, ModelsDelete, ModelsListMethod,
        ModelsPull, ModelsTest, SettingsChanged, SettingsGet, SettingsUpdate,
    },
    types::{
        app::{AppInfo, Empty},
        conversations::ConversationList,
        focus::{FocusRegion, LatestFocus},
    },
};

use crate::events::EventBus;

/// Shared runtime state. Cheap to clone.
#[derive(Clone)]
pub struct Core {
    inner: Arc<Inner>,
}

struct Inner {
    db: Database,
    ai: Arc<crate::ai::AiServices>,
    streams: crate::conversations::Streams,
    voice: crate::voice::VoiceService,
    events: EventBus,
    info: AppInfo,
    /// The region the user last focused. In memory only: pixels never reach the database.
    latest_focus: Mutex<Option<FocusRegion>>,
    tools: crate::tools::ToolService,
    teacher: crate::teacher::Teacher,
}

impl Core {
    pub fn new(db: Database, paths: &DataPaths, events: EventBus) -> Result<Self, IpcError> {
        // Answers interrupted by a crash or quit are marked cancelled, never left "streaming".
        if let Ok(n) = db
            .recover_streaming_messages()
            .map_err(|e| tracing::warn!(error = %e, "recovery failed"))
            && n > 0
        {
            tracing::info!(count = n, "recovered interrupted answers");
        }
        // Tool calls interrupted mid-approval or mid-action are recorded as failed.
        let _ = db.fail_unfinished_tool_calls();
        let schema_version = db
            .schema_version()
            .map_err(|e| IpcError::internal(e.to_string()))?;
        let info = AppInfo {
            core_version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_version: wisp_ipc::PROTOCOL_VERSION,
            schema_version,
            data_dir: paths.root().display().to_string(),
            started_at_ms: wisp_common::time::now_ms(),
        };
        Ok(Self {
            inner: Arc::new(Inner {
                db,
                ai: Arc::new(crate::ai::AiServices::default()),
                streams: Default::default(),
                voice: Default::default(),
                events,
                info,
                latest_focus: Mutex::new(None),
                tools: Default::default(),
                teacher: Default::default(),
            }),
        })
    }

    pub fn info(&self) -> &AppInfo {
        &self.inner.info
    }

    pub fn events(&self) -> &EventBus {
        &self.inner.events
    }

    /// Route one request. Unknown methods and host-targeted methods are rejected.
    pub async fn dispatch(&self, method: &str, params: Value) -> Result<Value, IpcError> {
        match method {
            AppInfoMethod::NAME => {
                self.call::<AppInfoMethod, _, _>(
                    params,
                    |c, _| async move { Ok(c.inner.info.clone()) },
                )
                .await
            }
            SettingsGet::NAME => {
                self.call::<SettingsGet, _, _>(params, |c, _| async move {
                    c.blocking(|db| db.load_settings()).await
                })
                .await
            }
            SettingsUpdate::NAME => {
                self.call::<SettingsUpdate, _, _>(params, |c, p| async move {
                    p.update.validate().map_err(IpcError::invalid_params)?;
                    let settings = c
                        .blocking(move |db| db.save_settings_section(&p.update))
                        .await?;
                    c.inner.events.publish::<SettingsChanged>(&settings);
                    Ok(settings)
                })
                .await
            }
            AccessibilityRank::NAME => {
                self.call::<AccessibilityRank, _, _>(params, |_, p| async move {
                    if !(1..=100).contains(&p.limit) || p.snapshot.nodes.len() > 600 {
                        return Err(IpcError::invalid_params(
                            "limit must be 1–100 and at most 600 nodes",
                        ));
                    }
                    Ok(wisp_accessibility::rank(
                        p.snapshot,
                        p.region,
                        p.cursor,
                        p.limit as usize,
                    ))
                })
                .await
            }
            ModelsListMethod::NAME => {
                self.call::<ModelsListMethod, _, _>(params, |c, _| async move {
                    let s = c.settings().await?;
                    Ok(c.inner.ai.list(&s.models).await)
                })
                .await
            }
            ModelsPull::NAME => {
                self.call::<ModelsPull, _, _>(params, |c, p| async move {
                    let s = c.settings().await?;
                    c.inner
                        .ai
                        .start_pull(&s.models, p.name, c.inner.events.clone())?;
                    Ok(Empty {})
                })
                .await
            }
            ModelsCancelPull::NAME => {
                self.call::<ModelsCancelPull, _, _>(params, |c, p| async move {
                    c.inner.ai.cancel_pull(&p.name);
                    Ok(Empty {})
                })
                .await
            }
            ModelsDelete::NAME => {
                self.call::<ModelsDelete, _, _>(params, |c, p| async move {
                    let s = c.settings().await?;
                    c.inner.ai.ollama(&s.models).delete(&p.name).await?;
                    Ok(Empty {})
                })
                .await
            }
            ModelsTest::NAME => {
                self.call::<ModelsTest, _, _>(params, |c, p| async move {
                    let s = c.settings().await?;
                    c.inner
                        .ai
                        .test(&s.models, s.privacy.mode, p.model, p.prompt)
                        .await
                })
                .await
            }
            AiRoute::NAME => {
                self.call::<AiRoute, _, _>(params, |c, p| async move {
                    let s = c.settings().await?;
                    let (route, _) = c
                        .inner
                        .ai
                        .route(
                            &s.models,
                            s.privacy.mode,
                            p.needs_vision,
                            p.allow_cloud,
                            0,
                            None,
                        )
                        .await?;
                    Ok(route)
                })
                .await
            }
            ConversationListMethod::NAME => {
                self.call::<ConversationListMethod, _, _>(params, |c, p| async move {
                    let limit = p.limit.clamp(1, 500);
                    let items = c
                        .blocking(move |db| db.list_conversations(p.query.as_deref(), limit))
                        .await?;
                    Ok(ConversationList { items })
                })
                .await
            }
            ConversationGet::NAME => {
                self.call::<ConversationGet, _, _>(params, |c, p| async move {
                    let id = valid_id(p.id)?;
                    c.blocking(move |db| db.get_conversation(&id))
                        .await?
                        .ok_or_else(|| {
                            IpcError::new(
                                wisp_ipc::ErrorCode::NotFound,
                                "That conversation no longer exists.",
                            )
                        })
                })
                .await
            }
            ConversationSend::NAME => {
                self.call::<ConversationSend, _, _>(params, |c, p| async move { c.send(p).await })
                    .await
            }
            wisp_ipc::methods::TeachControl::NAME => {
                self.call::<wisp_ipc::methods::TeachControl, _, _>(params, |c, p| async move {
                    c.control_lesson(p)
                })
                .await
            }
            wisp_ipc::methods::TeachCurrent::NAME => {
                self.call::<wisp_ipc::methods::TeachCurrent, _, _>(params, |c, _| async move {
                    Ok(c.current_lesson())
                })
                .await
            }
            wisp_ipc::methods::ToolListMethod::NAME => {
                self.call::<wisp_ipc::methods::ToolListMethod, _, _>(params, |_, _| async move {
                    Ok(wisp_ipc::types::tools::ToolList {
                        tools: wisp_computer::definitions(),
                    })
                })
                .await
            }
            wisp_ipc::methods::ToolRun::NAME => {
                self.call::<wisp_ipc::methods::ToolRun, _, _>(params, |c, p| async move {
                    c.run_tool(p).await
                })
                .await
            }
            wisp_ipc::methods::ToolRespond::NAME => {
                self.call::<wisp_ipc::methods::ToolRespond, _, _>(params, |c, p| async move {
                    c.respond_tool(p)
                })
                .await
            }
            wisp_ipc::methods::ToolActivityMethod::NAME => {
                self.call::<wisp_ipc::methods::ToolActivityMethod, _, _>(
                    params,
                    |c, p| async move {
                        if !(1..=500).contains(&p.limit) {
                            return Err(IpcError::invalid_params("limit must be 1–500"));
                        }
                        let items = c.blocking(move |db| db.list_tool_calls(p.limit)).await?;
                        Ok(wisp_ipc::types::tools::ToolActivity { items })
                    },
                )
                .await
            }
            wisp_ipc::methods::ToolGrantsMethod::NAME => {
                self.call::<wisp_ipc::methods::ToolGrantsMethod, _, _>(params, |c, _| async move {
                    c.tool_grants().await
                })
                .await
            }
            wisp_ipc::methods::ToolRevoke::NAME => {
                self.call::<wisp_ipc::methods::ToolRevoke, _, _>(params, |c, p| async move {
                    c.revoke_tool(p.id).await
                })
                .await
            }
            wisp_ipc::methods::ConversationShowGuidance::NAME => {
                self.call::<wisp_ipc::methods::ConversationShowGuidance, _, _>(
                    params,
                    |c, p| async move { c.show_guidance(&p.id).await },
                )
                .await
            }
            wisp_ipc::methods::ConversationRegenerate::NAME => {
                self.call::<wisp_ipc::methods::ConversationRegenerate, _, _>(
                    params,
                    |c, p| async move { c.regenerate(p).await },
                )
                .await
            }
            ConversationCancel::NAME => {
                self.call::<ConversationCancel, _, _>(params, |c, p| async move {
                    c.inner.streams.cancel(&valid_id(p.id)?);
                    Ok(Empty {})
                })
                .await
            }
            ConversationRename::NAME => {
                self.call::<ConversationRename, _, _>(params, |c, p| async move {
                    let id = valid_id(p.id)?;
                    let title = p.title.trim().chars().take(120).collect::<String>();
                    if title.is_empty() {
                        return Err(IpcError::invalid_params("title must not be empty"));
                    }
                    let id2 = id.clone();
                    c.blocking(move |db| db.rename_conversation(&id2, &title))
                        .await?;
                    c.summary_or_not_found(id).await
                })
                .await
            }
            ConversationPin::NAME => {
                self.call::<ConversationPin, _, _>(params, |c, p| async move {
                    let id = valid_id(p.id)?;
                    let id2 = id.clone();
                    c.blocking(move |db| db.pin_conversation(&id2, p.pinned))
                        .await?;
                    c.summary_or_not_found(id).await
                })
                .await
            }
            ConversationDelete::NAME => {
                self.call::<ConversationDelete, _, _>(params, |c, p| async move {
                    let id = valid_id(p.id)?;
                    if let Some(conv) = {
                        let id = id.clone();
                        c.blocking(move |db| db.get_conversation(&id)).await?
                    } {
                        for m in conv.messages.iter().filter(|m| {
                            m.status == wisp_ipc::types::conversations::MessageStatus::Streaming
                        }) {
                            c.inner.streams.cancel(&m.id);
                        }
                    }
                    c.blocking(move |db| db.delete_conversation(&id)).await?;
                    Ok(Empty {})
                })
                .await
            }
            VoiceStatusMethod::NAME => {
                self.call::<VoiceStatusMethod, _, _>(params, |c, _| async move {
                    c.voice_status().await
                })
                .await
            }
            VoiceFinish::NAME => {
                self.call::<VoiceFinish, _, _>(
                    params,
                    |c, p| async move { c.voice_finish(p).await },
                )
                .await
            }
            VoiceCancel::NAME => {
                self.call::<VoiceCancel, _, _>(params, |c, p| async move {
                    c.inner.voice.cancel(&p.session_id);
                    Ok(Empty {})
                })
                .await
            }
            VoiceSpeak::NAME => {
                self.call::<VoiceSpeak, _, _>(params, |c, p| async move {
                    c.speak_text(&p.text)?;
                    Ok(Empty {})
                })
                .await
            }
            VoiceStopSpeaking::NAME => {
                self.call::<VoiceStopSpeaking, _, _>(params, |c, _| async move {
                    c.stop_speaking();
                    Ok(Empty {})
                })
                .await
            }
            VoiceModelDownload::NAME => {
                self.call::<VoiceModelDownload, _, _>(params, |c, p| async move {
                    c.start_model_download(p.id)?;
                    Ok(Empty {})
                })
                .await
            }
            VoiceModelCancel::NAME => {
                self.call::<VoiceModelCancel, _, _>(params, |c, p| async move {
                    c.cancel_model_download(&p.id);
                    Ok(Empty {})
                })
                .await
            }
            VoiceModelRemove::NAME => {
                self.call::<VoiceModelRemove, _, _>(params, |c, p| async move {
                    c.remove_model(&p.id)?;
                    Ok(Empty {})
                })
                .await
            }
            FocusLatest::NAME => {
                self.call::<FocusLatest, _, _>(params, |c, _| async move {
                    Ok(LatestFocus {
                        region: c.latest_focus(),
                    })
                })
                .await
            }
            other => Err(IpcError::method_not_found(other)),
        }
    }

    pub(crate) fn inner_voice(&self) -> &crate::voice::VoiceService {
        &self.inner.voice
    }

    pub(crate) fn inner_streams(&self) -> &crate::conversations::Streams {
        &self.inner.streams
    }

    pub fn ai(&self) -> &Arc<crate::ai::AiServices> {
        &self.inner.ai
    }

    pub fn attach_host(&self, host: Arc<dyn wisp_providers::HostBridge>) {
        self.inner.ai.attach_host(host);
    }

    pub fn detach_host(&self) {
        self.inner.ai.detach_host();
    }

    async fn settings(&self) -> Result<wisp_ipc::types::settings::AppSettings, IpcError> {
        self.blocking(|db| db.load_settings()).await
    }

    async fn summary_or_not_found(
        &self,
        id: String,
    ) -> Result<wisp_ipc::types::conversations::ConversationSummary, IpcError> {
        let s = self
            .blocking(move |db| db.conversation_summary(&id))
            .await?
            .ok_or_else(|| {
                IpcError::new(
                    wisp_ipc::ErrorCode::NotFound,
                    "That conversation no longer exists.",
                )
            })?;
        self.inner
            .events
            .publish::<wisp_ipc::methods::ConversationUpdated>(&s);
        Ok(s)
    }

    pub(crate) fn teacher_state(&self) -> &crate::teacher::Teacher {
        &self.inner.teacher
    }

    pub(crate) fn tool_service(&self) -> &crate::tools::ToolService {
        &self.inner.tools
    }

    pub fn latest_focus(&self) -> Option<FocusRegion> {
        self.inner.latest_focus.lock().ok().and_then(|g| g.clone())
    }

    /// Handle host-originated events listed in the contract: validate the payload, update runtime
    /// state, and persist **metadata only** in the event log. Unknown or malformed events are dropped.
    pub async fn record_host_event(&self, name: &str, payload: Value) {
        use wisp_ipc::{
            EventKind,
            methods::{
                FocusCleared, FocusCompleted, FocusEndedEvent, FocusStartedEvent,
                InvocationEndedEvent, InvocationStartedEvent,
            },
        };
        fn parse<E: EventKind>(v: &Value) -> Option<E::Payload> {
            serde_json::from_value::<E::Payload>(v.clone()).ok()
        }
        if name == "audio.chunk" {
            if let Ok(chunk) = serde_json::from_value::<wisp_ipc::types::voice::AudioChunk>(payload)
                && wisp_common::ids::is_valid_id(&chunk.session_id)
            {
                self.inner.voice.append(&chunk.session_id, &chunk.pcm);
            }
            return;
        }
        if name == "apple.delta" {
            if let Ok(d) = serde_json::from_value::<wisp_ipc::types::models::AppleDelta>(payload) {
                self.inner.ai.apple_streams.deliver(&d.request_id, d.delta);
            }
            return;
        }
        let record: Option<Value> = match name {
            InvocationStartedEvent::NAME => {
                parse::<InvocationStartedEvent>(&payload).map(|_| payload.clone())
            }
            InvocationEndedEvent::NAME => {
                parse::<InvocationEndedEvent>(&payload).map(|_| payload.clone())
            }
            FocusStartedEvent::NAME => {
                parse::<FocusStartedEvent>(&payload).map(|_| payload.clone())
            }
            FocusEndedEvent::NAME => parse::<FocusEndedEvent>(&payload).map(|_| payload.clone()),
            FocusCompleted::NAME => match parse::<FocusCompleted>(&payload) {
                Some(region) => match wisp_screen::validate(&region) {
                    Ok(()) => {
                        let meta = serde_json::json!({
                            "kind": region.kind,
                            "displayId": region.display.id,
                            "scale": region.display.scale,
                            "cropPoints": [region.crop.width.round(), region.crop.height.round()],
                            "hasImage": region.image.is_some(),
                        });
                        if let Ok(mut g) = self.inner.latest_focus.lock() {
                            *g = Some(region);
                        }
                        Some(meta)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "focus region rejected");
                        None
                    }
                },
                None => None,
            },
            FocusCleared::NAME => {
                if let Ok(mut g) = self.inner.latest_focus.lock() {
                    *g = None;
                }
                Some(Value::Object(Default::default()))
            }
            _ => None,
        };
        let Some(meta) = record else {
            tracing::debug!(event = name, "host event ignored");
            return;
        };
        let name = name.to_owned();
        if let Err(e) = self.blocking(move |db| db.record_event(&name, &meta)).await {
            tracing::warn!(error = %e, "failed to record event");
        }
    }

    async fn call<M, F, Fut>(&self, params: Value, f: F) -> Result<Value, IpcError>
    where
        M: Method,
        F: FnOnce(Core, M::Params) -> Fut,
        Fut: std::future::Future<Output = Result<M::Result, IpcError>>,
    {
        // `null`/absent params are accepted for parameterless methods.
        let params = if params.is_null() {
            Value::Object(Default::default())
        } else {
            params
        };
        let typed: M::Params = serde_json::from_value(params)
            .map_err(|e| IpcError::invalid_params(format!("{}: {e}", M::NAME)))?;
        let result = f(self.clone(), typed).await?;
        serde_json::to_value(result).map_err(|e| IpcError::internal(e.to_string()))
    }

    /// Run a database operation on the blocking pool.
    pub(crate) async fn blocking<T, F>(&self, f: F) -> Result<T, IpcError>
    where
        T: Send + 'static,
        F: FnOnce(&Database) -> wisp_database::DbResult<T> + Send + 'static,
    {
        let db = self.inner.db.clone();
        tokio::task::spawn_blocking(move || f(&db))
            .await
            .map_err(|e| IpcError::internal(format!("task join: {e}")))?
            .map_err(|e| IpcError::internal(e.to_string()))
    }
}

fn valid_id(id: String) -> Result<String, IpcError> {
    if wisp_common::ids::is_valid_id(&id) {
        Ok(id)
    } else {
        Err(IpcError::invalid_params("invalid id"))
    }
}
