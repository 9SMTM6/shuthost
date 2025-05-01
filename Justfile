install_cross_toolchains:
    brew tap messense/macos-cross-toolchains
    # install x86_64-unknown-linux-gnu toolchain
    brew install x86_64-unknown-linux-gnu
    rustup target add x86_64-unknown-linux-gnu
    # install aarch64-unknown-linux-gnu toolchain
    brew install aarch64-unknown-linux-gnu
    rustup target add aarch64-unknown-linux-gnu

build_agent_linux_on_mac:
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_agent --target x86_64-unknown-linux-gnu
    # target/x86_64-unknown-linux-gnu/release/shuthost_agent

build_agent_mac:
    cargo build --release --bin shuthost_agent --target aarch64-apple-darwin 
    # target/aarch64-apple-darwin/release/shuthost_agent

build_controller_linux_on_mac:
    # CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_controller --target x86_64-unknown-linux-gnu
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_controller --target aarch64-unknown-linux-gnu
    # target/x86_64-unknown-linux-gnu/release/shuthost_controller
    # target/aarch64-unknown-linux-gnu/release/shuthost_controller

build_all: build_agent_linux_on_mac build_agent_mac build_controller_linux_on_mac
