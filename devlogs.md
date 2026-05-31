# kokoro-rust devlogs

## 2026-05-31: Project bootstrap
- Created Rust port of kokoro-tts (Python → Rust)
- PRDs in .dingllm/prds/v2-rust-port/
- Pre-converting .pt voice files → .kokoro format, hosting on GitHub releases
- Workspace: kokoro (lib) + kokoro-cli (binary)

## 2026-05-31: Implementation — complete
- Phase 1: workspace, error.rs (thiserror), types.rs (AudioSamples, Voice, Lang)
- Phase 2: normalize.rs (25 regex rules), vocab.rs (char→token LUT), phonemes.rs (espeak-ng FFI via pkg-config)
- Phase 3: cache.rs (download + cache dir), voice.rs (VoicePack from .kokoro format)
- Phase 4: model.rs (ONNX inference via ort v2.0.0-rc.12, named inputs)
- Phase 5: kokoro.rs (Kokoro struct with Mutex<State>, generate(), lazy loading)
- Phase 6: main.rs (clap CLI, producer-consumer streaming, WAV output, rodio playback)
- All crates compile cleanly

### Remaining
- Convert .pt voice files → .kokoro and host on GitHub releases
- Verify ONNX input/output names match model (currently assuming "tokens"/"style"/"speed"/"output")
- Test end-to-end with actual ONNX model and voice packs
- Write unit tests for normalize, vocab, voice pack parsing
