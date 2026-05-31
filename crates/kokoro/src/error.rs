use thiserror::Error;

#[derive(Error, Debug)]
pub enum KokoroError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] Box<ureq::Error>),

    #[error("ONNX runtime error: {0}")]
    Ort(#[from] ort::Error),

    #[error("espeak error: {0}")]
    Espeak(String),

    #[error("invalid voice: '{0}'")]
    InvalidVoice(String),

    #[error("voice not loaded")]
    VoiceNotLoaded,

    #[error("model not loaded")]
    ModelNotLoaded,

    #[error("empty token sequence")]
    EmptyTokens,

    #[error("voice pack missing style for batch_size=1")]
    VoicePackMissingStyle,

    #[error("invalid voice pack: {0}")]
    InvalidVoicePack(String),

    #[error("WAV write error: {0}")]
    Wav(#[from] hound::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("shape error: {0}")]
    Shape(#[from] ndarray::ShapeError),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, KokoroError>;

impl From<ureq::Error> for KokoroError {
    fn from(e: ureq::Error) -> Self {
        KokoroError::Http(Box::new(e))
    }
}
