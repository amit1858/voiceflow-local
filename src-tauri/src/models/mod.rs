//! Local speech model manager: registry, presence checks, checksum
//! verification, and progressive downloads.
//!
//! Delivery is **hybrid**: a tiny STT model and one default TTS voice are marked
//! `bundled` so the packaged app works fully offline on first run (fetched into
//! the bundle at package time — never committed to git). Larger/better STT
//! models and extra voices are OPTIONAL, checksum-verified downloads with
//! progress events streamed to the UI.
//!
//! Models live under `<app data>/models/{stt,tts}/<id>/`. The manager is
//! engine-agnostic (no sherpa dependency) so it always compiles, even in the
//! mock-only default build.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use crate::errors::VfError;

/// Whether a model is used for speech-to-text or text-to-speech.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Stt,
    Tts,
}

impl ModelKind {
    /// Subdirectory name under the models root.
    pub fn subdir(self) -> &'static str {
        match self {
            ModelKind::Stt => "stt",
            ModelKind::Tts => "tts",
        }
    }
}

/// The purpose of an individual file within a model, so the speech providers can
/// resolve the exact path they need without guessing from filenames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    SttEncoder,
    SttDecoder,
    SttTokens,
    TtsModel,
    TtsTokens,
    TtsLexicon,
}

/// One downloadable file belonging to a model.
pub struct ModelFile {
    pub role: FileRole,
    pub url: &'static str,
    pub filename: &'static str,
    /// Optional pinned SHA-256 (lowercase hex). When present the download is
    /// verified and rejected on mismatch; when `None` the file is accepted as-is
    /// (still fetched over TLS). Pin these to harden a release.
    pub sha256: Option<&'static str>,
}

/// A registered model (a set of files that together form a usable STT model or
/// TTS voice).
pub struct ModelEntry {
    pub id: &'static str,
    pub kind: ModelKind,
    pub display_name: &'static str,
    pub description: &'static str,
    /// Shipped with the app for offline first-run.
    pub bundled: bool,
    pub approx_mb: u32,
    pub files: &'static [ModelFile],
}

impl ModelEntry {
    /// Directory holding this model's files: `<root>/<kind>/<id>`.
    pub fn dir(&self, models_root: &Path) -> PathBuf {
        models_root.join(self.kind.subdir()).join(self.id)
    }

    /// Absolute path of `file` inside this model's directory.
    pub fn file_path(&self, models_root: &Path, file: &ModelFile) -> PathBuf {
        self.dir(models_root).join(file.filename)
    }

    /// Resolve the on-disk path for the file with `role`, if the model has one.
    pub fn path_for_role(&self, models_root: &Path, role: FileRole) -> Option<PathBuf> {
        self.files
            .iter()
            .find(|f| f.role == role)
            .map(|f| self.file_path(models_root, f))
    }

    /// True when every file is present and non-empty on disk.
    pub fn is_present(&self, models_root: &Path) -> bool {
        self.files.iter().all(|f| {
            let p = self.file_path(models_root, f);
            std::fs::metadata(&p).map(|m| m.len() > 0).unwrap_or(false)
        })
    }
}

/// The full built-in model registry.
pub static REGISTRY: &[ModelEntry] = &[
    // ---- STT: bundled tiny model (offline first-run) --------------------
    ModelEntry {
        id: "whisper-tiny-en",
        kind: ModelKind::Stt,
        display_name: "Whisper tiny (English)",
        description: "Small, fast English speech-to-text. Bundled for offline first run.",
        bundled: true,
        approx_mb: 100,
        files: &[
            ModelFile {
                role: FileRole::SttEncoder,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/main/tiny.en-encoder.int8.onnx",
                filename: "tiny.en-encoder.int8.onnx",
                sha256: None,
            },
            ModelFile {
                role: FileRole::SttDecoder,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/main/tiny.en-decoder.int8.onnx",
                filename: "tiny.en-decoder.int8.onnx",
                sha256: None,
            },
            ModelFile {
                role: FileRole::SttTokens,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/main/tiny.en-tokens.txt",
                filename: "tiny.en-tokens.txt",
                sha256: None,
            },
        ],
    },
    // ---- STT: optional larger/better model ------------------------------
    ModelEntry {
        id: "whisper-base-en",
        kind: ModelKind::Stt,
        display_name: "Whisper base (English)",
        description: "More accurate English speech-to-text. Optional download.",
        bundled: false,
        approx_mb: 155,
        files: &[
            ModelFile {
                role: FileRole::SttEncoder,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/main/base.en-encoder.int8.onnx",
                filename: "base.en-encoder.int8.onnx",
                sha256: None,
            },
            ModelFile {
                role: FileRole::SttDecoder,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/main/base.en-decoder.int8.onnx",
                filename: "base.en-decoder.int8.onnx",
                sha256: None,
            },
            ModelFile {
                role: FileRole::SttTokens,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/main/base.en-tokens.txt",
                filename: "base.en-tokens.txt",
                sha256: None,
            },
        ],
    },
    // ---- TTS: bundled default voice (offline first-run) -----------------
    ModelEntry {
        id: "vits-ljs",
        kind: ModelKind::Tts,
        display_name: "VITS LJSpeech (English, female)",
        description: "Natural English neural voice. Bundled default TTS voice.",
        bundled: true,
        approx_mb: 115,
        files: &[
            ModelFile {
                role: FileRole::TtsModel,
                url: "https://huggingface.co/csukuangfj/vits-ljs/resolve/main/vits-ljs.onnx",
                filename: "vits-ljs.onnx",
                sha256: None,
            },
            ModelFile {
                role: FileRole::TtsTokens,
                url: "https://huggingface.co/csukuangfj/vits-ljs/resolve/main/tokens.txt",
                filename: "tokens.txt",
                sha256: None,
            },
            ModelFile {
                role: FileRole::TtsLexicon,
                url: "https://huggingface.co/csukuangfj/vits-ljs/resolve/main/lexicon.txt",
                filename: "lexicon.txt",
                sha256: None,
            },
        ],
    },
];

/// Look up a registry entry by id.
pub fn find(id: &str) -> Option<&'static ModelEntry> {
    REGISTRY.iter().find(|e| e.id == id)
}

/// The default (bundled) model id for a given kind.
pub fn default_id(kind: ModelKind) -> &'static str {
    REGISTRY
        .iter()
        .find(|e| e.kind == kind && e.bundled)
        .or_else(|| REGISTRY.iter().find(|e| e.kind == kind))
        .map(|e| e.id)
        .unwrap_or("")
}

/// Serializable model descriptor for the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub kind: ModelKind,
    pub display_name: String,
    pub description: String,
    pub bundled: bool,
    pub approx_mb: u32,
    pub installed: bool,
}

/// List all registered models with their installed state.
pub fn list_models(models_root: &Path) -> Vec<ModelInfo> {
    REGISTRY
        .iter()
        .map(|e| ModelInfo {
            id: e.id.to_string(),
            kind: e.kind,
            display_name: e.display_name.to_string(),
            description: e.description.to_string(),
            bundled: e.bundled,
            approx_mb: e.approx_mb,
            installed: e.is_present(models_root),
        })
        .collect()
}

/// Progress event for a model download, streamed to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub file: String,
    pub file_index: usize,
    pub file_count: usize,
    pub received: u64,
    pub total: Option<u64>,
    pub done: bool,
}

/// Compute the lowercase-hex SHA-256 of a file.
pub fn sha256_hex(path: &Path) -> Result<String, VfError> {
    let bytes = std::fs::read(path).map_err(|e| VfError::ModelChecksumMismatch {
        detail: format!("could not read {} for hashing: {e}", path.display()),
    })?;
    Ok(sha256_hex_bytes(&bytes))
}

/// Compute the lowercase-hex SHA-256 of an in-memory buffer.
pub fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Verify `path` against an expected lowercase-hex SHA-256.
pub fn verify_checksum(path: &Path, expected: &str) -> Result<(), VfError> {
    let actual = sha256_hex(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(VfError::ModelChecksumMismatch {
            detail: format!(
                "checksum mismatch for {}: expected {expected}, got {actual}",
                path.display()
            ),
        })
    }
}

/// Download every file for model `id` into its model directory, verifying any
/// pinned checksums and reporting progress via `on_progress`. Files already
/// present with a matching checksum are skipped.
pub async fn download_model<F>(
    models_root: &Path,
    id: &str,
    client: &reqwest::Client,
    on_progress: F,
) -> Result<(), VfError>
where
    F: Fn(DownloadProgress),
{
    let entry = find(id).ok_or_else(|| VfError::UnknownModel { id: id.to_string() })?;
    let dir = entry.dir(models_root);
    std::fs::create_dir_all(&dir).map_err(|e| VfError::ModelDownloadFailed {
        detail: format!("could not create {}: {e}", dir.display()),
    })?;

    let file_count = entry.files.len();
    for (idx, file) in entry.files.iter().enumerate() {
        let dest = entry.file_path(models_root, file);

        // Skip if already present and (when pinned) checksum-valid.
        if std::fs::metadata(&dest).map(|m| m.len() > 0).unwrap_or(false) {
            let ok = match file.sha256 {
                Some(sum) => verify_checksum(&dest, sum).is_ok(),
                None => true,
            };
            if ok {
                let len = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                on_progress(DownloadProgress {
                    model_id: id.to_string(),
                    file: file.filename.to_string(),
                    file_index: idx,
                    file_count,
                    received: len,
                    total: Some(len),
                    done: true,
                });
                continue;
            }
        }

        download_one(client, file, &dest, id, idx, file_count, &on_progress).await?;

        if let Some(sum) = file.sha256 {
            verify_checksum(&dest, sum)?;
        }

        let len = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        on_progress(DownloadProgress {
            model_id: id.to_string(),
            file: file.filename.to_string(),
            file_index: idx,
            file_count,
            received: len,
            total: Some(len),
            done: true,
        });
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_one<F>(
    client: &reqwest::Client,
    file: &ModelFile,
    dest: &Path,
    model_id: &str,
    file_index: usize,
    file_count: usize,
    on_progress: &F,
) -> Result<(), VfError>
where
    F: Fn(DownloadProgress),
{
    let resp = client
        .get(file.url)
        .send()
        .await
        .map_err(|e| VfError::ModelDownloadFailed {
            detail: format!("request to {} failed: {e}", file.url),
        })?;

    if !resp.status().is_success() {
        return Err(VfError::ModelDownloadFailed {
            detail: format!("{} returned HTTP {}", file.url, resp.status()),
        });
    }

    let total = resp.content_length();
    let part = dest.with_extension("part");
    let mut out = tokio::fs::File::create(&part)
        .await
        .map_err(|e| VfError::ModelDownloadFailed {
            detail: format!("could not create {}: {e}", part.display()),
        })?;

    let mut received: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| VfError::ModelDownloadFailed {
            detail: format!("download stream error for {}: {e}", file.url),
        })?;
        out.write_all(&chunk)
            .await
            .map_err(|e| VfError::ModelDownloadFailed {
                detail: format!("could not write {}: {e}", part.display()),
            })?;
        received += chunk.len() as u64;
        on_progress(DownloadProgress {
            model_id: model_id.to_string(),
            file: file.filename.to_string(),
            file_index,
            file_count,
            received,
            total,
            done: false,
        });
    }

    out.flush().await.ok();
    drop(out);

    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| VfError::ModelDownloadFailed {
            detail: format!("could not finalize {}: {e}", dest.display()),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_bundled_defaults_for_both_kinds() {
        assert!(REGISTRY.iter().any(|e| e.kind == ModelKind::Stt && e.bundled));
        assert!(REGISTRY.iter().any(|e| e.kind == ModelKind::Tts && e.bundled));
        assert_eq!(default_id(ModelKind::Stt), "whisper-tiny-en");
        assert_eq!(default_id(ModelKind::Tts), "vits-ljs");
    }

    #[test]
    fn find_resolves_known_and_unknown() {
        assert!(find("whisper-tiny-en").is_some());
        assert!(find("does-not-exist").is_none());
    }

    #[test]
    fn stt_entry_exposes_encoder_decoder_tokens_roles() {
        let e = find("whisper-tiny-en").unwrap();
        let root = std::env::temp_dir();
        assert!(e.path_for_role(&root, FileRole::SttEncoder).is_some());
        assert!(e.path_for_role(&root, FileRole::SttDecoder).is_some());
        assert!(e.path_for_role(&root, FileRole::SttTokens).is_some());
        // A TTS role is absent on an STT model.
        assert!(e.path_for_role(&root, FileRole::TtsModel).is_none());
    }

    #[test]
    fn sha256_matches_known_vectors() {
        // SHA-256 of the empty string.
        assert_eq!(
            sha256_hex_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // SHA-256 of "abc".
        assert_eq!(
            sha256_hex_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn verify_checksum_detects_match_and_mismatch() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("vf-model-test-{}.bin", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"abc").unwrap();

        verify_checksum(
            &path,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .expect("matching checksum should pass");

        let err = verify_checksum(&path, "deadbeef").unwrap_err();
        assert_eq!(err.code(), "ModelChecksumMismatch");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn is_present_false_when_files_missing() {
        let e = find("whisper-tiny-en").unwrap();
        let root = std::env::temp_dir().join(format!("vf-empty-{}", uuid::Uuid::new_v4()));
        assert!(!e.is_present(&root));
    }
}
