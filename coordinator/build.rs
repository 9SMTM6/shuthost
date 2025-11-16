#![expect(clippy::indexing_slicing, reason = "This is fine at build time")]
use std::{collections::HashMap, fs, path::PathBuf, process};

use base64::{Engine as _, engine::general_purpose};
use eyre::{ContextCompat, Ok, WrapErr, bail, eyre};
use regex::Regex;
use resvg::usvg;
use sha2::{Digest, Sha256};
use tiny_skia::Pixmap;

macro_rules! include_frontend_asset {
    ($path:expr) => {
        include_str!(concat!("../frontend/assets/", $path))
    };
}

const RERUN_IF: &str = "cargo::rerun-if-changed=frontend/assets";

// Note:
// 1. cargo::rerun-if-changed= only works if its in a project subdirectory, so use the symlinked frontend directory (./frontend/assets) for that
// 2. But to not break the build on windows (needed for client testing) please only use non-symlinked paths to assets (../frontend/assets).
// 3. This means the frontend isn't correctly rebuilt on Windows, but that's a documented issue.

const FRONTEND_DIR: &str = "../frontend";

#[cfg(not(target_os = "windows"))]
const NPM_BIN: &str = "npm";
#[cfg(target_os = "windows")]
const NPM_BIN: &str = "npm.cmd";

fn emit_build_warnings() {
    #[allow(
        clippy::allow_attributes,
        reason = "This seems cleanest way to do this."
    )]
    #[allow(
        unused_mut,
        reason = "This will receive false positives when no build warning is emitted."
    )]
    let mut build_warnings = Vec::<&'static str>::new();

    #[cfg(target_os = "windows")]
    {
        let warning = "Windows builds are currently only supported for internal testing purposes and should not be used in production.";
        build_warnings.push(warning);
        println!("cargo::warning={warning}");
    }

    #[cfg(not(feature = "include_linux_agents"))]
    {
        let warning = "No linux agents embedded. Trying to install any linux agents from the coordinator will result in errors";
        build_warnings.push(warning);
        println!("cargo::warning={warning}");
    }

    #[cfg(not(feature = "include_macos_agents"))]
    {
        let warning = "No MacOS agents embedded. Trying to install any MacOS agents from the coordinator will result in errors";
        build_warnings.push(warning);
        println!("cargo::warning={warning}");
    }

    println!(
        "cargo::rustc-env=BUILD_WARNINGS={build_warnings}",
        build_warnings = build_warnings.join(";")
    );
}

fn main() -> eyre::Result<()> {
    set_workspace_root()?;

    setup_npm()?;

    run_npm_build()?;

    // Generate PNG icons from SVG (placed into frontend/assets/icons).
    generate_png_icons()?;

    // Process HTML templates.
    process_templates()?;

    // Generate hashes for all inline scripts in templates.
    generate_inline_script_hashes()?;

    emit_build_warnings();

    Ok(())
}

fn set_workspace_root() -> eyre::Result<()> {
    let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = workspace_dir
        .parent()
        .wrap_err("expected absolute path in CARGO_MANIFEST_DIR")?;
    let mut path_str = workspace_dir.to_string_lossy().to_string();
    if cfg!(target_os = "windows") {
        path_str = path_str.replace('/', "\\");
        path_str.push('\\');
    } else {
        path_str.push('/');
    }
    println!("cargo::rustc-env=WORKSPACE_ROOT={}", path_str);
    Ok(())
}

fn run_npm_build() -> eyre::Result<()> {
    println!("{RERUN_IF}/styles.tailwind.css");
    println!("{RERUN_IF}/app.ts");
    println!("{RERUN_IF}/index.tmpl.html");
    println!("{RERUN_IF}/login.tmpl.html");
    println!("{RERUN_IF}/partials");
    println!("{RERUN_IF}/client_install_requirements_gotchas.md");
    println!("{RERUN_IF}/agent_install_requirements_gotchas.md");

    process::Command::new(NPM_BIN)
        .arg("run")
        .arg("build")
        .current_dir(FRONTEND_DIR)
        .status()
        .map(|it| {
            if it.success() {
                Ok(())
            } else {
                bail!("npm run build failed with {it}")
            }
        })
        .wrap_err("Failed to npm run build")?
}

fn setup_npm() -> eyre::Result<()> {
    // Check npm
    process::Command::new(NPM_BIN)
        .arg("--version")
        .output()
        .wrap_err("Ensure node/npm is installed")?;

    process::Command::new(NPM_BIN)
        .arg("ci")
        .current_dir(FRONTEND_DIR)
        .status()
        .map(|it| {
            if it.success() {
                Ok(())
            } else {
                bail!("npm ci failed with {it}")
            }
        })
        .wrap_err("Failed to npm ci")?
}

fn generate_png_icons() -> eyre::Result<()> {
    println!("{RERUN_IF}/favicon.svg");
    let out_dir = PathBuf::from("../frontend/assets/generated/icons");
    if !out_dir.exists() {
        fs::create_dir_all(&out_dir).wrap_err_with(|| format!("creating {}", out_dir.display()))?;
    }

    let svg_data = include_bytes!("../frontend/assets/favicon.svg");

    let opt = usvg::Options {
        resources_dir: Some(PathBuf::from("../frontend/assets/")),
        ..Default::default()
    };
    let rtree =
        usvg::Tree::from_str(std::str::from_utf8(svg_data)?, &opt).wrap_err("parsing SVG")?;

    // sizes to emit: favicons, apple-touch, and PWA sizes
    let sizes: [u32; _] = [32, 48, 64, 128, 180, 192, 512];
    let scaling_sizes = sizes.map(|it| it as f32 / 400.0);

    for (&size, scaling) in sizes.iter().zip(scaling_sizes) {
        let mut pixmap = Pixmap::new(size, size)
            .ok_or_else(|| eyre!("failed to allocate pixmap {size}x{size}"))?;

        // Render the SVG into the pixmap using resvg's render
        let fit_to = tiny_skia::Transform::from_scale(scaling, scaling);
        resvg::render(&rtree, fit_to, &mut pixmap.as_mut());
        let out_png = out_dir.join(format!("icon-{size}.png"));
        pixmap
            .save_png(out_png.as_path())
            .wrap_err(format!("saving {}", out_png.display()))?;
    }

    Ok(())
}

/// note that this will silently ignore any non module code!
fn generate_inline_script_hashes() -> eyre::Result<()> {
    let script_regex = Regex::new(r#"<script type="module"[^>]*>([\s\S]*?)<\/script>"#)?;
    let mut hashes = std::collections::HashSet::new();

    let served_html_files = [
        "../frontend/assets/generated/index.html",
        "../frontend/assets/generated/login.html",
        "../frontend/assets/partials/external_auth_config.tmpl.html",
    ];
    for file_path in served_html_files {
        let content = fs::read_to_string(file_path)?;
        for cap in script_regex.captures_iter(&content) {
            if let Some(script_content) = cap.get(1) {
                let hash = Sha256::digest(script_content.as_str().as_bytes());
                let hash_b64 = general_purpose::STANDARD.encode(hash);
                let hash_tok = format!("'sha256-{}'", hash_b64);
                hashes.insert(hash_tok);
            }
        }
    }

    let mut hash_list: Vec<_> = hashes.into_iter().collect();
    hash_list.sort();
    let hashes_str = hash_list.join(" ");
    println!("cargo::rustc-env=INLINE_SCRIPT_HASHES={}", hashes_str);
    Ok(())
}

fn process_templates() -> eyre::Result<()> {
    fn short_hash(content: &[u8]) -> String {
        let hash = Sha256::digest(content);
        let hash_hex = hex::encode(hash);
        hash_hex[..8].to_string()
    }

    let generated_dir = PathBuf::from("../frontend/assets/generated");
    fs::create_dir_all(&generated_dir)?;

    let styles_css = include_frontend_asset!("generated/styles.css");
    let styles_short_hash = short_hash(styles_css.as_bytes());
    let styles_integrity = format!(
        "sha256-{}",
        general_purpose::STANDARD.encode(Sha256::digest(styles_css.as_bytes()))
    );

    let favicon_short_hash = short_hash(include_frontend_asset!("favicon.svg").as_bytes());

    let sizes = [32, 48, 64, 128, 180, 192, 512];
    let mut icon_hashes = HashMap::new();
    for &size in &sizes {
        let png_path = format!("../frontend/assets/generated/icons/icon-{size}.png");
        let png = fs::read(&png_path)?;
        let short_hash = short_hash(&png);
        icon_hashes.insert(size, short_hash);
    }

    let arch_simplified_short_hash =
        short_hash(include_frontend_asset!("generated/architecture_simplified.svg").as_bytes());

    let arch_complete_short_hash =
        short_hash(include_frontend_asset!("generated/architecture.svg").as_bytes());

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
        .replace("{ js }", include_frontend_asset!("generated/app.js"))
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{ version }", env!("CARGO_PKG_VERSION"));
    fs::write(generated_dir.join("index.html"), content)?;

    // Process login.tmpl.html
    let login_content = include_frontend_asset!("login.tmpl.html")
        .replace("{ html_head }", &html_head)
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
        .replace("{ version }", env!("CARGO_PKG_VERSION"));
    fs::write(generated_dir.join("login.html"), login_content)?;

    Ok(())
}
