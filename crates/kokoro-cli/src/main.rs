use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use clap::Parser;
use crossbeam_channel::bounded;
use rodio::{OutputStream, Sink};

use kokoro::{AudioSamples, Kokoro, Voice, SAMPLING_RATE};

const VOICE: Voice = Voice::AfSky;

#[derive(Parser)]
#[command(name = "kokoro-tts", version, about = "Kokoro Text-to-Speech CLI")]
struct Cli {
    /// Text to speak
    text: String,

    /// Write audio to WAV file instead of playing
    #[arg(short, long)]
    output: Option<std::path::PathBuf>,

    /// Suppress non-error output
    #[arg(short, long)]
    quiet: bool,
}

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let cli = Cli::parse();

    if let Some(path) = cli.output {
        match generate_wav(&cli.text, &path) {
            Ok(_) => {
                if !cli.quiet {
                    eprintln!("Wrote: {}", path.display());
                }
            }
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        speak(&cli.text);
    }
}

fn generate_wav(text: &str, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let kk = Kokoro::new();
    let audio = kk.generate(text, VOICE)?;
    write_wav(path, &audio)?;
    Ok(())
}

fn write_wav(path: &std::path::Path, audio: &AudioSamples) -> Result<(), Box<dyn std::error::Error>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: audio.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in &audio.data {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
}

fn speak(text: &str) {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_ctrlc = stop.clone();

    ctrlc::set_handler(move || {
        stop_ctrlc.store(true, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    let sentences = split_sentences(text);
    let chunks: Vec<String> = sentences
        .chunks(2)
        .map(|c| c.join(" "))
        .collect();

    let text_for_thread = chunks.clone();

    let (tx, rx) = bounded::<Option<Vec<f32>>>(2);

    let tts_thread = {
        let stop = stop.clone();
        thread::spawn(move || {
            let kk = Kokoro::new();
            for chunk in &text_for_thread {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match kk.generate(chunk, VOICE) {
                    Ok(audio) => {
                        if tx.send(Some(audio.data)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("TTS error: {e}");
                        break;
                    }
                }
            }
            let _ = tx.send(None);
        })
    };

    let (_stream, stream_handle) = match OutputStream::try_default() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to open audio output: {e}");
            return;
        }
    };

    let sink = match Sink::try_new(&stream_handle) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create audio sink: {e}");
            return;
        }
    };

    for received in rx {
        match received {
            Some(audio) => {
                let source = rodio::buffer::SamplesBuffer::new(1, SAMPLING_RATE, audio);
                sink.append(source);
            }
            None => break,
        }
        if stop.load(Ordering::SeqCst) {
            sink.stop();
            break;
        }
    }

    sink.sleep_until_end();
    let _ = tts_thread.join();
}

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
