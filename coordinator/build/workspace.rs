use eyre::{ContextCompat, Ok};
use std::path::PathBuf;

pub fn set_root() -> eyre::Result<()> {
    let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = workspace_dir
        .parent()
        .wrap_err("expected absolute path in CARGO_MANIFEST_DIR")?;
    let mut path_str = workspace_dir.to_string_lossy().to_string();
    if cfg!(target_os = "windows") {
        path_str = path_str.replace('/', "\\");
        path_str.push('\\');
    } else {
        path_str.push('/');
    }
    println!("cargo::rustc-env=WORKSPACE_ROOT={path_str}");
    Ok(())
}
