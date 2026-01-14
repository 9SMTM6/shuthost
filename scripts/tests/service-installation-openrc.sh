#!/bin/sh

# Test service installation on OpenRC (Alpine-like)

set -e

. ./scripts/helpers.sh

build_musl

# Build the container
docker build -f scripts/tests/Containerfile.alpine -t shuthost-test-alpine .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --workdir /repo --env-file scripts/tests/coverage.env shuthost-test-alpine /bin/sh -c "
./scripts/tests/coordinator_and_agent_service_installation.sh ./target/release/shuthost_coordinator
# rc-service shuthost_host_agent status
"
