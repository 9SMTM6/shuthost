use eyre::{Ok, WrapErr as _, bail};
use std::process;

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
        .env("npm_config_cache", "/tmp/.npm")
        .env("PUPPETEER_SKIP_DOWNLOAD", "true")
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

pub fn run(task: &str) -> eyre::Result<()> {
    process::Command::new(NPM_BIN)
        .arg("run")
        .arg(task)
        .current_dir(FRONTEND_DIR)
        .status()
        .map(|it| {
            if it.success() {
                Ok(())
            } else {
                bail!("npm run {task} failed with {it}")
            }
        })
        .wrap_err(format!("Failed to npm run {task}"))?
}

// TODO: consider some more generic spawn method, to be used for different tasks during build.
/// Spawns an npm script in the background, returning the child process.
/// Call [`join`] with the returned child to wait for it and check success.
pub fn spawn(task: &str) -> eyre::Result<process::Child> {
    process::Command::new(NPM_BIN)
        .arg("run")
        .arg(task)
        .current_dir(FRONTEND_DIR)
        .spawn()
        .wrap_err(format!("Failed to spawn npm run {task}"))
}

/// Waits for a child started by [`spawn`] and returns an error if it failed.
pub fn join(mut child: process::Child, task: &str) -> eyre::Result<()> {
    let status = child.wait().wrap_err(format!("Failed to wait for npm run {task}"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("npm run {task} failed with {status}")
    }
}