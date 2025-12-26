//! Build script for the ShutHost Coordinator.
//!
//! This build script performs several tasks to prepare the frontend assets and generate necessary files
//! for the Rust application. It is responsible for:
//!
//! - Setting the workspace root environment variable.
//! - Installing and running npm to build the frontend assets.
//! - Generating PNG icons from the SVG favicon at various sizes.
//! - Processing HTML templates by replacing placeholders with hashed asset paths and content.
//! - Generating SHA256 hashes for inline scripts in served HTML files for Content Security Policy (CSP).
//! - Emitting build warnings based on configuration (e.g., Windows builds, missing agent features).
//!
//! Note: This script assumes a specific directory structure with a symlinked or adjacent `frontend` directory.
//! On Windows, some rerun-if-changed directives may not work correctly due to symlinks, but this is documented.
//!
//! # Environment Variables Set
//!
//! - `WORKSPACE_ROOT`: The root path of the workspace.
//! - `BUILD_WARNINGS`: Semicolon-separated list of build warnings.
//! - `INLINE_SCRIPT_HASHES`: Space-separated list of SHA256 hashes for inline scripts.
//! - Various `ASSET_HASH_*` variables for hashed asset paths.
//!
//! # Rerun Conditions
//!
//! The script informs Cargo to rerun the build if certain frontend asset files change, ensuring
//! that modifications to templates, styles, or icons trigger a rebuild.
mod about;
mod csp;
mod icons;
mod npm;
mod templates;
mod warnings;
mod workspace;

use eyre::Ok;

fn main() -> eyre::Result<()> {
    workspace::set_root()?;

    npm::setup()?;

    println!("cargo::rerun-if-changed=frontend/assets/styles.tailwind.css");
    println!("cargo::rerun-if-changed=frontend/assets/app.ts");
    println!("cargo::rerun-if-changed=frontend/assets/index.tmpl.html");
    println!("cargo::rerun-if-changed=frontend/assets/login.tmpl.html");
    println!("cargo::rerun-if-changed=frontend/assets/partials");
    println!("cargo::rerun-if-changed=frontend/assets/about.hbs");
    npm::run("build")?;

    println!("cargo::rerun-if-changed=frontend/package.json");
    println!("cargo::rerun-if-changed=frontend/package-lock.json");
    npm::run("generate-npm-licenses")?;
    about::build_html()?;

    println!("cargo::rerun-if-changed=frontend/assets/favicon.svg");
    icons::generate_pngs()?;

    println!("cargo::rerun-if-changed=frontend/assets/manifest.tmpl.json");
    println!("cargo::rerun-if-changed=frontend/assets/client_install_requirements_gotchas.md");
    println!("cargo::rerun-if-changed=frontend/assets/agent_install_requirements_gotchas.md");
    templates::process()?;

    csp::generate_hashes()?;

    warnings::emit();

    Ok(())
}
