use std::fs;
use std::path::PathBuf;
use std::process::Command;

struct Agent {
    name: &'static str,
    rel_path: &'static str,
    included: bool,
}

const ALL_AGENTS: &[Agent] = &[
    Agent {
        name: "linux_x86_64",
        rel_path: "target/x86_64-unknown-linux-musl/release/shuthost_host_agent",
        included: cfg!(feature = "include_linux_musl_x86_64_agent"),
    },
    Agent {
        name: "linux_aarch64",
        rel_path: "target/aarch64-unknown-linux-musl/release/shuthost_host_agent",
        included: cfg!(feature = "include_linux_musl_aarch64_agent"),
    },
    Agent {
        name: "macos_aarch64",
        rel_path: "target/aarch64-apple-darwin/release/shuthost_host_agent",
        included: cfg!(feature = "include_macos_aarch64_agent"),
    },
    Agent {
        name: "macos_x86_64",
        rel_path: "target/x86_64-apple-darwin/release/shuthost_host_agent",
        included: cfg!(feature = "include_macos_x86_64_agent"),
    },
    Agent {
        name: "windows_x86_64",
        rel_path: "target/x86_64-pc-windows-msvc/release/shuthost_host_agent.exe",
        included: cfg!(feature = "include_windows_x86_64_agent"),
    },
    Agent {
        name: "windows_aarch64",
        rel_path: "target/aarch64-pc-windows-msvc/release/shuthost_host_agent.exe",
        included: cfg!(feature = "include_windows_aarch64_agent"),
    },
];

fn check_stale_agents(build_warnings: &mut Vec<String>) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = manifest_dir.parent().map(PathBuf::from) else {
        return;
    };

    let Some(coordinator_commit) = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() == 40)
    else {
        return;
    };

    println!("cargo::rerun-if-changed=.git/HEAD");

    let mut no_git_info: Vec<&str> = Vec::new();
    let mut stale: Vec<&str> = Vec::new();

    for agent in ALL_AGENTS.iter().filter(|a| a.included) {
        let binary_path = workspace_root.join(agent.rel_path);
        println!("cargo::rerun-if-changed={}", binary_path.display());

        let Ok(bytes) = fs::read(&binary_path) else {
            // Missing binary will already cause a compile error via include_bytes!
            continue;
        };

        if bytes
            .windows("unknown-commit".len())
            .any(|w| w == b"unknown-commit")
        {
            no_git_info.push(agent.name);
        } else if !bytes
            .windows(coordinator_commit.len())
            .any(|w| w == coordinator_commit.as_bytes())
        {
            stale.push(agent.name);
        }
    }

    if !no_git_info.is_empty() {
        build_warnings.push(format!(
            "The following agents were built without git info — cannot verify if they are up to date: {}.",
            no_git_info.join(", ")
        ));
    }

    if !stale.is_empty() {
        build_warnings.push(format!(
            "The following agents may be out of date (not built at commit {}): {}. Rebuild them before releasing.",
            &coordinator_commit[..8],
            stale.join(", "),
        ));
    }
}

pub fn emit() -> Result<(), eyre::Report> {
    #[expect(clippy::allow_attributes, reason = "Feature dependent code")]
    #[allow(unused_mut, reason = "Feature-dependent code")]
    let mut build_warnings = Vec::<String>::new();

    #[cfg(target_os = "windows")]
    {
        build_warnings.push("Windows builds are currently only supported for internal testing purposes and should not be used in production.".to_string());
    }

    let missing_agents: Vec<&str> = ALL_AGENTS
        .iter()
        .filter(|a| !a.included)
        .map(|a| a.name)
        .collect();

    if !missing_agents.is_empty() {
        build_warnings.push(format!(
            "The following agents are not embedded: {}. Trying to install any missing agents from the coordinator will result in errors.",
            missing_agents.join(", ")
        ));
    }

    check_stale_agents(&mut build_warnings);

    for warning in &build_warnings {
        println!("cargo::warning={warning}");
    }

    println!(
        "cargo::rustc-env=BUILD_WARNINGS={build_warnings}",
        build_warnings = build_warnings.join(";")
    );

    return Ok(());
}
