# 004: Voice Pack Parsing (.pt binary format)

**Dependencies**: 000_overview
**Severity**: high
**Python source**: `src/kokoro_tts/kokoro.py` lines 165–184 (voice constants), 214–232 (download + torch.load)

## Current Behavior (Python)

Voice packs are PyTorch `.pt` files downloaded from HuggingFace:

```python
self._voice = torch.load(voice_path, weights_only=True)
# self._voice is a list-like collection of tensors
# voice_pack[token_count] → style reference tensor, shape [1, 256]
ref_s = self._voice[len(tokens)].numpy()
```

Each voice file is ~several MB. 11 voices × ~5–10MB each = ~55–110MB cached total.

## Desired Behavior (Rust)

Parse `.pt` files in pure Rust without libtorch dependency. Load into structured `VoicePack` type that provides O(1) style reference lookup by token count.

---

## The .pt File Format (PyTorch Serialization)

PyTorch's newer serialization format (default since PyTorch 1.6) is a **ZIP archive** with:

```
voice_name.pt  (ZIP file)
├── archive/
│   ├── version           # text file containing version number (e.g., "6")
│   ├── data.pkl          # pickle-serialized metadata (tensor names, shapes, dtypes, storage refs)
│   └── data/
│       ├── 0             # raw binary data: tensor 0
│       ├── 1             # raw binary data: tensor 1
│       ├── ...           # one file per tensor
│       └── N
```

### `archive/version`

A single integer as ASCII text. Typically `6` for newer PyTorch versions.

### `archive/data.pkl`

A Python **pickle** file containing the serialized object. With `weights_only=True`, torch only allows safe deserialization — essentially a list of tensor reconstruction instructions.

For a voice pack, the pickle contains something conceptually like:
```python
OrderedDict([
    (0, tensor_rebuild(storage=..., storage_offset=0, size=[1,256], stride=[256,1])),
    (1, tensor_rebuild(storage=..., storage_offset=256, size=[1,256], stride=[256,1])),
    ...
    (510, tensor_rebuild(storage=..., storage_offset=..., size=[1,256], stride=[256,1])),
])
```

Each tensor is reconstructed via `torch._utils._rebuild_tensor_v2(storage, storage_offset, size, stride)`.

The **storage** object references one of the raw data files (`data/0`, `data/1`, etc.) plus dtype and device info.

### `archive/data/<N>`

Raw bytes. Each file is a tensor's binary data in row-major (C-contiguous) order. Dtype determines byte size: float32 = 4 bytes, int64 = 8 bytes, etc. For voice packs, all tensors are float32.

---

## Parsing Strategy: Pre-Convert .pt → .kokoro (Chosen)

Pure Rust pickle parsing is fragile (torch custom pickle opcodes). `tch-rs` adds ~2GB libtorch dependency. Instead:

**One-time conversion**: A Python script converts `.pt` → `.kokoro` format. Run once at first setup, or we pre-convert and bundle with the crate.

### .kokoro File Format

Simple binary format: 4-byte metadata length + JSON metadata + concatenated raw f32 tensor data.

```python
# scripts/convert_voices.py
import torch, json, struct, sys

voice_path = sys.argv[1]
data = torch.load(voice_path, weights_only=True)

# data is dict/list of tensors (one per batch size / token count)
# Each tensor: shape [1, 256], dtype float32
# We re-index by batch dimension (key = first dim of tensor)

metadata = {}
all_data = bytearray()
for key, tensor in data.items():
    arr = tensor.numpy().astype(np.float32)
    metadata[str(key)] = {
        "offset": len(all_data),
        "size": arr.nbytes,
        "shape": list(arr.shape),
    }
    all_data.extend(arr.tobytes())

with open(voice_path.replace('.pt', '.kokoro'), 'wb') as f:
    meta_json = json.dumps(metadata).encode()
    f.write(struct.pack('<I', len(meta_json)))
    f.write(meta_json)
    f.write(all_data)
```

### Rust Parser

```rust
use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize)]
struct TensorMeta {
    offset: usize,
    size: usize,
    shape: Vec<usize>,
}

fn load_kokoro_voice(path: &Path) -> Result<HashMap<usize, Vec<f32>>, KokoroError> {
    let data = std::fs::read(path)?;
    let meta_len = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    let meta: HashMap<String, TensorMeta> = serde_json::from_slice(&data[4..4+meta_len])?;
    let blob = &data[4+meta_len..];

    let mut tensors = HashMap::new();
    for (key, tm) in meta {
        let key: usize = key.parse().unwrap();
        let raw = &blob[tm.offset..tm.offset + tm.size];
        let vec: Vec<f32> = raw.chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        tensors.insert(key, vec);
    }
    Ok(tensors)
}
```

### When Conversion Happens

**Build time, by us**. We pre-convert all 11 `.pt` voice files to `.kokoro` format and host them as release artifacts (or in the repo). The Rust binary downloads `.kokoro` files directly — no Python needed at runtime.

1. Run `scripts/convert_voices.py` once per voice, outputting `.kokoro` files
2. Host `.kokoro` files as GitHub release assets on the `kokoro-rust` repo
3. Rust binary downloads `.kokoro` from our releases, not `.pt` from HF
4. Model file (`kokoro-v0_19.onnx`) still downloads from HF directly (no conversion needed)

---

## VoicePack Rust Type

```rust
use std::collections::HashMap;

/// A voice pack: contains style reference tensors indexed by batch dimension.
///
/// Loaded from a .kokoro file (pre-converted from PyTorch .pt).
/// We always use batch_size=1 for single-sequence inference.
pub struct VoicePack {
    /// Key: batch dimension. Value: style reference as f32 vector.
    /// Always contains entry for key `1` (batch_size=1).
    styles: HashMap<usize, Vec<f32>>,
    /// Shape of each style tensor (for validation).
    style_shape: Vec<usize>,
}

impl VoicePack {
    /// Load voice pack from a .kokoro file.
    pub fn load(path: &std::path::Path) -> Result<Self, KokoroError>;

    /// Get style reference for batch_size=1 (single-sequence inference).
    /// Panics if no entry exists — should never happen for valid voice packs.
    pub fn get_style(&self) -> &[f32] {
        self.styles.get(&1).map(|v| v.as_slice())
            .expect("Voice pack missing style for batch_size=1")
    }

    /// Shape of each style tensor.
    pub fn style_shape(&self) -> &[usize] {
        &self.style_shape
    }
}
```

### Validation During Load

- ZIP file is valid
- `archive/version` exists and is readable
- At least one tensor entry exists
- All tensors have consistent dtype (`float32`) and shape
- Tensor count ≤ 511 (0 to 510 token positions)

---

## Voice File URLs

Constructed from HuggingFace base + commit hash + voice filename:

```rust
impl Voice {
    pub fn commit_hash(&self) -> &'static str {
        match self {
            Voice::Af =>     "3767727882dd08a67a1b91a7513c28dc3887a9e9",
            Voice::AfBella => "3767727882dd08a67a1b91a7513c28dc3887a9e9",
            Voice::AfSarah => "3767727882dd08a67a1b91a7513c28dc3887a9e9",
            Voice::AmAdam =>  "b869fc97ed68d0ada08e84f5b4bc6a97e346f0a5",
            Voice::AmMichael => "b869fc97ed68d0ada08e84f5b4bc6a97e346f0a5",
            Voice::BfEmma =>   "a67f11354c3e38c58c3327498bc4bd1e57e71c50",
            Voice::BfIsabella => "a67f11354c3e38c58c3327498bc4bd1e57e71c50",
            Voice::BmGeorge =>  "a67f11354c3e38c58c3327498bc4bd1e57e71c50",
            Voice::BmLewis =>   "a67f11354c3e38c58c3327498bc4bd1e57e71c50",
            Voice::AfNicole =>  "8228a351f87c8a6076502c1e3b7e72e821ebec9a",
            Voice::AfSky =>     "7e9ebc5be7f66a1843b585b63d19d55b5d58ce30",
        }
    }
}
```

URL format: `https://huggingface.co/hexgrad/Kokoro-82M/resolve/{commit}/voices/{name}.pt`

---

## Acceptance Criteria

1. Convert all 11 voice `.pt` → `.kokoro` via `scripts/convert_voices.py`
2. `get_style()` returns `&[f32]` with correct shape for batch_size=1
3. Panic/error on corrupted or invalid files (fail fast)
4. Memory: voice pack fits within reasonable bounds (<200MB total for all voices)
5. Style tensor values match Python `voice_pack[1].numpy()` byte-for-byte
