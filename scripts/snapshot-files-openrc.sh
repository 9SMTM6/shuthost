#!/bin/sh
# Wrapper script for snapshot-files-container.sh using openrc base image.

set -e

. ./scripts/snapshot-files-common.sh

if [ -n "$1" ]; then
    HOST_BINARY="$1"
else
    build_musl
    HOST_BINARY="./target/x86_64-unknown-linux-musl/release/shuthost_coordinator"
fi

trap cleanup EXIT

do_snapshot "docker.io/heywoodlh/openrc:latest" "apk update && apk add curl patch file" "./install-file-snapshots/openrc" "$HOST_BINARY" "rc-service shuthost_coordinator restart" "rc-service shuthost_coordinator stop"

do_diff "./install-file-snapshots/openrc"
