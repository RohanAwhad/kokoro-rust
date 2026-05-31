# 000: Rust Port — Architecture & Overview

**Severity**: high (greenfield)
**Source**: `/Users/rawhad/1_Projects/personal_projects/kokoro-tts` (Python v0.1.0)

## Current Behavior (Python)

A ~400-line Python library wrapping Kokoro-82M ONNX TTS model. Two entry points:

1. **Python API**: `Kokoro().generate(text, voice_name) → np.ndarray`
2. **CLI**: `kokoro-tts "Hello world"` streams audio via `sounddevice`

Dependencies: `onnxruntime`, `phonemizer` (espeak), `torch` (voice pack loading), `nltk` (sentence tokenize), `sounddevice`, `numpy`.

## Desired Behavior (Rust)

Feature-complete port with equivalent public API, same audio quality (byte-identical output for known inputs), faster startup, and no Python runtime dependency.

## Crate Structure

Single workspace with two crates:

```
kokoro-rust/
  Cargo.toml              # workspace root
  crates/
    kokoro/               # lib crate — all core logic
      Cargo.toml
      src/
        lib.rs            # pub mod declarations, re-exports
        normalize.rs      # normalize_text() — regex substitution pipeline
        phonemes.rs       # phonemize() — espeak FFI bindings
        vocab.rs          # get_vocab(), tokenize() — char→int LUT
        model.rs          # ONNX model load/run, KokoroModel struct
        voice.rs          # VoicePack — .pt parsing, download, caching
        kokoro.rs         # Kokoro struct — public API, generate()
        error.rs          # Error enum, Result alias
        types.rs          # AudioSamples, VoiceId, Lang — shared types
        cache.rs          # Cache dir logic, download helpers
      tests/
        normalize_test.rs
        phonemes_test.rs
        vocab_test.rs
        model_test.rs
        voice_test.rs
        integration_test.rs
    kokoro-cli/           # binary crate — CLI entry point
      Cargo.toml
      src/
        main.rs           # arg parsing, producer-consumer streaming
```

### Why a workspace with two crates?

- `kokoro` lib can be consumed by other Rust programs (like `soundfile` bindings)
- `kokoro-cli` only depends on `kokoro` + audio playback deps (`cpal`)
- Clean separation of library vs application

## Dependency Map (Rust Crates)

| Purpose | Rust Crate | Version Target | Notes |
|---------|-----------|---------------|-------|
| ONNX inference | `ort` | latest | Bindings to ONNX Runtime C API |
| Phonemization | `espeak-sys` or `espeak` | latest | FFI to espeak-ng (already required) |
| Text normalization | `regex` | latest | Python `re.sub()` replacement |
| Audio playback | `cpal` | latest | Cross-platform audio output |
| WAV file I/O | `hound` | latest | Writing `.wav` files |
| Voice pack parsing | `zip` + `serde-pickle` | latest | Parse .pt (PyTorch zip-pickle) |
| HTTP download | `ureq` or `reqwest` | latest | Download model + voices from HF |
| CLI arg parsing | `clap` | latest | derive-based argument parser |
| Logging | `log` + `env_logger` | latest | Controlled via `LOGGING_LEVEL` env |
| Async (optional) | `tokio` | latest | For future non-blocking download |
| Numeric | `ndarray` | latest | Rust equivalent of numpy arrays |
| Sentence tokenize | `punkt`-based or regex | — | Simple rule-based, no NLTK dep |
| Temp/cache dir | `dirs` | latest | XDG-compliant cache directory |

**Heavy dependency decision**: `tch-rs` (libtorch bindings, ~2GB) vs. manual `.pt` parsing.

- **Decision**: Parse `.pt` in pure Rust (zip + pickle). libtorch is too heavy for a thin wrapper.
- **Fallback**: If pickle parsing is infeasible, pre-convert voice packs to `.npy` with a one-time Python script (shipped in `scripts/`).

## Design Principles

### 1. Fail fast — no silent degradation
Let errors propagate. Don't hide behind `unwrap()` in library code — use `Result<T, KokoroError>` everywhere.

### 2. Lazy initialization
Model (ONNX session) and voice packs loaded only on first `generate()` call, mimicking Python's `self._sess is None` pattern.

### 3. Minimal allocations in hot path
Pre-allocate buffers. Reuse `InferenceSession` across calls. Avoid cloning audio data.

### 4. Typed voice/language identifiers
Use enums, not magic strings:
```rust
pub enum Lang { Am, Br }
pub enum Voice { Af, AfBella, AfSarah, AmAdam, AmMichael, BfEmma, BfIsabella, BmGeorge, BmLewis, AfNicole, AfSky }
```

### 5. Sentence chunking strategy (matching Python)
- Split text into sentences via punctuation (`.`, `!`, `?`)
- Group sentences into chunks of ≤25 words
- Each chunk ≤510 tokens after phonemization
- Run ONNX inference per chunk, concatenate results

### 6. ONNX runtime session is thread-unsafe
`ort::Session` is `!Send + !Sync`. The `Kokoro` struct handles this — either single-threaded use or wrap in a mutex for the CLI's producer-consumer pattern.

## Data Flow (End to End)

```
User Text (String)
  │
  ▼
normalize_text()         — regex substitutions, quote/num normalization
  │
  ▼
phonemize()             — espeak-ng: text → IPA phoneme string
  │
  ▼
tokenize()              — VOCAB LUT: phoneme chars → Vec<u32>
  │
  ▼
Sentence Chunker        — NLTK-style sent_tokenize → chunks ≤25 words
  │
  ▼  [per chunk]
Voice Pack Lookup       — voice_pack[token_count] → style tensor (f32)
  │
  ▼
ONNX Session Run        — inputs: [tokens, style, speed=1.0] → f32 audio
  │
  ▼
Concatenate Chunks      — Vec<f32> final audio buffer
  │
  ▼
[CLI: audio playback via cpal]
[API: return Vec<f32> or write .wav via hound]
```

## Cargo.toml (Workspace Root)

```toml
[workspace]
members = ["crates/kokoro", "crates/kokoro-cli"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
```

## Kokoro Lib Crate Dependencies

```toml
[package]
name = "kokoro"
version = "0.1.0"
edition = "2021"

[dependencies]
ort = "2"
regex = "1"
hound = "3"
ureq = { version = "3", features = ["json"] }
dirs = "5"
log = "0.4"
ndarray = "0.16"
serde = { version = "1", features = ["derive"] }
serde-pickle = "1"
zip = "2"
thiserror = "2"
espeak = "0.2"           # or espeak-sys for manual FFI
```

## Kokoro CLI Crate Dependencies

```toml
[package]
name = "kokoro-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "kokoro-tts"
path = "src/main.rs"

[dependencies]
kokoro = { path = "../kokoro" }
clap = { version = "4", features = ["derive"] }
cpal = "0.15"
log = "0.4"
env_logger = "0.11"
crossbeam-channel = "0.5"   # MPMC channel for producer-consumer
ctrlc = "3"                   # signal handling
```

## Files NOT to Port

- `examples/simple.py` and `examples/long_text.py` — these are usage examples, not library code. The tests and README serve as documentation.
- `nltk.download('punkt_tab')` call — NLTK model download is a Python-side setup step. Replace with bundled rule-based sentence tokenizer.

## Key Differences from Python

| Python | Rust | Rationale |
|--------|------|-----------|
| `np.ndarray` | `Vec<f32>` or `ndarray::Array1<f32>` | `ndarray` if we need shape manipulation, else `Vec<f32>` |
| `os.path.expanduser("~")` | `dirs::cache_dir()` | XDG-compliant cache dir |
| `phonemizer` library | FFI to espeak-ng via `espeak-sys` | Same underlying C library |
| `torch.load(..., weights_only=True)` | zip + pickle parser | Avoid ~2GB libtorch dep |
| `nltk.sent_tokenize` | Rule-based split or own impl | NLTK is Python-only; replace with a 20-line regex-based split |
| `sounddevice` | `cpal` | Same capability, idiomatic Rust |
| `threading` + `queue.Queue` | `std::thread` + `crossbeam-channel` | Same producer-consumer pattern |

## Resolved Design Decisions

1. **Voice pack parsing**: Pre-convert `.pt` → `.kokoro` custom format via Python script. Ship conversion script in `scripts/`. Run once at first setup.
2. **Audio output type**: `AudioSamples { sample_rate, data: Vec<f32> }` newtype.
3. **Empty input**: Return empty `AudioSamples`, not an error.
4. **Thread safety**: `Kokoro` is `!Send + !Sync` (wraps `ort::Session`). Internal `Mutex<KokoroState>` for CLI producer-consumer. Callers wrap in `Arc<Mutex<Kokoro>>` if needed.
5. **CLI voice**: Hardcode `af_sky` (match Python CLI). No `--voice` flag.
6. **espeak FFI**: `espeak-sys` (raw FFI) for full control over phoneme flags.
7. **Audio playback**: `rodio` for initial implementation (simpler API). Upgrade to `cpal` ring buffer later if latency is an issue.
8. **Voice pack indexing**: Indexed by batch dimension (key=1 for single-sequence). `voice_pack[1]` is the correct style reference, matching Python.
