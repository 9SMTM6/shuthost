use std::process;

use eyre::{Ok, WrapErr as _, bail};

#[cfg(not(target_os = "windows"))]
const NPM_BIN: &str = "pnpm";
#[cfg(target_os = "windows")]
const NPM_BIN: &str = "pnpm.cmd";

const FRONTEND_DIR: &str = "../frontend";

pub const fn pnpm_bin() -> &'static str {
    NPM_BIN
}

pub fn setup() -> eyre::Result<()> {
    // Check pnpm
    process::Command::new(NPM_BIN)
        .arg("--version")
        .output()
        .wrap_err("Ensure pnpm is installed")?;

    let output = process::Command::new(NPM_BIN)
        .arg("install")
        .arg("--frozen-lockfile")
        .current_dir(FRONTEND_DIR)
        // TODO: remove hotfix
        .env("CI", "true")
        .output()
        .wrap_err("Failed to pnpm install")?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        bail!("pnpm install failed with {}", output.status);
    }
    Ok(())
}

pub fn run(task: &str) -> eyre::Result<()> {
    let output = process::Command::new(NPM_BIN)
        .arg("run")
        .arg(task)
        .current_dir(FRONTEND_DIR)
        .output()
        .wrap_err(format!("Failed to pnpm run {task}"))?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        bail!("pnpm run {task} failed with {}", output.status);
    }
    Ok(())
}
