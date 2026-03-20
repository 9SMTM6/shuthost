#!/bin/sh

set -eu

print_help() {
    echo "Usage: $0 [-t tag] [-b branch] [-i] [-h] [-- <binary-args>]"
    echo "Install ShutHost host agent binary."
    echo "Options:"
    echo "  -t tag       Specify a release tag to download."
    echo "  -b branch    Specify a branch; tag will be 'nightly_release<branch>'."
    echo "  -i           Pass --help to the host agent install subcommand and exit."
    echo "  -h           Show this help message."
    echo "  -- <args>    Pass additional arguments to the agent install subcommand."
    echo "               Use -i to see available install subcommand arguments."
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
INSTALL_HELP=false
while getopts "t:b:ih" opt; do
    case $opt in
        t) TAG="$OPTARG" ;;
        b) BRANCH="$OPTARG" ;;
        i) INSTALL_HELP=true ;;
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
# computed shift count would be negative.
shift_count=$((OPTIND - 2))
if [ "$shift_count" -lt 0 ]; then
    shift_count=0
fi
shift "$shift_count"

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

echo "$OS"

echo "$BINARY_ARGS"

echo "Downloading binary from $DOWNLOAD_FILE_URL ..."

curl -fLO "$DOWNLOAD_FILE_URL"

if ! $INSTALL_HELP; then
    verify_checksum
fi

# Extract the archive
tar -xzf "$FILENAME"

# Run the installer
if $INSTALL_HELP; then
    ./shuthost_host_agent install --help
else
    # shellcheck disable=SC2086
    run_as_elevated ./shuthost_host_agent install $BINARY_ARGS

    echo "Installation complete!"
    echo
fi
