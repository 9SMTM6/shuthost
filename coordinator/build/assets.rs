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

pub fn compute_hashes() -> eyre::Result<()> {
    let generated_dir = PathBuf::from("../frontend/assets/generated");
    fs::create_dir_all(&generated_dir)?;

    let styles_css = fs::read_to_string("../frontend/assets/generated/app.css")
        .wrap_err("Failed to read generated app.css")?;
    let styles_hash = url_hash(styles_css.as_bytes());
    let styles_integrity = integrity_hash(&styles_css);

    let app_js = fs::read("../frontend/assets/generated/app.js")
        .wrap_err("Failed to read generated app.js")?;
    let app_js_url_hash = url_hash(&app_js);
    let app_js_integrity = integrity_hash(&app_js);

    let (icon_hashes, svg_hashes) = compute_icon_hashes()?;
    let manifest_hash = generate_manifest(&generated_dir, &svg_hashes, &icon_hashes)?;
    set_cargo_env_vars(
        &styles_hash,
        &app_js_url_hash,
        &app_js_integrity,
        &manifest_hash,
        &icon_hashes,
        &svg_hashes,
    );
    let build_data = BuildData {
        styles_hash: &styles_hash,
        styles_integrity: &styles_integrity,
        manifest_hash: &manifest_hash,
        icon_hashes: &icon_hashes,
        svg_hashes: &svg_hashes,
        app_js_hash: &app_js_url_hash,
        app_js_integrity: &app_js_integrity,
        description: env!("CARGO_PKG_DESCRIPTION"),
        repository: env!("CARGO_PKG_REPOSITORY"),
        version: VERSION,
    };
    generate_index_html(&generated_dir, &build_data)
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
    app_js_url_hash: &str,
    app_js_integrity: &str,
    manifest_hash: &str,
    icon_hashes: &HashMap<u32, String>,
    svg_hashes: &HashMap<String, String>,
) {
    println!("cargo::rustc-env=ASSET_HASH_STYLES_CSS={styles_hash}");
    println!("cargo::rustc-env=ASSET_HASH_APP_JS={app_js_url_hash}");
    println!("cargo::rustc-env=CSP_APP_JS_HASH='{app_js_integrity}'");
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

#[derive(serde::Serialize)]
struct BuildData<'all> {
    styles_hash: &'all str,
    styles_integrity: &'all str,
    manifest_hash: &'all str,
    icon_hashes: &'all HashMap<u32, String>,
    svg_hashes: &'all HashMap<String, String>,
    app_js_hash: &'all str,
    app_js_integrity: &'all str,
    description: &'static str,
    repository: &'static str,
    version: &'static str,
}

fn generate_index_html(generated_dir: &Path, data: &BuildData<'_>) -> eyre::Result<()> {
    let template = fs::read_to_string("../frontend/assets/page.template.html")
        .wrap_err("Failed to read page.template.html")?;

    // Embed build-data JSON safely inside a <script type="application/json"> tag.
    // serde_json::to_string doesn't escape </ by default, so we must prevent accidental
    // script tag termination.
    let build_data_json = serde_json::to_string(data)
        .expect("build data serialization should not fail")
        .replace("</", r"<\/");

    let html = template
        .replace("{{STYLES_HASH}}", data.styles_hash)
        .replace("{{STYLES_INTEGRITY}}", data.styles_integrity)
        .replace("{{MANIFEST_HASH}}", data.manifest_hash)
        .replace("{{ICON_HASH_32}}", &data.icon_hashes[&32])
        .replace("{{ICON_HASH_48}}", &data.icon_hashes[&48])
        .replace("{{ICON_HASH_64}}", &data.icon_hashes[&64])
        .replace("{{ICON_HASH_128}}", &data.icon_hashes[&128])
        .replace("{{ICON_HASH_180}}", &data.icon_hashes[&180])
        .replace("{{FAVICON_SVG_HASH}}", &data.svg_hashes["favicon"])
        .replace("{{DESCRIPTION}}", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{{BUILD_DATA_JSON}}", &build_data_json)
        .replace("{{APP_JS_HASH}}", data.app_js_hash)
        .replace("{{APP_JS_INTEGRITY}}", data.app_js_integrity);

    fs::write(generated_dir.join("index.html"), html)?;
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
