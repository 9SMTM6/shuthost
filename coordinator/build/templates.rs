#![expect(clippy::indexing_slicing, reason = "This is fine at build time")]
use base64::{Engine as _, engine::general_purpose};
use eyre::WrapErr;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, path::PathBuf};

macro_rules! include_frontend_asset {
    ($path:expr) => {
        include_str!(concat!("../../frontend/assets/", $path))
    };
}

trait TemplateExt {
    fn include_svgs(&self, svg_hashes: &HashMap<String, String>) -> String;
    fn include_png_icons(&self, icon_hashes: &HashMap<u32, String>) -> String;
    fn insert_metadata(&self) -> String;
    fn insert_js_warnings(&self) -> String;
    fn insert_footer(&self) -> String;
    fn no_logout(&self) -> String;
    fn no_demo_differences_or_not_in_demo(&self) -> String;
    fn insert_header_tmpl(&self) -> String;
    fn insert_header_not_main_page(&self) -> String;
    fn insert_html_head(
        &self,
        title: &str,
        svg_hashes: &HashMap<String, String>,
        manifest_hash: &str,
        styles_hash: &str,
        styles_integrity: &str,
        icon_hashes: &HashMap<u32, String>,
    ) -> String;
}

impl<T: AsRef<str>> TemplateExt for T {
    fn include_svgs(&self, svg_hashes: &HashMap<String, String>) -> String {
        let mut result = self.as_ref().to_string();
        for (asset, hash) in svg_hashes.iter() {
            result = result.replace(
                &format!("{{ {} }}", asset),
                &format!("./{}.{}.svg", asset, hash),
            );
        }
        result
    }

    fn include_png_icons(&self, icon_hashes: &HashMap<u32, String>) -> String {
        let mut result = self.as_ref().to_string();
        for (size, hash) in icon_hashes.iter() {
            result = result.replace(
                &format!("{{ icon_{} }}", size),
                &format!("./icons/icon-{}.{}.png", size, hash),
            );
        }
        result
    }

    fn insert_metadata(&self) -> String {
        let s = self.as_ref();
        s.replace("{ description }", env!("CARGO_PKG_DESCRIPTION"))
            .replace("{ repository }", env!("CARGO_PKG_REPOSITORY"))
            .replace("{ version }", env!("CARGO_PKG_VERSION"))
    }

    /// Note that this doesnt provide the needed JS to show the warnings, these are only in `app.ts`, as parts of the code are used there,
    /// This means that where that isn't included all this does show the noscript warning.
    fn insert_js_warnings(&self) -> String {
        self.as_ref().replace(
            "{ js_warnings }",
            include_frontend_asset!("partials/js_warnings.html"),
        )
    }

    fn insert_footer(&self) -> String {
        self.as_ref()
            .replace(
                "{ footer }",
                include_frontend_asset!("partials/footer.tmpl.html"),
            )
            .insert_metadata()
    }

    fn no_logout(&self) -> String {
        self.as_ref().replace("{ maybe_logout }", "")
    }

    fn insert_header_tmpl(&self) -> String {
        self.as_ref()
            .replace(
                "{ header }",
                include_frontend_asset!("partials/header.tmpl.html"),
            )
            .replace("{ maybe_demo_disclaimer }", "")
    }

    fn no_demo_differences_or_not_in_demo(&self) -> String {
        self.as_ref().replace("{ maybe_demo_disclaimer }", "")
    }

    /// Sites other than the SPA main page get
    /// * no logout
    ///   * mostly since its easier like that
    /// * and no demo disclaimer
    ///   * since they're not actually differing in behavior there
    fn insert_header_not_main_page(&self) -> String {
        self.as_ref()
            .insert_header_tmpl()
            .no_demo_differences_or_not_in_demo()
            .no_logout()
    }

    fn insert_html_head(
        &self,
        title: &str,
        svg_hashes: &HashMap<String, String>,
        manifest_hash: &str,
        styles_hash: &str,
        styles_integrity: &str,
        icon_hashes: &HashMap<u32, String>,
    ) -> String {
        let html_head_content = include_frontend_asset!("partials/html_head.tmpl.html")
            .include_svgs(svg_hashes)
            .replace(
                "{ manifest }",
                &format!("./manifest.{}.json", manifest_hash),
            )
            .replace("{ styles }", &format!("./styles.{}.css", styles_hash))
            .replace("{ styles_integrity }", styles_integrity)
            .include_png_icons(icon_hashes)
            .replace("{ title }", title);

        self.as_ref().replace("{ html_head }", &html_head_content)
    }
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

    let sizes: [u32; _] = [32, 48, 64, 128, 180, 192, 512];
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

    let mut svg_hashes = HashMap::new();
    svg_hashes.insert("favicon".to_string(), favicon_short_hash);
    svg_hashes.insert(
        "architecture_simplified".to_string(),
        arch_simplified_short_hash,
    );
    svg_hashes.insert("architecture".to_string(), arch_complete_short_hash);

    // Process manifest.tmpl.json
    let manifest_content = include_frontend_asset!("manifest.tmpl.json")
        .include_svgs(&svg_hashes)
        .include_png_icons(&icon_hashes)
        .insert_metadata();
    let manifest_short_hash = short_hash(manifest_content.as_bytes());
    fs::write(generated_dir.join("manifest.json"), &manifest_content)?;
    println!(
        "cargo::rustc-env=ASSET_HASH_MANIFEST_JSON={}",
        manifest_short_hash
    );

    // Process index.tmpl.html
    let content = include_frontend_asset!("index.tmpl.html")
        .insert_html_head(
            "ShutHost Coordinator",
            &svg_hashes,
            &manifest_short_hash,
            &styles_short_hash,
            &styles_integrity,
            &icon_hashes,
        )
        .insert_js_warnings()
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
        .insert_header_tmpl()
        .insert_footer()
        .include_svgs(&svg_hashes)
        .replace(
            "{ js }",
            &fs::read_to_string("../frontend/assets/generated/app.js")
                .wrap_err("Failed to read generated app.js")?,
        )
        .insert_metadata();
    fs::write(generated_dir.join("index.html"), content)?;

    // Process login.tmpl.html
    let login_content = include_frontend_asset!("login.tmpl.html")
        .insert_html_head(
            "Login • ShutHost",
            &svg_hashes,
            &manifest_short_hash,
            &styles_short_hash,
            &styles_integrity,
            &icon_hashes,
        )
        .insert_js_warnings()
        .insert_header_not_main_page()
        .insert_footer()
        .insert_metadata();
    fs::write(generated_dir.join("login.html"), login_content)?;

    // Process about.tmpl.html
    let about_content = fs::read_to_string(generated_dir.join("about.tmpl.html"))?
        .insert_html_head(
            "Dependencies and Licenses",
            &svg_hashes,
            &manifest_short_hash,
            &styles_short_hash,
            &styles_integrity,
            &icon_hashes,
        )
        .insert_header_not_main_page()
        .insert_footer()
        .insert_metadata();
    fs::write(generated_dir.join("about.html"), about_content)?;

    Ok(())
}

fn generate_encoded_hash(content: impl AsRef<[u8]>) -> eyre::Result<String> {
    let hash = Sha256::digest(content);
    let hash_b64 = general_purpose::STANDARD.encode(hash);
    Ok(format!("sha256-{}", hash_b64))
}
