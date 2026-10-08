# Security

## Implemented

- **IPC validation** on all three hops; unknown methods are rejected before forwarding; web messages
  accepted only from Wisp's own main frame.
- **Navigation lockdown**: web views load only `wisp://app` (or the dev server); external links open
  in the default browser; CSP restricts scripts to the app origin.
- **Scheme handler confinement**: bundled assets served only from inside `Resources/web`; traversal and
  symlink escapes return 404.
- **Log redaction**: every core log line passes through `wisp_security::redact` (API keys, bearer
  tokens, `password=`-style pairs).
- **Path confinement** helpers (`wisp_security::paths::confine`) and filename sanitization for imports.
- **No plaintext secrets**: schema has no key columns; secret scanning in `check-architecture.sh`.
- **Process isolation**: inference runs in `wisp-core`, supervised and restarted by the host.
- **Hardened runtime** with minimal entitlements; no App Sandbox (D-013).

## Planned (by phase)

| Phase | Control |
|---|---|
| 6 | Keychain-backed provider secrets; privacy-mode enforcement in the router |
| 11 | Tool schemas, risk levels, policy engine, approvals, audit trail |
| 11 | Shell: parse → classify → block destructive → approve → restricted execution |
| 13 | Agent permission scoping; no implicit grants |
| 8/16 | Model and file downloads verified by checksum and size before use |
| 18 | Full security audit (spec §102–104) |
