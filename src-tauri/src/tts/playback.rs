//! Local audio playback for synthesized TTS, via `rodio` (pure-Rust, no native
//! toolchain).
//!
//! `rodio`'s `OutputStream` is not `Send`, so playback runs on a dedicated
//! thread that owns the stream + sink. The returned [`ActivePlayback`] holds a
//! stop flag and the join handle so a new Speak (or Stop) can interrupt the
//! current one. Errors (no output device, sink failure) surface synchronously
//! as typed [`VfError`]s.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::errors::VfError;
use crate::tts::Synthesized;

/// Handle to in-progress playback. Dropping it (or calling [`stop`]) stops audio.
pub struct ActivePlayback {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ActivePlayback {
    /// Stop playback and wait for the audio thread to unwind.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}

impl Drop for ActivePlayback {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Play `audio` on the default output device. Returns once playback has begun
/// (or fails fast with a typed error if there is no output device). Audio
/// continues on a background thread until it finishes or is stopped.
pub fn play(audio: Synthesized) -> Result<ActivePlayback, VfError> {
    use rodio::buffer::SamplesBuffer;
    use rodio::{OutputStream, Sink};

    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), VfError>>();

    let thread = std::thread::spawn(move || {
        // The stream + handle must stay alive for the whole playback, so they
        // live on this thread's stack.
        let (_stream, handle) = match OutputStream::try_default() {
            Ok(s) => s,
            Err(_) => {
                let _ = ready_tx.send(Err(VfError::NoAudioOutputDevice));
                return;
            }
        };
        let sink = match Sink::try_new(&handle) {
            Ok(s) => s,
            Err(e) => {
                let _ = ready_tx.send(Err(VfError::TtsPlaybackFailed { detail: e.to_string() }));
                return;
            }
        };

        let buf = SamplesBuffer::new(1, audio.sample_rate, audio.samples);
        sink.append(buf);
        let _ = ready_tx.send(Ok(()));

        // Poll until the queue drains or a stop is requested.
        while !sink.empty() {
            if stop_thread.load(Ordering::Relaxed) {
                sink.stop();
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(ActivePlayback { stop, thread: Some(thread) }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = thread.join();
            Err(VfError::TtsPlaybackFailed {
                detail: "playback thread ended before starting".to_string(),
            })
        }
    }
}
