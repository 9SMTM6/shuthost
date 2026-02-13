//! Utilities to detect service management capabilities on the host system.

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "linux")]
pub mod openrc;
#[cfg(target_os = "linux")]
pub mod systemd;

use std::path;

/// Returns `true` if the current process is running as superuser (root).
#[cfg(unix)]
#[expect(
    clippy::absolute_paths,
    reason = "we don't want to add a bunch of imports behind cfg attributes"
)]
#[must_use]
pub fn is_superuser() -> bool {
    nix::unistd::geteuid().as_raw() == 0
}

/// Returns `true` if the system uses `OpenRC` (checks `/run/openrc` or `/etc/init.d`).
#[must_use]
pub fn is_openrc() -> bool {
    path::Path::new("/run/openrc").exists() || path::Path::new("/etc/init.d").exists()
}
