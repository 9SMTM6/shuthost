use base64::{Engine as _, engine::general_purpose};
use eyre::Ok;
use regex::Regex;
use sha2::{Digest as _, Sha256};
use std::fs;

pub fn generate_hashes() -> eyre::Result<()> {
    let script_regex = Regex::new(r#"<script type="module"[^>]*>([\s\S]*?)<\/script>"#)?;
    let mut script_hashes = std::collections::HashSet::new();

    let served_html_files = [
        "../frontend/assets/generated/index.html",
        "../frontend/assets/generated/login.html",
        "../frontend/assets/partials/external_auth_config.tmpl.html",
    ];
    for file_path in served_html_files {
        let content = fs::read_to_string(file_path)?;
        for cap in script_regex.captures_iter(&content) {
            if let Some(script_content) = cap.get(1) {
                let hash_tok = generate_encoded_hash(script_content.as_str().as_bytes())?;
                script_hashes.insert(format!("'{hash_tok}'"));
            }
        }
    }

    let mut script_hash_list: Vec<_> = script_hashes.into_iter().collect();
    script_hash_list.sort();
    let script_hashes_str = script_hash_list.join(" ");
    println!("cargo::rustc-env=CSP_INLINE_SCRIPTS_HASHES={script_hashes_str}");

    // // Generate CSP hash for manifest
    // let manifest_hash = generate_csp_hash_from_file("../frontend/assets/generated/manifest.json")?;
    // println!("cargo::rustc-env=CSP_MANIFEST_HASH={}", manifest_hash);

    // // Generate CSP hash for styles
    // let styles_hash = generate_csp_hash_from_file("../frontend/assets/generated/styles.css")?;
    // println!("cargo::rustc-env=CSP_STYLES_HASH={}", styles_hash);

    Ok(())
}

/// Generate a CSP-compatible SHA256 hash for content
pub fn generate_encoded_hash(content: impl AsRef<[u8]>) -> eyre::Result<String> {
    let hash = Sha256::digest(content);
    let hash_b64 = general_purpose::STANDARD.encode(hash);
    Ok(format!("sha256-{hash_b64}"))
}

// /// Generate a CSP-compatible SHA256 hash for a file
// fn generate_csp_hash_from_file(file_path: &str) -> eyre::Result<String> {
//     let content = fs::read_to_string(file_path)?;
//     generate_encoded_hash(content.as_str())
// }
