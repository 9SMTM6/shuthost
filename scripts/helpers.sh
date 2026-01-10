#!/bin/sh

build_musl() {
    docker build -t shuthost-builder -f scripts/build.Containerfile .
    docker run --rm \
        -v "$(pwd):/src" \
        -v "$HOME/.cargo/registry:/usr/local/cargo/registry" \
        -v "$HOME/.cargo/git:/usr/local/cargo/git" \
        shuthost-builder sh -c "\
            cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl &&\
            cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-musl --features=include_linux_musl_x86_64_agent\
        "
}

elevate_privileges() {
    cmd="$*"
    if command -v sudo >/dev/null 2>&1; then
        # shellcheck disable=SC2086
        sudo $cmd
    elif command -v doas >/dev/null 2>&1; then
        # shellcheck disable=SC2086
        doas $cmd
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
    if [ "${CI_MODE:-false}" = true ]; then
        echo "CI mode: Skipping checksum verification prompt."
        return
    fi
    echo "Please verify this checksum against the one provided for $FILENAME on the releases page:"
    echo "$BASE_URL"
    echo
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
}

embed_script() {
    script_file="$1"
    tag="$2"
    while IFS= read -r line; do
        case "$line" in
        ". "*)
            sourced="${line#". "}"
            if [ -f "$sourced" ]; then
                if [ "$sourced" = "./scripts/helpers.sh" ]; then
                    embed_script "$sourced" "$tag"
                    if [ -n "$tag" ]; then
                        echo "BASE_URL=\"https://github.com/9SMTM6/shuthost/releases/tag/${tag}\""
                        echo "DOWNLOAD_URL=\"https://github.com/9SMTM6/shuthost/releases/download/${tag}\""
                    fi
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
    tag="$1"
    mkdir -p target/scripts
    find scripts -name "*.sh" -type f | while read -r file; do
        relative="${file#scripts/}"
        mkdir -p "target/scripts/$(dirname "$relative")"
        embed_script "$file" "$tag" > "target/scripts/$relative"
        chmod +x "target/scripts/$relative"
    done
}

# provide default values for BASE_URL and DOWNLOAD_URL for when helpers.sh is sourced and not embedded
BASE_URL="https://github.com/9SMTM6/shuthost/releases/latest/"
# shellcheck disable=SC2034 # DOWNLOAD_URL is used externally
DOWNLOAD_URL="https://github.com/9SMTM6/shuthost/releases/latest/download"
