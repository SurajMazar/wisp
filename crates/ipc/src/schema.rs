//! Builds the contract document consumed by the TypeScript/Swift code generators.

use serde_json::{Value, json};

use crate::{
    PROTOCOL_VERSION,
    error::IpcError,
    methods::{event_specs, method_specs},
};

pub fn contract_document() -> Value {
    let mut generator = schemars::generate::SchemaSettings::draft07().into_generator();
    // Ensure shared types are emitted as definitions even if no method references them yet.
    let error = generator.subschema_for::<IpcError>();

    let methods: Vec<Value> = method_specs()
        .into_iter()
        .map(|m| {
            json!({
                "name": m.name,
                "target": m.target,
                "params": (m.params)(&mut generator),
                "result": (m.result)(&mut generator),
            })
        })
        .collect();
    let events: Vec<Value> = event_specs()
        .into_iter()
        .map(|e| json!({ "name": e.name, "payload": (e.payload)(&mut generator) }))
        .collect();

    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "error": error,
        "methods": methods,
        "events": events,
        "definitions": generator.definitions(),
    })
}
