//! Temp WAV path management with RAII cleanup.
//!
//! Privacy requirement: captured audio must never persist. [`TempWav`] holds a
//! path in an app-owned OS temp subdirectory and deletes the file on `Drop`, so the WAV is
//! removed on every code path — success, early return, or panic-unwind — in
//! addition to the explicit `cleanup()` the pipeline calls when done.

use std::path::{Path, PathBuf};

use crate::errors::VfError;

const TEMP_SUBDIR: &str = "voiceflow-local";

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(TEMP_SUBDIR).join("audio")
}

/// An RAII guard around a temporary WAV file. Dropping it best-effort deletes
/// the file if it still exists.
pub struct TempWav {
    path: PathBuf,
}

impl TempWav {
    /// Create a new unique temp path like `voiceflow-<uuid>.wav` in the OS temp
    /// directory. The file itself is created later by the WAV writer.
    pub fn new() -> Result<Self, VfError> {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).map_err(|e| VfError::TempAudioFailed {
            detail: format!("could not create app temp directory {}: {e}", dir.display()),
        })?;
        let name = format!("{}.wav", uuid::Uuid::new_v4());
        Ok(TempWav {
            path: dir.join(name),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Explicitly delete the file now. Safe to call multiple times; a
    /// subsequent `Drop` becomes a no-op because the file no longer exists.
    pub fn cleanup(&self) -> Result<(), VfError> {
        if self.path.exists() {
            std::fs::remove_file(&self.path).map_err(|e| VfError::TempCleanupFailed {
                detail: format!("could not remove {}: {e}", self.path.display()),
            })?;
        }
        Ok(())
    }
}

impl Drop for TempWav {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CleanupReport {
    pub removed: usize,
    pub failures: Vec<String>,
}

/// Remove stale VoiceFlow-owned WAVs only. Files in the OS temp root or
/// unrelated files inside the app directory are never touched.
pub fn cleanup_stale() -> Result<CleanupReport, VfError> {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).map_err(|e| VfError::TempCleanupFailed {
        detail: format!("could not create/read {}: {e}", dir.display()),
    })?;
    let entries = std::fs::read_dir(&dir).map_err(|e| VfError::TempCleanupFailed {
        detail: format!("could not read {}: {e}", dir.display()),
    })?;
    let mut report = CleanupReport::default();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                report.failures.push(e.to_string());
                continue;
            }
        };
        let path = entry.path();
        let is_owned_wav = path.extension().and_then(|v| v.to_str()) == Some("wav")
            && path
                .file_stem()
                .and_then(|v| v.to_str())
                .and_then(|v| uuid::Uuid::parse_str(v).ok())
                .is_some();
        if !is_owned_wav {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => report.removed += 1,
            Err(e) => report.failures.push(format!("{}: {e}", path.display())),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_dummy(path: &Path) {
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(b"RIFFdummy").unwrap();
    }

    #[test]
    fn path_is_in_temp_dir_and_named() {
        let t = TempWav::new().unwrap();
        let name = t.path().file_name().unwrap().to_string_lossy().to_string();
        assert!(name.ends_with(".wav"));
        assert!(uuid::Uuid::parse_str(name.trim_end_matches(".wav")).is_ok());
        assert!(t.path().starts_with(temp_dir()));
    }

    #[test]
    fn explicit_cleanup_removes_file() {
        let t = TempWav::new().unwrap();
        write_dummy(t.path());
        assert!(t.path().exists());
        t.cleanup().unwrap();
        assert!(!t.path().exists());
        // Idempotent: second cleanup is a no-op.
        t.cleanup().unwrap();
    }

    #[test]
    fn drop_removes_file() {
        let path;
        {
            let t = TempWav::new().unwrap();
            path = t.path().to_path_buf();
            write_dummy(&path);
            assert!(path.exists());
        } // t dropped here
        assert!(!path.exists(), "Drop must delete the temp WAV");
    }

    #[test]
    fn stale_cleanup_ignores_unrelated_files() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let owned = dir.join(format!("{}.wav", uuid::Uuid::new_v4()));
        let unrelated = dir.join("keep-me.wav");
        write_dummy(&owned);
        write_dummy(&unrelated);
        let report = cleanup_stale().unwrap();
        assert!(report.removed >= 1);
        assert!(!owned.exists());
        assert!(unrelated.exists());
        std::fs::remove_file(unrelated).unwrap();
    }
}
