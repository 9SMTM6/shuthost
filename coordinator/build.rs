use std::{path::PathBuf, process};

fn main() {
    let workspace_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_dir = workspace_dir.parent().unwrap();
    println!("cargo:rustc-env=WORKSPACE_ROOT={}/", workspace_dir.display());

    let frontend_dir = "frontend";
    let rerun_if = "cargo::rerun-if-changed=frontend/assets";
    println!("{rerun_if}/styles.tailwind.css");
    println!("{rerun_if}/app.ts");
    println!("{rerun_if}/index.tmpl.html");
    println!("{rerun_if}/login.tmpl.html");
    println!("{rerun_if}/partials");

    // Check npm
    if process::Command::new("npm").arg("--version").output().is_err() {
        eprintln!("npm is not installed. Please install node/npm.");
        process::exit(1);
    }

    let status = process::Command::new("npm").arg("ci").current_dir(frontend_dir).status();
    match status {
        Ok(s) if s.success() => {}
        Ok(_) => { eprintln!("npm ci failed."); process::exit(1); }
        Err(e) => { eprintln!("Failed to run npm ci: {e}"); process::exit(1); }
    }

    let status = process::Command::new("npm").arg("run").arg("build").current_dir(frontend_dir).status();
    match status {
        Ok(s) if s.success() => {
            println!("npm build completed successfully.");
        }
        Ok(_) => { eprintln!("npm build failed."); process::exit(1); }
        Err(e) => { eprintln!("Failed to run npm build: {e}"); process::exit(1); }
    }
}
