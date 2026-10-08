//! Method and event registry.
//!
//! Every IPC method is a marker type implementing [`Method`], declared once in
//! [`define_methods!`]. The same table drives Rust dispatch, JSON Schema generation and
//! therefore the generated Zod/Swift clients — adding a method here is the only way to
//! make it callable.

use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};

use crate::types::{app::*, invocation::*, settings::*};

/// Which side implements a method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    /// Implemented by `wisp-core`; callable by the host and the UI.
    Core,
    /// Implemented by the Swift host; callable by `wisp-core` and the UI.
    Host,
}

pub trait Method {
    const NAME: &'static str;
    const TARGET: Target;
    type Params: Serialize + DeserializeOwned + JsonSchema;
    type Result: Serialize + DeserializeOwned + JsonSchema;
}

pub trait EventKind {
    const NAME: &'static str;
    type Payload: Serialize + DeserializeOwned + JsonSchema;
}

/// Description of a method used by the schema generator.
pub struct MethodSpec {
    pub name: &'static str,
    pub target: Target,
    pub params: fn(&mut schemars::SchemaGenerator) -> schemars::Schema,
    pub result: fn(&mut schemars::SchemaGenerator) -> schemars::Schema,
}

pub struct EventSpec {
    pub name: &'static str,
    pub payload: fn(&mut schemars::SchemaGenerator) -> schemars::Schema,
}

macro_rules! define_methods {
    ($( $(#[$meta:meta])* $ty:ident = $name:literal, $target:ident, $params:ty => $result:ty; )*) => {
        $(
            $(#[$meta])*
            pub struct $ty;
            impl Method for $ty {
                const NAME: &'static str = $name;
                const TARGET: Target = Target::$target;
                type Params = $params;
                type Result = $result;
            }
        )*

        pub fn method_specs() -> Vec<MethodSpec> {
            vec![$(MethodSpec {
                name: $name,
                target: Target::$target,
                params: |g| g.subschema_for::<$params>(),
                result: |g| g.subschema_for::<$result>(),
            }),*]
        }
    };
}

macro_rules! define_events {
    ($( $(#[$meta:meta])* $ty:ident = $name:literal, $payload:ty; )*) => {
        $(
            $(#[$meta])*
            pub struct $ty;
            impl EventKind for $ty {
                const NAME: &'static str = $name;
                type Payload = $payload;
            }
        )*

        pub fn event_specs() -> Vec<EventSpec> {
            vec![$(EventSpec { name: $name, payload: |g| g.subschema_for::<$payload>() }),*]
        }
    };
}

define_methods! {
    /// Version and runtime information about the core.
    AppInfoMethod = "app.info", Core, Empty => AppInfo;
    /// Read all settings (defaults filled in).
    SettingsGet = "settings.get", Core, Empty => AppSettings;
    /// Replace one settings section and persist it.
    SettingsUpdate = "settings.update", Core, SettingsUpdateParams => AppSettings;

    /// Live state of each invocation channel (keyboard, mouse, trackpad, menu bar).
    HostInvocationStatus = "host.invocation.status", Host, Empty => InvocationStatus;
    /// Capture the next shortcut the user presses (Escape cancels).
    HostShortcutRecord = "host.shortcut.record", Host, ShortcutRecordParams => ShortcutRecordResult;
    /// Check a shortcut against macOS, common app shortcuts and Wisp's own bindings.
    HostShortcutCheck = "host.shortcut.check", Host, ShortcutCheckParams => ShortcutCheckResult;
    /// Open a System Settings privacy pane.
    HostOpenSettingsPane = "host.system.openSettingsPane", Host, OpenSettingsPaneParams => Empty;
    /// Show or hide the floating assistant.
    HostAssistantSetVisible = "host.assistant.setVisible", Host, AssistantVisibility => Empty;
}

define_events! {
    /// Emitted once after migrations have run and the core accepts requests.
    CoreReady = "core.ready", AppInfo;
    /// Emitted after any settings section changes.
    SettingsChanged = "settings.changed", AppSettings;
    /// The user summoned the assistant.
    InvocationStartedEvent = "invocation.started", InvocationStarted;
    /// The invocation finished (released, toggled off, dismissed, cancelled).
    InvocationEndedEvent = "invocation.ended", InvocationEnded;
    /// An invocation channel changed state (e.g. permission granted).
    InvocationStatusChanged = "invocation.statusChanged", InvocationStatus;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn method_and_event_names_are_unique_and_namespaced() {
        let mut seen = HashSet::new();
        for m in method_specs() {
            assert!(m.name.contains('.'), "{} must be namespaced", m.name);
            assert!(seen.insert(m.name), "duplicate method {}", m.name);
            assert_eq!(
                m.name.starts_with("host."),
                m.target == Target::Host,
                "{}",
                m.name
            );
        }
        let mut seen = HashSet::new();
        for e in event_specs() {
            assert!(seen.insert(e.name), "duplicate event {}", e.name);
        }
    }
}
