#!/bin/sh

# Test service installation on OpenRC (Alpine-like)

set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$SCRIPT_DIR/../.."

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
