//! Microphone capture via `cpal`.
//!
//! `cpal`'s input stream is not `Send` on Windows (WASAPI), so the stream is
//! owned by a dedicated capture thread. The thread builds the stream, confirms
//! start-up over a "ready" channel (so mic/permission errors surface
//! synchronously to the caller), accumulates mono `f32` samples, and — when
//! signalled to stop — resamples to 16 kHz, converts to `i16`, and writes the
//! WAV. The finished audio is returned via the thread's join handle.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use crate::audio::wav::{write_wav_16k_mono, TARGET_SAMPLE_RATE};
use crate::errors::VfError;

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
    /// there is no microphone / permission was denied).
    pub fn start(output_path: PathBuf) -> Result<Self, VfError> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), VfError>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let thread_path = output_path.clone();
        let thread = std::thread::spawn(move || {
            capture_loop(thread_path, ready_tx, stop_rx)
        });

        // Wait for the capture thread to report whether the stream started.
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(ActiveRecording { stop_tx, thread, output_path }),
            Ok(Err(e)) => {
                let _ = thread.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                Err(VfError::AudioCaptureFailed {
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
            Err(_) => Err(VfError::AudioCaptureFailed {
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

    let err_buffer = Arc::new(Mutex::new(None::<String>));
    let err_slot = Arc::clone(&err_buffer);
    let err_fn = move |e: cpal::StreamError| {
        if let Ok(mut slot) = err_slot.lock() {
            *slot = Some(e.to_string());
        }
    };

    // Build a stream appropriate to the device's native sample format.
    let stream_result = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                push_mono(&cb_buffer, data, channels, |s| s);
            },
            err_fn,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                push_mono(&cb_buffer, data, channels, |s| s as f32 / i16::MAX as f32);
            },
            err_fn,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            &config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                push_mono(&cb_buffer, data, channels, |s| {
                    (s as f32 - u16::MAX as f32 / 2.0) / (u16::MAX as f32 / 2.0)
                });
            },
            err_fn,
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

    // Block until asked to stop (or the sender is dropped).
    let _ = stop_rx.recv();

    // Stop the stream and flush.
    drop(stream);

    if let Some(msg) = err_buffer.lock().ok().and_then(|g| g.clone()) {
        return Err(VfError::AudioCaptureFailed { detail: msg });
    }

    let captured = {
        let mut guard = buffer.lock().map_err(|_| VfError::AudioCaptureFailed {
            detail: "sample buffer poisoned".into(),
        })?;
        std::mem::take(&mut *guard)
    };

    if captured.is_empty() {
        return Err(VfError::AudioCaptureFailed {
            detail: "no audio was captured".into(),
        });
    }

    let resampled = resample_linear(&captured, sample_rate, TARGET_SAMPLE_RATE);
    let pcm: Vec<i16> = resampled
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect();

    write_wav_16k_mono(&output_path, &pcm)?;
    Ok(())
}

/// Down-mix an interleaved frame buffer to mono and append to `buffer`.
fn push_mono<T: Copy>(
    buffer: &Arc<Mutex<Vec<f32>>>,
    data: &[T],
    channels: usize,
    convert: impl Fn(T) -> f32,
) {
    if channels == 0 {
        return;
    }
    if let Ok(mut guard) = buffer.lock() {
        if channels == 1 {
            guard.extend(data.iter().map(|&s| convert(s)));
        } else {
            for frame in data.chunks(channels) {
                let sum: f32 = frame.iter().map(|&s| convert(s)).sum();
                guard.push(sum / frame.len() as f32);
            }
        }
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
        VfError::AudioCaptureFailed { detail: msg.to_string() }
    }
}
