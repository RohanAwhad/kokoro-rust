#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.9"
# dependencies = ["torch>=2.0", "numpy"]
# ///
"""Convert PyTorch .pt voice files to .kokoro format for kokoro-rust."""
import sys
import json
import struct
import torch


def convert(input_path: str, output_path: str) -> None:
    data = torch.load(input_path, weights_only=True)
    arr = data.numpy().astype("float32")

    metadata = {"0": {"offset": 0, "size": arr.nbytes, "shape": list(arr.shape)}}
    meta_json = json.dumps(metadata).encode()
    blob = arr.tobytes()

    with open(output_path, "wb") as f:
        f.write(struct.pack("<I", len(meta_json)))
        f.write(meta_json)
        f.write(blob)

    print(f"Converted: {input_path} -> {output_path}")
    print(f"  Shape: {arr.shape}")
    print(f"  Dtype: {arr.dtype}")
    print(f"  Size: {arr.nbytes} bytes ({arr.nbytes / 1024 / 1024:.1f} MB)")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <input.pt> <output.kokoro>")
        sys.exit(1)
    convert(sys.argv[1], sys.argv[2])
