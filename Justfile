install_cross_toolchains_on_apple_silicon:
    rustup target add x86_64-apple-darwin

    brew tap messense/macos-cross-toolchains
    
    brew install x86_64-unknown-linux-gnu
    rustup target add x86_64-unknown-linux-gnu
    
    brew install aarch64-unknown-linux-gnu
    rustup target add aarch64-unknown-linux-gnu
    
    brew install x86_64-unknown-linux-musl
    rustup target add x86_64-unknown-linux-musl

    brew install aarch64-unknown-linux-musl
    rustup target add aarch64-unknown-linux-musl

build_linux_host_agents:
    # install cross compilation toolchains (e.g. from musl.cc)
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-gnu &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-gnu &
    # running gnu linkers for musl targets generally works, and these are more widely available on distros. The reverse may also be true on musl based distros
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-musl &
    wait

build_all_host_agents_on_mac:
    cargo build --release --bin shuthost_host_agent --target aarch64-apple-darwin &
    cargo build --release --bin shuthost_host_agent --target x86_64-apple-darwin &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-gnu &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-gnu &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc cargo build --release --bin shuthost_host_agent --target aarch64-unknown-linux-musl &
    wait

build_coordinator_on_mac:
    cargo build --release --bin shuthost_coordinator --target aarch64-apple-darwin &
    cargo build --release --bin shuthost_coordinator --target x86_64-apple-darwin &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-gnu &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc cargo build --release --bin shuthost_coordinator --target aarch64-unknown-linux-gnu &
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-musl &
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc cargo build --release --bin shuthost_coordinator --target aarch64-unknown-linux-musl &
    wait

build_all: build_all_host_agents_on_mac build_coordinator_on_mac

build_graphs:
    dot coordinator/assets/architecture.dot -Tsvg -ocoordinator/assets/architecture.svg
    dot coordinator/assets/architecture_simplified.dot -Tsvg -ocoordinator/assets/architecture_simplified.svg

clean:
    cargo clean && cargo fetch
    cd coordinator/assets && rm -rf node_modules && npm ci

update_dependencies:
    cargo update
    cd coordinator/assets && npm update

test_all:
    cargo test --no-default-features --workspace --all-targets

ci_cargo_deny:
    cargo +stable --locked deny check --hide-inclusion-graph --graph duplicates_tree

alias deny := ci_cargo_deny

ci_semver_updates:
    cargo +stable --locked generate-lockfile

semver_updates:
    cargo +stable generate-lockfile

ci_typo:
    typos
