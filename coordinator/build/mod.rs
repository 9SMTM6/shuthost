//! Build script for the `ShutHost` Coordinator.
//!
//! This build script performs several tasks to prepare the frontend assets and generate necessary files
//! for the Rust application. It is responsible for:
//!
//! - Setting the workspace root environment variable.
//! - Installing and running pnpm to build the frontend assets.
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
#![expect(
    clippy::indexing_slicing,
    reason = "panicking during build becomes a compilation error"
)]
extern crate alloc;
extern crate core;

mod about;
mod assets;
mod icons;
mod pnpm;
mod tasks;
mod warnings;
mod workspace;

use std::env;

use eyre::Ok;

fn main() -> eyre::Result<()> {
    workspace::set_root()?;

    // Enable frontend debug mode when building the coordinator in debug profile, or when the
    // SHUTHOST_FRONTEND_DEBUG env var is set at compile time. `option_env!` makes Cargo
    // automatically re-run this build script if the variable changes.
    let frontend_debug = env::var("PROFILE")
        .as_deref()
        .is_ok_and(|val| val == "debug")
        || option_env!("SHUTHOST_FRONTEND_DEBUG").is_some();
    if frontend_debug {
        // SAFETY: no threads have been spawned yet; this is a single write before tasks::spawn.
        unsafe {
            env::set_var("SHUTHOST_DEBUG_BUILD", "1");
        }
    }

    cfg!(debug_assertions);

    pnpm::setup()?;

    const ON_CHANGE: &str = "cargo::rerun-if-changed=frontend";

    // Spawn typecheck in parallel — it produces no output files so it is
    // independent of the other build steps.
    println!("{ON_CHANGE}/package.json");
    println!("{ON_CHANGE}/src/index.tsx");
    println!("{ON_CHANGE}/src/sharedComponents");
    println!("{ON_CHANGE}/src/pages");
    println!("{ON_CHANGE}/src/helpers");
    println!("{ON_CHANGE}/src/htmlPartials");
    println!("{ON_CHANGE}/src/page.template.html");
    println!("{ON_CHANGE}/vite.config.ts");
    println!("{ON_CHANGE}/tsconfig.json");
    let typecheck = tasks::spawn("typecheck", || pnpm::run("typecheck"));

    println!("cargo::rerun-if-changed=deny.toml");
    println!("{ON_CHANGE}/pnpm-lock.yaml");
    let about_json = tasks::spawn("build-about-json", about::build_json);

    let warnings = tasks::spawn("warnings", warnings::emit);

    println!("{ON_CHANGE}/src/client_controller_interaction.d2");
    println!("{ON_CHANGE}/src/deployment.d2");
    println!("{ON_CHANGE}/src/direct_control_comparison.d2");
    println!("{ON_CHANGE}/src/host_agent_interaction.d2");
    println!("{ON_CHANGE}/build-diagrams.ts");
    let build_diagrams = tasks::spawn("build-diagrams", || pnpm::run("build:diagrams"));

    println!("{ON_CHANGE}/src/prerender.tsx");
    println!("{ON_CHANGE}/src/vite.config.ssr.ts");
    let prerender = tasks::spawn("build-prerender", || pnpm::run("build:prerender"));

    println!("{ON_CHANGE}/src/generated/favicon.svg");
    let pngs = tasks::spawn("generate-png-icons", icons::generate_pngs);

    // Icons and the manifest/build-data.json must be ready before the pnpm build
    // because vite.config.ts reads build-data.json at config-load time.
    println!("{ON_CHANGE}/src/manifest.tmpl.json");
    println!("{ON_CHANGE}/src/styles.tailwind.css");
    println!("{ON_CHANGE}/src/htmlPartials/client_install_requirements_gotchas.md");
    println!("{ON_CHANGE}/src/htmlPartials/agent_install_requirements_gotchas.md");
    let main_frontend_assets = tasks::spawn("build-frontend", move || {
        tasks::join(build_diagrams)?;
        pnpm::run("build")?;
        tasks::join(prerender)?;
        tasks::join(pngs)?;
        assets::generate_frontend_assets()
    });

    tasks::join(about_json)?;
    tasks::join(main_frontend_assets)?;

    // Block until the parallel typecheck finishes, surfacing any type errors.
    tasks::join(typecheck)?;

    tasks::join(warnings)?;

    Ok(())
}
