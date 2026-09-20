use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

fn main() {
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let bundled = [
            (
                "resources/models/stt/whisper-tiny-en/tiny.en-encoder.int8.onnx",
                "0ce578b827c94a961aacb8fa14b02f096504b337e5c94be37c36238cbe3e8bc6",
            ),
            (
                "resources/models/stt/whisper-tiny-en/tiny.en-decoder.int8.onnx",
                "06c0e6ff6348d427e51839219d1c886c18cfdf411e629e33f5e1679bff9c1527",
            ),
            (
                "resources/models/stt/whisper-tiny-en/tiny.en-tokens.txt",
                "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930",
            ),
        ];
        for (relative, expected) in bundled {
            let path = manifest.join(relative);
            let mut file = std::fs::File::open(&path).unwrap_or_else(|error| {
                panic!(
                    "required consumer STT resource {} is missing: {error}. Run `npm run prepare:release-models`.",
                    path.display()
                )
            });
            let mut hasher = Sha256::new();
            let mut buffer = vec![0u8; 1024 * 1024];
            loop {
                let read = file.read(&mut buffer).unwrap_or_else(|error| {
                    panic!("could not hash bundled model {}: {error}", path.display())
                });
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            let actual = format!("{:x}", hasher.finalize());
            assert_eq!(
                actual,
                expected,
                "bundled consumer STT resource {} failed SHA-256 verification",
                path.display()
            );
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    std::thread::Builder::new()
        .name("tauri-build".to_string())
        .stack_size(16 * 1024 * 1024)
        .spawn(tauri_build::build)
        .expect("failed to start Tauri build helper")
        .join()
        .expect("Tauri build helper panicked")
}
