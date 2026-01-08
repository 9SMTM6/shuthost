pub fn emit() {
    #[expect(clippy::allow_attributes, reason = "Feature dependent code")]
    #[allow(unused_mut, reason = "Feature-dependent code")]
    let mut build_warnings = Vec::<String>::new();

    #[cfg(target_os = "windows")]
    {
        build_warnings.push("Windows builds are currently only supported for internal testing purposes and should not be used in production.".to_string());
    }

    let missing_agents: [&str; _] = [
        #[cfg(not(feature = "include_linux_x86_64_agent"))]
        "linux_x86_64",
        #[cfg(not(feature = "include_linux_aarch64_agent"))]
        "linux_aarch64",
        #[cfg(not(feature = "include_linux_musl_x86_64_agent"))]
        "linux_musl_x86_64",
        #[cfg(not(feature = "include_linux_musl_aarch64_agent"))]
        "linux_musl_aarch64",
        #[cfg(not(feature = "include_macos_aarch64_agent"))]
        "macos_aarch64",
        #[cfg(not(feature = "include_macos_x86_64_agent"))]
        "macos_x86_64",
        #[cfg(not(feature = "include_windows_x86_64_agent"))]
        "windows_x86_64",
        #[cfg(not(feature = "include_windows_aarch64_agent"))]
        "windows_aarch64",
    ];

    #[expect(clippy::allow_attributes, reason = "Feature-dependent code")]
    #[allow(clippy::const_is_empty, reason = "Feature-dependent code")]
    if !missing_agents.is_empty() {
        build_warnings.push(format!(
            "The following agents are not embedded: {}. Trying to install any missing agents from the coordinator will result in errors.",
            missing_agents.join(", ")
        ));
    }

    for warning in &build_warnings {
        println!("cargo::warning={warning}");
    }

    println!(
        "cargo::rustc-env=BUILD_WARNINGS={build_warnings}",
        build_warnings = build_warnings.join(";")
    );
}
