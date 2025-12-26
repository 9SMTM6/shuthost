#!/bin/sh

set -e

# Helper script to install the ShutHost coordinator binary
# Based on the installation steps from README.md

cleanup() {
    rm -f "$FILENAME" shuthost_coordinator
}

trap cleanup EXIT

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
    COMPUTED_CHECKSUM=$(shasum -a 256 "$FILENAME" | cut -d' ' -f1)
    echo "Computed checksum: $COMPUTED_CHECKSUM"
    echo
    echo "Please verify this checksum against the one provided on the releases page:"
    echo "https://github.com/9SMTM6/shuthost/releases/latest"
    echo
    printf "Have you verified the checksum? (y/n/C): "
    read REPLY
    echo
    case "$REPLY" in
        [Yy]*)
            ;;
        [Nn]*)
            echo "Checksum verification aborted. Installation cancelled."
            exit 1
            ;;
        *)
            ;;
    esac
}

echo "ShutHost Coordinator Binary Installer"
echo "====================================="
echo

set -v

detect_platform

echo "Detected platform: $TARGET_TRIPLE"
echo

# Construct download URL and filename
BASE_URL="https://github.com/9SMTM6/shuthost/releases/latest/download"
FILENAME="shuthost_coordinator-${TARGET_TRIPLE}.tar.gz"
DOWNLOAD_URL="${BASE_URL}/${FILENAME}"

curl -L -o "$FILENAME" "$DOWNLOAD_URL"

# Extract the archive
tar -xzf "$FILENAME"

verify_checksum

# Run the installer
sudo ./shuthost_coordinator install

set +v

echo "Installation complete!"
echo "Access the WebUI at http://localhost:8080"
echo
