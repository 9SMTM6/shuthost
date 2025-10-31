use std::{fs, path::PathBuf, process};

use base64::{Engine as _, engine::general_purpose};
use eyre::{ContextCompat, Ok, WrapErr, bail, eyre};
use regex::Regex;
use resvg::usvg;
use sha2::{Digest, Sha256};
use tiny_skia::Pixmap;

const RERUN_IF: &str = "cargo::rerun-if-changed=frontend/assets";

const FRONTEND_DIR: &str = "frontend";

fn main() -> eyre::Result<()> {
    set_workspace_root()?;

    setup_npm()?;

    run_npm_build()?;

    // Generate PNG icons from SVG (placed into frontend/assets/icons).
    generate_png_icons()?;

    // Process HTML templates.
    process_templates()?;

    // Generate hashes for all inline scripts in templates.
    generate_inline_script_hashes()
}

fn set_workspace_root() -> eyre::Result<()> {
    let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = workspace_dir
        .parent()
        .wrap_err("expected absolute path in CARGO_MANIFEST_DIR")?;
    println!(
        "cargo:rustc-env=WORKSPACE_ROOT={}/",
        workspace_dir.display()
    );
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

    process::Command::new("npm")
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
    if process::Command::new("npm")
        .arg("--version")
        .output()
        .is_err()
    {
        bail!("npm is not installed. Please install node/npm.");
    }

    process::Command::new("npm")
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
    let out_dir = PathBuf::from("frontend/assets/generated/icons");
    if !out_dir.exists() {
        fs::create_dir_all(&out_dir).wrap_err_with(|| format!("creating {}", out_dir.display()))?;
    }

    let svg_data = include_bytes!("frontend/assets/favicon.svg");

    let opt = usvg::Options {
        resources_dir: Some(PathBuf::from("frontend/assets/")),
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
        "frontend/assets/generated/index.html",
        "frontend/assets/generated/login.html",
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
    println!("cargo:rustc-env=INLINE_SCRIPT_HASHES={}", hashes_str);
    Ok(())
}

fn process_templates() -> eyre::Result<()> {
    let generated_dir = PathBuf::from("frontend/assets/generated");
    fs::create_dir_all(&generated_dir)?;

    // Read generated app.js
    let app_js = fs::read_to_string("frontend/assets/generated/app.js")?;

    // Process index.tmpl.html
    let content = include_str!("frontend/assets/index.tmpl.html")
        .replace(
            "{ html_head }",
            include_str!("frontend/assets/partials/html_head.tmpl.html"),
        )
        .replace("{ title }", "ShutHost Coordinator")
        .replace(
            "{ architecture_documentation }",
            include_str!("frontend/assets/partials/architecture.html"),
        )
        .replace(
            "{ client_install_requirements_gotchas }",
            include_str!("frontend/assets/client_install_requirements_gotchas.md"),
        )
        .replace(
            "{ agent_install_requirements_gotchas }",
            include_str!("frontend/assets/agent_install_requirements_gotchas.md"),
        )
        .replace(
            "{ header }",
            include_str!("frontend/assets/partials/header.tmpl.html"),
        )
        .replace(
            "{ footer }",
            include_str!("frontend/assets/partials/footer.tmpl.html"),
        )
        .replace("{ js }", &app_js)
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{ version }", env!("CARGO_PKG_VERSION"));
    fs::write(generated_dir.join("index.html"), content)?;

    // Process login.tmpl.html
    let login_content = include_str!("frontend/assets/login.tmpl.html")
        .replace(
            "{ html_head }",
            include_str!("frontend/assets/partials/html_head.tmpl.html"),
        )
        .replace("{ title }", "Login • ShutHost")
        .replace(
            "{ header }",
            include_str!("frontend/assets/partials/header.tmpl.html"),
        )
        .replace(
            "{ footer }",
            include_str!("frontend/assets/partials/footer.tmpl.html"),
        )
        .replace("{ maybe_logout }", "")
        .replace("{ maybe_demo_disclaimer }", "")
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
        .replace("{ version }", env!("CARGO_PKG_VERSION"));
    fs::write(generated_dir.join("login.html"), login_content)?;

    // Process manifest.tmpl.json
    let manifest_content = include_str!("frontend/assets/manifest.tmpl.json")
        .replace("{ description }", env!("CARGO_PKG_DESCRIPTION"));
    fs::write(generated_dir.join("manifest.json"), manifest_content)?;

    Ok(())
}
