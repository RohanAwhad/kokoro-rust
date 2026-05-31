# 002: ONNX Inference Engine

**Dependencies**: 000_overview, 001_text_pipeline
**Severity**: high
**Python source**: `src/kokoro_tts/kokoro.py` lines 119–160, 235–246, 281–296

## Current Behavior (Python)

Two code paths for model execution exist in the Python codebase:

### Path A: `forward()` function (original HuggingFace code, lines 125–147)
Uses the sub-module ONNX graph (not the combined ONNX model). Calls `model.bert()`, `model.predictor`, `model.decoder()` etc. This is the original research code path and uses the PyTorch model, not the exported ONNX combined model.

### Path B: `Kokoro.generate()` method (lines 281–296)
Uses a **combined ONNX model** (`kokoro-v0_19.onnx`) with 3 inputs and 1 output. This is the path the wrapper uses and the one we must port.

ONNX Inference:
```python
audio = self._sess.run(None, dict(
    tokens=[[0, *tokens, 0]],     # int64, shape [1, 1, n+2]
    style=ref_s,                   # float32, shape from voice pack
    speed=np.ones(1, dtype=np.float32),  # float32 scalar
))[0]
```

## Desired Behavior (Rust)

Load the ONNX model once, run inference with `ort` crate. Produce identical f32 audio output to Python for identical token+style inputs.

---

## Model File

- **URL**: `https://huggingface.co/hexgrad/Kokoro-82M/resolve/{commit_hash}/kokoro-v0_19.onnx`
- **Default commit**: `3095858c40fc22e28c46429da9340dfda1f8cf28`
- **Size**: ~300 MB
- **Cache path**: `~/.cache/kokoro-tts/kokoro-v0_19.onnx` (XDG: `$XDG_CACHE_HOME/kokoro-tts/`)

## ONNX Model I/O Contract

From the Python `sess.run()` call:

### Inputs

| Name | Dtype | Shape | Description |
|------|-------|-------|-------------|
| `tokens` | int64 | `[1, 1, N]` | Token sequence. N = num_tokens + 2 (with [0, ...tokens..., 0] padding). Values from VOCAB LUT. Max N = 512 (510 content + 2 padding). |
| `style` | float32 | `[1, S]` | Voice style reference tensor. Shape depends on model architecture (likely `[1, 256]` from `forward()` code). Indexed from voice pack by token count. |
| `speed` | float32 | `[1]` | Playback speed multiplier. Default 1.0. |

### Outputs

| Index | Dtype | Shape | Description |
|-------|-------|-------|-------------|
| 0 | float32 | `[1, T]` | Raw audio samples at 24000 Hz sample rate. T varies with input length. |

**Important**: The actual input/output **names** may differ. We must inspect the ONNX model to get exact names. The Python code uses dict keys `"tokens"`, `"style"`, `"speed"` — these must match the ONNX model's named inputs.

## Rust Implementation (`model.rs`)

### Loading the Model

```rust
use ort::{Session, session::SessionOutputs};

pub struct KokoroModel {
    session: Session,
}

impl KokoroModel {
    /// Load ONNX model from file path.
    /// Returns error if file doesn't exist or model is invalid.
    pub fn load(model_path: &std::path::Path) -> Result<Self, KokoroError> {
        let session = Session::builder()?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            .with_intra_threads(num_cpus::get())?
            .commit_from_file(model_path)?;
        Ok(Self { session })
    }

    /// Run inference. Returns audio samples as Vec<f32>.
    pub fn run(
        &self,
        tokens: &[i64],      // [1, 1, N] flattened? or use ndarray
        style: &[f32],       // [1, S]
        speed: f32,          // scalar
    ) -> Result<Vec<f32>, KokoroError> {
        // Build input tensors
        // Run session
        // Extract output
    }
}
```

### ORT Crate API Notes

The `ort` crate (v2.x) uses `ndarray` for tensor data. We must convert between Rust slices/vectors and ndarray arrays.

```rust
use ndarray::{Array, IxDyn};
use ort::value::Tensor;

// Creating input tensors
let tokens_shape = vec![1i64, 1, tokens.len() as i64];
let tokens_tensor = Tensor::from_array(
    Array::from_shape_vec(IxDyn(&tokens_shape), tokens.to_vec())?
)?;

let style_shape = vec![1i64, style.len() as i64];
let style_tensor = Tensor::from_array(
    Array::from_shape_vec(IxDyn(&style_shape), style.to_vec())?
)?;

let speed_shape = vec![1i64];
let speed_tensor = Tensor::from_array(
    Array::from_shape_vec(IxDyn(&speed_shape), vec![speed])?
)?;

// Run inference
let outputs: SessionOutputs = self.session.run(
    ort::inputs![
        "tokens" => tokens_tensor,
        "style" => style_tensor,
        "speed" => speed_tensor,
    ]?
)?;

// Extract output
let audio: Array<f32, IxDyn> = outputs["output_name"].try_extract()?.view().to_owned();
let audio_vec: Vec<f32> = audio.iter().copied().collect();
```

### Thread Safety

`ort::Session` is `!Send + !Sync`. The `Kokoro` struct (PRD 003) manages this:

- **Single-threaded use**: `Kokoro` struct itself is not `Send`, owned by one thread
- **Multi-threaded (CLI)**: Wrap `Kokoro` in `Arc<Mutex<Kokoro>>` for the producer-consumer pattern. The TTS thread acquires the lock, runs `generate()`, releases lock.

### Performance Optimizations

1. **Session reuse**: Load ONNX session once, reuse for all `generate()` calls.
2. **Intra-thread parallelism**: Set `with_intra_threads()` to number of CPU cores.
3. **Memory arena**: The `ort` crate may use its own allocator. Don't wrap with unnecessary copies.
4. **Pre-allocate token buffer**: Reuse `Vec<i64>` across chunk generations.
5. **ONNX graph optimization level**: Use `Level3` (maximum) — applies all optimizations.

### Unknown: Input/Output Names

We MUST run a diagnostic step before implementing. Options:

1. **Inspect with Python**: `session.get_inputs()` and `session.get_outputs()` to get names and shapes.
2. **Inspect with `ort` CLI**: `python -c "import onnxruntime; s=onnxruntime.InferenceSession('model.onnx'); print([i.name for i in s.get_inputs()])"`
3. **Hardcode assumption**: Assume names are `"tokens"`, `"style"`, `"speed"` for inputs and `"output"` for output. Verify during testing.

## Voice Pack Indexing

`len(tokens)` after wrapping in `[[0, *tokens, 0]]` gives the **batch size** (always 1). The voice pack is indexed by batch dimension, not token count. `self._voice[1]` is the correct style reference for single-sequence inference.

## Acceptance Criteria

1. Load `kokoro-v0_19.onnx` and run inference without errors
2. Same input tokens + style + speed produces byte-identical audio output to Python
3. Handle edge cases: 0 tokens (empty), 1 token, 510 tokens (max), beyond 510 (truncate)
4. Session survives multiple `run()` calls without memory leaks
5. Performance: inference time comparable to Python (within 10%)
