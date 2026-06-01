# AGENTS.md — kokoro-rust

## Architecture
- Workspace: `kokoro` (lib) + `kokoro-cli` (binary named `kokoro-tts`)
- CLI sends text → lib phonemizes (espeak-ng FFI) → tokenizes (vocab LUT) → runs ONNX model (ort v2.0.0-rc.12) → outputs f32 PCM audio at 24kHz
- All mutable state held in `Kokoro` struct behind `Mutex<KokoroState>` — model/voice loaded lazily

## System dependencies
- **espeak-ng >= 1.50** — required via `pkg-config` (see `crates/kokoro/build.rs`). Not installable via cargo.
  - macOS: `brew install espeak-ng`
  - Linux: `apt install espeak-ng libespeak-ng-dev pkg-config`
- **uv** — needed only for `scripts/convert_voices.py` (voice pack conversion)
- **ONNX Runtime** — pulled automatically by the `ort` crate, no system install needed

## Build & run
```bash
cargo build --release              # workspaces builds both crates
cargo run --bin kokoro-tts -- "text"          # streaming playback (rodio)
cargo run --bin kokoro-tts -- -o out.wav "text"  # write to WAV (hound)
cargo run --bin kokoro-tts -- -o out.wav -q "text"  # quiet mode
cargo run --bin e2e                            # debug: normalize→phonemes→tokens→generate
```
- CLI is dual-mode: without `-o` → streams through rodio; with `-o` → writes WAV via hound.

No tests (tests/ dir exists but empty), no CI, no lint/format config.

## Prerequisites before `cargo build` succeeds
1. `espeak-ng >= 1.50` must be findable via `pkg-config` (checked in `crates/kokoro/build.rs`)
2. ONNX model auto-downloaded at first use to `~/Library/Caches/kokoro-tts/kokoro-v0_19.onnx` (macOS) or `~/.cache/kokoro-tts/` (Linux)
3. Voice packs must be **manually pre-converted** from `.pt` → `.kokoro` format. Auto-download will **fail** at runtime: `Voice::voice_url()` fetches raw `.pt` from HuggingFace but saves it to a `.kokoro` path, and `VoicePack::load()` expects the custom binary format (JSON header + f32 blob), not PyTorch format.

## Voice pipeline gotcha
- `Voice::voice_url()` returns a `.pt` URL (`{HF_BASE_URL}/main/voices/{name}.pt`), but `voice_path()` constructs a `.kokoro` path. Auto-download downloads `.pt` bytes to a `.kokoro` path → `VoicePack::load()` panics.
- Convert manually: `./scripts/convert_voices.py /path/to/af_sky.pt ~/Library/Caches/kokoro-tts/voices/af_sky.kokoro`
- `convert_voices.py` uses `uv` with inline deps (`torch>=2.0`, `numpy`) — heavyweight one-time dependency.
- The default voice is `Voice::AfSky` (hardcoded in CLI `main.rs:11`)

## Sentence splitting: two implementations
- **Lib** (`kokoro.rs:134-162`): `sentence_tokenize()` uses punctuation + "space followed by uppercase" heuristic for sentence boundaries.
- **CLI streaming** (`main.rs:152-169`): `split_sentences()` is a simpler naive split on `.`, `!`, `?` — then groups sentences into pairs (`chunks(2)`) for producer-consumer streaming.
- These are independent; changing one won't affect the other.

## Debug/example binaries
```bash
cargo run --bin e2e                           # normalize→phonemes→tokens→generate (hardcoded "Hello world.")
cargo run --example phoneme_debug              # test phonemize() with a few fixed strings
```
- `e2e.rs` optionally compares output against Python reference at `/tmp/kokoro_python_audio.f32`

## ONNX model contract
- Input names hardcoded: `"tokens"` (i64), `"style"` (f32), `"speed"` (f32) — see `model.rs:36-39`
- Output name: `"audio"` — see `model.rs:42`
- If the upstream model changes names, `model.rs` must be updated

## Key constants
| Constant | Value | Location |
|----------|-------|----------|
| Sampling rate | 24000 | `types.rs:4` |
| Max tokens | 510 | `types.rs:6` |
| Max chunk words | 25 | `types.rs:8` |
| Model commit | `3095858c...` | `types.rs:10` |
| Model filename | `kokoro-v0_19.onnx` | `types.rs:18` |
| Cache dir name | `kokoro-tts` | `types.rs:16` |

## Logging
Controlled by `RUST_LOG` env var (defaults to `info` in CLI, `main.rs:29-31`). Uses `log` + `env_logger`.

## Dependencies of note
- `ort = "2.0.0-rc.12"` — pre-release, API surface may change
- `hound` — WAV I/O (used in both lib and CLI)
- `ureq` — blocking HTTP for model/voice downloads
- `fancy-regex` — needed alongside `regex` for lookahead/lookbehind in `normalize.rs` and `phonemes.rs`; standard `regex` crate does not support these features
