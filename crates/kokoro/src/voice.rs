use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use crate::error::{KokoroError, Result};
use crate::types::Voice;

#[derive(Deserialize)]
struct TensorMeta {
    offset: usize,
    size: usize,
    shape: Vec<usize>,
}

pub struct VoicePack {
    styles: HashMap<usize, Vec<f32>>,
    style_shape: Vec<usize>,
}

impl VoicePack {
    pub fn load(path: &Path) -> Result<Self> {
        let data = std::fs::read(path)?;

        if data.len() < 4 {
            return Err(KokoroError::InvalidVoicePack("file too small".into()));
        }

        let meta_len = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;

        if 4 + meta_len > data.len() {
            return Err(KokoroError::InvalidVoicePack("invalid metadata length".into()));
        }

        let meta: HashMap<String, TensorMeta> =
            serde_json::from_slice(&data[4..4 + meta_len])?;
        let blob = &data[4 + meta_len..];

        let mut styles = HashMap::new();
        let mut shape = Vec::new();

        for (key, tm) in &meta {
            if tm.offset + tm.size > blob.len() {
                return Err(KokoroError::InvalidVoicePack(format!(
                    "tensor {key} offset+size exceeds blob"
                )));
            }
            let raw = &blob[tm.offset..tm.offset + tm.size];
            let vec: Vec<f32> = raw
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            if vec.len() * 4 != tm.size {
                return Err(KokoroError::InvalidVoicePack(format!(
                    "tensor {key} size mismatch"
                )));
            }
            if shape.is_empty() {
                shape = tm.shape.clone();
            }
            let key_num: usize = key.parse().map_err(|_| {
                KokoroError::InvalidVoicePack(format!("invalid key: {key}"))
            })?;
            styles.insert(key_num, vec);
        }

        if styles.is_empty() {
            return Err(KokoroError::InvalidVoicePack("no tensors found".into()));
        }

        Ok(Self {
            styles,
            style_shape: shape,
        })
    }

    pub fn get_style(&self) -> &[f32] {
        let tensor = self.styles.get(&0).expect("Voice pack empty");
        let shape = &self.style_shape;

        if shape.len() == 3 {
            let rows = shape[0];
            let _batch = shape[1];
            let dims = shape[2];
            let stride = dims;
            let offset = 1 * stride;
            &tensor[offset..offset + dims]
        } else {
            tensor.as_slice()
        }
    }

    pub fn style_shape(&self) -> &[usize] {
        &self.style_shape
    }
}

impl Voice {
    pub fn voice_url(&self) -> String {
        format!(
            "{}/main/voices/{}.pt",
            crate::types::HF_BASE_URL,
            self.filename()
        )
    }

    pub fn voice_path(&self, cache_dir: &Path) -> PathBuf {
        let voice_dir = cache_dir.join("voices");
        voice_dir.join(format!("{}.kokoro", self.filename()))
    }
}
