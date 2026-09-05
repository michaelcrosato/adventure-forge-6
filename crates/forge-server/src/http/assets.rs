//! Compile-time browser asset allowlist.
//!
//! The build script emits the generated table in OUT_DIR.  There is
//! intentionally no runtime filesystem fallback: a server binary carries
//! exactly the asset set that was checked during its build.

pub(super) struct Asset {
    pub(super) path: &'static str,
    pub(super) content_type: &'static str,
    pub(super) bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/forge_server_browser_assets.rs"));

#[cfg(test)]
#[path = "../../asset_manifest.rs"]
mod generator;

#[cfg(test)]
mod tests {
    use super::generator::{self, AssetFile};
    use super::{build_id, entries, get};
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "forge-server-assets-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&root).expect("fixture directory is unique");
            Self { root }
        }

        fn write(&self, relative: &str, bytes: &[u8]) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture parent directory");
            }
            fs::write(path, bytes).expect("fixture asset");
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove exact fixture directory");
        }
    }

    fn valid_fixture(label: &str) -> Fixture {
        let fixture = Fixture::new(label);
        fixture.write(
            "index.html",
            b"<!doctype html><html><head><link rel=\"stylesheet\" href=\"/assets/app.css\"></head><body><script type=\"module\" src=\"/assets/app.js\"></script></body></html>",
        );
        fixture.write("assets/app.js", b"console.log('ready');");
        fixture.write("assets/app.css", b"body { color: black; }");
        fixture
    }

    #[test]
    fn manifest_is_nonempty_and_contains_index() {
        assert!(!entries().is_empty());
        let index = get("/index.html").expect("embedded index.html");
        assert_eq!(index.path, "/index.html");
        assert!(!index.bytes.is_empty());
    }

    #[test]
    fn build_id_is_a_stable_lower_hex_digest() {
        let id = build_id();
        assert_eq!(id.len(), 64);
        assert!(
            id.bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
    }

    #[test]
    fn every_manifest_path_is_an_exact_positive_lookup() {
        for asset in entries() {
            let found = get(asset.path).expect("manifest route must resolve");
            assert_eq!(found.path, asset.path);
            assert_eq!(found.content_type, asset.content_type);
            assert_eq!(found.bytes, asset.bytes);
        }
    }

    #[test]
    fn unsafe_and_non_allowlisted_paths_are_rejected_without_decoding() {
        for path in [
            "/../index.html",
            "/assets/../index.html",
            "/assets/%2e%2e/index.html",
            "/assets/%2Findex.js",
            "/assets\\index.js",
            "/assets/index.js?query=1",
            "/assets/index.js#fragment",
            "/assets/./index.js",
            "/assets//index.js",
        ] {
            assert!(get(path).is_none(), "unexpected asset route: {path}");
        }
    }

    #[test]
    fn public_index_has_no_capability_or_source_map_material() {
        let index = get("/index.html").expect("embedded index.html");
        let html = String::from_utf8(index.bytes.to_vec())
            .expect("index.html must be UTF-8")
            .to_ascii_lowercase();
        assert!(!html.contains("token"));
        assert!(!html.contains("authorization"));
        assert!(!html.contains("sourcemappingurl="));
        assert!(entries().iter().all(|asset| !asset.path.ends_with(".map")));
    }

    #[test]
    fn generator_rejects_missing_index_unsafe_paths_and_unexpected_extensions() {
        let missing = Fixture::new("missing-index");
        missing.write("assets/app.js", b"ready");
        assert!(generator::load(missing.path()).is_err());

        let hidden = valid_fixture("hidden");
        hidden.write("assets/.hidden.js", b"hidden");
        assert!(generator::load(hidden.path()).is_err());

        let unsafe_name = valid_fixture("unsafe-name");
        unsafe_name.write("assets/app%2ejs", b"unsafe");
        assert!(generator::load(unsafe_name.path()).is_err());

        let unexpected = valid_fixture("unexpected-extension");
        unexpected.write("assets/readme.txt", b"not an asset");
        assert!(generator::load(unexpected.path()).is_err());
    }

    #[test]
    fn generator_rejects_source_maps_and_index_capability_material() {
        let map = valid_fixture("map");
        map.write("assets/app.map", b"source map");
        assert!(generator::load(map.path()).is_err());

        let reference = valid_fixture("map-reference");
        reference.write("assets/app.js", b"//# sourceMappingURL=app.js.map");
        assert!(generator::load(reference.path()).is_err());

        let token = Fixture::new("html-token");
        token.write(
            "index.html",
            b"<html><body>token must not be embedded</body></html>",
        );
        token.write("assets/app.js", b"ready");
        assert!(generator::load(token.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn generator_rejects_symlink_files_and_directories() {
        use std::os::unix::fs::symlink;

        let file = valid_fixture("symlink-file");
        symlink(
            file.path().join("assets/app.js"),
            file.path().join("assets/link.js"),
        )
        .expect("create file symlink");
        assert!(generator::load(file.path()).is_err());

        let directory = valid_fixture("symlink-directory");
        fs::create_dir(directory.path().join("assets/real")).expect("real asset directory");
        fs::write(directory.path().join("assets/real/chunk.js"), b"chunk")
            .expect("real nested asset");
        symlink(
            directory.path().join("assets/real"),
            directory.path().join("assets/link"),
        )
        .expect("create directory symlink");
        assert!(generator::load(directory.path()).is_err());
    }

    #[test]
    fn duplicate_manifest_paths_are_rejected_before_embedding() {
        let make = |path: &str, content_type: &'static str, bytes: &[u8]| AssetFile {
            path: path.to_owned(),
            absolute_path: PathBuf::from("/fixture").join(path.trim_start_matches('/')),
            content_type,
            bytes: bytes.to_vec(),
        };
        let duplicate = vec![
            make("/index.html", "text/html; charset=utf-8", b"<html></html>"),
            make("/assets/app.js", "text/javascript; charset=utf-8", b"one"),
            make("/assets/app.js", "text/javascript; charset=utf-8", b"two"),
        ];
        assert!(generator::from_assets(duplicate).is_err());
    }

    #[test]
    fn ui_id_is_independent_of_directory_and_creation_order_but_changes_with_bytes() {
        let first = valid_fixture("determinism-first");
        let second = Fixture::new("determinism-second");
        second.write("assets/app.css", b"body { color: black; }");
        second.write("assets/app.js", b"console.log('ready');");
        second.write(
            "index.html",
            b"<!doctype html><html><head><link rel=\"stylesheet\" href=\"/assets/app.css\"></head><body><script type=\"module\" src=\"/assets/app.js\"></script></body></html>",
        );
        let first_manifest = generator::load(first.path()).expect("first manifest");
        let second_manifest = generator::load(second.path()).expect("second manifest");
        assert_eq!(first_manifest.build_id, second_manifest.build_id);
        assert_eq!(
            first_manifest
                .assets
                .iter()
                .map(|asset| asset.path.as_str())
                .collect::<Vec<_>>(),
            second_manifest
                .assets
                .iter()
                .map(|asset| asset.path.as_str())
                .collect::<Vec<_>>()
        );

        second.write("assets/app.js", b"console.log('changed');");
        let changed_manifest = generator::load(second.path()).expect("changed manifest");
        assert_ne!(first_manifest.build_id, changed_manifest.build_id);
    }

    #[test]
    fn generator_writes_the_allowlist_module_from_the_admitted_manifest() {
        let fixture = valid_fixture("generated");
        let manifest = generator::load(fixture.path()).expect("fixture manifest");
        let output = Fixture::new("generated-output");
        generator::write_generated(&manifest, output.path()).expect("generated module");
        let generated = fs::read_to_string(output.path().join("forge_server_browser_assets.rs"))
            .expect("generated module is readable");
        assert!(generated.contains("include_bytes!"));
        assert!(generated.contains("pub(super) fn get"));
        assert!(generated.contains(&manifest.build_id));
    }
}
