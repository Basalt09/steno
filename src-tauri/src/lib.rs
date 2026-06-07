use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use whisper_rs::WhisperContext;

mod audio;
mod transcribe;

// ─── App State ───────────────────────────────────────────────────────────────

pub struct AppState {
    /// True while mic is actively capturing audio
    pub is_recording: Arc<AtomicBool>,
    /// Raw f32 PCM samples from mic (mono)
    pub audio_buffer: Arc<Mutex<Vec<f32>>>,
    /// Actual sample rate reported by cpal at runtime
    pub sample_rate: Arc<Mutex<u32>>,
    /// Whisper model, loaded once on first use and reused for every transcription
    pub whisper: Arc<Mutex<Option<Arc<WhisperContext>>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: Arc::new(Mutex::new(44100)), // cpal default; updated at runtime
            whisper: Arc::new(Mutex::new(None)),
        }
    }
}

// ─── Tauri Commands ──────────────────────────────────────────────────────────

#[tauri::command]
fn is_recording(state: State<AppState>) -> bool {
    state.is_recording.load(Ordering::SeqCst)
}

// ─── Typing helper ───────────────────────────────────────────────────────────

fn type_text(text: &str) {
    use enigo::{Enigo, Keyboard, Settings};

    // Small delay: gives time for hotkey release to return focus to target window
    std::thread::sleep(std::time::Duration::from_millis(150));

    match Enigo::new(&Settings::default()) {
        Ok(mut enigo) => {
            if let Err(e) = enigo.text(text) {
                eprintln!("enigo type error: {e}");
            }
        }
        Err(e) => eprintln!("enigo init error: {e}"),
    }
}

// ─── App Entry Point ─────────────────────────────────────────────────────────

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle().clone();

            // Clone state handles for use in the shortcut callback
            let state: State<AppState> = app.state();
            let is_recording = state.is_recording.clone();
            let audio_buffer = state.audio_buffer.clone();
            let sample_rate = state.sample_rate.clone();
            let whisper = state.whisper.clone();

            // Register Ctrl+Shift+Space as push-to-talk
            // NOTE: ShortcutState::Pressed / Released fires on key-down and key-up.
            //       If the OS doesn't report Released events for your shortcut combo,
            //       swap to a toggle approach: one press starts, next press stops.
            let shortcut = Shortcut::new(Some(Modifiers::SHIFT), Code::Space);

            app.handle()
                .global_shortcut()
                .on_shortcut(shortcut, move |_app_handle, _shortcut, event| {
                    match event.state() {
                        ShortcutState::Pressed => {
                            // Only start if not already recording
                            if !is_recording.load(Ordering::SeqCst) {
                                let _ = handle.emit("recording-start", ());
                                audio::start_recording(
                                    is_recording.clone(),
                                    audio_buffer.clone(),
                                    sample_rate.clone(),
                                );
                            }
                        }

                        ShortcutState::Released => {
                            if is_recording.load(Ordering::SeqCst) {
                                let _ = handle.emit("transcribing", ());

                                // Capture clones for the transcription thread
                                let is_rec = is_recording.clone();
                                let buf = audio_buffer.clone();
                                let sr = sample_rate.clone();
                                let whisper = whisper.clone();
                                let handle2 = handle.clone();

                                std::thread::spawn(move || {
                                    // Stop recording and retrieve buffer
                                    let pcm = audio::stop_recording(is_rec, buf);
                                    let input_sr = *sr.lock().unwrap();

                                    // Load the model once and cache it. Re-reading the
                                    // multi-hundred-MB model on every utterance was the main
                                    // source of latency; subsequent calls reuse this instance.
                                    let ctx = {
                                        let mut cache = whisper.lock().unwrap();
                                        if cache.is_none() {
                                            let model_path = transcribe::resolve_model_path();
                                            match transcribe::load_model(&model_path) {
                                                Ok(c) => *cache = Some(Arc::new(c)),
                                                Err(e) => {
                                                    eprintln!("Model load error: {e}");
                                                    let _ = handle2.emit("transcription-error", &e);
                                                    return;
                                                }
                                            }
                                        }
                                        cache.clone().unwrap()
                                    };

                                    match transcribe::transcribe(&ctx, &pcm, input_sr) {
                                        Ok(text) if !text.trim().is_empty() => {
                                            // Type the text at the current cursor position
                                            type_text(&text);
                                            let _ = handle2.emit("transcription", &text);
                                        }
                                        Ok(_) => {
                                            // Empty transcription — nothing to type
                                            let _ = handle2.emit("transcription", "[silence]");
                                        }
                                        Err(e) => {
                                            eprintln!("Transcription error: {e}");
                                            let _ = handle2.emit("transcription-error", &e);
                                        }
                                    }
                                });
                            }
                        }
                    }
                })?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![is_recording])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
