use kokoro::{Kokoro, Voice};

fn main() {
    let text = "Hello world.";
    eprintln!("Input: {text:?}");

    // Show intermediate steps
    let normalized = kokoro::normalize::normalize_text(text);
    eprintln!("Normalized: {normalized:?}");

    let phonemes = kokoro::phonemes::phonemize(&normalized, kokoro::types::Voice::AfSky.lang(), false);
    match &phonemes {
        Ok(ph) => eprintln!("Phonemes ({}): {ph:?}", ph.len()),
        Err(e) => eprintln!("Phoneme error: {e}"),
    }
    let phonemes = phonemes.unwrap();

    let tokens = kokoro::vocab::tokenize(&phonemes);
    eprintln!("Tokens ({}): {tokens:?}", tokens.len());

    let kk = Kokoro::new();
    let voice = Voice::AfSky;
    let audio = match kk.generate(text, voice) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {e}");
            return;
        }
    };

    eprintln!("Sample rate: {}", audio.sample_rate);
    eprintln!("Samples: {}", audio.len());
    eprintln!("Duration: {:.2}s", audio.duration_secs());

    let first_10 = &audio.data[..10.min(audio.data.len())];
    eprintln!("First 10 samples: {:?}", first_10);

    let sum: f32 = audio.data.iter().sum();
    eprintln!("Checksum (sum): {sum:.6}");

    // Compare with Python
    match std::fs::read("/tmp/kokoro_python_audio.f32") {
        Ok(py_bytes) => {
            let py_data: Vec<f32> = py_bytes
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            let py_sum: f32 = py_data.iter().sum();
            eprintln!("\n=== Comparison ===");
            eprintln!("Python samples: {}", py_data.len());
            eprintln!("Rust samples:   {}", audio.data.len());
            eprintln!("Python sum:  {py_sum:.6}");
            eprintln!("Rust sum:    {sum:.6}");
            if (py_sum - sum).abs() < 0.001 {
                eprintln!("MATCH! Audio is byte-identical.");
            } else {
                eprintln!("MISMATCH - audio differs.");
                eprintln!("Py first 10:  {:?}", &py_data[..10.min(py_data.len())]);
                eprintln!("Rust first 10: {:?}", first_10);
            }
        }
        Err(e) => eprintln!("Could not read Python reference: {e}"),
    }
}

