#!/bin/sh

set -eu

# Helper script to install the ShutHost host agent binary

. ./scripts/helpers.sh

cleanup() {
    rm -f "$FILENAME" shuthost_host_agent
}

trap cleanup EXIT

echo "ShutHost Host Agent Binary Installer"
echo "===================================="
echo

set -v

detect_platform

echo "Detected platform: $TARGET_TRIPLE"
echo

# Construct download URL and filename
FILENAME="shuthost_host_agent-${TARGET_TRIPLE}.tar.gz"
DOWNLOAD_FILE_URL="${DOWNLOAD_URL}/${FILENAME}"

curl -fLO "$DOWNLOAD_FILE_URL"

verify_checksum

# Extract the archive
tar -xzf "$FILENAME"

# Run the installer
run_as_elevated ./shuthost_host_agent install

set +v

echo "Installation complete!"
echo