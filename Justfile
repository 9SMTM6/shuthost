build_linux_on_mac:
    mkdir -p shuthost_agent || true
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_agent --target x86_64-unknown-linux-gnu
    # target/x86_64-unknown-linux-gnu/release/shuthost_agent

build_mac:
    mkdir -p shuthost_agent || true
    cargo build --release --bin shuthost_agent --target aarch64-apple-darwin 
    # target/aarch64-apple-darwin/release/shuthost_agent

build_all: build_linux_on_mac build_mac
