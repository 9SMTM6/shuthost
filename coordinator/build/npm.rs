use std::process;
use eyre::{WrapErr, bail, Ok};

#[cfg(not(target_os = "windows"))]
const NPM_BIN: &str = "npm";
#[cfg(target_os = "windows")]
const NPM_BIN: &str = "npm.cmd";

const FRONTEND_DIR: &str = "../frontend";

pub fn setup() -> eyre::Result<()> {
    // Check npm
    process::Command::new(NPM_BIN)
        .arg("--version")
        .output()
        .wrap_err("Ensure node/npm is installed")?;

    process::Command::new(NPM_BIN)
        .arg("ci")
        .current_dir(FRONTEND_DIR)
        .status()
        .map(|it| {
            if it.success() {
                Ok(())
            } else {
                bail!("npm ci failed with {it}")
            }
        })
        .wrap_err("Failed to npm ci")?
}

pub fn run_build() -> eyre::Result<()> {
    process::Command::new(NPM_BIN)
        .arg("run")
        .arg("build")
        .current_dir(FRONTEND_DIR)
        .status()
        .map(|it| {
            if it.success() {
                Ok(())
            } else {
                bail!("npm run build failed with {it}")
            }
        })
        .wrap_err("Failed to npm run build")?
}