use std::process::{Command, exit};

fn main() {
    // Check if `tsc` is installed
    let tsc_check = Command::new("tsc").arg("--version").output();

    if tsc_check.is_err() {
        eprintln!(
            "TypeScript is not installed. Please install it using:\n\n    npm install --global typescript\n"
        );
        exit(1);
    }

    // Run the TypeScript compiler
    let tsc_build = Command::new("tsc")
        .arg("-p")
        .arg("assets/tsconfig.json")
        .status();

    if let Err(error) = tsc_build {
        eprintln!("Failed to build TypeScript files: {}", error);
        exit(1);
    }

    if !tsc_build.unwrap().success() {
        eprintln!("TypeScript build failed.");
        exit(1);
    }

    println!("TypeScript build completed successfully.");
}
