#!/bin/sh

set -eu

# Helper script to install the ShutHost coordinator binary

# This embeds the script during the release process. 
# That build script then gets released as an asset, with a tagged download URL.
. ./scripts/helpers.sh

cleanup() {
    rm -f "$FILENAME" shuthost_coordinator
}

trap cleanup EXIT

echo "ShutHost Coordinator Binary Installer"
echo "====================================="
echo

set -v

detect_platform

echo "Detected platform: $TARGET_TRIPLE"
echo

# Construct download URL and filename
FILENAME="shuthost_coordinator-${TARGET_TRIPLE}.tar.gz"
DOWNLOAD_FILE_URL="${DOWNLOAD_URL}/${FILENAME}"

echo "Downloading binary from $DOWNLOAD_FILE_URL ..."

curl -fLO "$DOWNLOAD_FILE_URL"

verify_checksum

# Extract the archive
tar -xzf "$FILENAME"

# Run the installer
run_as_elevated ./shuthost_coordinator install "$(whoami)"

set +v

echo "Installation complete!"
echo "Access the WebUI at http://localhost:8080"
echo
