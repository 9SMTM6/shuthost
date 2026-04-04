use alloc::collections::{BTreeMap, BTreeSet};
use std::{collections::HashMap, fs, process::Command};

use cargo_about::licenses::config;
use eyre::Context as _;
use krates::Utf8PathBuf as PathBuf;
use serde::{Deserialize, Serialize};
use spdx::text as spdx_text;

const VERSION: &str = shuthost_common::version_string!();
use spdx::{LicenseItem, Licensee, expression::Expression};
use toml::from_str as toml_from_str;
use url::Url;

#[derive(Deserialize)]
struct DenyConfig {
    licenses: Licenses,
}

#[derive(Deserialize)]
struct Licenses {
    allow: Vec<String>,
}

fn about_config() -> eyre::Result<config::Config> {
    let deny: DenyConfig = toml_from_str(include_str!("../../deny.toml"))?;
    let accepted: Vec<Licensee> = deny
        .licenses
        .allow
        .into_iter()
        .map(|s| s.parse::<Licensee>().expect("valid license"))
        .collect();

    Ok(config::Config {
        accepted,
        targets: vec![
            "x86_64-unknown-linux-gnu".to_string(),
            "aarch64-unknown-linux-gnu".to_string(),
            "x86_64-unknown-linux-musl".to_string(),
            "aarch64-unknown-linux-musl".to_string(),
            "x86_64-apple-darwin".to_string(),
            "aarch64-apple-darwin".to_string(),
            "x86_64-pc-windows-msvc".to_string(),
            "aarch64-pc-windows-msvc".to_string(),
        ],
        ..Default::default()
    })
}

#[derive(Serialize)]
enum Ecosystem {
    Rust,
    Npm,
}

#[derive(Serialize)]
struct Author {
    name: String,
    email: Option<String>,
}

#[derive(Serialize)]
struct CombinedEntry {
    name: String,
    version: String,
    ecosystem: Ecosystem,
    #[serde(serialize_with = "serialize_license")]
    license: Expression,
    license_html: String,
    authors: Vec<Author>,
    repository: Option<Url>,
}

fn serialize_license<S>(license: &Expression, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = license.to_string();
    serializer.serialize_str(&s)
}

fn parse_url(should_be_url: Option<&str>) -> Result<Option<Url>, eyre::Error> {
    Ok(match should_be_url {
        Some(r) => Some(Url::parse(r).wrap_err("invalid repository url")?),
        None => None,
    })
}

fn parse_author(author_str: &str) -> Author {
    if let Some(start) = author_str.find('<')
        && let Some(end) = author_str.find('>')
    {
        let name = author_str[..start].trim().to_string();
        let email = author_str[start + 1..end].trim().to_string();
        return Author {
            name: if name.is_empty() { email.clone() } else { name },
            email: Some(email),
        };
    }
    Author {
        name: author_str.to_string(),
        email: None,
    }
}

// For now, skip direct spdx::text lookups (varies by spdx crate version).
// We will fall back to reading license files where available.
fn process_entry(
    name: String,
    version: String,
    ecosystem: Ecosystem,
    license_expr_str: String,
    authors: Vec<Author>,
    repository: Option<Url>,
    licenses_set: &mut BTreeSet<LicenseItem>,
) -> eyre::Result<CombinedEntry> {
    // parse SPDX expression
    let expr: Expression = license_expr_str
        .parse()
        .wrap_err(format!("invalid SPDX expression for {name}"))?;

    let mut license_html = license_expr_str;

    // collect license ids from the expression requirements
    let requirements: BTreeSet<_> = expr
        .requirements()
        .map(|req| req.req.license.clone())
        .collect();
    licenses_set.extend(requirements.iter().cloned());
    for license in &requirements {
        let license_str = license.to_string();
        license_html = license_html.replace(
            &license_str,
            &format!("<a href=\"#license-{license_str}\" class=\"link\">{license_str}</a>",),
        );
    }

    Ok(CombinedEntry {
        name,
        version,
        ecosystem,
        license: expr,
        license_html,
        authors,
        repository,
    })
}

// For now, skip direct spdx::text lookups (varies by spdx crate version).
// We will fall back to reading license files where available.
fn get_spdx_text(id: &str) -> Option<String> {
    // spdx::text exports LICENSE_TEXTS: &[(&str, &str)]
    for &(k, v) in spdx_text::LICENSE_TEXTS {
        if k == id {
            return Some(v.to_string());
        }
    }
    None
}

pub fn build_json() -> eyre::Result<()> {
    let mut combined = Vec::<CombinedEntry>::new();
    let mut licenses_set = BTreeSet::<LicenseItem>::new();

    process_rust_crates(&mut combined, &mut licenses_set)?;
    process_pnpm_packages(&mut combined, &mut licenses_set)?;;

    generate_about_json(&combined, &licenses_set)
}

fn process_rust_crates(
    combined: &mut Vec<CombinedEntry>,
    licenses_set: &mut BTreeSet<LicenseItem>,
) -> eyre::Result<()> {
    let cfg = about_config()?;

    let manifest_path = PathBuf::from("../Cargo.toml");

    let krates = cargo_about::get_all_crates(
        &manifest_path,
        false,  // no_default_features
        false,  // all_features
        vec![], // features
        true,   // workspace
        krates::LockOptions {
            frozen: false,
            locked: false,
            offline: false,
        },
        &cfg,
        &[], // target
    )
    .map_err(eyre::Report::msg)
    .wrap_err("failed to get crate information")?;

    for k in krates.krates() {
        let license_str = k
            .license
            .clone()
            .ok_or_else(|| eyre::eyre!("crate '{}' has no license", k.name))?;

        combined.push(process_entry(
            k.name.clone(),
            k.version.to_string(),
            Ecosystem::Rust,
            license_str,
            k.authors.iter().map(|a| parse_author(a)).collect(),
            parse_url(k.repository.as_deref())?,
            licenses_set,
        )?);
    }

    Ok(())
}

fn process_pnpm_packages(
    combined: &mut Vec<CombinedEntry>,
    licenses_set: &mut BTreeSet<LicenseItem>,
) -> eyre::Result<()> {
    // Run `pnpm licenses list --json` and parse its output directly.
    // Format: { "<spdx-id>": [ { name, versions, license, author?, homepage? }, ... ], ... }
    #[derive(Deserialize)]
    struct PnpmPackageInfo {
        name: String,
        versions: Vec<String>,
        author: Option<String>,
        homepage: Option<String>,
    }

    let output = Command::new(super::pnpm::pnpm_bin())
        .arg("licenses")
        .arg("list")
        .arg("--json")
        .current_dir("../frontend")
        .output()
        .wrap_err("Failed to run pnpm licenses list --json")?;
    if !output.status.success() {
        eyre::bail!(
            "pnpm licenses list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let pnpm_map: HashMap<String, Vec<PnpmPackageInfo>> =
        serde_json::from_slice(&output.stdout).wrap_err("Failed to parse pnpm licenses JSON")?;

    for (license_id, packages) in pnpm_map {
        for pkg in packages {
            for version in &pkg.versions {
                combined.push(process_entry(
                    pkg.name.clone(),
                    version.clone(),
                    Ecosystem::Npm,
                    license_id.clone(),
                    pkg.author
                        .as_deref()
                        .map(parse_author)
                        .into_iter()
                        .collect(),
                    parse_url(pkg.homepage.as_deref())?,
                    licenses_set,
                )?);
            }
        }
    }

    Ok(())
}

fn generate_about_json(
    combined: &[CombinedEntry],
    licenses_set: &BTreeSet<LicenseItem>,
) -> eyre::Result<()> {
    let licenses_map: BTreeMap<_, _> = licenses_set
        .iter()
        .map(|itm| {
            (
                itm.to_string(),
                get_spdx_text(&itm.to_string()).expect("License text should be available"),
            )
        })
        .collect();

    let data = serde_json::json!({
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "repository": env!("CARGO_PKG_REPOSITORY"),
        "version": VERSION,
        "entries": combined,
        "licenses": licenses_map,
    });
    fs::write(
        "../frontend/assets/generated/about-data.json",
        serde_json::to_string_pretty(&data)?,
    )?;
    Ok(())
}
