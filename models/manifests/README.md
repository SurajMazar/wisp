# Model manifests

Metadata for downloadable local models. **No model binaries live in the repository** — they are
downloaded into `~/Library/Application Support/Wisp/models/` and verified against the SHA-256 here.

Each entry (validated by `crates/audio` / the model manager when they land in Phase 8):

| Field | Meaning |
|---|---|
| `id` | Stable identifier, e.g. `whisper-small` |
| `type` | `stt` \| `tts` \| `vad` \| `embedding` |
| `provider` | Runtime that executes it, e.g. `whisper.cpp`, `onnxruntime` |
| `format` | `ggml`, `onnx`, … |
| `languages` | BCP-47 codes or `["multilingual"]` |
| `sizeBytes` | Exact download size, used for disk-space checks |
| `license` | SPDX identifier; only redistributable licenses are listed |
| `source` | Hugging Face repository + revision (commit hash, never a moving branch) |
| `files` | `[{ path, url, sha256, sizeBytes }]` |
| `capabilities` | e.g. `["speech-to-text"]` |
| `requirements` | e.g. `{ "minMemoryBytes": … }` |

Entries are added only with checksums verified against the pinned revision.
