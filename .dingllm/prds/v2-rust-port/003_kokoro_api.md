# 003: Kokoro API

**Dependencies**: 000_overview, 001_text_pipeline, 002_onnx_inference, 004_voice_pack_parsing
**Severity**: high
**Python source**: `src/kokoro_tts/kokoro.py` lines 163–299

## Current Behavior (Python)

The `Kokoro` class provides:
- 11 voice constants with HuggingFace commit hashes
- Lazy model download + ONNX session creation
- Lazy voice download + `.pt` file loading
- `generate(text, voice_name) → np.ndarray` — full TTS pipeline
- Sentence chunking (max 25 words, max 510 tokens)
- Static SAMPLING_RATE = 24000

## Desired Behavior (Rust)

```rust
use kokoro::{Kokoro, Voice, AudioSamples};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kk = Kokoro::new()?;  // or Kokoro::with_cache_dir("/custom/path")
    let audio: AudioSamples = kk.generate("Hello world!", Voice::AfSky)?;
    // audio.sample_rate == 24000
    // audio.data: Vec<f32>
    Ok(())
}
```

---

## Public API Surface

### Types (`types.rs`)

```rust
use std::fmt;
use serde::{Serialize, Deserialize};

/// Audio sample rate in Hz.
pub const SAMPLING_RATE: u32 = 24_000;

/// Audio output container.
#[derive(Clone)]
pub struct AudioSamples {
    pub sample_rate: u32,
    pub data: Vec<f32>,
}

impl AudioSamples {
    /// Duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        self.data.len() as f64 / self.sample_rate as f64
    }
}

/// English variant (determines espeak voice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// American English (voice code 'a')
    Am,
    /// British English (voice code 'b')
    Br,
}

/// Available voices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Voice {
    Af,
    AfBella,
    AfSarah,
    AmAdam,
    AmMichael,
    BfEmma,
    BfIsabella,
    BmGeorge,
    BmLewis,
    AfNicole,
    AfSky,
}

impl Voice {
    /// All available voices as a slice.
    pub const ALL: &[Voice] = &[
        Voice::Af, Voice::AfBella, Voice::AfSarah,
        Voice::AmAdam, Voice::AmMichael,
        Voice::BfEmma, Voice::BfIsabella,
        Voice::BmGeorge, Voice::BmLewis,
        Voice::AfNicole, Voice::AfSky,
    ];

    /// HF commit hash for this voice's .pt file.
    pub fn commit_hash(&self) -> &'static str { ... }

    /// Language variant (a=American, b=British).
    pub fn lang(&self) -> Lang {
        match self {
            Voice::Af | Voice::AfBella | Voice::AfSarah
            | Voice::AmAdam | Voice::AmMichael
            | Voice::AfNicole | Voice::AfSky => Lang::Am,
            Voice::BfEmma | Voice::BfIsabella
            | Voice::BmGeorge | Voice::BmLewis => Lang::Br,
        }
    }

    /// espeak language code.
    pub fn espeak_lang(&self) -> &'static str {
        match self.lang() {
            Lang::Am => "en-us",
            Lang::Br => "en-gb",
        }
    }

    /// Voice filename (on HF).
    pub fn filename(&self) -> &'static str {
        match self {
            Voice::Af => "af",
            Voice::AfBella => "af_bella",
            Voice::AfSarah => "af_sarah",
            Voice::AmAdam => "am_adam",
            Voice::AmMichael => "am_michael",
            Voice::BfEmma => "bf_emma",
            Voice::BfIsabella => "bf_isabella",
            Voice::BmGeorge => "bm_george",
            Voice::BmLewis => "bm_lewis",
            Voice::AfNicole => "af_nicole",
            Voice::AfSky => "af_sky",
        }
    }
}

impl fmt::Display for Voice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.filename())
    }
}
```

### Kokoro Struct (`kokoro.rs`)

```rust
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Kokoro {
    cache_dir: PathBuf,
    model_url: String,

    // Lazy-initialized state
    state: Mutex<KokoroState>,
}

struct KokoroState {
    model: Option<KokoroModel>,       // from PRD 002
    voice_pack: Option<VoicePack>,    // from PRD 004
    current_voice: Option<Voice>,     // tracks which voice is loaded
}
```

#### Public Methods

```rust
impl Kokoro {
    /// Create with default cache dir (~/.cache/kokoro-tts).
    pub fn new() -> Result<Self, KokoroError>;

    /// Create with custom cache directory.
    pub fn with_cache_dir(cache_dir: impl Into<PathBuf>) -> Self;

    /// Generate audio from text using the given voice.
    /// This is the main entry point.
    pub fn generate(&self, text: &str, voice: Voice) -> Result<AudioSamples, KokoroError>;

    /// Get cache directory path.
    pub fn cache_dir(&self) -> &PathBuf;
}
```

### Generate Pipeline

The `generate()` method implements the full pipeline. Must match Python logic:

```rust
impl Kokoro {
    pub fn generate(&self, text: &str, voice: Voice) -> Result<AudioSamples, KokoroError> {
        // 1. Sentence tokenize (rule-based, see below)
        let sentences = sentence_tokenize(text);

        // 2. Group sentences into chunks ≤25 words
        let chunks = chunk_sentences(&sentences, 25);

        // 3. Ensure voice is loaded (lazy)
        self.ensure_voice_loaded(voice)?;

        // 4. Ensure model is loaded (lazy)
        self.ensure_model_loaded()?;

        // 5. Generate audio per chunk
        let mut audio_chunks: Vec<Vec<f32>> = Vec::new();
        for chunk in &chunks {
            let lang = voice.lang();

            // 5a. Phonemize + tokenize
            let tokens = text_to_tokens(chunk, lang)?;

            // 5b. Truncate to 510 tokens if needed
            let tokens = if tokens.len() > 510 {
                log::warn!("Chunk truncated from {} to 510 tokens", tokens.len());
                tokens[..510].to_vec()
            } else {
                tokens
            };

            if tokens.is_empty() {
                continue;  // skip empty chunks
            }

            // 5c. Get style reference from voice pack (indexed by batch size = 1)
            let style_ref = self.state.lock().unwrap()
                .voice_pack.as_ref()
                .ok_or(KokoroError::VoiceNotLoaded)?
                .get_style()?
                .to_vec();

            // 5d. Pad tokens with [0, ..., 0]
            let mut padded_tokens = vec![0i64];
            padded_tokens.extend(tokens.iter().map(|&t| t as i64));
            padded_tokens.push(0);

            // 5e. Run ONNX inference
            let audio = self.state.lock().unwrap()
                .model.as_ref()
                .ok_or(KokoroError::ModelNotLoaded)?
                .run(&padded_tokens, &style_ref, 1.0)?;

            audio_chunks.push(audio);
        }

        // 6. Concatenate all chunk audio
        let total_len: usize = audio_chunks.iter().map(|c| c.len()).sum();
        let mut combined = Vec::with_capacity(total_len);
        for chunk in audio_chunks {
            combined.extend(chunk);
        }

        Ok(AudioSamples {
            sample_rate: SAMPLING_RATE,
            data: combined,
        })
    }
}
```

### Lazy Loading (Internal)

```rust
impl Kokoro {
    fn ensure_model_loaded(&self) -> Result<(), KokoroError> {
        let mut state = self.state.lock().unwrap();
        if state.model.is_some() {
            return Ok(());
        }

        // Download model if needed
        let model_path = self.cache_dir.join("kokoro-v0_19.onnx");
        if !model_path.exists() {
            download_file(&self.model_url, &model_path)?;
        }

        // Load ONNX session
        state.model = Some(KokoroModel::load(&model_path)?);
        Ok(())
    }

    fn ensure_voice_loaded(&self, voice: Voice) -> Result<(), KokoroError> {
        let mut state = self.state.lock().unwrap();
        if state.current_voice == Some(voice) && state.voice_pack.is_some() {
            return Ok(());
        }

        // Download voice if needed
        let voice_dir = self.cache_dir.join("voices");
        let voice_path = voice_dir.join(format!("{}.pt", voice.filename()));
        if !voice_path.exists() {
            let voice_url = build_voice_url(voice);
            download_file(&voice_url, &voice_path)?;
        }

        // Parse .pt file
        state.voice_pack = Some(VoicePack::load(&voice_path)?);
        state.current_voice = Some(voice);
        Ok(())
    }
}
```

### Sentence Tokenization

The Python code uses `nltk.sent_tokenize()`. Rust implementation should be rule-based:

```rust
/// Rule-based sentence tokenizer.
/// Splits on . ! ? followed by space/capital letter.
/// Approximates nltk.sent_tokenize behavior.
fn sentence_tokenize(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();

    for i in 0..bytes.len() {
        if matches!(bytes[i], b'.' | b'!' | b'?') {
            // Check if followed by space and capital letter or end of text
            let is_sentence_end = i + 1 >= bytes.len()
                || (bytes[i + 1] == b' ' && i + 2 < bytes.len() && bytes[i + 2].is_ascii_uppercase());

            if is_sentence_end {
                let sentence = text[start..=i].trim().to_string();
                if !sentence.is_empty() {
                    sentences.push(sentence);
                }
                start = i + 1;
            }
        }
    }

    // Last sentence
    let tail = text[start..].trim().to_string();
    if !tail.is_empty() {
        sentences.push(tail);
    }

    sentences
}
```

### Chunking Logic

```rust
/// Group sentences into chunks where each chunk has ≤ max_words words.
fn chunk_sentences(sentences: &[String], max_words: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut current_len = 0;

    for sent in sentences {
        let words: Vec<&str> = sent.split_whitespace().collect();
        if current_len + words.len() > max_words {
            if !current.is_empty() {
                chunks.push(current.join(" "));
                current.clear();
                current_len = 0;
            }
        }
        current.extend(words.iter().copied());
        current_len += words.len();
    }

    if !current.is_empty() {
        chunks.push(current.join(" "));
    }

    chunks
}
```

## Cache and Download Logic (`cache.rs`)

### Cache Directory

```rust
pub fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kokoro-tts")
}
```

### HTTP Download

```rust
use std::io::Read;

pub fn download_file(url: &str, dest: &Path) -> Result<(), KokoroError> {
    log::info!("Downloading: {}", url);

    // Create parent directory
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let response = ureq::get(url).call()?;
    let mut body = Vec::new();
    response.into_reader().read_to_end(&mut body)?;

    std::fs::write(dest, &body)?;
    log::info!("Downloaded to: {}", dest.display());
    Ok(())
}
```

### Model URL Construction

```rust
const MODEL_COMMIT: &str = "3095858c40fc22e28c46429da9340dfda1f8cf28";
const HF_BASE: &str = "https://huggingface.co/hexgrad/Kokoro-82M/resolve";

pub fn model_url() -> String {
    format!("{HF_BASE}/{MODEL_COMMIT}/kokoro-v0_19.onnx")
}

/// Voice pack URL — downloaded from our GitHub releases as .kokoro (pre-converted from .pt).
const VOICE_RELEASE_BASE: &str = "https://github.com/RohanAwhad/kokoro-rust/releases/download/voices";

pub fn voice_url(voice: Voice) -> String {
    format!("{VOICE_RELEASE_BASE}/{}.kokoro", voice.filename())
}
```

## Acceptance Criteria

1. `Kokoro::new()` creates instance without downloading model or voice
2. First `generate()` call downloads model + voice, loads ONNX session
3. Subsequent calls reuse loaded resources
4. Switching voices triggers new voice download + parse
5. `generate("Hello world!", Voice::AfSky)` produces valid f32 audio
6. Byte-identical output to Python for same inputs
7. All 11 voices work correctly
8. Long text (>25 words) is chunked correctly
