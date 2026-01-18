#!/bin/sh

set -e

build_musl() {
    #  we build musl binaries in a container, and fake the release builds by copying the debug builds to release paths
    docker build --network host -t shuthost-builder -f scripts/build.Containerfile .
    run_as_elevated chown -R "$(id -u)":"$(id -g)" target/
    run_as_elevated chown -R "$(id -u)":"$(id -g)" frontend/node_modules/ || true
    run_as_elevated chown -R "$(id -u)":"$(id -g)" ~/.npm || true
    # Clean node_modules to avoid npm tar errors when mounting from host into container
    rm -rf frontend/node_modules
    docker run --rm \
        --user "$(id -u):$(id -g)" \
        -v "$(pwd):/src" \
        -v "$HOME/.cargo/registry:/usr/local/cargo/registry" \
        -v "$HOME/.cargo/git:/usr/local/cargo/git" \
        shuthost-builder sh -c "\
            set -e
            sudo chown -R $(id -u):$(id -g) ~/.npm || true
            sudo chown -R $(id -u):$(id -g) frontend/node_modules/ || true
            mkdir -p target/x86_64-unknown-linux-musl/release target/x86_64-unknown-linux-musl/debug target/release target/debug
            # build with coverage support
            eval \"\$(cargo llvm-cov show-env --export-prefix --remap-path-prefix)\"
            cargo build --bin shuthost_host_agent
            cp ./target/debug/shuthost_host_agent ./target/release/
            cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-musl/debug/
            # copy agent debug build to release path for inclusion in coordinator
            cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-musl/release/
            cargo build --bin shuthost_coordinator --features=include_linux_musl_x86_64_agent
            cp ./target/debug/shuthost_coordinator ./target/release/
            cp ./target/debug/shuthost_coordinator ./target/x86_64-unknown-linux-musl/debug/
            # copy coordinator debug build to release path for other scripts that expect the binary there
            cp ./target/debug/shuthost_coordinator ./target/x86_64-unknown-linux-musl/release/
            chmod +x ./target/**/shuthost_*
        "
}

build_gnu() {
    # Check if the cargo llvm-cov subcommand exists and set up the environment
    if cargo llvm-cov --help > /dev/null 2>&1; then
        eval "$(cargo llvm-cov show-env --export-prefix --remap-path-prefix)"
    fi
    mkdir -p target/x86_64-unknown-linux-gnu/release target/x86_64-unknown-linux-gnu/debug target/release target/debug target/x86_64-unknown-linux-musl/release/

    # Build the coordinator binary
    cargo build --bin shuthost_host_agent
    cp ./target/debug/shuthost_host_agent ./target/release/
    cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-gnu/debug/
    # copy agent debug build to release path for inclusion in coordinator
    cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-gnu/release/
    # also fake the musl agent, since some scripts only look for that...
    cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-musl/release/            
    cargo build --bin shuthost_coordinator --features include_linux_x86_64_agent,include_linux_musl_x86_64_agent
    cp ./target/debug/shuthost_coordinator ./target/release/
    cp ./target/debug/shuthost_coordinator ./target/x86_64-unknown-linux-gnu/debug/
    # copy coordinator debug build to release path for other scripts that expect the binary there
    cp ./target/debug/shuthost_coordinator ./target/x86_64-unknown-linux-gnu/release/
}

elevate_privileges() {
    cmd="$*"
    if command -v sudo >/dev/null 2>&1; then
        sudo sh -c "$cmd"
    elif command -v doas >/dev/null 2>&1; then
        doas sh -c "$cmd"
    else
        echo "Error: Neither sudo nor doas found. Please install sudo or doas."
        exit 1
    fi
}

run_as_elevated() {
    if [ "$(id -u)" -eq 0 ]; then
        sh -c "$*"
    else
        elevate_privileges "$*"
    fi
}

detect_platform() {
    # Detect architecture
    ARCH="$(uname -m)"
    case "$ARCH" in
        x86_64) ARCH="x86_64" ;;
        aarch64 | arm64) ARCH="aarch64" ;;
        *)
            echo "Unsupported architecture: $ARCH"
            echo "Supported: x86_64, aarch64"
            exit 1
            ;;
    esac

    # Detect OS
    OS="$(uname -s)"
    case "$OS" in
        Linux)
            PLATFORM="linux-musl"  # Prefer musl for better compatibility
            TARGET_TRIPLE="${ARCH}-unknown-${PLATFORM}"
            ;;
        Darwin)
            # shellcheck disable=SC2034 # TARGET_TRIPLE is used externally
            TARGET_TRIPLE="${ARCH}-apple-darwin"
            ;;
        *)
            echo "Unsupported OS: $OS"
            echo "Supported: Linux, macOS (Darwin)"
            exit 1
            ;;
    esac
}

verify_checksum() {
    # Compute checksum
    echo "Computing SHA256 checksum..."
    COMPUTED_CHECKSUM=$(sha256sum "$FILENAME" | cut -d' ' -f1)
    echo "Computed checksum: $COMPUTED_CHECKSUM"
    echo
    echo "Please verify this checksum against the one provided for $FILENAME on the releases page:"
    echo "$BASE_URL"
    echo
    if [ -t 0 ]; then
        printf "Have you verified the checksum? (y/N): "
        read REPLY
        echo
        case "$REPLY" in
            [Yy]*)
                ;;
            *)
                echo "Checksum verification aborted. Installation cancelled."
                exit 1
                ;;
        esac
    else
        echo "Non-interactive mode: Skipping checksum verification prompt (defaulting to yes)."
    fi
}

embed_script() {
    script_file="$1"
    while IFS= read -r line; do
        case "$line" in
        ". "*)
            sourced="${line#". "}"
            if [ -f "$sourced" ]; then
                if [ "$sourced" = "./scripts/helpers.sh" ]; then
                    embed_script "$sourced"
                else
                    cat "$sourced"
                fi
            else
                echo "$line"
            fi
            ;;
        *)
            echo "$line"
            ;;
        esac
    done < "$script_file"
}

generate_embedded_scripts() {
    mkdir -p target/scripts
    find scripts -name "*.sh" -type f | while read -r file; do
        relative="${file#scripts/}"
        mkdir -p "target/scripts/$(dirname "$relative")"
        embed_script "$file" > "target/scripts/$relative"
        chmod +x "target/scripts/$relative"
    done
}

wait_for_coordinator_ready() {
    port="${1:-8080}"
    printf 'Waiting for coordinator to be ready on port %s...\n' "$port"
    i=1
    while [ "$i" -le 30 ]; do
        if curl -fsSL "http://localhost:$port/login" >/dev/null 2>&1; then
            printf 'Coordinator is ready!\n'
            return 0
        fi
        i=$((i + 1))
        sleep 1
    done
    printf 'Coordinator did not become ready within 30 seconds\n'
    return 1
}

wait_for_agent_ready() {
    printf 'Waiting for agent to be ready...\n'
    i=1
    while [ "$i" -le 30 ]; do
        if run_as_elevated pgrep -af shuthost_host_agent >/dev/null 2>&1; then
            printf 'Agent is ready!\n'
            return 0
        fi
        i=$((i + 1))
        sleep 1
    done
    printf 'Agent did not become ready within 30 seconds\n'
    return 1
}

# provide default values for BASE_URL and DOWNLOAD_URL
BASE_URL="https://github.com/9SMTM6/shuthost/releases/latest/"
# shellcheck disable=SC2034 # DOWNLOAD_URL is used externally
DOWNLOAD_URL="https://github.com/9SMTM6/shuthost/releases/latest/download"
