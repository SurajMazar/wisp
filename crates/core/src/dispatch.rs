//! Method dispatch: decode typed params, run the handler, encode the typed result.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use wisp_common::paths::DataPaths;
use wisp_database::Database;
use wisp_ipc::{
    IpcError, Method,
    methods::{AppInfoMethod, FocusLatest, SettingsChanged, SettingsGet, SettingsUpdate},
    types::{
        app::AppInfo,
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
    events: EventBus,
    info: AppInfo,
    /// The region the user last focused. In memory only: pixels never reach the database.
    latest_focus: Mutex<Option<FocusRegion>>,
}

impl Core {
    pub fn new(db: Database, paths: &DataPaths, events: EventBus) -> Result<Self, IpcError> {
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
                events,
                info,
                latest_focus: Mutex::new(None),
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
    async fn blocking<T, F>(&self, f: F) -> Result<T, IpcError>
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
