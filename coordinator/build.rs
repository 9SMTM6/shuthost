use std::path::PathBuf;

fn main() {
    let workspace_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_dir = workspace_dir.parent().unwrap();
    println!("cargo:rustc-env=WORKSPACE_ROOT={}/", workspace_dir.display());
}
