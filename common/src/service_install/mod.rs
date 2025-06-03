#[cfg(target_os = "linux")]
pub mod systemd;
pub mod macos;
#[cfg(target_os = "linux")]
pub mod openrc;
pub mod serviceless;

pub fn is_superuser() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn is_systemd() -> bool {
    std::path::Path::new("/run/systemd/system").exists()
}

pub fn is_openrc() -> bool {
    std::path::Path::new("/run/openrc").exists() || std::path::Path::new("/etc/init.d").exists()
}
