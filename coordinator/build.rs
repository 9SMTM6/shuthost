use std::{path::PathBuf, process, fs};
use eyre::{bail, eyre, ContextCompat, Ok, WrapErr};
use resvg::usvg;
use tiny_skia::Pixmap;

const RERUN_IF: &'static str = "cargo::rerun-if-changed=frontend/assets";

const FRONTEND_DIR: &'static str = "frontend";

fn main() -> eyre::Result<()> {
    set_workspace_root()?;

    setup_npm()?;

    run_npm_build()?;

    // Generate PNG icons from SVG (placed into frontend/assets/icons).
    generate_png_icons()
}

fn set_workspace_root() -> eyre::Result<()> {
    let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = workspace_dir.parent().wrap_err("expected absolute path in CARGO_MANIFEST_DIR")?;
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

    process::Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir(FRONTEND_DIR)
        .status()
        .map(|it| if it.success() {Ok(())} else {Err(eyre!("npm run build failed with {it}"))})
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
        .map(|it| if it.success() {Ok(())} else {Err(eyre!("npm ci failed with {it}"))})
        .wrap_err("Failed to npm ci")?
}

fn generate_png_icons() -> eyre::Result<()> {
    println!("{RERUN_IF}/favicon.svg");
    let out_dir = PathBuf::from("frontend/assets/icons");
    if !out_dir.exists() {
        fs::create_dir_all(&out_dir).wrap_err_with(|| format!("creating {}", out_dir.display()))?;
    }

    let svg_data = include_bytes!("frontend/assets/favicon.svg");

    let mut opt = usvg::Options::default();
    opt.resources_dir = Some(PathBuf::from("frontend/assets/"));
    let rtree = usvg::Tree::from_str(std::str::from_utf8(svg_data)?, &opt).wrap_err("parsing SVG")?;

    // sizes to emit: favicons, apple-touch, and PWA sizes
    let sizes: &[u32] = &[32, 48, 64, 128, 180, 192, 512];

    for &size in sizes {
        let mut pixmap = Pixmap::new(size, size)
            .ok_or_else(|| eyre::eyre!("failed to allocate pixmap {}x{}", size, size))?;

        // Render the SVG into the pixmap using resvg's render
        let fit_to = tiny_skia::Transform::from_scale(size as f32, size as f32);
        resvg::render(
            &rtree,
            fit_to,
            &mut pixmap.as_mut(),
        );
        let out_png = out_dir.join(format!("icon-{}.png", size));
        pixmap
            .save_png(out_png.as_path())
            .wrap_err(format!("saving {}", out_png.display()))?;
    }

    Ok(())
}
