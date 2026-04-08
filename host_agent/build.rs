use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=.git/HEAD");

    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() == 40)
        .unwrap_or_else(|| "unknown-commit".to_string());

    println!("cargo::rustc-env=SHUTHOST_BUILD_COMMIT={commit}");
}
