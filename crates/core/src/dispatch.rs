//! Method dispatch: decode typed params, run the handler, encode the typed result.

use std::sync::Arc;

use serde_json::Value;
use wisp_common::paths::DataPaths;
use wisp_database::Database;
use wisp_ipc::{
    IpcError, Method,
    methods::{AppInfoMethod, SettingsChanged, SettingsGet, SettingsUpdate},
    types::app::AppInfo,
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
            inner: Arc::new(Inner { db, events, info }),
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
            other => Err(IpcError::method_not_found(other)),
        }
    }

    /// Persist metadata of host-originated events listed in the contract (e.g. invocations).
    /// Unknown or malformed events are dropped, never stored.
    pub async fn record_host_event(&self, name: &str, payload: Value) {
        use wisp_ipc::{
            EventKind,
            methods::{InvocationEndedEvent, InvocationStartedEvent},
        };
        let valid = match name {
            InvocationStartedEvent::NAME => serde_json::from_value::<
                <InvocationStartedEvent as EventKind>::Payload,
            >(payload.clone())
            .is_ok(),
            InvocationEndedEvent::NAME => serde_json::from_value::<
                <InvocationEndedEvent as EventKind>::Payload,
            >(payload.clone())
            .is_ok(),
            _ => false,
        };
        if !valid {
            tracing::debug!(event = name, "host event ignored");
            return;
        }
        let name = name.to_owned();
        if let Err(e) = self
            .blocking(move |db| db.record_event(&name, &payload))
            .await
        {
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
