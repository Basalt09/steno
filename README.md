# Steno

**Free, local, private push-to-talk dictation for Windows.**

Hold `Shift+Space`, speak, release — and what you said is typed into whatever window you were
focused on. No cloud. No subscription. No telemetry. Your voice never leaves your machine.

> **English only for now.** A multilingual version (Whisper supports 99 languages) is on the
> table for a future release if there's demand.

---

## Why

Every popular voice-dictation tool — Wispr Flow, Superwhisper, Otter — streams your microphone
audio to someone else's servers. Steno runs the same kind of speech recognition (OpenAI's open
Whisper model) **entirely on your CPU**. The audio lives in memory for the length of your
utterance, gets transcribed locally, and is gone. No network calls. No account. No
subscription. No background listening.

Open-source so you can verify all of that — see [SECURITY.md](SECURITY.md) and the code.

## Download (Windows x64)

| File | Size | What it is |
|---|---|---|
| `Steno_0.1.0_x64-setup.exe` | 171 MB | **Installer (recommended).** Standard Windows setup wizard. Adds a Start Menu entry. |
| `steno-0.1.0-windows-x64.zip` | 177 MB | **Portable.** Unzip anywhere, double-click `steno.exe`. No install, no registry. |
| `Steno_0.1.0_x64_en-US.msi` | 177 MB | MSI installer (enterprise-style). |
| `SHA256SUMS` | — | Checksums to verify your download. |

→ [Latest release](../../releases/latest)

### First-run warning (SmartScreen)

Windows will show "Microsoft Defender SmartScreen prevented an unrecognized app from starting"
the first time you run it. This is **not** a virus warning — it's the default response to any
unsigned executable. To bypass:

1. Click **More info**.
2. Click **Run anyway**.

This goes away the second time. The permanent fix is a code-signing certificate or the
Microsoft Store, neither of which this project can afford yet. Full details in
[SECURITY.md](SECURITY.md).

## How it works

1. Hold `Shift+Space`.
2. Speak.
3. Release.

The transcribed text appears in whatever app currently has keyboard focus — your editor, your
chat, anywhere a normal keypress would go. Recent transcriptions also show in the Steno window
with a copy button next to each one.

## What it needs from Windows

To work, Steno requires three permissions:

- **Microphone access** — to capture your voice.
- **Keyboard simulation** — to type the transcribed text into other apps.
- **Global hotkey** — to detect Shift+Space anywhere on the system.

These are the same capabilities a keylogger uses. The difference is that Steno is open-source
and makes zero network calls — both verifiable. See [SECURITY.md](SECURITY.md).

---

## Build from source

If you don't trust the prebuilt binary, build it yourself. The released binary is built from
the same tag with no extra steps.

### Prerequisites (with admin)

```bash
winget install Rustlang.Rustup
winget install OpenJS.NodeJS
winget install Kitware.CMake
winget install LLVM.LLVM
winget install Microsoft.VisualStudio.2022.BuildTools
# In VS Build Tools installer → check "Desktop development with C++"
```

Restart terminal after installing.

### Prerequisites (no admin — pip user-scope)

```bash
pip install cmake libclang
export PATH="$(python -c 'import cmake;print(cmake.CMAKE_BIN_DIR)'):$PATH"
export LIBCLANG_PATH="$(python -c 'import clang,os;print(os.path.join(os.path.dirname(clang.__file__),"native"))')"
```

### Setup

```bash
npm install                                    # JS deps
# place the model file:
curl -L -o models/ggml-small.en-q5_1.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en-q5_1.bin
```

Generate icons if `src-tauri/icons/` is missing:
```bash
npm run tauri icon src-tauri/icons/icon.png   # needs a 1024x1024 source
```

### Build

```bash
npm run tauri dev      # launch in dev mode (first build ~15-30 min — whisper.cpp compile)
npm run tauri build    # produce signed-ready exe + MSI/NSIS installers
```

Outputs:
- `src-tauri/target/release/steno.exe` (standalone, model in `target/release/models/`)
- `src-tauri/target/release/bundle/msi/Steno_*.msi`
- `src-tauri/target/release/bundle/nsis/Steno_*-setup.exe`

## Model options

The default `ggml-small.en-q5_1.bin` (~182 MB) is a quantized small.en model — good accuracy
and friendly on CPU/temps. To swap, change `MODEL_FILENAME` in
`src-tauri/src/transcribe.rs` and place the new model file in `models/`.

| Model | Size | Speed | Notes |
|---|---|---|---|
| `ggml-tiny.en-q5_1.bin` | ~32 MB | fastest | OK for very clear speech only |
| `ggml-base.en-q5_1.bin` | ~57 MB | very fast | Good balance for low-spec machines |
| `ggml-small.en-q5_1.bin` | ~182 MB | fast | **Default — best speed/accuracy trade-off** |
| `ggml-medium.en-q5_1.bin` | ~514 MB | slow | Higher accuracy, longer CPU bursts |

Bigger model = better accuracy on edge words/names, longer transcription bursts, hotter CPU.

## Architecture

```
[Hold Shift+Space]
    → Rust: cpal starts recording mic → Vec<f32> buffer
[Release Shift+Space]
    → Rust: recording stops
    → Rust: resample buffer to 16kHz (whisper requirement)
    → Rust: whisper-rs (cached WhisperContext) transcribes audio → String
    → Rust: enigo simulates keyboard typing at current cursor
    → Frontend: receives "transcription" event, appends to the recent list
                (per-entry copy button + selectable text)
```

CPU threads for transcription are hard-capped at 2 (see `transcription_threads()` in
`transcribe.rs`) to keep peak power and temperature low. Raise the cap for speed at the cost
of heat, or switch to a smaller model.

## Support the work

Steno is free and will stay free. If it saves you typing time, you can buy me a coffee:
[ko-fi.com/basalt09](https://ko-fi.com/basalt09) — or use the **Sponsor** button at the top
of this repo.

## License

MIT — use it however you like, modify, redistribute, fork. Just don't claim you wrote it from
scratch.
