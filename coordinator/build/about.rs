use handlebars::Handlebars;
use krates::Utf8PathBuf as PathBuf;
use serde::{Deserialize, Serialize};
use spdx::{LicenseItem, Licensee, expression::Expression};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::read_to_string,
};
use url::Url;

#[derive(Deserialize)]
struct DenyConfig {
    licenses: Licenses,
}

#[derive(Deserialize)]
struct Licenses {
    allow: Vec<String>,
}

fn about_config() -> eyre::Result<cargo_about::licenses::config::Config> {
    let deny: DenyConfig = toml::from_str(include_str!("../../deny.toml"))?;
    let accepted: Vec<Licensee> = deny
        .licenses
        .allow
        .into_iter()
        .map(|s| s.parse::<Licensee>().expect("valid license"))
        .collect();

    Ok(cargo_about::licenses::config::Config {
        accepted,
        targets: vec![
            "x86_64-unknown-linux-gnu".to_string(),
            "aarch64-unknown-linux-gnu".to_string(),
            "x86_64-unknown-linux-musl".to_string(),
            "aarch64-unknown-linux-musl".to_string(),
            "x86_64-apple-darwin".to_string(),
            "aarch64-apple-darwin".to_string(),
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
        Some(r) => Some(Url::parse(r).map_err(|e| eyre::eyre!("invalid repository url {}", e))?),
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
        .map_err(|e| eyre::eyre!("invalid SPDX expression for {}: {e}", name))?;

    let mut license_html = license_expr_str;

    // collect license ids from the expression requirements
    let requirements: BTreeSet<_> = expr
        .requirements()
        .map(|req| req.req.license.clone())
        .collect();
    licenses_set.extend(requirements.iter().cloned());
    for license in requirements.iter() {
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
    for &(k, v) in spdx::text::LICENSE_TEXTS.iter() {
        if k == id {
            return Some(v.to_string());
        }
    }
    None
}

pub fn build_html() -> eyre::Result<()> {
    let manifest_path = PathBuf::from("../Cargo.toml");

    let cfg = about_config()?;

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
    .map_err(|e| eyre::eyre!("{e}"))?;

    // collect crate entries and license texts map
    let mut combined = Vec::<CombinedEntry>::new();
    let mut licenses_set = BTreeSet::<LicenseItem>::new();

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
            &mut licenses_set,
        )?);
    }

    // read npm license json (generated by frontend build)
    // parse npm JSON into typed struct map
    #[derive(Deserialize)]
    struct NpmInfo {
        licenses: Option<String>,
        license: Option<String>,
        repository: Option<String>,
        publisher: Option<String>,
        email: Option<String>,
        #[serde(flatten)]
        _other: HashMap<String, serde_json::Value>,
    }

    let npm_map: HashMap<String, NpmInfo> = serde_json::from_str(&read_to_string(
        "../frontend/assets/generated/npm-licenses.json",
    )?)?;
    for (pkgkey, info) in npm_map.into_iter() {
        let license_str = info
            .licenses
            .or(info.license)
            .ok_or_else(|| eyre::eyre!("npm package '{}' missing license", pkgkey))?;

        // split pkgkey into name and version using last '@'
        let (name, version) = if let Some(idx) = pkgkey.rfind('@') {
            let (n, v) = pkgkey.split_at(idx);
            // v starts with '@', drop it
            (n.to_string(), v[1..].to_string())
        } else {
            (pkgkey.clone(), String::from(""))
        };

        combined.push(process_entry(
            name,
            version,
            Ecosystem::Npm,
            license_str
                .strip_suffix("*")
                .unwrap_or(&license_str)
                .to_string(), // remove trailing '*' if present, comes from license checker inferring the license
            if let &Some(ref publisher) = &info.publisher {
                vec![Author {
                    name: publisher.clone(),
                    email: info.email.clone(),
                }]
            } else if let &Some(ref email) = &info.email {
                vec![Author {
                    name: email.clone(),
                    email: Some(email.clone()),
                }]
            } else {
                vec![]
            },
            parse_url(info.repository.as_deref())?,
            &mut licenses_set,
        )?);
    }

    let licenses_map: BTreeMap<_, _> = licenses_set
        .iter()
        .map(|itm| {
            (
                itm.to_string(),
                get_spdx_text(&itm.to_string()).expect("License text should be available"),
            )
        })
        .collect();

    // Render HTML with Handlebars
    let hb = Handlebars::new();
    let template = include_str!("../../frontend/assets/about.tmpl.hbs");
    let data = serde_json::json!({
        "entries": combined,
        "licenses": licenses_map,
    });
    let html = hb
        .render_template(template, &data)
        .map_err(|e| eyre::eyre!("Handlebars error: {}", e))?;
    std::fs::write("../frontend/assets/generated/about.tmpl.html", &html)?;
    Ok(())
}
