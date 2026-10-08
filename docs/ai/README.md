# AI providers, models and routing (design — Phase 6)

- `AIProvider` trait in `crates/ai`; implementations in `crates/providers/{apple,ollama,openai,anthropic,gemini,openai-compatible}`.
- Apple Foundation Models execute in the Swift host (`host.apple.*`); availability is read from
  `SystemLanguageModel.default.availability`, never assumed. Verified available on the dev Mac (2026-10-08).
- The router picks a model per task from required capability, privacy mode, user preference,
  availability and measured latency. **Local Only never falls back to cloud.**
- General, vision, STT, TTS and embedding models are configured independently.
