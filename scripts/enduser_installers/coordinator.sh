#!/bin/sh

set -eu

# Print help (does not exit)
print_help() {
    echo "Usage: $0 [-t tag] [-b branch] [-h] [-- <binary-args>]"
    echo "Install ShutHost coordinator binary."
    echo "Options:"
    echo "  -t tag       Specify a release tag to download."
    echo "  -b branch    Specify a branch; tag will be 'nightly_release<branch>'."
    echo "  -h           Show this help message."
    echo "  -- <args>    Pass additional arguments to the coordinator install subcommand."
    echo "               See repository path: docs/examples/cli_help_output/coordinator_install.txt for subcommand help."
    echo "If no options, defaults to latest release."
}

# Parse command line options
TAG=""
BRANCH=""
while getopts "t:b:h" opt; do
    case $opt in
        t) TAG="$OPTARG" ;;
        b) BRANCH="$OPTARG" ;;
        h) print_help; exit 0 ;;
        *) echo "Invalid option" >&2; print_help; exit 1 ;;
    esac
done

# Shift away the options parsed by getopts so remaining args start at first non-option.
# getopts leaves OPTIND pointing to the next positional argument. We subtract 2 here
# because we want to keep the optional "--" separator in the remaining args (so
# we can validate that the user passed it).
#
# `shift` rejects negative values; when no options are provided, OPTIND=1, so the
# computed shift count would be negative. We attempt the desired shift and fall back
# to `shift 0` if the computed value is invalid.
shift "$((OPTIND - 2))" 2>/dev/null || shift 0

# Parse binary args (remaining args after literal --)
BINARY_ARGS=""
if [ $# -gt 0 ]; then
    if [ "$1" = "--" ]; then
        shift
        BINARY_ARGS="$*"
    else
        echo "Unexpected arguments: $*" >&2
        print_help
        exit 1
    fi
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

detect_platform

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

# Construct download URL and filename
FILENAME="shuthost_coordinator-${TARGET_TRIPLE}.tar.gz"
DOWNLOAD_FILE_URL="${DOWNLOAD_URL}/${FILENAME}"


echo "$ARCH"

echo "$PLATFORM"

echo "$BINARY_ARGS"

echo "$TAG"

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
