# CLAUDE.md

Operating guide for working in this repo. Read it at the start of every session.

## What this is

**Steno** — a push-to-talk desktop dictation app (renamed from "Voice Typer"; see SHIP_BRIEF.md). Hold `Shift+Space`, speak,
release, and the transcribed text is typed into whatever app currently has focus.
(Hotkey is registered in [lib.rs](src-tauri/src/lib.rs); bare `Space` is intentionally avoided
because a global shortcut swallows the key system-wide.)

- **Shell:** Tauri 2 (Rust backend + system webview)
- **Frontend:** React 18 + TypeScript + Vite (status UI only — all logic lives in Rust)
- **Audio:** `cpal` captures the mic into an `f32` PCM buffer
- **Transcription:** `whisper-rs` (compiles `whisper.cpp` from source) runs locally, offline
- **Typing:** `enigo` simulates keystrokes at the cursor
- **Hotkey:** `tauri-plugin-global-shortcut` (push-to-talk via Pressed/Released)

### Data flow
```
Hold hotkey  → cpal records mic → Vec<f32> buffer (native sample rate)
Release      → stop capture → resample to 16 kHz → whisper-rs → String
             → enigo types it at the cursor → frontend gets a "transcription" event
```

## Layout

| Path | Role |
|------|------|
| `src-tauri/src/lib.rs` | App entry, state, hotkey registration, typing, event emit |
| `src-tauri/src/audio.rs` | `cpal` capture: `start_recording` / `stop_recording` |
| `src-tauri/src/transcribe.rs` | `load_model` (cached), `transcribe`, `resolve_model_path`, `transcription_threads`, resample; `MODEL_FILENAME` const |
| `src-tauri/tauri.conf.json` | Window, tray, bundle, icon, resource config |
| `src-tauri/capabilities/default.json` | Permission grants (global-shortcut, shell) |
| `src/App.tsx` | Status UI + recent transcript list with per-entry copy button (`navigator.clipboard`) and selectable text |
| `models/ggml-*.en.bin` | Whisper model (not in repo — download separately) |
| `SECURITY.md` | Public trust doc — permissions, SmartScreen workaround, verify-by-build, no-network claim |
| `.github/FUNDING.yml` | Sponsor button config (Ko-fi recommended; lines commented until activated) |
| `release/` | Build-output staging for GitHub Releases (portable zip + installers + `SHA256SUMS`). Regenerate via the staging block at the bottom of "Build & run". Not committed. |

## Build & run

The build compiles `whisper.cpp`, so CMake **and** libclang must be reachable (see gotchas).
On this machine both are pip-installed and passed in per build:

```bash
npm install            # JS deps

# whisper.cpp build deps (pip, no admin needed):
export PATH="$(python -c 'import cmake;print(cmake.CMAKE_BIN_DIR)'):$PATH"
export LIBCLANG_PATH="$(python -c 'import clang,os;print(os.path.join(os.path.dirname(clang.__file__),"native"))')"

npm run tauri dev      # compile Rust + launch app (first build is 15–30 min: whisper.cpp)
npm run tauri build    # standalone exe + MSI/NSIS installers
```

- Double-clickable result: `src-tauri/target/release/steno.exe`, with the model in
  `target/release/models/` beside it. Installers land in `src-tauri/target/release/bundle/`.
- To swap models, change `MODEL_FILENAME` in `src-tauri/src/transcribe.rs`.

## Performance

- **Model is cached in `AppState`** (`whisper: Arc<Mutex<Option<Arc<WhisperContext>>>>`),
  loaded once on first utterance and reused. Do NOT go back to loading it per-call —
  re-reading the multi-hundred-MB model each time was the dominant latency.
- **Threads:** `transcribe.rs::transcription_threads()` is hard-capped at 4 (and ~quarter of
  logical cores). Whisper runs every active core flat-out with AVX, so active-core count drives
  the thermal spike — on this 24-thread machine, 12 threads hit 100 °C (the throttle point), so
  the cap deliberately stays far below that. Raise the cap for speed at the cost of heat.
- Bigger wins if ever needed: smaller model (`ggml-base.en` ≈ 2-3x faster), or the `cuda`
  feature on `whisper-rs` for GPU (needs the CUDA toolkit + NVIDIA GPU).

## Environment gotchas (this machine, learned the hard way)

These are the non-obvious prerequisites that block a clean build:

1. **CMake is required** (`whisper-rs-sys` builds `whisper.cpp` via the `cmake` crate).
   If it's not on `PATH`, the simplest no-admin install is `pip install cmake`, then
   prepend its bin dir (`python -c "import cmake; print(cmake.CMAKE_BIN_DIR)"`) to `PATH`
   for the build. The README suggests `winget install Kitware.CMake` (needs elevation).
2. **MSVC C++ build tools** must be installed (Visual Studio "Desktop development with C++").
   The `cmake`/`cc` crates auto-detect it — no need to launch a Developer shell.
3. **libclang is effectively required** (this bit us). `whisper-rs-sys` generates bindings with
   `bindgen` (needs libclang). Its fallback to *bundled* bindings is BROKEN against `whisper-rs
   0.11.1`: the bundled `bindings.rs` types the grammar enum constants as `u32` while the wrapper
   expects `i32`, so the build fails with `mismatched types`. So do NOT set
   `WHISPER_DONT_GENERATE_BINDINGS=1`. Install libclang (`pip install libclang`) and point bindgen
   at it via `LIBCLANG_PATH` = the dir containing `libclang.dll`.
4. **Icons must exist before building.** `tauri.conf.json` references `icons/*` which
   `generate_context!()` embeds at compile time — a missing icon fails the build *after* the
   long whisper compile. Generate them with `npm run tauri icon <1024px.png>`.
5. **Model file is runtime-only**, not needed to compile. Put `ggml-small.en.bin` in `models/`
   (project root). `transcribe::resolve_model_path()` finds it in dev or in a build: it checks
   `models/` next to the exe, then walks up parent dirs (covers `target/debug` → repo root), then
   the CWD. `tauri build` copies `models/` next to the release exe automatically.
6. **`tray-icon` Cargo feature is required.** `tauri.conf.json` defines `app.trayIcon`, so
   `generate_context!()` emits a `set_tray_icon` call gated behind `tauri`'s `tray-icon` feature.
   Cargo.toml must have `tauri = { ..., features = ["tray-icon"] }` or the build fails with
   "no method named `set_tray_icon`".
7. **`bundle.resources` glob must match ≥1 file.** It's `"../models/*"` (relative to `src-tauri/`,
   = project-root `models/`). Tauri fails the build if the glob matches nothing — hence the
   `models/README.md` placeholder so it resolves before the model is downloaded.

## Version-pinned API notes (don't "fix" these — they're correct for the locked versions)

- `tauri = 2.x`: `.emit()` is on the **`Emitter`** trait (not `Manager`). Import `tauri::Emitter`.
- `tauri-plugin-global-shortcut = 2.3`: `ShortcutEvent` re-exports `global_hotkey::GlobalHotKeyEvent`,
  which has both a `state` field and a `state()` method. `Shortcut::new(Some(mods), Code::X)`.
- `whisper-rs = 0.11`: `WhisperContext::new_with_params(&str, WhisperContextParameters)`,
  `FullParams::new(SamplingStrategy::Greedy { best_of })`, `state.full(params, &[f32])`,
  `state.full_n_segments()`, `state.full_get_segment_text(i)`.

---

# How I work in this repo

Adapted from the operating principles this project was set up with.

## Workflow orchestration

1. **Plan first for non-trivial work.** Anything that's 3+ steps or an architectural decision
   gets a plan before code. If something goes sideways, stop and re-plan — don't keep pushing.
2. **Use subagents to keep context clean.** Offload research, exploration, and parallel analysis.
   One focused task per subagent.
3. **Self-improvement loop.** After any correction from the user, capture the pattern as a rule
   here (or in a lessons file) so the same mistake doesn't recur.
4. **Verify before claiming done.** Never mark a task complete without proving it works — run the
   build, read the actual output, demonstrate the behavior. "Would a staff engineer approve this?"
5. **Demand elegance (balanced).** For non-trivial changes, ask "is there a cleaner way?" Skip the
   ceremony for simple, obvious fixes — don't over-engineer.
6. **Autonomous bug fixing.** Given a bug report, just fix it: read the logs/errors/failing build,
   find the root cause, resolve it. Minimize hand-holding and don't ask permission for routine steps.

## Core principles

- **Simplicity first** — make each change as small as it can be; touch minimal code.
- **No laziness** — find root causes, not temporary patches. Senior-developer standards.
- **Minimal impact** — only change what's necessary; avoid introducing new bugs.
- **Evidence over assertion** — when reporting status, say what you ran and what it printed.
  If the build failed, show the error; if a step was skipped, say so.
