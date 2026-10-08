# IPC

## Wire format

Newline-delimited JSON. One envelope per line, ≤ 16 MiB.

```jsonc
{"kind":"request","v":1,"id":"u7","method":"settings.update","params":{"update":{"section":"appearance","value":{"theme":"dark","accent":"default"}}}}
{"kind":"response","v":1,"id":"u7","result":{ /* AppSettings */ }}
{"kind":"response","v":1,"id":"u8","error":{"code":"invalidParams","message":"The request was malformed.","details":"…"}}
{"kind":"event","v":1,"event":"settings.changed","payload":{ /* AppSettings */ }}
```

Unknown envelope fields, unknown methods and wrong protocol versions are rejected on every side.

## Adding a method

1. Define params/result types in `crates/ipc/src/types/<domain>.rs` (`serde` + `JsonSchema`, `deny_unknown_fields`).
2. Add one line to `define_methods!` in `crates/ipc/src/methods.rs` (name, target, params ⇒ result).
3. Implement it: core methods in `crates/core/src/dispatch.rs`; host methods via `IPCRouter.register` in Swift.
4. `pnpm contracts` regenerates `packages/contracts/src/generated/contracts.ts` and `apps/macos/Sources/IPC/Generated/Contracts.swift`.
5. Call it from React with `ipc().call("domain.method", params)` — fully typed and validated.

## Routing

| From → To | Path |
|---|---|
| React → core | `window.webkit.messageHandlers.wisp` → `IPCRouter` (validates, re-ids) → `CoreProcess` stdin |
| React → host | `IPCRouter` → registered Swift handler |
| core → host | core writes a request; `CoreProcess` → `IPCRouter.handleHost` → response on stdin |
| core events | `CoreProcess` → `IPCRouter.broadcast` → every web surface + native subscribers |

Messages from web views are accepted only from the main frame of `wisp://app` (or the configured dev server).
