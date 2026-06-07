# Whisper models

Place the Whisper GGML model here. The app loads the file named by `MODEL_FILENAME`
in `src-tauri/src/transcribe.rs` (default: `ggml-small.en.bin`).

Download from: https://huggingface.co/ggerganov/whisper.cpp/tree/main

```
models/ggml-small.en.bin   ← put it here (project root)
```

## Runtime path note

`transcribe::resolve_model_path()` finds the model automatically — keeping it here in the
project-root `models/` works for both dev and a packaged build:

- **dev:** the resolver walks up from the exe (`target/debug`) to this `models/` folder
- **prod:** `tauri build` copies this folder next to the `.exe`
  (via the `bundle.resources` mapping `../models/* -> models/` in `tauri.conf.json`)

This `README.md` also exists so the `bundle.resources` glob matches at least one file
even before the model is downloaded (Tauri fails the build on an empty resource glob).
