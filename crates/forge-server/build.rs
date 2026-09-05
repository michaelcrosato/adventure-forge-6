#[path = "asset_manifest.rs"]
mod asset_manifest;

use std::{env, path::PathBuf};

fn main() {
    if let Err(error) = generate() {
        panic!("could not embed browser assets: {error}");
    }
}

fn generate() -> Result<(), String> {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("CARGO_MANIFEST_DIR is unset")?);
    let dist_dir = manifest_dir.join("../../browser/dist");
    println!("cargo:rerun-if-changed={}", dist_dir.display());

    let manifest = asset_manifest::load(&dist_dir)?;
    for asset in &manifest.assets {
        println!("cargo:rerun-if-changed={}", asset.absolute_path.display());
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is unset")?);
    asset_manifest::write_generated(&manifest, &out_dir)
}
