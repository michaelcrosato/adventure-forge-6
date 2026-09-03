use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = fs::metadata(&path).expect("verifier input must be readable");
        if metadata.is_dir() {
            let mut entries: Vec<_> = fs::read_dir(&path)
                .expect("verifier input directory must be readable")
                .map(|entry| entry.expect("verifier input entry must be readable").path())
                .collect();
            entries.sort();
            pending.extend(entries.into_iter().rev());
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let mut inputs = files_under(&manifest_dir.join("src"));
    inputs.extend([
        manifest_dir.join("Cargo.toml"),
        manifest_dir.join("build.rs"),
        repo_root.join("Cargo.lock"),
        repo_root.join("rust-toolchain.toml"),
    ]);
    inputs.sort();

    let mut declarations = Vec::new();
    for path in &inputs {
        let bytes = fs::read(path).unwrap_or_else(|error| {
            panic!("cannot read verifier input {}: {error}", path.display())
        });
        let relative = path
            .strip_prefix(repo_root)
            .expect("verifier input must be inside the repository")
            .to_string_lossy()
            .replace('\\', "/");
        declarations.push(format!("{relative}\0{}\n", sha256_hex(&bytes)));
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src").display()
    );

    let verifier_id = sha256_hex(declarations.join("").as_bytes());
    let output = format!("pub const VERIFIER_ID: &str = \"{verifier_id}\";\n");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("verifier_id.rs"), output)
        .expect("generated verifier identity must be writable");
}
