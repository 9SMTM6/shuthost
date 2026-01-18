#!/bin/sh
# Wrapper script for common.sh using openrc base image.

set -e

. ./scripts/snapshot_files/common.sh
. ./scripts/helpers.sh

# Configuration
CONTAINERFILE="scripts/snapshot_files/Containerfile.openrc"
RESTART_CMD="rc-service shuthost_coordinator restart"
STOP_CMD="rc-service shuthost_coordinator stop"
BASE_IMAGE="shuthost-openrc"
OUTPUT_DIR="./tests/install-file-snapshots/openrc"

if [ -n "$1" ]; then
    directory="./target/x86_64-unknown-linux-musl/debug"
    mkdir -p ${directory}
    cp "$1" "${directory}/shuthost_coordinator"
else
    build_musl
fi

trap cleanup EXIT

do_snapshot

do_diff
