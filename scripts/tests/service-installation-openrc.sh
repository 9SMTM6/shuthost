#!/bin/sh

# Test service installation on OpenRC (Alpine-like)

set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# Only change into the repository root during local testing when the
# repository marker exists adjacent to this script.
if [ -f "$SCRIPT_DIR/../helpers.sh" ] || [ -f "$SCRIPT_DIR/helpers.sh" ]; then
    cd "$SCRIPT_DIR/../.."
fi

. ./scripts/helpers.sh

if [ -n "$1" ]; then
    directory="./target/x86_64-unknown-linux-musl/debug"
    mkdir -p ${directory}
    cp "$1" "${directory}/shuthost_coordinator"
else
    build_musl
fi

# Build the container
docker build -f scripts/tests/Containerfile.alpine -t shuthost-test-alpine .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --workdir /repo --env-file scripts/tests/coverage.env shuthost-test-alpine /bin/sh -c "
./scripts/tests/coordinator_and_agent_service_installation.sh ./target/debug/shuthost_coordinator
# rc-service shuthost_host_agent status
"
