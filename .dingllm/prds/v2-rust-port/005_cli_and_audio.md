# 005: CLI & Audio I/O

**Dependencies**: 000_overview, 003_kokoro_api
**Severity**: high
**Python source**: `src/kokoro_tts/cli.py` (90 lines), `examples/long_text.py` (90 lines)

## Current Behavior (Python)

CLI with producer-consumer threading for real-time streaming playback:

```bash
kokoro-tts "Hello world! I am Kokoro text-to-speech model."
```

Internal flow:
1. Split text into sentences (on `.`, `!`, `?`)
2. Group sentences into chunks of 2 sentences each
3. **Producer thread** (TTS worker): generate audio for each chunk, push to queue
4. **Consumer thread** (player): pop from queue, play via `sounddevice`
5. Sentinel `None` signals end of stream
6. `signal.SIGINT`/`SIGTERM` handlers stop playback

Uses `af_sky` voice hardcoded.

## Desired Behavior (Rust)

```
kokoro-tts "Hello world!"
```

Same behavior, implemented with:
- `clap` for argument parsing
- `std::thread` + `crossbeam-channel` for producer-consumer
- `cpal` for audio playback
- `ctrlc` for signal handling

---

## CLI Entry Point (`crates/kokoro-cli/src/main.rs`)

```rust
use clap::Parser;
use crossbeam_channel::bounded;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kokoro::{Kokoro, Voice, AudioSamples, SAMPLING_RATE};

const VOICE: Voice = Voice::AfSky;

/// Kokoro Text-to-Speech CLI — speak text from the command line.
#[derive(Parser)]
#[command(name = "kokoro-tts", version, about)]
struct Cli {
    /// Text to speak
    text: String,

}

fn main() {
    env_logger::init();
    let cli = Cli::parse();
    speak(cli.text);
}
```

### Audio Playback with cpal

```rust
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

struct AudioPlayer {
    stream: cpal::Stream,
}

impl AudioPlayer {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or("No audio output device found")?;

        let config = cpal::StreamConfig {
            channels: 1,
            sample_rate: cpal::SampleRate(SAMPLING_RATE),
            buffer_size: cpal::BufferSize::Default,
        };

        // Stream will be created per-playback (see play_audio below)
        unimplemented!("Stream setup in play_audio function")
    }
}
```

Actually, cpal's API requires creating a stream with a callback. For our producer-consumer pattern, we need a blocking API. Options:

**Option A: Use `rodio` crate** (simpler API):
```rust
use rodio::{OutputStream, Sink, Source};
use std::time::Duration;

fn play_audio(audio: &[f32]) -> Result<(), Box<dyn std::error::Error>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let source = rodio::buffer::SamplesBuffer::new(
        1,                           // channels
        SAMPLING_RATE,               // sample rate
        audio.to_vec(),              // samples
    );
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
```

`rodio` is simpler but less flexible for streaming. For the producer-consumer pattern where we want to start playback before all audio is generated, we'd need a more sophisticated approach.

**Option B: Use `cpal` directly with ring buffer**:
More complex but gives fine-grained control. The player thread writes to a ring buffer that cpal's callback reads from.

**Decision**: Use `rodio` for initial implementation (simpler). The Python producer-consumer pattern provides smoother playback for long text, but `rodio`'s `Sink` can accept chunks as they arrive. Upgrade to cpal ring buffer later if latency/jank is an issue.

### Producer-Consumer Pattern

```rust
fn speak(text: String) {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    // Handle Ctrl+C
    ctrlc::set_handler(move || {
        stop_clone.store(true, Ordering::SeqCst);
    }).expect("Failed to set Ctrl+C handler");

    // Split text into sentences
    let sentences = split_sentences(&text);
    // Group into chunks of 2 sentences
    let chunks: Vec<String> = sentences
        .chunks(2)
        .map(|c| c.join(" "))
        .collect();

    // Create TTS engine (lazy; model loads on first use)
    let kk = Kokoro::new().expect("Failed to create Kokoro instance");

    // Channel: TTS worker → Player
    let (tx, rx) = bounded::<Option<Vec<f32>>>(2);  // queue size 2

    // TTS worker thread (producer)
    let tts_thread = {
        let stop = stop.clone();
        thread::spawn(move || {
            for chunk in &chunks {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match kk.generate(chunk, VOICE) {
                    Ok(audio) => {
                        if tx.send(Some(audio.data)).is_err() {
                            break;  // receiver dropped
                        }
                    }
                    Err(e) => {
                        eprintln!("TTS error: {e}");
                        break;
                    }
                }
            }
            let _ = tx.send(None);  // sentinel: end of stream
        })
    };

    // Player (consumer) — runs on main thread with rodio
    let (_stream, stream_handle) = rodio::OutputStream::try_default()
        .expect("Failed to open audio output");

    let sink = rodio::Sink::try_new(&stream_handle)
        .expect("Failed to create audio sink");

    for received in rx {
        match received {
            Some(audio) => {
                let source = rodio::buffer::SamplesBuffer::new(
                    1, SAMPLING_RATE, audio,
                );
                sink.append(source);
                // Sleep a tiny bit to let the queue drain
                thread::sleep(Duration::from_millis(10));
            }
            None => break,  // end of stream
        }
        if stop.load(Ordering::SeqCst) {
            sink.stop();
            break;
        }
    }

    // Wait for playback to finish
    sink.sleep_until_end();
    let _ = tts_thread.join();
}
```

### Sentence Splitting (CLI-specific)

The CLI uses a simpler split than NLTK — just split on `.`, `!`, `?`:

```rust
/// Split text on sentence-ending punctuation.
fn split_sentences(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        buf.push(ch);
        if ch == '.' || ch == '!' || ch == '?' {
            let s = buf.trim().to_string();
            if !s.is_empty() {
                parts.push(s);
            }
            buf.clear();
        }
    }
    let tail = buf.trim().to_string();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts.into_iter().filter(|s| !s.is_empty()).collect()
}
```

---

## WAV File Output

For the library crate (not CLI), provide a convenience function to write audio to `.wav`:

```rust
use hound::{WavWriter, SampleFormat};

/// Write audio samples to a WAV file.
pub fn write_wav(path: &std::path::Path, audio: &AudioSamples) -> Result<(), KokoroError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: audio.sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(path, spec)?;
    for &sample in &audio.data {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
}
```

Or expose it as a CLI flag:

```rust
#[arg(short, long)]
output: Option<PathBuf>,  // --output audio.wav
```

---

## CLI Flags Summary

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `text` (positional) | String | required | Text to speak |
| `-o, --output` | Path | none | Write to WAV file instead of playing |
| `-q, --quiet` | bool | false | Suppress download progress messages |
| `--cache-dir` | Path | XDG cache | Custom cache directory |

---

## Acceptance Criteria

1. `kokoro-tts "Hello world"` plays audio through default speakers
2. Ctrl+C stops playback cleanly (no zombie threads)
3. `kokoro-tts -o test.wav "Hello"` writes valid WAV file
4. Long text (>100 words) streams without excessive latency
5. Missing audio device → graceful error (not panic)
