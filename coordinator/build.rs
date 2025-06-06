use std::process::{Command, exit};

fn main() {
    // Inform Cargo to rerun the build script if these files change
    // TODO: for some reason this doesnt seem to work properly...
    println!("cargo::rerun-if-changed=assets/styles.css");
    println!("cargo::rerun-if-changed=assets/app.ts");
    println!("cargo::rerun-if-changed=assets/index.tmpl.html");

    // Check if `npm` is installed
    let npm_check = Command::new("npm").arg("--version").output();

    if npm_check.is_err() {
        eprintln!(
            "npm is not installed. Please install it from https://nodejs.org/ or using your package manager."
        );
        exit(1);
    }

    // Run `npm ci` to ensure dependencies are installed
    let npm_ci = Command::new("npm").arg("ci").current_dir("assets").status();

    if let Err(error) = npm_ci {
        eprintln!("Failed to run npm ci: {}", error);
        exit(1);
    }

    if !npm_ci.unwrap().success() {
        eprintln!("npm ci failed.");
        exit(1);
    }

    // Run `npm run build` in the assets directory
    let npm_build = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("assets")
        .status();

    if let Err(error) = npm_build {
        eprintln!("Failed to run npm build: {}", error);
        exit(1);
    }

    if !npm_build.unwrap().success() {
        eprintln!("npm build failed.");
        exit(1);
    }

    println!("npm build completed successfully.");
}
