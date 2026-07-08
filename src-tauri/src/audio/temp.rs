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
        let t = TempWav::new();
        let name = t.path().file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("voiceflow-"));
        assert!(name.ends_with(".wav"));
        assert!(t.path().starts_with(std::env::temp_dir()));
    }

    #[test]
    fn explicit_cleanup_removes_file() {
        let t = TempWav::new();
        write_dummy(t.path());
        assert!(t.path().exists());
        t.cleanup();
        assert!(!t.path().exists());
        // Idempotent: second cleanup is a no-op.
        t.cleanup();
    }

    #[test]
    fn drop_removes_file() {
        let path;
        {
            let t = TempWav::new();
            path = t.path().to_path_buf();
            write_dummy(&path);
            assert!(path.exists());
        } // t dropped here
        assert!(!path.exists(), "Drop must delete the temp WAV");
    }
}
