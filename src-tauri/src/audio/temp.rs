//! Temp WAV path management with RAII cleanup.
//!
//! Privacy requirement: captured audio must never persist. [`TempWav`] holds a
//! path in the OS temp dir and deletes the file on `Drop`, so the WAV is
//! removed on every code path — success, early return, or panic-unwind — in
//! addition to the explicit `cleanup()` the pipeline calls when done.

use std::path::{Path, PathBuf};

/// An RAII guard around a temporary WAV file. Dropping it best-effort deletes
/// the file if it still exists.
pub struct TempWav {
    path: PathBuf,
}

impl TempWav {
    /// Create a new unique temp path like `voiceflow-<uuid>.wav` in the OS temp
    /// directory. The file itself is created later by the WAV writer.
    pub fn new() -> Self {
        let name = format!("voiceflow-{}.wav", uuid::Uuid::new_v4());
        let path = std::env::temp_dir().join(name);
        TempWav { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Explicitly delete the file now. Safe to call multiple times; a
    /// subsequent `Drop` becomes a no-op because the file no longer exists.
    pub fn cleanup(&self) {
        if self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Drop for TempWav {
    fn drop(&mut self) {
        self.cleanup();
    }
}
