#![expect(clippy::indexing_slicing, reason = "This is fine at build time")]
use base64::{Engine as _, engine::general_purpose};
use eyre::{Ok, WrapErr};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, path::PathBuf};

macro_rules! include_frontend_asset {
    ($path:expr) => {
        include_str!(concat!("../../frontend/assets/", $path))
    };
}

pub fn process() -> eyre::Result<()> {
    fn short_hash(content: &[u8]) -> String {
        let hash = Sha256::digest(content);
        let hash_hex = hex::encode(hash);
        hash_hex[..8].to_string()
    }

    let generated_dir = PathBuf::from("../frontend/assets/generated");
    fs::create_dir_all(&generated_dir)?;

    let styles_css = fs::read_to_string("../frontend/assets/generated/styles.css")
        .wrap_err("Failed to read generated styles.css")?;
    let styles_short_hash = short_hash(styles_css.as_bytes());
    let styles_integrity = generate_encoded_hash(&styles_css)?;

    let favicon_short_hash = short_hash(include_frontend_asset!("favicon.svg").as_bytes());

    let sizes = [32, 48, 64, 128, 180, 192, 512];
    let mut icon_hashes = HashMap::new();
    for &size in &sizes {
        let png_path = format!("../frontend/assets/generated/icons/icon-{size}.png");
        let png = fs::read(&png_path)?;
        let short_hash = short_hash(&png);
        icon_hashes.insert(size, short_hash);
    }

    let arch_simplified_svg =
        fs::read_to_string("../frontend/assets/generated/architecture_simplified.svg")
            .wrap_err("Failed to read generated architecture_simplified.svg")?;
    let arch_simplified_short_hash = short_hash(arch_simplified_svg.as_bytes());

    let arch_complete_svg = fs::read_to_string("../frontend/assets/generated/architecture.svg")
        .wrap_err("Failed to read generated architecture.svg")?;
    let arch_complete_short_hash = short_hash(arch_complete_svg.as_bytes());

    println!(
        "cargo::rustc-env=ASSET_HASH_STYLES_CSS={}",
        styles_short_hash
    );
    println!(
        "cargo::rustc-env=ASSET_HASH_FAVICON_SVG={}",
        favicon_short_hash
    );
    for &size in &sizes {
        let hash = &icon_hashes[&size];
        println!("cargo::rustc-env=ASSET_HASH_ICON_{}_PNG={}", size, hash);
    }
    println!(
        "cargo::rustc-env=ASSET_HASH_ARCHITECTURE_SIMPLIFIED_SVG={}",
        arch_simplified_short_hash
    );
    println!(
        "cargo::rustc-env=ASSET_HASH_ARCHITECTURE_SVG={}",
        arch_complete_short_hash
    );

    // Process manifest.tmpl.json
    let manifest_content = include_frontend_asset!("manifest.tmpl.json")
        .replace(
            "{ icon_192_src }",
            &format!("./icons/icon-192.{}.png", icon_hashes[&192]),
        )
        .replace(
            "{ icon_512_src }",
            &format!("./icons/icon-512.{}.png", icon_hashes[&512]),
        )
        .replace(
            "{ favicon_src }",
            &format!("./favicon.{}.svg", favicon_short_hash),
        )
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"));
    let manifest_short_hash = short_hash(manifest_content.as_bytes());
    fs::write(generated_dir.join("manifest.json"), &manifest_content)?;
    println!(
        "cargo::rustc-env=ASSET_HASH_MANIFEST_JSON={}",
        manifest_short_hash
    );

    let html_head_template = include_frontend_asset!("partials/html_head.tmpl.html");
    let html_head = html_head_template
        .replace(
            "{ manifest_href }",
            &format!("./manifest.{}.json", manifest_short_hash),
        )
        .replace(
            "{ favicon_href }",
            &format!("./favicon.{}.svg", favicon_short_hash),
        )
        .replace(
            "{ styles_href }",
            &format!("./styles.{}.css", styles_short_hash),
        )
        .replace("{ styles_integrity }", &styles_integrity)
        .replace(
            "{ icon_32_href }",
            &format!("./icons/icon-32.{}.png", icon_hashes[&32]),
        )
        .replace(
            "{ icon_48_href }",
            &format!("./icons/icon-48.{}.png", icon_hashes[&48]),
        )
        .replace(
            "{ icon_64_href }",
            &format!("./icons/icon-64.{}.png", icon_hashes[&64]),
        )
        .replace(
            "{ icon_128_href }",
            &format!("./icons/icon-128.{}.png", icon_hashes[&128]),
        )
        .replace(
            "{ icon_180_href }",
            &format!("./icons/icon-180.{}.png", icon_hashes[&180]),
        );

    // Process index.tmpl.html
    let content = include_frontend_asset!("index.tmpl.html")
        .replace("{ html_head }", &html_head)
        .replace(
            "{ js_warnings }",
            include_frontend_asset!("partials/js_warnings.tmpl.html"),
        )
        .replace("{ title }", "ShutHost Coordinator")
        .replace(
            "{ architecture_documentation }",
            include_frontend_asset!("partials/architecture.html"),
        )
        .replace(
            "{ platform_support }",
            include_frontend_asset!("partials/platform_support.md"),
        )
        .replace(
            "{ client_install_requirements_gotchas }",
            include_frontend_asset!("client_install_requirements_gotchas.md"),
        )
        .replace(
            "{ agent_install_requirements_gotchas }",
            include_frontend_asset!("agent_install_requirements_gotchas.md"),
        )
        .replace(
            "{ header }",
            include_frontend_asset!("partials/header.tmpl.html"),
        )
        .replace(
            "{ footer }",
            include_frontend_asset!("partials/footer.tmpl.html"),
        )
        .replace(
            "{ favicon_src }",
            &format!("./favicon.{}.svg", favicon_short_hash),
        )
        .replace(
            "{ architecture_simplified_src }",
            &format!(
                "./architecture_simplified.{}.svg",
                arch_simplified_short_hash
            ),
        )
        .replace(
            "{ architecture_src }",
            &format!("./architecture.{}.svg", arch_complete_short_hash),
        )
        .replace(
            "{ js }",
            &fs::read_to_string("../frontend/assets/generated/app.js")
                .wrap_err("Failed to read generated app.js")?,
        )
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{ repository }", env!("CARGO_PKG_REPOSITORY"))
        .replace("{ version }", env!("CARGO_PKG_VERSION"));
    fs::write(generated_dir.join("index.html"), content)?;

    // Process login.tmpl.html
    let login_content = include_frontend_asset!("login.tmpl.html")
        .replace("{ html_head }", &html_head)
        .replace(
            "{ js_warnings }",
            include_frontend_asset!("partials/js_warnings.tmpl.html"),
        )
        .replace("{ title }", "Login • ShutHost")
        .replace(
            "{ header }",
            include_frontend_asset!("partials/header.tmpl.html"),
        )
        .replace(
            "{ footer }",
            include_frontend_asset!("partials/footer.tmpl.html"),
        )
        .replace("{ maybe_logout }", "")
        .replace("{ maybe_demo_disclaimer }", "")
        .replace(
            "{ favicon_src }",
            &format!("./favicon.{}.svg", favicon_short_hash),
        )
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{ repository }", env!("CARGO_PKG_REPOSITORY"))
        .replace("{ version }", env!("CARGO_PKG_VERSION"));
    fs::write(generated_dir.join("login.html"), login_content)?;

    // Process about.tmpl.html
    let about_content = fs::read_to_string(generated_dir.join("about.tmpl.html"))?
        .replace("{ html_head }", &html_head)
        .replace("{ title }", "Dependencies and Licenses")
        .replace(
            "{ header }",
            include_frontend_asset!("partials/header.tmpl.html"),
        )
        .replace(
            "{ footer }",
            include_frontend_asset!("partials/footer.tmpl.html"),
        )
        .replace("{ maybe_logout }", "")
        .replace("{ maybe_demo_disclaimer }", "")
        .replace(
            "{ favicon_src }",
            &format!("./favicon.{}.svg", favicon_short_hash),
        )
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{ repository }", env!("CARGO_PKG_REPOSITORY"))
        .replace("{ version }", env!("CARGO_PKG_VERSION"));
    fs::write(generated_dir.join("about.html"), about_content)?;

    Ok(())
}

fn generate_encoded_hash(content: impl AsRef<[u8]>) -> eyre::Result<String> {
    let hash = Sha256::digest(content);
    let hash_b64 = general_purpose::STANDARD.encode(hash);
    Ok(format!("sha256-{}", hash_b64))
}
