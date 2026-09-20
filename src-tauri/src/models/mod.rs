//! Local speech model manager: registry, presence checks, checksum
//! verification, and progressive downloads.
//!
//! Delivery is **hybrid**: the default tiny STT model is staged and verified at
//! package time so the consumer installer works offline on first run. Other
//! models and voices are optional, checksum-verified downloads.
//!
//! Models live under `<app data>/models/{stt,tts}/<id>/`. The manager is
//! engine-agnostic (no sherpa dependency) so it always compiles, even in the
//! mock-only default build.

use std::io::Read;
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
    /// Required pinned SHA-256 (lowercase hex).
    pub sha256: &'static str,
}

/// A registered model (a set of files that together form a usable STT model or
/// TTS voice).
pub struct ModelEntry {
    pub id: &'static str,
    pub kind: ModelKind,
    pub display_name: &'static str,
    pub description: &'static str,
    /// Actually staged into the consumer installer by `prepare-release-models.ps1`.
    pub bundled: bool,
    pub approx_mb: u32,
    pub source: &'static str,
    pub revision: &'static str,
    pub license: &'static str,
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

    /// Verify every registered file is present, non-empty, and matches its
    /// immutable SHA-256 pin.
    pub fn verify(&self, models_root: &Path) -> Result<(), VfError> {
        for file in self.files {
            let path = self.file_path(models_root, file);
            let metadata = std::fs::metadata(&path).map_err(|_| VfError::ModelMissing {
                expected_path: path.display().to_string(),
                hint: "Download the model from Settings or reinstall the consumer package."
                    .to_string(),
            })?;
            if metadata.len() == 0 {
                return Err(VfError::ModelInvalid {
                    detail: format!("{} is empty", path.display()),
                });
            }
            verify_checksum(&path, file.sha256)?;
        }
        Ok(())
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
        source: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en",
        revision: "d026532c022fa99fd789d6b32446a1df7b6bfc43",
        license: "MIT (OpenAI Whisper model); retain upstream attribution",
        files: &[
            ModelFile {
                role: FileRole::SttEncoder,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-encoder.int8.onnx",
                filename: "tiny.en-encoder.int8.onnx",
                sha256: "0ce578b827c94a961aacb8fa14b02f096504b337e5c94be37c36238cbe3e8bc6",
            },
            ModelFile {
                role: FileRole::SttDecoder,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-decoder.int8.onnx",
                filename: "tiny.en-decoder.int8.onnx",
                sha256: "06c0e6ff6348d427e51839219d1c886c18cfdf411e629e33f5e1679bff9c1527",
            },
            ModelFile {
                role: FileRole::SttTokens,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-tiny.en/resolve/d026532c022fa99fd789d6b32446a1df7b6bfc43/tiny.en-tokens.txt",
                filename: "tiny.en-tokens.txt",
                sha256: "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930",
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
        source: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en",
        revision: "59eea950fc76df2453efb57e6c0fd334548e8ffe",
        license: "MIT (OpenAI Whisper model); retain upstream attribution",
        files: &[
            ModelFile {
                role: FileRole::SttEncoder,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-encoder.int8.onnx",
                filename: "base.en-encoder.int8.onnx",
                sha256: "ef6b936f4c9b1d90a3b68634b60c4ed8576b26172b33c2535ec0e933c9edb823",
            },
            ModelFile {
                role: FileRole::SttDecoder,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-decoder.int8.onnx",
                filename: "base.en-decoder.int8.onnx",
                sha256: "f7162ad6db2dbef16cfaeaa7f945b9d7dd9c1b8d472f6aca82f2273d185e4d41",
            },
            ModelFile {
                role: FileRole::SttTokens,
                url: "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-base.en/resolve/59eea950fc76df2453efb57e6c0fd334548e8ffe/base.en-tokens.txt",
                filename: "base.en-tokens.txt",
                sha256: "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930",
            },
        ],
    },
    // ---- TTS: bundled default voice (offline first-run) -----------------
    ModelEntry {
        id: "vits-ljs",
        kind: ModelKind::Tts,
        display_name: "VITS LJSpeech (English, female)",
        description: "Natural English neural voice. Optional download.",
        bundled: false,
        approx_mb: 115,
        source: "https://huggingface.co/csukuangfj/vits-ljs",
        revision: "7ac337c834f318e45a34037cb3371cc3929187ff",
        license: "Apache-2.0; LJ Speech dataset is public domain",
        files: &[
            ModelFile {
                role: FileRole::TtsModel,
                url: "https://huggingface.co/csukuangfj/vits-ljs/resolve/7ac337c834f318e45a34037cb3371cc3929187ff/vits-ljs.onnx",
                filename: "vits-ljs.onnx",
                sha256: "5bbd273797a9ecf8d94bd6ec02ad16cb41cbb85f055ad98d528ced3e44c9b31a",
            },
            ModelFile {
                role: FileRole::TtsTokens,
                url: "https://huggingface.co/csukuangfj/vits-ljs/resolve/7ac337c834f318e45a34037cb3371cc3929187ff/tokens.txt",
                filename: "tokens.txt",
                sha256: "5fee2c6b238d712287f2ecb08f34a8a8b413bcb7390862ef6fb6fd6f0f8d3a17",
            },
            ModelFile {
                role: FileRole::TtsLexicon,
                url: "https://huggingface.co/csukuangfj/vits-ljs/resolve/7ac337c834f318e45a34037cb3371cc3929187ff/lexicon.txt",
                filename: "lexicon.txt",
                sha256: "bdccfc6da71c45c48e2e0056fcf0aab760577c5f959f6c1b5eb3e3e916fd5a0e",
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
    pub verified: bool,
    pub source: String,
    pub revision: String,
    pub license: String,
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
            verified: e.verify(models_root).is_ok(),
            source: e.source.to_string(),
            revision: e.revision.to_string(),
            license: e.license.to_string(),
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
    let mut file = std::fs::File::open(path).map_err(|e| VfError::ModelChecksumMismatch {
        detail: format!("could not read {} for hashing: {e}", path.display()),
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| VfError::ModelChecksumMismatch {
                detail: format!("could not hash {}: {e}", path.display()),
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize()))
}

/// Compute the lowercase-hex SHA-256 of an in-memory buffer.
#[cfg(test)]
pub fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_digest(hasher.finalize())
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    let digest = digest.as_ref();
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

        // Skip only when the existing file matches the immutable checksum.
        if std::fs::metadata(&dest)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
        {
            if verify_checksum(&dest, file.sha256).is_ok() {
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
            std::fs::remove_file(&dest).map_err(|e| VfError::ModelDownloadFailed {
                detail: format!("could not remove corrupt {}: {e}", dest.display()),
            })?;
        }

        download_one(client, file, &dest, id, idx, file_count, &on_progress).await?;
        verify_checksum(&dest, file.sha256)?;

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
    let mut part_guard = PartialDownload::new(part.clone());
    let mut out =
        tokio::fs::File::create(&part)
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

    out.flush()
        .await
        .map_err(|e| VfError::ModelDownloadFailed {
            detail: format!("could not flush {}: {e}", part.display()),
        })?;
    out.sync_all()
        .await
        .map_err(|e| VfError::ModelDownloadFailed {
            detail: format!("could not sync {}: {e}", part.display()),
        })?;
    drop(out);

    verify_checksum(&part, file.sha256)?;

    tokio::fs::rename(&part, dest)
        .await
        .map_err(|e| VfError::ModelDownloadFailed {
            detail: format!("could not finalize {}: {e}", dest.display()),
        })?;
    part_guard.disarm();

    Ok(())
}

struct PartialDownload {
    path: PathBuf,
    armed: bool,
}

impl PartialDownload {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PartialDownload {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Copy verified package resources into app data on first run. Existing valid
/// files are preserved; corrupt files are replaced from the verified bundle.
pub fn install_bundled_models(resource_root: &Path, models_root: &Path) -> Result<(), VfError> {
    for entry in REGISTRY.iter().filter(|entry| entry.bundled) {
        let bundled_dir = resource_root
            .join("models")
            .join(entry.kind.subdir())
            .join(entry.id);
        for file in entry.files {
            let source = bundled_dir.join(file.filename);
            verify_checksum(&source, file.sha256).map_err(|e| VfError::ModelInvalid {
                detail: format!("bundled model verification failed: {e}"),
            })?;
            let destination = entry.file_path(models_root, file);
            if destination.exists() && verify_checksum(&destination, file.sha256).is_ok() {
                continue;
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| VfError::ModelInvalid {
                    detail: format!("could not create {}: {e}", parent.display()),
                })?;
            }
            let staged = destination.with_extension("installing");
            std::fs::copy(&source, &staged).map_err(|e| VfError::ModelInvalid {
                detail: format!(
                    "could not copy bundled model {} to {}: {e}",
                    source.display(),
                    staged.display()
                ),
            })?;
            verify_checksum(&staged, file.sha256)?;
            if destination.exists() {
                std::fs::remove_file(&destination).map_err(|e| VfError::ModelInvalid {
                    detail: format!("could not replace {}: {e}", destination.display()),
                })?;
            }
            std::fs::rename(&staged, &destination).map_err(|e| VfError::ModelInvalid {
                detail: format!("could not activate {}: {e}", destination.display()),
            })?;
        }
        entry.verify(models_root)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_bundled_defaults_for_both_kinds() {
        assert!(REGISTRY
            .iter()
            .any(|e| e.kind == ModelKind::Stt && e.bundled));
        assert!(!REGISTRY
            .iter()
            .any(|e| e.kind == ModelKind::Tts && e.bundled));
        assert_eq!(default_id(ModelKind::Stt), "whisper-tiny-en");
        assert_eq!(default_id(ModelKind::Tts), "vits-ljs");
    }

    #[test]
    fn every_registry_file_has_an_immutable_url_and_sha256() {
        for entry in REGISTRY {
            assert!(entry.revision.len() >= 40);
            for file in entry.files {
                assert!(file.url.contains(entry.revision));
                assert_eq!(file.sha256.len(), 64);
                assert!(file.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
            }
        }
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

    #[test]
    fn verify_rejects_present_but_corrupt_model() {
        let entry = find("whisper-tiny-en").unwrap();
        let root = std::env::temp_dir().join(format!("vf-corrupt-{}", uuid::Uuid::new_v4()));
        let dir = entry.dir(&root);
        std::fs::create_dir_all(&dir).unwrap();
        for file in entry.files {
            std::fs::write(dir.join(file.filename), b"not a model").unwrap();
        }
        assert!(entry.is_present(&root));
        let err = entry.verify(&root).unwrap_err();
        assert_eq!(err.code(), "ModelChecksumMismatch");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn partial_download_guard_removes_incomplete_file() {
        let path = std::env::temp_dir().join(format!("vf-part-{}.part", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"incomplete").unwrap();
        {
            let _guard = PartialDownload::new(path.clone());
        }
        assert!(!path.exists());
    }
}
