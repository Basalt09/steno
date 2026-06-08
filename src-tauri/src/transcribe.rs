use std::path::{Path, PathBuf};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Change this to swap models without touching anything else.
/// Options: ggml-tiny.en.bin | ggml-base.en.bin | ggml-small.en.bin | ggml-medium.en.bin
/// Quantized variants (lighter on CPU + thermals): append `-q5_1` or `-q8_0` to small/base.
pub const MODEL_FILENAME: &str = "ggml-base.en-q5_1.bin";

/// Find the model file regardless of how the app was launched.
///
/// Checks `models/<MODEL_FILENAME>` next to the executable first (the layout used by a
/// double-clicked release build / installer), then walks up parent directories (so a model
/// dropped in the project-root `models/` works during `tauri dev`, where the exe lives in
/// `target/debug/`), and finally the current working directory. Returns the first match, or
/// the next-to-exe path as a sensible default for the "model not found" error message.
pub fn resolve_model_path() -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("models").join(MODEL_FILENAME));
            // Walk up toward a project root (covers target/debug -> repo root in dev).
            let mut cur = dir;
            for _ in 0..4 {
                match cur.parent() {
                    Some(parent) => {
                        candidates.push(parent.join("models").join(MODEL_FILENAME));
                        cur = parent;
                    }
                    None => break,
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("models").join(MODEL_FILENAME));
    }

    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }
    // Nothing found — return the first candidate (next-to-exe) for a sensible error message.
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("models").join(MODEL_FILENAME))
}

/// Load the Whisper model from disk. Do this once and reuse the returned context for every
/// transcription — loading the multi-hundred-MB model is by far the slow part.
///
/// # Errors
/// Returns a descriptive string if the file is missing or the model can't be loaded.
pub fn load_model(model_path: &Path) -> Result<WhisperContext, String> {
    if !model_path.exists() {
        return Err(format!(
            "Model not found at '{}'. \
             Download {} from https://huggingface.co/ggerganov/whisper.cpp/tree/main \
             and place it in the models/ folder.",
            model_path.display(),
            MODEL_FILENAME
        ));
    }

    WhisperContext::new_with_params(
        &model_path.to_string_lossy(),
        WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Failed to load model: {e}"))
}

/// CPU threads to use for transcription. Hard-capped at 2. Whisper runs every active core
/// flat-out with AVX, so the *number* of active cores is what drives the thermal spike.
/// History on the 24-thread reference machine:
///   - 12 threads + small.en-q5_1  → 100 °C (throttle)
///   - 4  threads + small.en-q5_1  → 87  °C
///   - 2  threads + small.en-q5_1  → 75 °C peak, but ~6-10s processing on longer utterances
///   - 2  threads + base.en-q5_1   → 75 °C peak, ~2-3s processing  ← shipped
/// Raise the upper bound of the clamp if you want more speed at the cost of more heat.
fn transcription_threads() -> i32 {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 4).clamp(2, 2))
        .unwrap_or(2) as i32
}

/// Transcribe a mono PCM buffer sampled at `input_sample_rate` Hz using a preloaded model.
///
/// Whisper requires 16 kHz mono f32 PCM. This function resamples automatically.
pub fn transcribe(
    ctx: &WhisperContext,
    audio: &[f32],
    input_sample_rate: u32,
) -> Result<String, String> {
    if audio.is_empty() {
        return Err("Audio buffer is empty — nothing to transcribe".into());
    }

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_n_threads(transcription_threads());
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_token_timestamps(false);

    // Resample to 16 kHz (whisper.cpp hard requirement)
    let audio_16k = if input_sample_rate != 16_000 {
        resample(audio, input_sample_rate, 16_000)
    } else {
        audio.to_vec()
    };

    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Failed to create whisper state: {e}"))?;

    state
        .full(params, &audio_16k)
        .map_err(|e| format!("Transcription failed: {e}"))?;

    let num_segments = state
        .full_n_segments()
        .map_err(|e| format!("Could not get segment count: {e}"))?;

    let mut result = String::new();
    for i in 0..num_segments {
        if let Ok(seg) = state.full_get_segment_text(i) {
            result.push_str(&seg);
        }
    }

    Ok(result.trim().to_string())
}

/// Linear interpolation resample from `from_rate` to `to_rate`.
/// Good enough for voice; not suitable for music.
fn resample(audio: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || audio.is_empty() {
        return audio.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = ((audio.len() as f64) / ratio).ceil() as usize;

    (0..output_len)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;

            if idx + 1 < audio.len() {
                // Linear interpolation between adjacent samples
                audio[idx] * (1.0 - frac) + audio[idx + 1] * frac
            } else if idx < audio.len() {
                audio[idx]
            } else {
                0.0
            }
        })
        .collect()
}
