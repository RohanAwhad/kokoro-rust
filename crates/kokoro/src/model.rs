use ort::value::{DynTensorValueType, Tensor};

use crate::error::Result;

pub struct KokoroModel {
    session: ort::session::Session,
}

impl KokoroModel {
    pub fn load(model_path: &std::path::Path) -> Result<Self> {
        let session = ort::session::Session::builder()?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| -> ort::Error { e.into() })?
            .with_intra_threads(num_cpus())
            .map_err(|e| -> ort::Error { e.into() })?
            .commit_from_file(model_path)?;
        Ok(Self { session })
    }

    pub fn run(&mut self, tokens: &[i64], style: &[f32], speed: f32) -> Result<Vec<f32>> {
        let tokens_tensor = Tensor::<i64>::from_array((
            vec![1usize, tokens.len()],
            tokens.to_vec(),
        ))?;

        let style_tensor = Tensor::<f32>::from_array((
            vec![1usize, style.len()],
            style.to_vec(),
        ))?;

        let speed_tensor = Tensor::<f32>::from_array((
            vec![1usize],
            vec![speed],
        ))?;

        let outputs = self.session.run(ort::inputs![
            "tokens" => tokens_tensor,
            "style" => style_tensor,
            "speed" => speed_tensor,
        ])?;

        let output = outputs["audio"]
            .view()
            .downcast::<DynTensorValueType>()?;
        let (_, data) = output.try_extract_tensor::<f32>()?;

        Ok(data.to_vec())
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
