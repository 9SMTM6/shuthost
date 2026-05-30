#!/bin/sh
# Run the coordinator installer update test inside an Alpine/OpenRC Docker container.
# This wrapper mimics the direct-control Alpine wrapper.

set -eu

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# Only change into the repository root during local testing when the
# repository marker exists adjacent to this script.
if [ -f "$SCRIPT_DIR/../helpers.sh" ] || [ -f "$SCRIPT_DIR/helpers.sh" ]; then
    cd "$SCRIPT_DIR/../.."
fi

. ./scripts/helpers.sh

if [ -n "${1:-}" ]; then
    mkdir -p ./target/x86_64-unknown-linux-musl/debug/
    cp "$1" ./target/x86_64-unknown-linux-musl/debug/shuthost_coordinator
else
    build_musl
fi

# Build the Alpine OpenRC test container and run the coordinator installer update test inside it.
 docker build --pull="newer" -f scripts/tests/Containerfile.alpine -t shuthost-installer-update-alpine .

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
