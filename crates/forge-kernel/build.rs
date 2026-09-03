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
        let metadata = fs::metadata(&path).expect("build input must be readable");
        if metadata.is_dir() {
            let mut entries: Vec<_> = fs::read_dir(&path)
                .expect("build input directory must be readable")
                .map(|entry| entry.expect("build input entry must be readable").path())
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

fn digest_files(repo_root: &Path, paths: &[PathBuf]) -> String {
    let mut entries = Vec::new();
    for path in paths {
        let bytes = fs::read(path).expect("build input must be readable");
        let relative = path
            .strip_prefix(repo_root)
            .expect("build input must be under repository root")
            .to_string_lossy()
            .replace('\\', "/");
        entries.push(format!("{relative}\0{}\n", sha256_hex(&bytes)));
    }
    entries.sort();
    sha256_hex(entries.join("").as_bytes())
}

fn digest_file(path: &Path) -> String {
    match fs::read(path) {
        Ok(bytes) => sha256_hex(&bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            sha256_hex(format!("missing:{}", path.display()).as_bytes())
        }
        Err(error) => panic!("cannot read build input {}: {error}", path.display()),
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let kernel = repo_root.join("crates/forge-kernel");
    let compiler = repo_root.join("crates/forge-content");

    let kernel_source_files = files_under(&kernel.join("src"));
    let compiler_source_files = files_under(&compiler.join("src"));
    let kernel_manifest = kernel.join("Cargo.toml");
    let compiler_manifest = compiler.join("Cargo.toml");
    let kernel_build = kernel.join("build.rs");
    let compiler_build = compiler.join("build.rs");
    let cargo_manifest = repo_root.join("Cargo.toml");
    let cargo_lock = repo_root.join("Cargo.lock");
    let toolchain = repo_root.join("rust-toolchain.toml");
    let config = kernel.join("authoritative-config.json");
    let abi = kernel.join("schema-rules-abi.json");

    let mut kernel_inputs = kernel_source_files.clone();
    kernel_inputs.extend([
        kernel_manifest.clone(),
        kernel_build.clone(),
        config.clone(),
        abi.clone(),
    ]);
    let mut compiler_inputs = compiler_source_files.clone();
    compiler_inputs.extend([compiler_manifest.clone(), compiler_build.clone()]);
    let mut scripts_and_manifests = vec![
        kernel_manifest.clone(),
        compiler_manifest.clone(),
        kernel_build.clone(),
        compiler_build.clone(),
        cargo_manifest.clone(),
        toolchain.clone(),
        config.clone(),
        abi.clone(),
    ];
    scripts_and_manifests.sort();

    for path in kernel_inputs
        .iter()
        .chain(compiler_inputs.iter())
        .chain(scripts_and_manifests.iter())
    {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-changed={}", cargo_lock.display());

    let output = format!(
        "pub const KERNEL_SOURCE_SHA256: &str = \"{}\";\n\
         pub const COMPILER_SOURCE_SHA256: &str = \"{}\";\n\
         pub const CARGO_LOCK_SHA256: &str = \"{}\";\n\
         pub const RUST_TOOLCHAIN_SHA256: &str = \"{}\";\n\
         pub const AUTHORITATIVE_CONFIG_SHA256: &str = \"{}\";\n\
         pub const SCHEMA_RULES_ABI_SHA256: &str = \"{}\";\n\
         pub const BUILD_SCRIPTS_AND_MANIFESTS_SHA256: &str = \"{}\";\n\
         pub const SCHEMA_ABI_VERSION: &str = \"forge-schema-v1\";\n\
         pub const RULES_ABI_VERSION: &str = \"forge-rules-v1\";\n\
         pub const ENTROPY_ALGORITHM: &str = \"splitmix64-v1\";\n\
         pub const ENTROPY_ALGORITHM_SHA256: &str = \"{}\";\n",
        digest_files(&repo_root, &kernel_inputs),
        digest_files(&repo_root, &compiler_inputs),
        digest_file(&cargo_lock),
        digest_file(&toolchain),
        digest_file(&config),
        digest_file(&abi),
        digest_files(&repo_root, &scripts_and_manifests),
        sha256_hex(b"splitmix64-v1"),
    );
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("generated_build_manifest.rs"), output)
        .expect("generated build manifest must be writable");
}
