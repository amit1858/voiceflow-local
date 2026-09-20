//! WAV read/write helpers built on `hound`.
//!
//! VoiceFlow always works with **16 kHz, mono, 16-bit PCM** WAV files, which is
//! exactly what the sherpa-onnx speech engine expects. The recorder writes with
//! [`write_wav_16k_mono`] and the transcription provider reads back with
//! [`read_wav_as_f32_mono`].

use std::path::Path;

use hound::{SampleFormat, WavSpec, WavWriter};

use crate::errors::VfError;

/// Target sample rate for the pipeline (sherpa-onnx STT requirement).
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Write mono 16-bit PCM samples at 16 kHz to `path`.
pub fn write_wav_16k_mono(path: &Path, samples: &[i16]) -> Result<(), VfError> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec).map_err(|e| VfError::AudioCaptureFailed {
        detail: format!("WAV create failed: {e}"),
    })?;

    for &s in samples {
        writer
            .write_sample(s)
            .map_err(|e| VfError::AudioCaptureFailed {
                detail: format!("WAV write failed: {e}"),
            })?;
    }

    writer.finalize().map_err(|e| VfError::AudioCaptureFailed {
        detail: format!("WAV finalize failed: {e}"),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_valid_wav_for_empty_capture() {
        // A quick start/stop tap can yield zero samples; the recorder still
        // writes a valid (silent) WAV so the file exists and the pipeline can
        // proceed (mock mode returns its canned transcript regardless).
        let dir = std::env::temp_dir();
        let path = dir.join(format!("voiceflow-test-empty-{}.wav", std::process::id()));

        write_wav_16k_mono(&path, &[]).expect("empty WAV should write cleanly");
        assert!(path.exists(), "an empty capture must still produce a file");

        let (samples, rate) = read_wav_as_f32_mono(&path).expect("empty WAV should read back");
        assert!(samples.is_empty());
        assert_eq!(rate, TARGET_SAMPLE_RATE);

        let _ = std::fs::remove_file(&path);
    }
}

/// Read a WAV file and return normalized `f32` mono samples in `[-1.0, 1.0]`.
///
/// If the file is multi-channel it is down-mixed by averaging; if it is not
/// already 16 kHz the caller is responsible for resampling (the recorder always
/// writes 16 kHz, so this is a no-op in practice — but we still surface the rate).
///
/// Only used by the Sherpa transcription path; unused when built without the
/// `sherpa` feature.
#[cfg_attr(not(feature = "sherpa"), allow(dead_code))]
pub fn read_wav_as_f32_mono(path: &Path) -> Result<(Vec<f32>, u32), VfError> {
    let mut reader = hound::WavReader::open(path).map_err(|e| VfError::TranscriptionFailed {
        detail: format!("WAV open failed: {e}"),
    })?;

    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;

    let interleaved: Vec<f32> = match spec.sample_format {
        SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<Result<Vec<f32>, _>>()
                .map_err(|e| VfError::TranscriptionFailed {
                    detail: format!("WAV decode failed: {e}"),
                })?
        }
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|e| VfError::TranscriptionFailed {
                detail: format!("WAV decode failed: {e}"),
            })?,
    };

    // Down-mix to mono by averaging channels.
    let mono = if channels <= 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    Ok((mono, spec.sample_rate))
}
