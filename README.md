# kokoro-rust

Rust port of [kokoro-tts](https://github.com/RohanAwhad/kokoro-tts) — a fast wrapper around the Kokoro-82M ONNX text-to-speech model.

> ~1.7s to generate ~1.7s of audio (ONNX Runtime on M1 Mac)

## Install

```bash
# Prerequisites
brew install espeak-ng

# Clone
git clone https://github.com/RohanAwhad/kokoro-rust.git
cd kokoro-rust
```

## Setup

Download the ONNX model:

```bash
curl -L -o ~/Library/Caches/kokoro-tts/kokoro-v0_19.onnx \
  "https://huggingface.co/hexgrad/Kokoro-82M/resolve/main/kokoro-v0_19.onnx"
```

> **Linux**: cache dir is `~/.cache/kokoro-tts/` instead of `~/Library/Caches/kokoro-tts/`.

Download and convert a voice pack:

```bash
curl -L -o /tmp/af_sky.pt \
  "https://huggingface.co/hexgrad/Kokoro-82M/resolve/main/voices/af_sky.pt"

mkdir -p ~/Library/Caches/kokoro-tts/voices/
./scripts/convert_voices.py /tmp/af_sky.pt \
  ~/Library/Caches/kokoro-tts/voices/af_sky.kokoro
```

## Usage

**CLI** (streams audio in real-time):

```bash
cargo run --bin kokoro-tts -- "Hello world!"
```

Write to WAV:

```bash
cargo run --bin kokoro-tts -- -o output.wav "Hello world!"
```

**Python API:**

```python
from kokoro import Kokoro  # Rust via PyO3 (future)
```

**Rust API:**

```rust
use kokoro::{Kokoro, Voice};

let kk = Kokoro::new();
let audio = kk.generate("Hello world!", Voice::AfSky)?;
// audio.sample_rate == 24000
// audio.data: Vec<f32>
```

## Voices

| Voice | Language |
|-------|----------|
| `af` | American female |
| `af_bella` | American female |
| `af_sarah` | American female |
| `am_adam` | American male |
| `am_michael` | American male |
| `bf_emma` | British female |
| `bf_isabella` | British female |
| `bm_george` | British male |
| `bm_lewis` | British male |
| `af_nicole` | American female |
| `af_sky` | American female (CLI default) |

## Build

```bash
cargo build --release
# Binary: target/release/kokoro-tts
```
