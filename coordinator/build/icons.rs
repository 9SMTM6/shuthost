use eyre::{Ok, WrapErr, eyre};
use resvg::usvg;
use std::{fs, path::PathBuf};
use tiny_skia::Pixmap;

pub fn generate_pngs() -> eyre::Result<()> {
    let out_dir = PathBuf::from("../frontend/assets/generated/icons");
    if !out_dir.exists() {
        fs::create_dir_all(&out_dir).wrap_err_with(|| format!("creating {}", out_dir.display()))?;
    }

    let svg_data = include_bytes!("../../frontend/assets/favicon.svg");

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
