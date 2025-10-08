use std::process::{Command, exit};

fn main() {
    let frontend_dir = "frontend";
    let assets_dir = "frontend/assets";
    println!("cargo::rerun-if-changed={}/styles.tailwind.css", assets_dir);
    println!("cargo::rerun-if-changed={}/app.ts", assets_dir);
    println!("cargo::rerun-if-changed={}/index.tmpl.html", assets_dir);
    println!("cargo::rerun-if-changed={}/login.tmpl.html", assets_dir);
    println!("cargo::rerun-if-changed={}/partials", assets_dir);

    // Check npm
    if Command::new("npm").arg("--version").output().is_err() {
        eprintln!("npm is not installed. Please install node/npm.");
        exit(1);
    }

    let status = Command::new("npm").arg("ci").current_dir(frontend_dir).status();
    match status {
        Ok(s) if s.success() => {}
        Ok(_) => { eprintln!("npm ci failed."); exit(1); }
        Err(e) => { eprintln!("Failed to run npm ci: {e}"); exit(1); }
    }

    let status = Command::new("npm").arg("run").arg("build").current_dir(frontend_dir).status();
    match status {
        Ok(s) if s.success() => {
            println!("npm build completed successfully.");
        }
        Ok(_) => { eprintln!("npm build failed."); exit(1); }
        Err(e) => { eprintln!("Failed to run npm build: {e}"); exit(1); }
    }
}
