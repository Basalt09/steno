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
| `models/ggml-*.bin` | Whisper model — current default `ggml-base.en-q5_1.bin` (~57 MB, quantized for thermals + speed). Not committed; `.gitignore` excludes `.bin` files. Download via curl from HuggingFace at build time. |
| `LICENSE` | MIT, © 2026 Basalt09 |
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
- **Threads:** `transcribe.rs::transcription_threads()` is hard-capped at **2** (via
  `(n.get() / 4).clamp(2, 2)`). Whisper runs every active core flat-out with AVX, so the
  number of active cores is what drives the thermal spike. Measured on the 24-thread reference
  machine, one short sentence of speech:
  - 12 threads (old default): **100 °C** (throttle limit — DOA)
  - 4 threads: **87 °C** (better but still hot)
  - 2 threads + q5_1 small model: **75 °C** (the shipped config — comfortably off throttle)

  Raise the cap if you want speed at the cost of heat. Combine with a smaller model to bring
  burst duration down at the same peak.
- **Model size matters more than thread count for total heat AND latency.** The shipped
  `ggml-base.en-q5_1` (~57 MB) was chosen after real testing of `small.en-q5_1` showed
  unacceptable processing time (~6-10s per longer utterance at 2 threads) and a creeping peak
  on consecutive utterances. base.en cuts compute ~3×, dropping a long-sentence test to ~75 °C
  and feel-fast response. Real accuracy cost: occasional misses on proper nouns / technical
  terms ("paid dictation" → "Pete detection"). Honest trade-off documented in the README.
  If accuracy matters more than speed on a given build, switch `MODEL_FILENAME` back to
  `ggml-small.en-q5_1.bin`. GPU offload (`whisper-rs` `cuda` or `metal` features) is the only
  path that meaningfully cuts thermals further without sacrificing accuracy — but each needs the
  matching toolchain (CUDA, or a Mac for Metal).

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
5. **Model file is runtime-only**, not needed to compile. Put the file named by `MODEL_FILENAME`
   (current default `ggml-base.en-q5_1.bin`) in `models/` (project root).
   `transcribe::resolve_model_path()` finds it in dev or in a build: it checks `models/` next to
   the exe, then walks up parent dirs (covers `target/debug` → repo root), then the CWD.
   `tauri build` copies `models/` next to the release exe automatically.
6. **`tray-icon` Cargo feature is required.** `tauri.conf.json` defines `app.trayIcon`, so
   `generate_context!()` emits a `set_tray_icon` call gated behind `tauri`'s `tray-icon` feature.
   Cargo.toml must have `tauri = { ..., features = ["tray-icon"] }` or the build fails with
   "no method named `set_tray_icon`".
7. **`bundle.resources` glob must match ≥1 file.** It's `"../models/*"` (relative to `src-tauri/`,
   = project-root `models/`). Tauri fails the build if the glob matches nothing — hence the
   `models/README.md` placeholder so it resolves before the model is downloaded.

## Shipping & distribution

**v0.1.0 is live** at https://github.com/Basalt09/steno (first public release, 2026-06-07).
Distribution: GitHub Releases, unsigned, with the SmartScreen workaround documented in
[SECURITY.md](SECURITY.md). Funding: Ko-fi at https://ko-fi.com/basalt09
(`.github/FUNDING.yml` activates the Sponsor button on the repo).

**Binary swap mid-launch (2026-06-08, before any external posting):** the original v0.1.0
binaries used `small.en-q5_1` + 2 threads. Real-use testing showed long-sentence processing
took ~6-10s and the peak crept on consecutive utterances. The shipped binaries were replaced
in-place on the same v0.1.0 tag (no version bump) with `base.en-q5_1` — faster, same peak,
smaller download. This is safe pre-launch (zero downloads yet); post-launch it would require
a version bump.

### Release pipeline

After a clean `npm run tauri build`:

1. Stage the portable bundle (`steno.exe` + `models/` + a tiny `README.txt`) **OUTSIDE
   `target/`** — see gotcha below — then zip it via Python's `zipfile` module (no `zip` CLI
   on this machine). Force `steno/` as the in-zip folder prefix.
2. Copy the zip + both installers (`Steno_*.msi`, `Steno_*-setup.exe`) into `release/` at
   project root.
3. Generate `SHA256SUMS` with `sha256sum *.zip *.msi *.exe > SHA256SUMS`.
4. Drag the four files from `release/` into GitHub's release "Attach binaries" area in the web
   UI (no `gh` CLI installed here).

### Packaging gotchas (each cost real time on the first ship)

- **Stage the zip OUTSIDE `target/release/`.** Staging at `src-tauri/target/release/steno/`
  triggers a Windows-filesystem weirdness where `rm -rf` of that folder also nukes the sibling
  `steno.exe`. Stage in `/tmp/steno-stage/` and inject the `steno/` archive prefix at zip-time:
  `z.write(full, "steno/" + rel)`.
- **No `zip` CLI in this git-bash.** Use Python's `zipfile`; `python` is on PATH from the pip
  install of `cmake` / `libclang`.
- **Kill any running `steno.exe` before any rebuild** — Windows holds the binary lock and
  cargo's link step fails with `os error 5: Access is denied` mid-build. See
  [[kill-running-exe-before-rebuild]] in the memory system. Command:
  `tasklist //FI "IMAGENAME eq steno.exe" | grep -i steno && taskkill //F //IM steno.exe`.
- **Background bash wrappers ending in `echo "exit: $?"` mask the real exit code.** The
  task-notification reports success even if cargo failed. Verify by exe timestamp + grepping
  the output for `error`, not the wrapper's exit code. Or chain `exit $rc` at the very end.

### Git identity for this repo

Per-repo config so commits show the Basalt09 pseudonym instead of whatever the OS-level
global git config has:

```bash
git config user.name "Basalt09"
git config user.email "63625270+Basalt09@users.noreply.github.com"
```

Set **per-repo** (not globally). On a fresh clone of this repo on a new machine, re-set the
per-repo config before any commit, or the first push will use whatever the global git config
shows.

### v0.2.0 priorities (when there's demand signal)

In the order the verdict signal will probably surface them:

1. **macOS build.** GitHub Actions macOS runner is free; Tauri builds cleanly on Mac and
   gets Metal GPU acceleration for whisper-rs (huge thermal/speed win on Apple Silicon).
   Code-signing + notarization needs Apple Developer Program ($99/yr — defer until revenue).
2. **Multilingual.** Swap `MODEL_FILENAME` to `ggml-small-q5_1.bin` (no `.en` suffix, ~180 MB,
   99-language model) and set `params.set_language(None)` for auto-detect. UI dropdown if a
   user wants to pin a language.
3. **Signed Windows installer.** $19 Microsoft Partner Center registration → Microsoft Store →
   Microsoft signs the installer → SmartScreen warning disappears. Cheapest path; needs to
   wait until Gumroad/Ko-fi has covered the $19.

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
