//! Build script for the `ShutHost` Coordinator.
//!
//! This build script performs several tasks to prepare the frontend assets and generate necessary files
//! for the Rust application. It is responsible for:
//!
//! - Setting the workspace root environment variable.
//! - Installing and running npm to build the frontend assets.
//! - Generating PNG icons from the SVG favicon at various sizes.
//! - Hashing assets and writing `build-data.json` consumed by the TypeScript build.
//! - Emitting build warnings based on configuration (e.g., Windows builds, missing agent features).
//!
//! Note: This script assumes a specific directory structure with a symlinked or adjacent `frontend` directory.
//! On Windows, some rerun-if-changed directives may not work correctly due to symlinks, but this is documented.
//!
//! # Environment Variables Set
//!
//! - `WORKSPACE_ROOT`: The root path of the workspace.
//! - `BUILD_WARNINGS`: Semicolon-separated list of build warnings.
//! - Various `ASSET_HASH_*` variables for hashed asset paths.
//!
//! # Rerun Conditions
//!
//! The script informs Cargo to rerun the build if certain frontend asset files change, ensuring
//! that modifications to styles, icons, or the JS bundle trigger a rebuild.
extern crate alloc;
extern crate core;

mod about;
mod assets;
mod icons;
mod npm;
mod tasks;
mod warnings;
mod workspace;

use eyre::Ok;

fn main() -> eyre::Result<()> {
    workspace::set_root()?;

    npm::setup()?;

    const ON_ASSET_CHANGE: &str = "cargo::rerun-if-changed=frontend/assets";

    println!("cargo::rerun-if-changed=frontend/package.json");
    println!("{ON_ASSET_CHANGE}/app.tsx");
    println!("{ON_ASSET_CHANGE}/index.tsx");
    println!("{ON_ASSET_CHANGE}/components");
    println!("{ON_ASSET_CHANGE}/pages");
    println!("{ON_ASSET_CHANGE}/stores");
    println!("{ON_ASSET_CHANGE}/styles.tailwind.css");
    println!("{ON_ASSET_CHANGE}/partials");
    println!("cargo::rerun-if-changed=frontend/generate-pages.tsx");
    println!("cargo::rerun-if-changed=frontend/build-common.ts");

    // Spawn typecheck in parallel — it produces no output files so it is
    // independent of the other build steps.
    let typecheck = tasks::spawn("typecheck", || npm::run("typecheck"));

    println!("cargo::rerun-if-changed=deny.toml");
    println!("cargo::rerun-if-changed=frontend/package-lock.json");
    let about_json = tasks::spawn("build-about-json", || {
        npm::run("generate-npm-licenses")?;
        about::build_json()
    });

    // Icons and the manifest/build-data.json must be ready before the npm build
    // because vite.config.ts reads build-data.json at config-load time.
    println!("{ON_ASSET_CHANGE}/favicon.svg");
    println!("{ON_ASSET_CHANGE}/manifest.tmpl.json");
    println!("{ON_ASSET_CHANGE}/partials/client_install_requirements_gotchas.md");
    println!("{ON_ASSET_CHANGE}/partials/agent_install_requirements_gotchas.md");
    let main_frontend_assets = tasks::spawn("build-frontend", || {
        icons::generate_pngs()?;
        assets::write_pre_build_data()?;
        npm::run("build")?;
        assets::compute_hashes()
    });

    tasks::join(about_json)?;
    tasks::join(main_frontend_assets)?;

    npm::run("generate-pages")?;

    // Block until the parallel typecheck finishes, surfacing any type errors.
    tasks::join(typecheck)?;

    warnings::emit();

    Ok(())
}
