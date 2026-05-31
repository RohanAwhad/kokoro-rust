# 006: Types, Errors & Validation

**Dependencies**: 000_overview
**Severity**: medium

## Error Types (`error.rs`)

Centralized error enum using `thiserror`. All library functions return `Result<T, KokoroError>`.

```rust
use thiserror::Error;

/// All errors that can occur in the kokoro library.
#[derive(Error, Debug)]
pub enum KokoroError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] Box<ureq::Error>),

    #[error("ONNX runtime error: {0}")]
    Ort(#[from] ort::Error),

    #[error("Espeak error: {0}")]
    Espeak(String),

    #[error("Invalid voice name: '{0}'. Valid voices: af, af_bella, af_sarah, am_adam, am_michael, bf_emma, bf_isabella, bm_george, bm_lewis, af_nicole, af_sky")]
    InvalidVoice(String),

    #[error("Voice not loaded. Call load_voice() first.")]
    VoiceNotLoaded,

    #[error("Model not loaded. Call load_model() first.")]
    ModelNotLoaded,

    #[error("Empty token sequence. Text produced no phonemes.")]
    EmptyTokens,

    #[error("Voice pack missing style for {token_count} tokens (max: {max})")]
    VoicePackMissingStyle { token_count: usize, max: usize },

    #[error("Invalid voice pack file: {0}")]
    InvalidVoicePack(String),

    #[error("NDArray shape error: {0}")]
    Shape(#[from] ndarray::ShapeError),

    #[error("WAV write error: {0}")]
    Wav(#[from] hound::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Pickle parse error: {0}")]
    Pickle(String),

    #[error("{0}")]
    Other(String),
}

/// Library-wide Result type alias.
pub type Result<T> = std::result::Result<T, KokoroError>;
```

### Error Handling Policy
- **No `try/catch` internally unless the error is expected and recoverable** (per AGENTS.md).
- Errors bubble up via `?` operator.
- The CLI's `main()` may catch and pretty-print errors.
- `ureq::Error` is boxed to avoid large variant size. Could also use `Arc`.
- `ort::Error` may not implement `Send + Sync` depending on version — verify and adjust.

### Conversional Froms

We implement `From` for common std/3rd-party errors so `?` works ergonomically:

```rust
impl From<ureq::Error> for KokoroError {
    fn from(e: ureq::Error) -> Self {
        KokoroError::Http(Box::new(e))
    }
}
```

---

## Data Types (`types.rs`)

### Audio Samples

```rust
/// Audio buffer with sample rate metadata.
#[derive(Clone)]
pub struct AudioSamples {
    /// Sample rate in Hz (always 24000 for Kokoro).
    pub sample_rate: u32,

    /// Raw f32 audio samples. Range typically [-1.0, 1.0].
    pub data: Vec<f32>,
}

impl AudioSamples {
    /// Duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        self.data.len() as f64 / self.sample_rate as f64
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// True if no audio data.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
```

### Voice

```rust
/// Available TTS voices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Voice {
    Af,        // American female (default)
    AfBella,   // American female - Bella
    AfSarah,   // American female - Sarah
    AmAdam,    // American male - Adam
    AmMichael, // American male - Michael
    BfEmma,    // British female - Emma
    BfIsabella,// British female - Isabella
    BmGeorge,  // British male - George
    BmLewis,   // British male - Lewis
    AfNicole,  // American female - Nicole
    AfSky,     // American female - Sky
}
```

### Language

```rust
/// Language variant for espeak phonemization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// American English
    Am,
    /// British English
    Br,
}
```

### Vocabulary Entry

```rust
/// Single entry in the phoneme vocabulary.
/// Maps a Unicode character to its token ID.
#[derive(Debug, Clone)]
struct VocabEntry {
    character: char,
    token_id: u32,
}
```

---

## Validation Contracts

### `Kokoro::generate(text, voice)`

**Preconditions** (enforced by type system):
- `text: &str` — any valid UTF-8 string
- `voice: Voice` — enum, always valid

**Runtime validation**:
- `text.is_empty()` → return empty `AudioSamples` (not an error). Python returns `None`.
- Text produces 0 tokens after phonemization → return empty `AudioSamples` or `EmptyTokens` error.
- Voice pack lacks style ref for required token count → `VoicePackMissingStyle` error.

### `phonemize(text, lang, normalize)`

**Preconditions**:
- `text` is already normalized if `normalize=false` (caller's responsibility)
- `lang` is valid enum

**Runtime validation**:
- espeak returns empty phonemes → `Espeak("empty output")` error
- espeak FFI call fails → `Espeak("initialization failed")` error

### `VoicePack::load(path)`

**Preconditions**:
- `path` exists and is readable

**Runtime validation**:
- Not a valid ZIP file → `InvalidVoicePack("not a zip")`
- Missing `archive/data.pkl` → `InvalidVoicePack("missing data.pkl")`
- Corrupt tensor data → `InvalidVoicePack("bad tensor at index N")`
- Wrong dtype (not float32) → `InvalidVoicePack("expected float32")`

### `KokoroModel::load(path)`

**Preconditions**:
- `path` exists and is readable

**Runtime validation**:
- Not a valid ONNX file → propagated `ort::Error`
- Missing required inputs/outputs → propagated `ort::Error` on first `run()`

---

## Constants

```rust
/// Sample rate of all Kokoro audio output.
pub const SAMPLING_RATE: u32 = 24_000;

/// Maximum tokens per inference chunk (before padding).
pub const MAX_TOKENS: usize = 510;

/// Maximum words per sentence chunk.
pub const MAX_CHUNK_WORDS: usize = 25;

/// Default HuggingFace model commit hash.
pub const DEFAULT_MODEL_COMMIT: &str = "3095858c40fc22e28c46429da9340dfda1f8cf28";

/// HuggingFace base URL for Kokoro-82M.
pub const HF_BASE_URL: &str = "https://huggingface.co/hexgrad/Kokoro-82M/resolve";

/// Kokoro cache directory name (appended to XDG cache dir).
pub const CACHE_DIR_NAME: &str = "kokoro-tts";

/// ONNX model filename.
pub const MODEL_FILENAME: &str = "kokoro-v0_19.onnx";
```

---

## Unsafe Boundaries

| Boundary | Why Unsafe | Mitigation |
|----------|-----------|------------|
| espeak FFI | C library call | `unsafe { }` block, validate all inputs, `std::sync::Once` for init |
| ONNX runtime FFI | `ort` crate handles this internally | Trust the crate; add integration tests |
| Pickle parsing | Parsing untrusted binary format (downloaded from HF) | Validate all offsets/sizes before reading; fail on any parse error |

---

## Send + Sync Status

| Type | Send | Sync | Notes |
|------|------|------|-------|
| `KokoroError` | Yes | Yes | All variants contain Send+Sync types |
| `AudioSamples` | Yes | Yes | `Vec<f32>` is both |
| `Voice` | Yes | Yes | Copy type |
| `Kokoro` | **No** | **No** | Contains `Mutex<KokoroState>` which wraps `ort::Session` (!Send) |
| `VoicePack` | Yes | Yes | HashMap<usize, Vec<f32>> is both |
| `KokoroModel` | **No** | **No** | Contains `ort::Session` (!Send) |

For the CLI producer-consumer pattern, wrap `Kokoro` in `Arc<Mutex<Kokoro>>` so it can be shared between the TTS thread and potentially the main thread for cancelling.
