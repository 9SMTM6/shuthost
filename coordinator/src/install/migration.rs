use std::path::Path;
use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::{self as unix_fs, PermissionsExt as _};

use eyre::WrapErr as _;
use nix::unistd::User;

use crate::install::BINARY_NAME;

/// Migrates old config file and associated files from the old location to the new location.
///
/// This function checks if the old config exists and the new one doesn't, then moves the config
/// if needed and setting appropriate permissions and ownership. The old config file location is
/// determined internally based on the operating system and the provided username.
///
/// # Arguments
///
/// * `user` - The username for ownership.
/// * `new_config_location` - Path to the new config file location.
///
/// # Errors
///
/// Returns an error if file operations fail.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn migrate_old_config(user: &str, new_config_location: &Path) -> eyre::Result<()> {
    #[cfg(target_os = "linux")]
    let old_config_location = PathBuf::from(format!("/home/{user}/.config/{BINARY_NAME}.toml"));
    #[cfg(target_os = "macos")]
    let old_config_location = PathBuf::from(format!("/Users/{user}/.config/{BINARY_NAME}.toml"));

    // Move existing config from old location to new location if old exists and new doesn't
    let mut created_new_dir = false;
    if old_config_location.exists() && !new_config_location.exists() {
        if let Some(parent_dir) = new_config_location.parent()
            && !parent_dir.exists()
        {
            std::fs::create_dir_all(parent_dir).wrap_err("Failed to create config directory")?;
            created_new_dir = true;
        }
        std::fs::rename(&old_config_location, new_config_location).wrap_err(format!(
            "Failed to move config file from {} to {}",
            old_config_location.display(),
            new_config_location.display()
        ))?;
        println!("Moved config file from {old_config_location:?} to {new_config_location:?}");

        // Also move associated files (database and certificates) from old directory to new directory
        if let (Some(old_dir), Some(new_dir)) =
            (old_config_location.parent(), new_config_location.parent())
        {
            let files_to_move = [
                "shuthost.db",
                "shuthost.db-wal",
                "shuthost.db-shm",
                "tls_cert.pem",
                "tls_key.pem",
            ];
            for file_name in &files_to_move {
                let old_file = old_dir.join(file_name);
                let new_file = new_dir.join(file_name);
                if old_file.exists() && !new_file.exists() {
                    std::fs::rename(&old_file, &new_file).wrap_err(format!(
                        "Failed to move {} from {} to {}",
                        file_name,
                        old_file.display(),
                        new_file.display()
                    ))?;
                    println!("Moved {file_name} from {old_file:?} to {new_file:?}");
                }
            }
        }

        // Chown the new directory if it was created
        if created_new_dir && let Some(parent_dir) = new_config_location.parent() {
            std::fs::set_permissions(parent_dir, std::fs::Permissions::from_mode(0o700))?;

            let user_info = User::from_name(user)
                .wrap_err("Failed to get user info")?
                .ok_or_else(|| eyre::eyre!("User {user} not found"))?;
            unix_fs::chown(
                parent_dir,
                Some(user_info.uid.into()),
                Some(user_info.gid.into()),
            )?;

            println!("Chowned migrated config directory at {parent_dir:?} for {user}",);
        }
    }

    Ok(())
}
