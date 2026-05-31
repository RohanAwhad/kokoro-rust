# 007: Testing & Migration Strategy

**Dependencies**: All preceding PRDs
**Severity**: medium

## Test Strategy

### Test Tiers

| Tier | What | Framework | Speed | Scope |
|------|------|-----------|-------|-------|
| Unit | Individual functions | `#[test]` (built-in) | Fast (<1s) | Per-function correctness |
| Integration | Pipeline end-to-end | `#[test]` in `tests/` | Medium | Cross-module behavior |
| Parity | Python vs Rust output | Script + snapshot | Medium | Byte-identical output verification |
| Property | Random input fuzzing | `proptest` or manual | Slow | Invariants (no panic, consistent shapes) |

### Unit Tests (per module)

#### `normalize.rs` tests
```rust
#[test]
fn test_normalize_dr_abbreviation() {
    assert_eq!(normalize_text("Dr. Smith"), "Doctor Smith");
}

#[test]
fn test_normalize_money_dollars() {
    let result = normalize_text("$5.99");
    assert!(result.contains("5 dollars") && result.contains("99 cents"));
}

#[test]
fn test_normalize_empty_string() {
    assert_eq!(normalize_text(""), "");
}

#[test]
fn test_normalize_unicode_quotes() {
    assert_eq!(normalize_text("\u{201c}hello\u{201d}"), "\"hello\"");
}

#[test]
fn test_normalize_yeah() {
    assert_eq!(normalize_text("Yeah"), "Ye'a");
}
```

#### `phonemes.rs` tests
```rust
#[test]
fn test_phonemize_hello() {
    let result = phonemize("hello", Lang::Am, true).unwrap();
    assert!(!result.is_empty());
    // Verify contains known IPA symbols
    assert!(result.contains('h') || result.contains('ə'));
}

#[test]
fn test_phonemize_empty() {
    let result = phonemize("", Lang::Am, true).unwrap();
    assert_eq!(result, "");
}
```

#### `vocab.rs` tests
```rust
#[test]
fn test_vocab_size() {
    const EXPECTED_SIZE: usize = 251;  // verify with Python
    assert_eq!(VOCAB.len(), EXPECTED_SIZE);
}

#[test]
fn test_tokenize_known_phonemes() {
    let tokens = tokenize("həˈloʊ");
    assert!(!tokens.is_empty());
}

#[test]
fn test_tokenize_empty() {
    let tokens = tokenize("");
    assert!(tokens.is_empty());
}
```

#### `voice.rs` tests
```rust
#[test]
#[ignore = "requires network"]
fn test_download_and_parse_all_voices() {
    for voice in Voice::ALL {
        let pack = VoicePack::download_and_load(*voice).unwrap();
        assert!(pack.max_tokens() >= 1);
        // Verify style at token count 5 exists
        let style = pack.get_style(5).unwrap();
        assert!(!style.is_empty());
    }
}
```

#### `model.rs` tests
```rust
#[test]
#[ignore = "requires model download"]
fn test_model_load_and_infer() {
    let model = KokoroModel::load(Path::new("test_fixtures/model.onnx")).unwrap();
    // Test with known tokens and style
    let audio = model.run(&[0, 42, 0], &[0.0f32; 256], 1.0).unwrap();
    assert!(!audio.is_empty());
}
```

### Integration Tests

Located in `crates/kokoro/tests/`:

```rust
// tests/integration_test.rs
use kokoro::{Kokoro, Voice};

#[test]
#[ignore = "requires network + model download"]
fn test_generate_hello_world() {
    let kk = Kokoro::new().unwrap();
    let audio = kk.generate("Hello world!", Voice::AfSky).unwrap();
    assert!(!audio.is_empty());
    assert_eq!(audio.sample_rate, 24000);
    // A valid TTS of "Hello world" should be >1000 samples (>~40ms)
    assert!(audio.len() > 1000);
}

#[test]
#[ignore = "requires network + model download"]
fn test_all_voices_generate() {
    let kk = Kokoro::new().unwrap();
    for voice in Voice::ALL {
        let audio = kk.generate("Test.", *voice).unwrap();
        assert!(!audio.is_empty(), "Voice {:?} produced empty audio", voice);
    }
}

#[test]
#[ignore = "requires network + model download"]
fn test_long_text_chunking() {
    let kk = Kokoro::new().unwrap();
    let long_text = "This is a test. ".repeat(100);  // 400 words
    let audio = kk.generate(&long_text, Voice::AfSky).unwrap();
    assert!(!audio.is_empty());
}

#[test]
fn test_chunk_sentences_max_words() {
    let sentences: Vec<String> = (0..20).map(|i| format!("word{}", i)).collect();
    let chunks = chunk_sentences(&sentences, 5);
    assert_eq!(chunks.len(), 4);
    for chunk in &chunks {
        assert!(chunk.split_whitespace().count() <= 5);
    }
}
```

### Python Parity Tests

Critical for the port: verify Rust output matches Python byte-for-byte.

**Approach**:
1. Create a Python script that, given a text input, outputs the intermediate results at each stage:
   - After `normalize_text()`
   - After `phonemize()`
   - After `tokenize()`
   - Raw ONNX output (f32 binary)
2. Save these as test fixtures (`.txt`, `.bin` files)
3. Rust tests load fixtures and compare output at each stage

```rust
#[test]
fn test_parity_normalize() {
    let input = std::fs::read_to_string("test_fixtures/parity_input.txt").unwrap();
    let expected = std::fs::read_to_string("test_fixtures/parity_normalized.txt").unwrap();
    assert_eq!(normalize_text(&input), expected.trim());
}

#[test]
fn test_parity_tokenize() {
    let phonemes = std::fs::read_to_string("test_fixtures/parity_phonemes.txt").unwrap();
    let expected_tokens: Vec<u32> = serde_json::from_str(
        &std::fs::read_to_string("test_fixtures/parity_tokens.json").unwrap()
    ).unwrap();
    assert_eq!(tokenize(phonemes.trim()), expected_tokens);
}

#[test]
#[ignore = "requires model"]
fn test_parity_audio_output() {
    // Load known tokens + style ref from fixtures
    // Run ONNX inference
    // Compare f32 audio output byte-for-byte with Python output
}
```

### Property Tests (with `proptest`)

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn normalize_never_panics(s in "\\PC*") {
        let _ = normalize_text(&s);  // should never panic
    }

    #[test]
    fn tokenize_output_in_vocab_range(phonemes in "[a-zA-Zɑɐɒæɓʙβɔɕçɗɖðʤə]+") {
        let tokens = tokenize(&phonemes);
        for &t in &tokens {
            assert!(t < VOCAB.len() as u32);
        }
    }
}
```

---

## Migration Checklist

### Phase 1: Foundation
- [ ] Cargo workspace setup (`Cargo.toml`, `crates/kokoro/`, `crates/kokoro-cli/`)
- [ ] Error types (`error.rs`)
- [ ] Shared types (`types.rs`)

### Phase 2: Text Pipeline
- [ ] `normalize_text()` with all 25 regex rules + 3 callbacks
- [ ] espeak-ng FFI binding + `phonemize()`
- [ ] Vocabulary LUT + `tokenize()`
- [ ] `sentence_tokenize()` (rule-based)
- [ ] `chunk_sentences()` helper
- [ ] Parity tests pass for normalize → phonemize → tokenize

### Phase 3: Voice Packs
- [ ] `.pt` ZIP parser (or `.kokoro` custom format)
- [ ] `VoicePack::load()` with validation
- [ ] `get_style()` lookup
- [ ] `Voice` enum with all 11 voices + commit hashes
- [ ] HTTP download + caching (`cache.rs`)

### Phase 4: ONNX Inference
- [ ] `KokoroModel::load()` — load ONNX session
- [ ] `KokoroModel::run()` — inference with ndarray tensors
- [ ] Input/output shape validation
- [ ] Audio parity test with known inputs

### Phase 5: Kokoro API
- [ ] `Kokoro::new()` / `Kokoro::with_cache_dir()`
- [ ] Lazy model + voice loading (`ensure_*` methods)
- [ ] `Kokoro::generate()` — full pipeline
- [ ] Integration tests pass

### Phase 6: CLI
- [ ] `kokoro-cli` binary with clap
- [ ] Producer-consumer streaming with rodio
- [ ] Ctrl+C signal handling
- [ ] `--output` WAV flag
- [ ] `--voice` flag
- [ ] Smoke test: `kokoro-tts "Hello world"`

### Phase 7: Polish
- [ ] Logging at appropriate levels (debug for pipeline steps, info for download progress)
- [ ] Error messages are user-friendly
- [ ] README updated with Rust usage
- [ ] `--quiet` flag suppresses non-error output
- [ ] CI-friendly (headless test mode, skip audio playback in tests)

---

## Devlog Convention

Update `devlogs.md` in the kokoro-rust repo root after each phase. Format:

```markdown
## 2026-05-31: Phase 1 — Foundation
- Created workspace with crates/kokoro and crates/kokoro-cli
- Added error types, shared types, constants
- Verified: cargo build succeeds
```

---

## Acceptance Criteria (Overall)

1. `kokoro-tts "Hello world"` produces audible speech through speakers
2. `kokoro-tts -o output.wav "Hello world"` creates a valid WAV file
3. All 11 voices work
4. Byte-identical audio output to Python for same inputs (parity tests pass)
5. No Python runtime dependency
6. Model + voices download once and cache
7. Ctrl+C gracefully stops playback
8. `cargo test` passes all unit tests (integration tests marked `#[ignore]` for CI)
9. `cargo build --release` produces a single binary (~5-15MB, not counting ONNX runtime shared lib)
