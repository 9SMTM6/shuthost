use base64::{Engine as _, engine::general_purpose};
use eyre::WrapErr as _;
use sha2::{Digest as _, Sha256};
use shuthost_common::VERSION;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

macro_rules! include_frontend_asset {
    ($path:expr) => {
        include_str!(concat!("../../frontend/assets/", $path))
    };
}

pub fn compute_hashes() -> eyre::Result<()> {
    let generated_dir = PathBuf::from("../frontend/assets/generated");
    fs::create_dir_all(&generated_dir)?;

    let asset_hashes = hash_non_template_assets()?;
    let manifest_hash =
        generate_manifest(&generated_dir, &asset_hashes.svg_hashes, &asset_hashes.icon_hashes)?;
    set_cargo_env_vars(
        &asset_hashes.styles_hash,
        &manifest_hash,
        &asset_hashes.icon_hashes,
        &asset_hashes.svg_hashes,
    );
    write_build_data(
        &generated_dir,
        &asset_hashes.styles_hash,
        &asset_hashes.styles_integrity,
        &manifest_hash,
        &asset_hashes.icon_hashes,
        &asset_hashes.svg_hashes,
    )?;

    Ok(())
}

struct AssetHashes {
    styles_hash: String,
    styles_integrity: String,
    icon_hashes: HashMap<u32, String>,
    svg_hashes: HashMap<String, String>,
}

fn hash_non_template_assets() -> eyre::Result<AssetHashes> {
    let styles_css = fs::read_to_string("../frontend/assets/generated/app.css")
        .wrap_err("Failed to read generated app.css")?;
    let styles_hash = url_hash(styles_css.as_bytes());
    let styles_integrity = integrity_hash(&styles_css);

    let favicon_short_hash = url_hash(include_frontend_asset!("favicon.svg").as_bytes());

    let sizes: [u32; _] = [32, 48, 64, 128, 180, 192, 512];
    let mut icon_hashes = HashMap::new();
    for &size in &sizes {
        let png_path = format!("../frontend/assets/generated/icons/icon-{size}.png");
        let png = fs::read(&png_path)?;
        let short_hash = url_hash(&png);
        icon_hashes.insert(size, short_hash);
    }

    let mut svg_hashes = HashMap::new();
    svg_hashes.insert("favicon".to_string(), favicon_short_hash);

    Ok(AssetHashes {
        styles_hash,
        styles_integrity,
        icon_hashes,
        svg_hashes,
    })
}

fn generate_manifest(
    generated_dir: &Path,
    svg_hashes: &HashMap<String, String>,
    icon_hashes: &HashMap<u32, String>,
) -> eyre::Result<String> {
    let mut content = include_frontend_asset!("manifest.tmpl.json").to_string();
    for (asset, hash) in svg_hashes {
        content = content.replace(&format!("{{ {asset} }}"), &format!("./{asset}.{hash}.svg"));
    }
    for (size, hash) in icon_hashes {
        content = content.replace(
            &format!("{{ icon_{size} }}"),
            &format!("./icons/icon-{size}.{hash}.png"),
        );
    }
    content = content
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{ repository }", env!("CARGO_PKG_REPOSITORY"))
        .replace("{ version }", VERSION);
    fs::write(generated_dir.join("manifest.json"), &content)?;
    Ok(url_hash(content.as_bytes()))
}

fn set_cargo_env_vars(
    styles_hash: &str,
    manifest_hash: &str,
    icon_hashes: &HashMap<u32, String>,
    svg_hashes: &HashMap<String, String>,
) {
    println!("cargo::rustc-env=ASSET_HASH_STYLES_CSS={styles_hash}");
    println!("cargo::rustc-env=ASSET_HASH_MANIFEST_JSON={manifest_hash}");
    let sizes: [u32; _] = [32, 48, 64, 128, 180, 192, 512];
    for &size in &sizes {
        let hash = &icon_hashes[&size];
        println!("cargo::rustc-env=ASSET_HASH_ICON_{size}_PNG={hash}");
    }
    for (asset, hash) in svg_hashes {
        println!(
            "cargo::rustc-env=ASSET_HASH_{}_SVG={}",
            asset.to_uppercase(),
            hash
        );
    }
}

fn write_build_data(
    generated_dir: &Path,
    styles_hash: &str,
    styles_integrity: &str,
    manifest_hash: &str,
    icon_hashes: &HashMap<u32, String>,
    svg_hashes: &HashMap<String, String>,
) -> eyre::Result<()> {
    let icon_hashes_json: serde_json::Map<String, serde_json::Value> = icon_hashes
        .iter()
        .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.clone())))
        .collect();
    let svg_hashes_json: serde_json::Map<String, serde_json::Value> = svg_hashes
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    let build_data = serde_json::json!({
        "styles_hash": styles_hash,
        "styles_integrity": styles_integrity,
        "manifest_hash": manifest_hash,
        "icon_hashes": icon_hashes_json,
        "svg_hashes": svg_hashes_json,
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "repository": env!("CARGO_PKG_REPOSITORY"),
        "version": VERSION,
    });
    fs::write(
        generated_dir.join("build-data.json"),
        serde_json::to_string_pretty(&build_data)?,
    )?;
    Ok(())
}

fn integrity_hash(content: impl AsRef<[u8]>) -> String {
    let hash = Sha256::digest(content);
    let hash_b64 = general_purpose::STANDARD.encode(hash);
    format!("sha256-{hash_b64}")
}

/// A short hash for the purpose of cache busting
fn url_hash(content: &[u8]) -> String {
    let hash = Sha256::digest(content);
    let hash_hex = hex::encode(hash);
    hash_hex[..8].to_string()
}
