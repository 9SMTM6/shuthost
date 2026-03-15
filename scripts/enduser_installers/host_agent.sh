#!/bin/sh

set -eu

print_help() {
    echo "Usage: $0 [-t tag] [-b branch] [-h] [-- <binary-args>]"
    echo "Install ShutHost host agent binary."
    echo "Options:"
    echo "  -t tag       Specify a release tag to download."
    echo "  -b branch    Specify a branch; tag will be 'nightly_release<branch>'."
    echo "  -h           Show this help message."
    echo "  -- <args>    Pass additional arguments to the agent install subcommand."
    echo "               See repository path: docs/examples/cli_help_output/host_agent_install_linux.txt for subcommand help."
    echo "               Note: init-system options may differ by platform, but the default is usually correct."
    echo "If no options, defaults to latest release."
}

# Helper script to install the ShutHost host agent binary

# This script sources helpers.sh for utility functions and can be configured
# with command-line flags to specify a release tag or branch.
. ./scripts/helpers.sh

FILENAME=""
cleanup() {
    rm -f "$FILENAME" shuthost_host_agent
}

trap cleanup EXIT

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

# Shift away the options parsed by getopts so remaining args start at first non-option
shift "$((OPTIND - 2))" # IDK why this needs to be reduced by 2, but thats what works for me.

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

echo "ShutHost Host Agent Binary Installer"
echo "===================================="
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

detect_platform

set -v

# Construct download URL and filename
FILENAME="shuthost_host_agent-${TARGET_TRIPLE}.tar.gz"
DOWNLOAD_FILE_URL="${DOWNLOAD_URL}/${FILENAME}"

echo "$TAG"

echo "$ARCH"

echo "$PLATFORM"

echo "$BINARY_ARGS"

echo "Downloading binary from $DOWNLOAD_FILE_URL ..."

curl -fLO "$DOWNLOAD_FILE_URL"

verify_checksum

# Extract the archive
tar -xzf "$FILENAME"

# Run the installer
run_as_elevated ./shuthost_host_agent install $BINARY_ARGS

set +v

echo "Installation complete!"
echo
