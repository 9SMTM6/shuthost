#!/bin/sh
# Run the installer update tests inside a systemd Docker container.
# This wrapper mimics the direct-control test wrappers.

set -eu

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$SCRIPT_DIR/../.."

. ./scripts/helpers.sh

if [ -n "${1:-}" ]; then
    directory="./target/x86_64-unknown-linux-gnu/debug"
    mkdir -p "$directory"
    src="$1"
    dest="$directory/shuthost_coordinator"
    if [ "$src" != "$dest" ]; then
        cp "$src" "$dest"
    fi

    mkdir -p ./target/x86_64-unknown-linux-musl/release
    cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-musl/release/shuthost_host_agent
else
    build_gnu
fi

# Build the systemd test container and run the coordinator installer update test inside it.
docker build -f scripts/tests/Containerfile.systemd -t shuthost-installer-update-systemd .

docker run --rm -t --privileged \
  -v "$(pwd)":/repo \
  --workdir /repo \
  --env-file scripts/tests/coverage.env \
  -e CI_MODE=true \
  shuthost-installer-update-systemd /bin/sh -c "./scripts/tests/installer-update-coordinator.sh ./target/x86_64-unknown-linux-gnu/debug/shuthost_coordinator"
