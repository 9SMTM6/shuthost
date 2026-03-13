#!/bin/sh

set -eu

# Parse command line options
TAG=""
BRANCH=""
while getopts "t:b:h" opt; do
    case $opt in
        t) TAG="$OPTARG" ;;
        b) BRANCH="$OPTARG" ;;
        h) echo "Usage: $0 [-t tag] [-b branch] [-h] [-- <binary-args>]"
           echo "Install ShutHost coordinator binary."
           echo "Options:"
           echo "  -t tag       Specify a release tag to download."
           echo "  -b branch    Specify a branch; tag will be 'nightly_release<branch>'."
           echo "  -h           Show this help message."
           echo "  -- <args>    Pass additional arguments to the coordinator install subcommand."
           echo "               See repository path: docs/examples/cli_help_output/coordinator_install.txt for subcommand help."
           echo "If no options, defaults to latest release."
           exit 0 ;;
        *) echo "Usage: $0 [-t tag] [-b branch] [-h] [-- <binary-args>]" >&2; exit 1 ;;
    esac
done

# Parse binary args
BINARY_ARGS=""
if [ $# -gt 0 ] && [ "$1" = "--" ]; then
    shift
    BINARY_ARGS="$@"
fi

# Helper script to install the ShutHost coordinator binary

# This embeds the script during the release process. 
# That build script then gets released as an asset, with a tagged download URL.
. ./scripts/helpers.sh

FILENAME=""
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

# Construct download URL and filename
FILENAME="shuthost_coordinator-${TARGET_TRIPLE}.tar.gz"
DOWNLOAD_FILE_URL="${DOWNLOAD_URL}/${FILENAME}"

echo "Downloading binary from $DOWNLOAD_FILE_URL ..."

curl -fLO "$DOWNLOAD_FILE_URL"

verify_checksum

# Extract the archive
tar -xzf "$FILENAME"

# Run the installer
run_as_elevated ./shuthost_coordinator install "$(whoami)" $BINARY_ARGS

set +v

echo "Installation complete!"
echo "Access the WebUI at http://localhost:8080"
echo
