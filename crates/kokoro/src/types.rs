use serde::{Deserialize, Serialize};
use std::fmt;

pub const SAMPLING_RATE: u32 = 24_000;

pub const MAX_TOKENS: usize = 510;

pub const MAX_CHUNK_WORDS: usize = 25;

pub const DEFAULT_MODEL_COMMIT: &str = "3095858c40fc22e28c46429da9340dfda1f8cf28";

pub const HF_BASE_URL: &str = "https://huggingface.co/hexgrad/Kokoro-82M/resolve";

pub const VOICE_RELEASE_BASE: &str = "https://github.com/RohanAwhad/kokoro-rust/releases/download/voices";

pub const CACHE_DIR_NAME: &str = "kokoro-tts";

pub const MODEL_FILENAME: &str = "kokoro-v0_19.onnx";

#[derive(Clone)]
pub struct AudioSamples {
    pub sample_rate: u32,
    pub data: Vec<f32>,
}

impl AudioSamples {
    pub fn duration_secs(&self) -> f64 {
        self.data.len() as f64 / self.sample_rate as f64
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Am,
    Br,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    pub const ALL: &[Voice] = &[
        Voice::Af,
        Voice::AfBella,
        Voice::AfSarah,
        Voice::AmAdam,
        Voice::AmMichael,
        Voice::BfEmma,
        Voice::BfIsabella,
        Voice::BmGeorge,
        Voice::BmLewis,
        Voice::AfNicole,
        Voice::AfSky,
    ];

    pub fn lang(&self) -> Lang {
        match self {
            Voice::Af
            | Voice::AfBella
            | Voice::AfSarah
            | Voice::AmAdam
            | Voice::AmMichael
            | Voice::AfNicole
            | Voice::AfSky => Lang::Am,
            Voice::BfEmma
            | Voice::BfIsabella
            | Voice::BmGeorge
            | Voice::BmLewis => Lang::Br,
        }
    }

    pub fn espeak_lang(&self) -> &'static str {
        match self.lang() {
            Lang::Am => "en-us",
            Lang::Br => "en-gb",
        }
    }

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
