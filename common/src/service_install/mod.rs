//! Utilities to detect service management capabilities on the host system.

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "linux")]
pub mod openrc;
#[cfg(target_os = "linux")]
pub mod systemd;

/// Returns `true` if the current process is running as superuser (root).
#[cfg(unix)]
#[must_use]
pub fn is_superuser() -> bool {
    nix::unistd::geteuid().as_raw() == 0
}

/// Returns `true` if the system uses `OpenRC` (checks `/run/openrc` or `/etc/init.d`).
#[must_use]
pub fn is_openrc() -> bool {
    std::path::Path::new("/run/openrc").exists() || std::path::Path::new("/etc/init.d").exists()
}
