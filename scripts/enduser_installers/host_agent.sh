#!/bin/sh

set -eu

# Helper script to install the ShutHost host agent binary

# This script sources helpers.sh for utility functions and can be configured
# with command-line flags to specify a release tag or branch.
. ./scripts/helpers.sh

cleanup() {
    rm -f "$FILENAME" shuthost_host_agent
}

trap cleanup EXIT

echo "ShutHost Host Agent Binary Installer"
echo "===================================="
echo

# Parse command line options
TAG=""
BRANCH=""
while getopts "t:b:h" opt; do
    case $opt in
        t) TAG="$OPTARG" ;;
        b) BRANCH="$OPTARG" ;;
        h) echo "Usage: $0 [-t tag] [-b branch] [-h]"
           echo "Install ShutHost host agent binary."
           echo "Options:"
           echo "  -t tag       Specify a release tag to download."
           echo "  -b branch    Specify a branch; tag will be 'nightly_release<branch>'."
           echo "  -h           Show this help message."
           echo "If no options, defaults to latest release."
           exit 0 ;;
        *) echo "Usage: $0 [-t tag] [-b branch] [-h]" >&2; exit 1 ;;
    esac
done

# Determine the tag
if [ -n "$BRANCH" ]; then
    TAG="nightly_release_$BRANCH"
fi

# Set URLs based on tag
if [ -n "$TAG" ]; then
    BASE_URL="https://github.com/9SMTM6/shuthost/releases/tag/$TAG"
    DOWNLOAD_URL="https://github.com/9SMTM6/shuthost/releases/download/$TAG"
else
    BASE_URL="https://github.com/9SMTM6/shuthost/releases/latest/"
    DOWNLOAD_URL="https://github.com/9SMTM6/shuthost/releases/latest/download"
fi

set -v

detect_platform

echo "Detected platform: $TARGET_TRIPLE"
echo

# Construct download URL and filename
FILENAME="shuthost_host_agent-${TARGET_TRIPLE}.tar.gz"
DOWNLOAD_FILE_URL="${DOWNLOAD_URL}/${FILENAME}"

echo "Downloading binary from $DOWNLOAD_FILE_URL ..."

curl -fLO "$DOWNLOAD_FILE_URL"

verify_checksum

# Extract the archive
tar -xzf "$FILENAME"

# Run the installer
run_as_elevated ./shuthost_host_agent install

set +v

echo "Installation complete!"
echo
