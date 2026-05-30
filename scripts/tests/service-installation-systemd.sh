#!/bin/sh

# Test service installation on systemd (Ubuntu-like)

set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# Only change into the repository root during local testing when the
# repository marker exists adjacent to this script.
if [ -f "$SCRIPT_DIR/../helpers.sh" ] || [ -f "$SCRIPT_DIR/helpers.sh" ]; then
    cd "$SCRIPT_DIR/../.."
fi

. ./scripts/helpers.sh

if [ -n "$1" ]; then
    directory="./target/x86_64-unknown-linux-gnu/debug"
    mkdir -p ${directory}
    cp "$1" "${directory}/shuthost_coordinator"
else
    build_gnu
fi

# Build the container
 docker build --pull="newer" -f scripts/tests/Containerfile.systemd -t shuthost-test-systemd .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --workdir /repo --env-file scripts/tests/coverage.env shuthost-test-systemd /bin/sh -c "
./scripts/tests/coordinator_and_agent_service_installation.sh ./target/debug/shuthost_coordinator
"
