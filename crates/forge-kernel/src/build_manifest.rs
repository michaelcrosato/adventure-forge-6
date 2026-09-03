use crate::hash::sha256_json;
use serde::Serialize;

include!(concat!(env!("OUT_DIR"), "/generated_build_manifest.rs"));

/// Repository-controlled build provenance.  There is intentionally no public
/// constructor: a content document can provide data, but cannot claim that it
/// was produced by a different rules/compiler/toolchain build.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BuildManifest {
    kernel_source_sha256: String,
    compiler_source_sha256: String,
    cargo_lock_sha256: String,
    rust_toolchain_sha256: String,
    authoritative_config_sha256: String,
    schema_rules_abi_sha256: String,
    build_scripts_and_manifests_sha256: String,
    entropy_algorithm_sha256: String,
    schema_abi_version: String,
    rules_abi_version: String,
    entropy_algorithm: String,
}

impl BuildManifest {
    pub fn generated() -> Self {
        Self {
            kernel_source_sha256: KERNEL_SOURCE_SHA256.to_owned(),
            compiler_source_sha256: COMPILER_SOURCE_SHA256.to_owned(),
            cargo_lock_sha256: CARGO_LOCK_SHA256.to_owned(),
            rust_toolchain_sha256: RUST_TOOLCHAIN_SHA256.to_owned(),
            authoritative_config_sha256: AUTHORITATIVE_CONFIG_SHA256.to_owned(),
            schema_rules_abi_sha256: SCHEMA_RULES_ABI_SHA256.to_owned(),
            build_scripts_and_manifests_sha256: BUILD_SCRIPTS_AND_MANIFESTS_SHA256.to_owned(),
            entropy_algorithm_sha256: ENTROPY_ALGORITHM_SHA256.to_owned(),
            schema_abi_version: SCHEMA_ABI_VERSION.to_owned(),
            rules_abi_version: RULES_ABI_VERSION.to_owned(),
            entropy_algorithm: ENTROPY_ALGORITHM.to_owned(),
        }
    }

    pub fn kernel_source_sha256(&self) -> &str {
        &self.kernel_source_sha256
    }

    pub fn compiler_source_sha256(&self) -> &str {
        &self.compiler_source_sha256
    }

    pub fn cargo_lock_sha256(&self) -> &str {
        &self.cargo_lock_sha256
    }

    pub fn rust_toolchain_sha256(&self) -> &str {
        &self.rust_toolchain_sha256
    }

    pub fn authoritative_config_sha256(&self) -> &str {
        &self.authoritative_config_sha256
    }

    pub fn schema_rules_abi_sha256(&self) -> &str {
        &self.schema_rules_abi_sha256
    }

    pub fn build_scripts_and_manifests_sha256(&self) -> &str {
        &self.build_scripts_and_manifests_sha256
    }

    pub fn entropy_algorithm_sha256(&self) -> &str {
        &self.entropy_algorithm_sha256
    }

    pub fn schema_abi_version(&self) -> &str {
        &self.schema_abi_version
    }

    pub fn rules_abi_version(&self) -> &str {
        &self.rules_abi_version
    }

    pub fn entropy_algorithm(&self) -> &str {
        &self.entropy_algorithm
    }

    pub fn digest(&self) -> String {
        sha256_json(self).expect("build manifest must be serializable")
    }
}

#[cfg(test)]
mod tests {
    use super::BuildManifest;

    fn assert_digest(value: &str) {
        assert_eq!(value.len(), 64);
        assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn generated_manifest_is_stable_and_has_trusted_digests() {
        let first = BuildManifest::generated();
        let second = BuildManifest::generated();
        assert_eq!(first, second);
        assert_eq!(first.digest(), second.digest());

        for digest in [
            first.kernel_source_sha256(),
            first.compiler_source_sha256(),
            first.cargo_lock_sha256(),
            first.rust_toolchain_sha256(),
            first.authoritative_config_sha256(),
            first.schema_rules_abi_sha256(),
            first.build_scripts_and_manifests_sha256(),
            first.entropy_algorithm_sha256(),
        ] {
            assert_digest(digest);
        }
        assert!(!first.schema_abi_version().is_empty());
        assert!(!first.rules_abi_version().is_empty());
        assert!(!first.entropy_algorithm().is_empty());
    }

    #[test]
    fn generated_manifest_carries_only_repository_trusted_abi() {
        let manifest = BuildManifest::generated();
        assert_eq!(manifest.schema_abi_version(), "forge-schema-v1");
        assert_eq!(manifest.rules_abi_version(), "forge-rules-v1");
        assert_eq!(manifest.entropy_algorithm(), "splitmix64-v1");
    }
}
