# 001: Text Processing Pipeline

**Dependencies**: 000_overview
**Severity**: high
**Python source**: `src/kokoro_tts/kokoro.py` lines 16–117

## Current Behavior (Python)

Three-stage pipeline:
1. `normalize_text(text: str) → str` — regex-based text normalization
2. `phonemize(text: str, lang: str) → str` — espeak-ng IPA phoneme conversion
3. `tokenize(phonemes: str) → list[int]` — lookup each char in VOCAB LUT

Plus `split_num()`, `flip_money()`, `point_num()` helpers for normalization.

## Desired Behavior (Rust)

Pure Rust implementation of all three stages. Byte-identical output to Python for identical inputs (verified via test vectors).

### Why this matters

If text preprocessing differs by even one character, the ONNX model gets different token inputs and produces different audio. Parity in this pipeline is the foundation of the port.

---

## Stage 1: Text Normalization (`normalize.rs`)

### Function Signature

```rust
/// Normalize input text for phonemization.
/// Handles quotes, punctuation, numbers, money, abbreviations.
pub fn normalize_text(text: &str) -> String;
```

### Rule Pipeline (must match Python exactly)

Apply these transforms in order (match `kokoro.py` lines 57–83):

| Step | Python Pattern | Rust Regex | Description |
|------|---------------|------------|-------------|
| 1 | `text.replace(chr(8216), "'").replace(chr(8217), "'")` | `replace(''','\'').replace(''','\'')` | Unicode curly single quotes → ASCII |
| 2 | `text.replace('«', chr(8220)).replace('»', chr(8221))` | `replace('«','"').replace('»','"')` | Guillemets → double quotes |
| 3 | `text.replace(chr(8220), '"').replace(chr(8221), '"')` | Same | Curly double quotes → ASCII |
| 4 | `text.replace('(', '«').replace(')', '»')` | Same | Parens → guillemets |
| 5 | CJK punctuation → `,!. ,: ;? ` | Same | Chinese/Japanese punctuation normalization |
| 6 | `re.sub(r'[^\S \n]', ' ', text)` | Same | Non-space, non-newline whitespace → space |
| 7 | `re.sub(r'  +', ' ', text)` | Same | Collapse multiple spaces |
| 8 | `re.sub(r'(?<=\n) +(?=\n)', '', text)` | Same | Remove spaces between newlines |
| 9 | `re.sub(r'\bD[Rr]\.(?= [A-Z])', 'Doctor', text)` | Same | Dr. → Doctor |
| 10 | `re.sub(r'\b(?:Mr\.\|MR\.(?= [A-Z]))', 'Mister', text)` | Same | Mr. → Mister |
| 11 | `re.sub(r'\b(?:Ms\.\|MS\.(?= [A-Z]))', 'Miss', text)` | Same | Ms. → Miss |
| 12 | `re.sub(r'\b(?:Mrs\.\|MRS\.(?= [A-Z]))', 'Mrs', text)` | Same | Mrs. → Mrs (no dot) |
| 13 | `re.sub(r'\betc\.(?! [A-Z])', 'etc', text)` | Same | etc. → etc |
| 14 | `re.sub(r'(?i)\b(y)eah?\b', r"\1e'a", text)` | Same | yeah/yea → ye'a |
| 15 | `re.sub(r'\d*\.\d+\|\b\d{4}s?\b\|(?<!:)\b(?:[1-9]\|1[0-2]):[0-5]\d\b(?!:)', split_num)` | Regex | Number normalization (year, time, decimal) |
| 16 | `re.sub(r'(?<=\d),(?=\d)', '', text)` | Same | Remove commas between digits |
| 17 | `re.sub(r'(?i)[$£]\d+(?:\.\d+)?...)', flip_money, text)` | Regex | Dollar/pound amounts → words |
| 18 | `re.sub(r'\d*\.\d+', point_num, text)` | Same | Decimal numbers → "point" form |
| 19 | `re.sub(r'(?<=\d)-(?=\d)', ' to ', text)` | Same | Digit-digit range → "to" |
| 20 | `re.sub(r'(?<=\d)S', ' S', text)` | Same | Space before S after digit |
| 21 | `re.sub(r"(?<=[BCDFGHJ-NP-TV-Z])'?s\b", "'S", text)` | Same | Possessive s normalization |
| 22 | `re.sub(r"(?<=X')S\b", 's', text)` | Same | X's exception |
| 23 | `re.sub(r'(?:[A-Za-z]\.){2,} [a-z]', lambda...', text)` | Callback | Abbreviation dot → hyphen |
| 24 | `re.sub(r'(?i)(?<=[A-Z])\.(?=[A-Z])', '-', text)` | Same | U.S. → U-S |
| 25 | `text.strip()` | Same | Trim whitespace |

### Callback Functions (in Rust)

Each callback is a function that receives the matched `&str` and returns a `String`:

```rust
fn split_num(m: &str) -> String { ... }
fn flip_money(m: &str) -> String { ... }
fn point_num(m: &str) -> String { ... }
```

These are 1:1 translations of the Python logic — string parsing, no numeric computation.

### Regex Crate Notes

The Python regex uses `(?i)` for case-insensitive matching, `(?<=...)` for lookbehind, `(?!...)` for negative lookahead. The `regex` crate supports all of these.

For callback-based replacement (steps 15, 17, 18, 23), use `regex::Regex::replace_all` with a `Captures` closure. Steps 15 and 17 use `re.sub(pattern, callback_fn, text)` which `regex` supports natively.

---

## Stage 2: Phonemization (`phonemes.rs`)

### Function Signature

```rust
use crate::types::Lang;

/// Convert normalized text to IPA phonemes via espeak-ng.
pub fn phonemize(text: &str, lang: Lang, normalize: bool) -> Result<String, KokoroError>;
```

### espeak-ng FFI

The Python code uses `phonemizer` which wraps espeak-ng's C library:

```python
phonemizers = dict(
    a=phonemizer.backend.EspeakBackend(language='en-us', preserve_punctuation=True, with_stress=True),
    b=phonemizer.backend.EspeakBackend(language='en-gb', preserve_punctuation=True, with_stress=True),
)
```

In Rust, we call espeak-ng C API directly via `espeak-sys`. The relevant C functions:

```c
int espeak_Initialize(AUDIO_OUTPUT output, int buflength, const char *path, int options);
const char* espeak_TextToPhonemes(const void **textptr, int textmode, int phonememode);
```

Key requirements:
- **Language**: `en-us` (voice code `a`) or `en-gb` (voice code `b`)
- **Phoneme mode**: IPA output (mode `0x02` in espeak)
- **Preserve punctuation**: Pass non-zero `phoneme_options`
- **With stress**: Depends on espeak flags

### Phoneme Post-Processing (must match Python lines 110–116)

After espeak generates phonemes, apply these filters:

1. Replace `kəkˈoːɹoʊ` → `kˈoʊkəɹoʊ` (fix "kokoro" pronunciation)
2. Replace `kəkˈɔːɹəʊ` → `kˈəʊkəɹəʊ` (British kokoro fix)
3. `ʲ→j`, `r→ɹ`, `x→k`, `ɬ→l` — character substitutions
4. Insert space before `hˈʌndɹɪd` after lowercase/ɹ/ː
5. Remove `z` at end of sentence boundaries
6. For American English (`lang == 'a'`): fix `nˈaɪnti` → `nˈaɪndi` (ninety/nty fix)
7. Filter: keep only characters that exist in `VOCAB`
8. `trim()` the result

### espeak Initialization Constraint

espeak-ng is NOT thread-safe. `espeak_Initialize()` must be called once (likely on first `phonemize()` call or at `Kokoro` struct init). Use `std::sync::Once` for one-time init.

```rust
static ESPEAK_INIT: std::sync::Once = std::sync::Once::new();

fn ensure_espeak_initialized() {
    ESPEAK_INIT.call_once(|| {
        unsafe { espeak_sys::espeak_Initialize(
            espeak_sys::AUDIO_OUTPUT_AUDIO_OUTPUT_SYNCHRONOUS,
            0, std::ptr::null(), 0
        )};
    });
}
```

---

## Stage 3: Tokenization (`vocab.rs`)

### Vocabulary (`get_vocab()`)

A static character→integer lookup table. Must match Python VOCAB exactly.

```rust
use std::collections::HashMap;

lazy_static! {
    static ref VOCAB: HashMap<char, u32> = build_vocab();
}

fn build_vocab() -> HashMap<char, u32> {
    let _pad = '$';
    let _punctuation = ";:,.!?¡¿—…\"«»\"\" ";   // includes space
    let _letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let _letters_ipa = "ɑɐɒæɓʙβɔɕçɗɖðʤəɘɚɛɜɝɞɟʄɡɠɢʛɦɧħɥʜɨɪʝɭɬɫɮʟɱɯɰŋɳɲɴøɵɸθœɶʘɹɺɾɻʀʁɽʂʃʈʧʉʊʋⱱʌɣɤʍχʎʏʑʐʒʔʡʕʢǀǁǂǃˈˌːˑʼʴʰʱʲʷˠˤ˞↓↑→↗↘'̩'ᵻ";
    // Build as in Python: _pad + _punctuation chars + _letters chars + _letters_ipa chars
    // Each char mapped to its index
}
```

### Tokenize Function

```rust
/// Convert phoneme string to token IDs, filtering chars not in VOCAB.
pub fn tokenize(phonemes: &str) -> Vec<u32> {
    phonemes.chars()
        .filter_map(|c| VOCAB.get(&c))
        .copied()
        .collect()
}
```

### Vocabulary Size

The Python VOCAB has ~251 entries. Verify by counting: `$` (1) + punctuation (19 counting space) + letters (52) + IPA chars (~179) = ~251. The Rust implementation must have identical indices.

---

## Integration: Full Pipeline

```rust
use crate::normalize::normalize_text;
use crate::phonemes::phonemize;
use crate::vocab::tokenize;
use crate::types::Lang;

/// Full text → token pipeline.
pub fn text_to_tokens(text: &str, lang: Lang) -> Result<Vec<u32>, KokoroError> {
    let normalized = normalize_text(text);
    let phonemes = phonemize(&normalized, lang, false)?;  // norm=false because we already normalized
    Ok(tokenize(&phonemes))
}
```

Note: In the Python code, `phonemize()` calls `normalize_text()` internally when `norm=True`. We separate them for clarity — the `Kokoro` class calls `phonemize(text, lang)` internally which normalizes by default.

## Test Vectors

Must test with known English text and verify:
1. `normalize_text("Dr. Smith paid $5.99 in 2024.")` → `"Doctor Smith paid 5 dollars and 99 cents in twenty twenty four."`
2. `phonemize("hello", Lang::Am)` → IPA phoneme string
3. `tokenize("<known IPA string>")` → known token list
4. Compare full pipeline output byte-for-byte with Python

## Edge Cases

- Empty string → empty token list (return `None` in Python, return `vec![]` in Rust but the caller should handle gracefully)
- Text with only punctuation → tokens should contain only punctuation token IDs
- Input >510 tokens → truncate with warning (python behavior)
- Unicode normalization: Python normalizes curly quotes and CJK punctuation. Ensure Rust handles Unicode chars in regex (use `\u{...}` escapes or direct char literals).
