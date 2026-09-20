//! Microphone capture via `cpal`.
//!
//! `cpal`'s input stream is not `Send` on Windows (WASAPI), so the stream is
//! owned by a dedicated capture thread. The thread builds the stream, confirms
//! start-up over a "ready" channel (so mic/permission errors surface
//! synchronously to the caller), accumulates mono `f32` samples, and — when
//! signalled to stop — resamples to 16 kHz, converts to `i16`, and writes the
//! WAV. The finished audio is returned via the thread's join handle.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::audio::wav::{write_wav_16k_mono, TARGET_SAMPLE_RATE};
use crate::errors::VfError;

/// Minimum acceptable capture duration. A quick start/stop tap below this is
/// reported as [`VfError::RecordingTooShort`] instead of yielding an empty
/// transcript.
pub const MIN_CAPTURE_SECS: f32 = 0.35;

/// RMS level below which a capture is treated as silence (no speech). Normal
/// speech sits well above this; true silence is orders of magnitude lower.
pub const SILENCE_RMS: f32 = 0.0025;

/// Callback invoked ~10×/second with the current input level in `[0.0, 1.0]`
/// so the UI can show a live mic meter.
pub type LevelCb = Arc<dyn Fn(f32) + Send + Sync>;

/// Compute the RMS (root-mean-square) level of mono samples in `[-1, 1]`.
pub fn rms_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    ((sum_sq / samples.len() as f64).sqrt()) as f32
}

/// Guard a finished capture: reject taps that are too short or effectively
/// silent with a precise, typed error. Returns `Ok(())` for usable audio.
pub fn evaluate_capture(samples: &[f32], sample_rate: u32) -> Result<(), VfError> {
    let rate = sample_rate.max(1) as f32;
    let duration = samples.len() as f32 / rate;
    if duration < MIN_CAPTURE_SECS {
        return Err(VfError::RecordingTooShort {
            detail: format!(
                "the recording was {duration:.2}s; hold the hotkey and speak for at least \
{MIN_CAPTURE_SECS:.2}s."
            ),
        });
    }
    let level = rms_level(samples);
    if level < SILENCE_RMS {
        return Err(VfError::NoSpeechDetected {
            detail: format!(
                "the input level was near silence (rms {level:.4}). Move closer to the mic, \
unmute it, or check the Windows input device."
            ),
        });
    }
    Ok(())
}

/// Handle to an in-progress recording running on a background capture thread.
pub struct ActiveRecording {
    stop_tx: Sender<()>,
    thread: JoinHandle<Result<(), VfError>>,
    /// Where the WAV will be written when recording stops.
    output_path: PathBuf,
}

impl ActiveRecording {
    /// Start capturing from the default input device, writing to `output_path`
    /// when stopped. Returns once capture has actually begun (or fails fast if
    /// there is no microphone / permission was denied). `on_level` (optional) is
    /// called ~10×/second with the live input level for a UI mic meter.
    pub fn start(output_path: PathBuf, on_level: Option<LevelCb>) -> Result<Self, VfError> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), VfError>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let thread_path = output_path.clone();
        let thread =
            std::thread::spawn(move || capture_loop(thread_path, ready_tx, stop_rx, on_level));

        // Wait for the capture thread to report whether the stream started.
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(ActiveRecording {
                stop_tx,
                thread,
                output_path,
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(VfError::AudioStartFailed {
                    detail: "capture thread exited before start".into(),
                })
            }
        }
    }

    /// Signal the capture thread to stop, wait for the WAV to be written, and
    /// return the output path.
    pub fn stop(self) -> Result<PathBuf, VfError> {
        // If the receiver is already gone the thread has finished; ignore.
        let _ = self.stop_tx.send(());
        match self.thread.join() {
            Ok(Ok(())) => Ok(self.output_path),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(VfError::AudioStopFailed {
                detail: "capture thread panicked".into(),
            }),
        }
    }

    /// Stop capturing and discard the audio (used for cancel). Any partially
    /// written file is left for the caller's temp guard to clean up.
    pub fn cancel(self) {
        let _ = self.stop_tx.send(());
        let _ = self.thread.join();
    }
}

/// Runs on the capture thread. Builds the input stream, streams samples into a
/// shared buffer, then resamples + writes the WAV on stop.
fn capture_loop(
    output_path: PathBuf,
    ready_tx: Sender<Result<(), VfError>>,
    stop_rx: Receiver<()>,
    on_level: Option<LevelCb>,
) -> Result<(), VfError> {
    let host = cpal::default_host();
    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            let _ = ready_tx.send(Err(VfError::NoMicrophone));
            return Err(VfError::NoMicrophone);
        }
    };

    let default_config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => {
            let err = map_stream_error(&e.to_string());
            let _ = ready_tx.send(Err(err.clone()));
            return Err(err);
        }
    };

    let sample_rate = default_config.sample_rate().0;
    let channels = default_config.config().channels as usize;
    let sample_format = default_config.sample_format();
    let config: cpal::StreamConfig = default_config.into();

    // Shared, growable buffer of captured mono f32 samples.
    let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let cb_buffer = Arc::clone(&buffer);

    // Live input level (RMS of the most recent callback), shared with a ticker
    // thread that pushes it to the UI.
    let level = Arc::new(Mutex::new(0.0f32));
    let cb_level = Arc::clone(&level);

    let err_buffer = Arc::new(Mutex::new(None::<String>));
    let make_err_fn = {
        let err_buffer = Arc::clone(&err_buffer);
        move || {
            let err_slot = Arc::clone(&err_buffer);
            move |e: cpal::StreamError| {
                if let Ok(mut slot) = err_slot.lock() {
                    *slot = Some(e.to_string());
                }
            }
        }
    };

    // Build a stream appropriate to the device's native sample format.
    let stream_result = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            {
                let cb_buffer = Arc::clone(&cb_buffer);
                let cb_level = Arc::clone(&cb_level);
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    push_mono(&cb_buffer, &cb_level, data, channels, |s| s);
                }
            },
            make_err_fn(),
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            {
                let cb_buffer = Arc::clone(&cb_buffer);
                let cb_level = Arc::clone(&cb_level);
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    push_mono(&cb_buffer, &cb_level, data, channels, |s| {
                        s as f32 / i16::MAX as f32
                    });
                }
            },
            make_err_fn(),
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            {
                let cb_buffer = Arc::clone(&cb_buffer);
                let cb_level = Arc::clone(&cb_level);
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    push_mono(&cb_buffer, &cb_level, data, channels, |s| {
                        (s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0)
                    });
                }
            },
            make_err_fn(),
            None,
        ),
        other => {
            let err = VfError::AudioCaptureFailed {
                detail: format!("unsupported sample format: {other:?}"),
            };
            let _ = ready_tx.send(Err(err.clone()));
            return Err(err);
        }
    };

    let stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            let err = map_stream_error(&e.to_string());
            let _ = ready_tx.send(Err(err.clone()));
            return Err(err);
        }
    };

    if let Err(e) = stream.play() {
        let err = map_stream_error(&e.to_string());
        let _ = ready_tx.send(Err(err.clone()));
        return Err(err);
    }

    // Capture is live.
    let _ = ready_tx.send(Ok(()));

    // Emit the live input level to the UI ~10×/second until we stop.
    let ticker_running = Arc::new(AtomicBool::new(true));
    let ticker_handle = on_level.map(|cb| {
        let level_r = Arc::clone(&level);
        let running_r = Arc::clone(&ticker_running);
        std::thread::spawn(move || {
            while running_r.load(Ordering::Relaxed) {
                let v = level_r.lock().map(|g| *g).unwrap_or(0.0);
                cb(v);
                std::thread::sleep(Duration::from_millis(100));
            }
            // Final zero so the meter settles.
            cb(0.0);
        })
    });

    // Block until asked to stop (or the sender is dropped).
    let _ = stop_rx.recv();

    // Stop the stream and flush.
    drop(stream);

    // Stop the level ticker.
    ticker_running.store(false, Ordering::Relaxed);
    if let Some(h) = ticker_handle {
        let _ = h.join();
    }

    if let Some(msg) = err_buffer.lock().ok().and_then(|g| g.clone()) {
        return Err(VfError::AudioCaptureFailed { detail: msg });
    }

    let captured = {
        let mut guard = buffer.lock().map_err(|_| VfError::AudioCaptureFailed {
            detail: "sample buffer poisoned".into(),
        })?;
        std::mem::take(&mut *guard)
    };

    // Resample to 16 kHz mono first, then guard against too-short / silent
    // captures with a precise, typed error. A quick tap or a muted mic now
    // yields a friendly "too short / no speech detected" message instead of an
    // empty transcript. (The RAII temp guard still deletes any file.)
    let resampled = resample_linear(&captured, sample_rate, TARGET_SAMPLE_RATE);
    evaluate_capture(&resampled, TARGET_SAMPLE_RATE)?;

    let pcm: Vec<i16> = resampled
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect();

    write_wav_16k_mono(&output_path, &pcm)?;
    Ok(())
}

/// Down-mix an interleaved frame buffer to mono, append to `buffer`, and update
/// the shared live-level slot with this chunk's RMS.
fn push_mono<T: Copy>(
    buffer: &Arc<Mutex<Vec<f32>>>,
    level: &Arc<Mutex<f32>>,
    data: &[T],
    channels: usize,
    convert: impl Fn(T) -> f32,
) {
    if channels == 0 {
        return;
    }
    let mut chunk: Vec<f32> = Vec::new();
    if let Ok(mut guard) = buffer.lock() {
        if channels == 1 {
            for &s in data {
                let v = convert(s);
                chunk.push(v);
                guard.push(v);
            }
        } else {
            for frame in data.chunks(channels) {
                let sum: f32 = frame.iter().map(|&s| convert(s)).sum();
                let v = sum / frame.len() as f32;
                chunk.push(v);
                guard.push(v);
            }
        }
    }
    if let Ok(mut slot) = level.lock() {
        *slot = rms_level(&chunk);
    }
}

/// Simple linear resampler from `in_rate` to `out_rate` for mono f32 samples.
fn resample_linear(input: &[f32], in_rate: u32, out_rate: u32) -> Vec<f32> {
    if in_rate == out_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = out_rate as f64 / in_rate as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = input.get(idx).copied().unwrap_or(0.0);
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

/// Map a cpal error string to a typed [`VfError`], detecting permission issues.
fn map_stream_error(msg: &str) -> VfError {
    let lower = msg.to_lowercase();
    if lower.contains("denied")
        || lower.contains("permission")
        || lower.contains("access is denied")
    {
        VfError::MicPermissionDenied
    } else {
        VfError::AudioCaptureFailed {
            detail: msg.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(rms_level(&[]), 0.0);
        assert_eq!(rms_level(&[0.0; 1000]), 0.0);
    }

    #[test]
    fn rms_of_full_scale_square_is_one() {
        let sig: Vec<f32> = (0..1000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let level = rms_level(&sig);
        assert!((level - 1.0).abs() < 1e-6, "rms was {level}");
    }

    #[test]
    fn rms_of_half_amplitude_sine_is_about_0_35() {
        // A 0.5-amplitude sine has RMS = 0.5 / sqrt(2) ≈ 0.3536.
        let sig: Vec<f32> = (0..16000)
            .map(|i| 0.5 * (i as f32 * 2.0 * std::f32::consts::PI * 440.0 / 16000.0).sin())
            .collect();
        let level = rms_level(&sig);
        assert!((level - 0.3536).abs() < 0.01, "rms was {level}");
    }

    #[test]
    fn evaluate_rejects_too_short() {
        // 0.1s at 16 kHz = 1600 samples, below MIN_CAPTURE_SECS (0.35s).
        let sig = vec![0.5f32; 1600];
        let err = evaluate_capture(&sig, TARGET_SAMPLE_RATE).unwrap_err();
        assert!(matches!(err, VfError::RecordingTooShort { .. }));
    }

    #[test]
    fn evaluate_rejects_silence() {
        // 1s of near-silence: long enough, but below SILENCE_RMS.
        let sig = vec![0.0f32; TARGET_SAMPLE_RATE as usize];
        let err = evaluate_capture(&sig, TARGET_SAMPLE_RATE).unwrap_err();
        assert!(matches!(err, VfError::NoSpeechDetected { .. }));
    }

    #[test]
    fn evaluate_accepts_normal_speech_level() {
        // 1s of a 0.3-amplitude tone: long enough and above the silence floor.
        let sig: Vec<f32> = (0..TARGET_SAMPLE_RATE as usize)
            .map(|i| 0.3 * (i as f32 * 2.0 * std::f32::consts::PI * 220.0 / 16000.0).sin())
            .collect();
        assert!(evaluate_capture(&sig, TARGET_SAMPLE_RATE).is_ok());
    }
}
