use std::process::{Command, exit};
use std::path::PathBuf;
use std::str::FromStr;

fn main() {
    let assets_dir = PathBuf::from_str("frontend/assets").unwrap();
    println!("cargo::rerun-if-changed={}/styles.css", assets_dir.display());
    println!("cargo::rerun-if-changed={}/app.ts", assets_dir.display());
    println!("cargo::rerun-if-changed={}/index.tmpl.html", assets_dir.display());
    println!("cargo::rerun-if-changed={}/login.tmpl.html", assets_dir.display());
    println!("cargo::rerun-if-changed={}/partials", assets_dir.display());

    // Check npm
    if Command::new("npm").arg("--version").output().is_err() {
        eprintln!("npm is not installed. Please install node/npm.");
        exit(1);
    }

    let status = Command::new("npm").args(["ci", "--omit=dev"]).current_dir(&assets_dir).status();
    match status {
        Ok(s) if s.success() => {}
        Ok(_) => { eprintln!("npm ci failed."); exit(1); }
        Err(e) => { eprintln!("Failed to run npm ci: {e}"); exit(1); }
    }

    let status = Command::new("npm").arg("run").arg("build").current_dir(&assets_dir).status();
    match status {
        Ok(s) if s.success() => {
            println!("npm build completed successfully.");
        }
        Ok(_) => { eprintln!("npm build failed."); exit(1); }
        Err(e) => { eprintln!("Failed to run npm build: {e}"); exit(1); }
    }
}
