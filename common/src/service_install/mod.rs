//! Utilities to detect service management capabilities on the host system.

pub mod macos;
#[cfg(target_os = "linux")]
pub mod openrc;
pub mod serviceless;
#[cfg(target_os = "linux")]
pub mod systemd;

/// Returns `true` if the current process is running as superuser (root).
///
/// # Safety
///
/// This calls unsafe `geteuid`; ensure correct platform compatibility.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "freebsd"))]
pub fn is_superuser() -> bool {
    nix::unistd::geteuid().as_raw() == 0
}

/// Returns `true` if the system uses OpenRC (checks `/run/openrc` or `/etc/init.d`).
pub fn is_openrc() -> bool {
    std::path::Path::new("/run/openrc").exists() || std::path::Path::new("/etc/init.d").exists()
}
