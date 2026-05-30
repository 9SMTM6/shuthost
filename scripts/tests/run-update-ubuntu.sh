#!/bin/sh
# Run the installer update tests inside a systemd Docker container.
# This wrapper mimics the direct-control test wrappers.

set -eu

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# Only change into the repository root during local testing when the
# repository marker exists adjacent to this script.
if [ -f "$SCRIPT_DIR/../helpers.sh" ] || [ -f "$SCRIPT_DIR/helpers.sh" ]; then
    cd "$SCRIPT_DIR/../.."
fi

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
docker build --pull -f scripts/tests/Containerfile.systemd -t shuthost-installer-update-systemd .

docker run --rm -t --privileged \
  -v "$(pwd)":/repo \
  --workdir /repo \
  --env-file scripts/tests/coverage.env \
  -e CI_MODE=true \
  shuthost-installer-update-systemd /bin/sh -c "./scripts/tests/installer-update-nix.sh ./target/x86_64-unknown-linux-gnu/debug/shuthost_coordinator"
