#!/bin/sh
# Run the coordinator installer update test inside an Alpine/OpenRC Docker container.
# This wrapper mimics the direct-control Alpine wrapper.

set -eu

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$SCRIPT_DIR/../.."

. ./scripts/helpers.sh

if [ -n "${1:-}" ]; then
    directory="./target/x86_64-unknown-linux-musl/debug"
    mkdir -p "$directory"
    src="$1"
    dest="$directory/shuthost_coordinator"
    if [ "$src" != "$dest" ]; then
        cp "$src" "$dest"
    fi

    mkdir -p ./target/x86_64-unknown-linux-musl/release
    cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-musl/release/shuthost_host_agent
else
    build_musl
fi

# Build the Alpine OpenRC test container and run the coordinator installer update test inside it.
docker build -f scripts/tests/Containerfile.alpine -t shuthost-installer-update-alpine .

docker run --rm -t --privileged \
  -v "$(pwd)":/repo \
  --workdir /repo \
  --env-file scripts/tests/coverage.env \
  -e CI_MODE=true \
  shuthost-installer-update-alpine /bin/sh -c "cat >/etc/network/interfaces <<'EOF'
auto lo
iface lo inet loopback
EOF
./scripts/tests/installer-update-nix.sh ./target/x86_64-unknown-linux-musl/debug/shuthost_coordinator"
