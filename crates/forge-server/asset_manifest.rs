use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

pub struct AssetFile {
    pub path: String,
    pub absolute_path: PathBuf,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

pub struct Manifest {
    pub assets: Vec<AssetFile>,
    pub build_id: String,
}

pub fn load(dist_dir: &Path) -> Result<Manifest, String> {
    let mut assets = Vec::new();
    collect_directory(dist_dir, Path::new(""), &mut assets)?;
    from_assets(assets)
}

pub fn write_generated(manifest: &Manifest, out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir)
        .map_err(|error| format!("create {}: {error}", out_dir.display()))?;
    let generated_path = out_dir.join("forge_server_browser_assets.rs");
    let generated = render_generated_module(manifest)?;
    fs::write(&generated_path, generated)
        .map_err(|error| format!("write {}: {error}", generated_path.display()))?;
    Ok(())
}

pub fn from_assets(mut assets: Vec<AssetFile>) -> Result<Manifest, String> {
    assets.sort_by(|left, right| left.path.cmp(&right.path));
    if assets.is_empty() {
        return Err("browser/dist contains no assets".to_owned());
    }
    for pair in assets.windows(2) {
        if pair[0].path == pair[1].path {
            return Err(format!("duplicate browser asset path: {}", pair[0].path));
        }
    }
    if !assets.iter().any(|asset| asset.path == "/index.html") {
        return Err("browser/dist/index.html is required".to_owned());
    }
    for asset in &assets {
        validate_manifest_asset(asset)?;
    }
    let build_id = build_id(&assets);
    Ok(Manifest { assets, build_id })
}

fn collect_directory(
    root: &Path,
    relative_directory: &Path,
    assets: &mut Vec<AssetFile>,
) -> Result<(), String> {
    let directory = if relative_directory.as_os_str().is_empty() {
        root.to_owned()
    } else {
        root.join(relative_directory)
    };
    let metadata = fs::symlink_metadata(&directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "browser asset directory may not be a symlink: {}",
            directory.display()
        ));
    }
    if !metadata.is_dir() {
        return Err(format!(
            "browser asset root is not a directory: {}",
            directory.display()
        ));
    }

    let entries = fs::read_dir(&directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?;
    let mut children = entries
        .map(|entry| entry.map_err(|error| format!("read {}: {error}", directory.display())))
        .collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());

    for entry in children {
        let name = entry.file_name();
        validate_component(&name)?;
        let relative_path = relative_directory.join(&name);
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "browser asset may not be a symlink: {}",
                entry.path().display()
            ));
        }
        if file_type.is_dir() {
            collect_directory(root, &relative_path, assets)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(format!(
                "browser asset is not a regular file: {}",
                entry.path().display()
            ));
        }

        let path = relative_path
            .to_str()
            .ok_or_else(|| {
                format!(
                    "browser asset path is not UTF-8: {}",
                    entry.path().display()
                )
            })?
            .replace('\\', "/");
        validate_relative_path(&path)?;
        let bytes = fs::read(entry.path())
            .map_err(|error| format!("read browser asset {}: {error}", entry.path().display()))?;
        assets.push(AssetFile {
            path: format!("/{path}"),
            absolute_path: entry.path(),
            content_type: content_type(&path)?,
            bytes,
        });
    }
    Ok(())
}

fn validate_manifest_asset(asset: &AssetFile) -> Result<(), String> {
    let path = asset
        .path
        .strip_prefix('/')
        .ok_or_else(|| format!("browser asset path must be absolute: {}", asset.path))?;
    validate_relative_path(path)?;
    let expected_content_type = content_type(path)?;
    if expected_content_type != asset.content_type {
        return Err(format!("browser asset MIME mismatch: {}", asset.path));
    }
    if path != "index.html" && !path.starts_with("assets/") {
        return Err(format!(
            "browser asset must be index.html or live below assets/: {}",
            asset.path
        ));
    }
    if asset.path == "/index.html" {
        validate_index_html(&asset.bytes)?;
    }
    if asset
        .bytes
        .windows(b"sourceMappingURL=".len())
        .any(|window| window.eq_ignore_ascii_case(b"sourceMappingURL="))
    {
        return Err(format!("source map reference is forbidden: {}", asset.path));
    }
    Ok(())
}

fn validate_component(component: &OsStr) -> Result<(), String> {
    let component = component
        .to_str()
        .ok_or_else(|| "browser asset path is not UTF-8".to_owned())?;
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.starts_with('.')
        || !component.bytes().all(safe_path_byte)
    {
        return Err(format!(
            "unsafe browser asset path component: {component:?}"
        ));
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('%')
        || path.contains('?')
        || path.contains('#')
        || path
            .split('/')
            .any(|component| component == "." || component == "..")
        || !path
            .bytes()
            .all(|byte| byte == b'/' || safe_path_byte(byte))
    {
        return Err(format!("unsafe browser asset path: {path:?}"));
    }
    Ok(())
}

fn safe_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
}

fn content_type(path: &str) -> Result<&'static str, String> {
    let extension = Path::new(path)
        .extension()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("browser asset has no extension: {path}"))?;
    match extension {
        "html" => Ok("text/html; charset=utf-8"),
        "js" => Ok("text/javascript; charset=utf-8"),
        "css" => Ok("text/css; charset=utf-8"),
        "svg" => Ok("image/svg+xml"),
        "ico" => Ok("image/x-icon"),
        "map" => Err(format!("source maps are forbidden: {path}")),
        other => Err(format!(
            "unsupported browser asset extension .{other}: {path}"
        )),
    }
}

fn validate_index_html(bytes: &[u8]) -> Result<(), String> {
    let lower = bytes.iter().map(u8::to_ascii_lowercase).collect::<Vec<_>>();
    if lower
        .windows(b"token".len())
        .any(|window| window == b"token")
    {
        return Err("browser index.html must not contain a capability token".to_owned());
    }
    if lower
        .windows(b"authorization".len())
        .any(|window| window == b"authorization")
    {
        return Err("browser index.html must not contain authorization data".to_owned());
    }
    if lower
        .windows(b"sourcemappingurl=".len())
        .any(|window| window == b"sourcemappingurl=")
    {
        return Err("browser index.html must not contain a source map reference".to_owned());
    }
    Ok(())
}

fn build_id(assets: &[AssetFile]) -> String {
    let mut hasher = Sha256::new();
    for asset in assets {
        hash_frame(&mut hasher, asset.path.as_bytes());
        hash_frame(&mut hasher, asset.content_type.as_bytes());
        hash_frame(&mut hasher, &asset.bytes);
    }
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn hash_frame(hasher: &mut Sha256, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).expect("asset frame length fits in u64");
    hasher.update(length.to_le_bytes());
    hasher.update(bytes);
}

fn render_generated_module(manifest: &Manifest) -> Result<String, String> {
    let mut output = String::new();
    output.push_str("// @generated by forge-server/build.rs; do not edit.\n");
    for (index, asset) in manifest.assets.iter().enumerate() {
        let path = rust_string_literal(&asset.absolute_path)?;
        writeln!(
            &mut output,
            "static ASSET_{index}_BYTES: &[u8] = include_bytes!({path});"
        )
        .expect("writing to String cannot fail");
        writeln!(
            &mut output,
            "static ASSET_{index}: Asset = Asset {{ path: {:?}, content_type: {:?}, bytes: ASSET_{index}_BYTES }};",
            asset.path, asset.content_type
        )
        .expect("writing to String cannot fail");
    }
    output.push_str("static ASSETS: &[&Asset] = &[\n");
    for index in 0..manifest.assets.len() {
        writeln!(&mut output, "    &ASSET_{index},").expect("writing to String cannot fail");
    }
    output.push_str("];\n\n");
    writeln!(
        &mut output,
        "pub(super) fn entries() -> &'static [&'static Asset] {{ ASSETS }}"
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "pub(super) fn get(path: &str) -> Option<&'static Asset> {{ entries().iter().find(|asset| asset.path == path).copied() }}"
    )
    .expect("writing to String cannot fail");
    writeln!(
        &mut output,
        "pub(super) fn build_id() -> &'static str {{ {:?} }}",
        manifest.build_id
    )
    .expect("writing to String cannot fail");
    Ok(output)
}

fn rust_string_literal(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or_else(|| format!("browser asset path is not UTF-8: {}", path.display()))?;
    Ok(format!("{path:?}"))
}
