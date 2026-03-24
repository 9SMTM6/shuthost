use base64::{Engine as _, engine::general_purpose};
use eyre::WrapErr as _;
use sha2::{Digest as _, Sha256};
use shuthost_common::VERSION;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::icons::ICON_SIZES;

macro_rules! include_frontend_asset {
    ($path:expr) => {
        include_str!(concat!("../../frontend/assets/", $path))
    };
}

/// Writes a preliminary `build-data.json` before the npm build so that
/// `vite.config.ts` can load it (it reads `repository` at config time).
/// Icons must already be generated before calling this.
/// The full data (including CSS and JS hashes) is written by [`compute_hashes`].
pub fn write_pre_build_data() -> eyre::Result<()> {
    let generated_dir = PathBuf::from("../frontend/assets/generated");
    fs::create_dir_all(&generated_dir)?;

    let (icon_hashes, svg_hashes) = compute_icon_hashes()?;
    let manifest_hash = generate_manifest(&generated_dir, &svg_hashes, &icon_hashes)?;
    write_build_data(
        &generated_dir,
        &BuildData {
            styles_hash: "",
            styles_integrity: "",
            manifest_hash: &manifest_hash,
            icon_hashes: &icon_hashes,
            svg_hashes: &svg_hashes,
        },
    )
}

pub fn compute_hashes() -> eyre::Result<()> {
    let generated_dir = PathBuf::from("../frontend/assets/generated");
    fs::create_dir_all(&generated_dir)?;

    let styles_css = fs::read_to_string("../frontend/assets/generated/app.css")
        .wrap_err("Failed to read generated app.css")?;
    let styles_hash = url_hash(styles_css.as_bytes());
    let styles_integrity = integrity_hash(&styles_css);

    let app_js = fs::read("../frontend/assets/generated/app.js")
        .wrap_err("Failed to read generated app.js")?;
    let app_js_csp_hash = integrity_hash(&app_js);

    let (icon_hashes, svg_hashes) = compute_icon_hashes()?;
    let manifest_hash = generate_manifest(&generated_dir, &svg_hashes, &icon_hashes)?;
    set_cargo_env_vars(
        &styles_hash,
        &app_js_csp_hash,
        &manifest_hash,
        &icon_hashes,
        &svg_hashes,
    );
    write_build_data(
        &generated_dir,
        &BuildData {
            styles_hash: &styles_hash,
            styles_integrity: &styles_integrity,
            manifest_hash: &manifest_hash,
            icon_hashes: &icon_hashes,
            svg_hashes: &svg_hashes,
        },
    )
}

fn compute_icon_hashes() -> eyre::Result<(HashMap<u32, String>, HashMap<String, String>)> {
    let favicon_hash = url_hash(include_frontend_asset!("favicon.svg").as_bytes());
    let mut svg_hashes = HashMap::new();
    svg_hashes.insert("favicon".to_string(), favicon_hash);

    let mut icon_hashes = HashMap::new();
    for &size in &ICON_SIZES {
        let png = fs::read(format!(
            "../frontend/assets/generated/icons/icon-{size}.png"
        ))?;
        icon_hashes.insert(size, url_hash(&png));
    }

    Ok((icon_hashes, svg_hashes))
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
    app_js_csp_hash: &str,
    manifest_hash: &str,
    icon_hashes: &HashMap<u32, String>,
    svg_hashes: &HashMap<String, String>,
) {
    println!("cargo::rustc-env=ASSET_HASH_STYLES_CSS={styles_hash}");
    println!("cargo::rustc-env=CSP_APP_JS_HASH={app_js_csp_hash}");
    println!("cargo::rustc-env=ASSET_HASH_MANIFEST_JSON={manifest_hash}");
    for &size in &ICON_SIZES {
        println!(
            "cargo::rustc-env=ASSET_HASH_ICON_{size}_PNG={}",
            icon_hashes[&size]
        );
    }
    for (asset, hash) in svg_hashes {
        println!(
            "cargo::rustc-env=ASSET_HASH_{}_SVG={}",
            asset.to_uppercase(),
            hash
        );
    }
}

struct BuildData<'all> {
    styles_hash: &'all str,
    styles_integrity: &'all str,
    manifest_hash: &'all str,
    icon_hashes: &'all HashMap<u32, String>,
    svg_hashes: &'all HashMap<String, String>,
}

fn write_build_data(generated_dir: &Path, data: &BuildData<'_>) -> eyre::Result<()> {
    let icon_hashes_json: serde_json::Map<String, serde_json::Value> = data
        .icon_hashes
        .iter()
        .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.clone())))
        .collect();
    let svg_hashes_json: serde_json::Map<String, serde_json::Value> = data
        .svg_hashes
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    let build_data = serde_json::json!({
        "styles_hash": data.styles_hash,
        "styles_integrity": data.styles_integrity,
        "manifest_hash": data.manifest_hash,
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
    let hash_hex = hex::encode(Sha256::digest(content));
    hash_hex[..8].to_string()
}
