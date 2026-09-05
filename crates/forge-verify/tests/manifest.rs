use forge_content::parse_and_compile_production;
use forge_kernel::{BuildManifest, sha256_hex_bytes, sha256_json};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// This oracle discovers inputs from the reviewed ownership contract. It does
// not read generated constants, import build.rs, or ask the build for its list.
fn source_paths(root: &Path, relative: &str, paths: &mut BTreeSet<String>) {
    for entry in fs::read_dir(root.join(relative)).expect("authoritative source directory exists") {
        let entry = entry.expect("authoritative entry is readable");
        let name = format!(
            "{relative}/{}",
            entry.file_name().to_str().expect("UTF-8 path")
        );
        let kind = entry.file_type().expect("authoritative entry has a type");
        assert!(
            !kind.is_symlink(),
            "authoritative source must not be a symlink"
        );
        if kind.is_dir() {
            source_paths(root, &name, paths);
        } else {
            assert!(
                kind.is_file(),
                "authoritative source must be a regular file"
            );
            paths.insert(name);
        }
    }
}

fn file_digest(root: &Path, name: &str) -> String {
    sha256_hex_bytes(&fs::read(root.join(name)).expect("authoritative input exists"))
}

fn input_digest(root: &Path, source_dir: Option<&str>, extra: &[&str]) -> String {
    let mut paths: BTreeSet<String> = extra.iter().map(|path| (*path).to_owned()).collect();
    if let Some(directory) = source_dir {
        source_paths(root, directory, &mut paths);
    }
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(file_digest(root, &path).as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex_bytes(&bytes)
}

#[test]
fn generated_manifest_matches_independent_input_digests() {
    let root = root();
    let manifest = BuildManifest::generated();
    assert_eq!(
        manifest.kernel_source_sha256(),
        input_digest(
            &root,
            Some("crates/forge-kernel/src"),
            &[
                "crates/forge-kernel/Cargo.toml",
                "crates/forge-kernel/build.rs",
                "crates/forge-kernel/authoritative-config.json",
                "crates/forge-kernel/schema-rules-abi.json",
            ]
        ),
        "manifest omitted authoritative kernel input"
    );
    assert_eq!(
        manifest.compiler_source_sha256(),
        input_digest(
            &root,
            Some("crates/forge-content/src"),
            &[
                "crates/forge-content/Cargo.toml",
                "crates/forge-content/build.rs",
            ]
        ),
        "manifest omitted authoritative compiler input"
    );
    assert_eq!(
        manifest.replay_source_sha256(),
        input_digest(
            &root,
            Some("crates/forge-replay/src"),
            &["crates/forge-replay/Cargo.toml"]
        ),
        "manifest omitted authoritative replay input"
    );
    for (actual, path) in [
        (manifest.cargo_lock_sha256(), "Cargo.lock"),
        (manifest.rust_toolchain_sha256(), "rust-toolchain.toml"),
        (
            manifest.authoritative_config_sha256(),
            "crates/forge-kernel/authoritative-config.json",
        ),
        (
            manifest.schema_rules_abi_sha256(),
            "crates/forge-kernel/schema-rules-abi.json",
        ),
    ] {
        assert_eq!(
            actual,
            file_digest(&root, path),
            "manifest omitted input {path}"
        );
    }
    assert_eq!(
        manifest.build_scripts_and_manifests_sha256(),
        input_digest(
            &root,
            None,
            &[
                "Cargo.toml",
                "rust-toolchain.toml",
                "crates/forge-kernel/Cargo.toml",
                "crates/forge-kernel/build.rs",
                "crates/forge-kernel/authoritative-config.json",
                "crates/forge-kernel/schema-rules-abi.json",
                "crates/forge-content/Cargo.toml",
                "crates/forge-content/build.rs",
                "crates/forge-replay/Cargo.toml",
            ]
        ),
        "manifest omitted build configuration input"
    );
    let abi: Value = serde_json::from_slice(
        &fs::read(root.join("crates/forge-kernel/schema-rules-abi.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest.schema_abi_version(),
        abi["schema_abi"].as_str().unwrap()
    );
    assert_eq!(
        manifest.rules_abi_version(),
        abi["rules_abi"].as_str().unwrap()
    );
    assert_eq!(
        manifest.entropy_algorithm(),
        abi["entropy_algorithm"].as_str().unwrap()
    );
    assert_eq!(
        manifest.entropy_algorithm_sha256(),
        sha256_hex_bytes(abi["entropy_algorithm"].as_str().unwrap().as_bytes())
    );

    let content = parse_and_compile_production(include_str!("../../../content/split-tide.json"))
        .expect("production content compiles");
    assert_eq!(content.manifest(), &manifest);
    // Reconstruct the serialized identity payload without using has_valid_build_id.
    let mut payload = serde_json::to_value(&content).unwrap();
    let build_id = payload.as_object_mut().unwrap().remove("build_id").unwrap();
    assert_eq!(
        build_id.as_str().unwrap(),
        sha256_json(&payload).unwrap(),
        "compiled build omitted its manifest or content"
    );
    // A disposable-copy driver compares these commitments before and after real
    // input changes and restores the file. This test itself never writes sources.
    println!(
        "MANIFEST_PROBE {} {}",
        manifest.digest(),
        content.build_id()
    );
}
