use std::path::PathBuf;
use std::sync::Mutex;

use crate::cache;
use crate::error::{KokoroError, Result};
use crate::model::KokoroModel;
use crate::phonemes;
use crate::types::{AudioSamples, Voice, HF_BASE_URL, MAX_CHUNK_WORDS, MAX_TOKENS, MODEL_FILENAME, SAMPLING_RATE};
use crate::vocab;
use crate::voice::VoicePack;

struct KokoroState {
    model: Option<KokoroModel>,
    voice_pack: Option<VoicePack>,
    current_voice: Option<Voice>,
}

pub struct Kokoro {
    cache_dir: PathBuf,
    state: Mutex<KokoroState>,
}

impl Kokoro {
    pub fn new() -> Self {
        Self::with_cache_dir(cache::default_cache_dir())
    }

    pub fn with_cache_dir(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            state: Mutex::new(KokoroState {
                model: None,
                voice_pack: None,
                current_voice: None,
            }),
        }
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    pub fn generate(&self, text: &str, voice: Voice) -> Result<AudioSamples> {
        let sentences = sentence_tokenize(text);
        let chunks = chunk_sentences(&sentences, MAX_CHUNK_WORDS);

        if chunks.is_empty() {
            return Ok(AudioSamples {
                sample_rate: SAMPLING_RATE,
                data: vec![],
            });
        }

        self.ensure_voice_loaded(voice)?;
        self.ensure_model_loaded()?;

        let lang = voice.lang();
        let mut audio_chunks: Vec<Vec<f32>> = Vec::new();

        for chunk in &chunks {
            let phonemes = phonemes::phonemize(chunk, lang, true)?;
            let tokens = vocab::tokenize(&phonemes);

            if tokens.is_empty() {
                continue;
            }

            let tokens = if tokens.len() > MAX_TOKENS {
                log::warn!("Chunk truncated from {} to {MAX_TOKENS} tokens", tokens.len());
                tokens[..MAX_TOKENS].to_vec()
            } else {
                tokens
            };

            let mut state = self.state.lock().unwrap();
            let pack = state.voice_pack.as_ref().ok_or(KokoroError::VoiceNotLoaded)?;
            let style_ref = pack.get_style().to_vec();
            let model = state.model.as_mut().ok_or(KokoroError::ModelNotLoaded)?;

            let mut padded_tokens: Vec<i64> = vec![0];
            padded_tokens.extend(tokens.iter().map(|&t| t as i64));
            padded_tokens.push(0);

            let audio = model.run(&padded_tokens, &style_ref, 1.0)?;
            audio_chunks.push(audio);
        }

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

    fn ensure_model_loaded(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.model.is_some() {
            return Ok(());
        }

        let model_path = self.cache_dir.join(MODEL_FILENAME);
        if !model_path.exists() {
            let url = format!("{HF_BASE_URL}/{}/{MODEL_FILENAME}", crate::types::DEFAULT_MODEL_COMMIT);
            cache::download_file(&url, &model_path)?;
        }

        state.model = Some(KokoroModel::load(&model_path)?);
        Ok(())
    }

    fn ensure_voice_loaded(&self, voice: Voice) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.current_voice == Some(voice) && state.voice_pack.is_some() {
            return Ok(());
        }

        let voice_path = voice.voice_path(&self.cache_dir);
        if !voice_path.exists() {
            let url = voice.voice_url();
            cache::download_file(&url, &voice_path)?;
        }

        state.voice_pack = Some(VoicePack::load(&voice_path)?);
        state.current_voice = Some(voice);
        Ok(())
    }
}

fn sentence_tokenize(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();

    for i in 0..bytes.len() {
        if matches!(bytes[i], b'.' | b'!' | b'?') {
            let is_end = i + 1 >= bytes.len()
                || (bytes[i + 1] == b' '
                    && i + 2 < bytes.len()
                    && bytes[i + 2].is_ascii_uppercase());

            if is_end {
                let sentence = text[start..=i].trim().to_string();
                if !sentence.is_empty() {
                    sentences.push(sentence);
                }
                start = i + 1;
            }
        }
    }

    let tail = text[start..].trim().to_string();
    if !tail.is_empty() {
        sentences.push(tail);
    }

    sentences
}

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
