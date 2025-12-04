#!/bin/sh
# Wrapper script for snapshot-files-container.sh using openrc base image.

set -e

# Build the binaries in a container
podman build -t shuthost-builder -f scripts/Containerfile.build .
podman run --rm -v "$(pwd)/target:/host-target" shuthost-builder sh -c "cp -r /src/target/* /host-target/"

./scripts/snapshot-files-container.sh "docker.io/heywoodlh/openrc:latest" "apk update && apk add curl patch file" "./install-file-snapshots/openrc" "./target/x86_64-unknown-linux-musl/release/shuthost_coordinator" "openrc"
