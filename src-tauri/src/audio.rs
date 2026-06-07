use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

/// Start capturing audio from the default input device.
///
/// Spawns a thread that:
///   1. Initialises cpal and opens the default mic
///   2. Stores the actual sample rate so transcribe.rs can resample correctly
///   3. Mixes down to mono and appends samples to `audio_buffer`
///   4. Keeps the stream alive until `is_recording` goes false
pub fn start_recording(
    is_recording: Arc<AtomicBool>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: Arc<Mutex<u32>>,
) {
    is_recording.store(true, Ordering::SeqCst);
    audio_buffer.lock().unwrap().clear();

    let recording_flag = is_recording.clone();
    let buffer_clone = audio_buffer.clone();

    std::thread::spawn(move || {
        let host = cpal::default_host();

        let device = match host.default_input_device() {
            Some(d) => d,
            None => {
                eprintln!("[audio] No input device found");
                recording_flag.store(false, Ordering::SeqCst);
                return;
            }
        };

        let config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[audio] Could not get input config: {e}");
                recording_flag.store(false, Ordering::SeqCst);
                return;
            }
        };

        // Store the actual sample rate (typically 44100 or 48000)
        let sr = config.sample_rate().0;
        *sample_rate.lock().unwrap() = sr;
        let channels = config.channels() as usize;

        eprintln!("[audio] Recording at {sr} Hz, {channels}ch");

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                if recording_flag.load(Ordering::SeqCst) {
                    // Mix down to mono by averaging channels
                    let mono: Vec<f32> = if channels > 1 {
                        data.chunks(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                            .collect()
                    } else {
                        data.to_vec()
                    };
                    buffer_clone.lock().unwrap().extend_from_slice(&mono);
                }
            },
            |err| eprintln!("[audio] Stream error: {err}"),
            None, // no timeout
        );

        match stream {
            Ok(s) => {
                if let Err(e) = s.play() {
                    eprintln!("[audio] Could not start stream: {e}");
                    return;
                }
                // Keep this thread (and therefore the stream) alive while recording.
                // The stream is dropped when this thread exits, automatically stopping capture.
                while is_recording.load(Ordering::SeqCst) {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                eprintln!("[audio] Recording stopped");
                // `s` drops here → stream closes cleanly
            }
            Err(e) => eprintln!("[audio] Could not build stream: {e}"),
        }
    });
}

/// Signal stop, wait for the capture thread to flush its last callback,
/// then return a snapshot of the full PCM buffer.
pub fn stop_recording(
    is_recording: Arc<AtomicBool>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
) -> Vec<f32> {
    is_recording.store(false, Ordering::SeqCst);

    // Give the audio callback thread time to flush any in-flight samples
    std::thread::sleep(std::time::Duration::from_millis(150));

    let buf = audio_buffer.lock().unwrap().clone();
    eprintln!("[audio] Captured {} samples", buf.len());
    buf
}
