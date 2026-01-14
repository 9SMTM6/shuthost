#!/bin/sh
# Snapshot files created during shuthost installation using docker-compose in podman-in-podman (rootful).
# Creates images at different steps and diffs filesystem changes.
# Requires podman.

set -ev

. ./scripts/snapshot_files/common.sh
. ./scripts/helpers.sh

if [ -n "$1" ]; then
    # we need this here for the build of the container in the compose setup
    directory="./target/x86_64-unknown-linux-musl/debug"
    mkdir -p ${directory}
    cp "$1" "${directory}/shuthost_coordinator"
else
    build_musl
fi

# Configuration
OUTPUT_DIR="./install-file-snapshots/compose-and-self-extracting"
BASE_IMAGE="shuthost-compose"

trap cleanup EXIT

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Build base image with dependencies
podman build -f scripts/snapshot_files/Containerfile.compose -t "$BASE_IMAGE" .

# Run container from base image with privileged for podman-in-podman
podman run -d -t --privileged --name "temp-$BASE_IMAGE-container" "$BASE_IMAGE" sleep infinity

# Copy config file and modify it
podman cp docs/examples/example_config.toml "temp-$BASE_IMAGE-container":/workspace/coordinator_config/config.toml
podman exec -w /workspace "temp-$BASE_IMAGE-container" sed -i 's/# \[server\.tls\]/[server.tls]/' coordinator_config/config.toml
podman exec -w /workspace "temp-$BASE_IMAGE-container" chmod 600 coordinator_config/config.toml

# Copy docker-compose file and modify it to build locally
podman cp docs/examples/docker-compose.yml "temp-$BASE_IMAGE-container":/workspace/docker-compose.yml
podman exec -w /workspace "temp-$BASE_IMAGE-container" sed -i 's|image: ghcr.io/9smtm6/shuthost/shuthost-coordinator:latest|build: .|' docker-compose.yml

# Run podman-compose up
podman exec -w /workspace "temp-$BASE_IMAGE-container" sh -c "podman compose up -d"

# Wait a bit for the service to start
sleep 5

# Commit to coordinator image
podman commit "temp-$BASE_IMAGE-container" "$BASE_IMAGE-coordinator-installed"

# Now install the agent in the same container
# This will end up installing the self-extracting-shell agent, since it can't detect an init system in this container.
# We override the OS to linux-musl since the coordinator we built only contains that agent.
# TODO also test:
  # curl -k -fsSLO 'https://localhost:8081/download/host_agent_installer.ps1'; pwsh -ExecutionPolicy Bypass -File ./host_agent_installer.ps1 https://localhost:8081 --os linux-musl &&
  # curl -k -fsSL https://localhost:8080/download/host_agent_installer.sh | sh -s https://localhost:8080 --os linux-musl --init-system=self-extracting-pwsh
podman exec -w /workspace "temp-$BASE_IMAGE-container" sh -c "
  curl -k -fsSL https://localhost:8080/download/host_agent_installer.sh | sh -s https://localhost:8080 --os linux-musl &&
  curl -k -fsSLO 'https://localhost:8080/download/host_agent_installer.ps1'; pwsh -ExecutionPolicy Bypass -File ./host_agent_installer.ps1 https://localhost:8080 --os linux-musl --init-system=self-extracting-pwsh
" || true

# Commit to final installed image
podman commit "temp-$BASE_IMAGE-container" "$BASE_IMAGE-agent-installed"

# Generate direct control scripts
#  we need to specify the output path, otherwise it'll contain the randomly generated docker hostname
podman exec "temp-$BASE_IMAGE-container" sh -c "
    ./shuthost_host_agent_self_extracting generate-direct-control --output /root/shuthost_direct_control &&
    ./shuthost_host_agent_self_extracting.ps1 generate-direct-control --output /root/shuthost_direct_control --type pwsh
"

# Commit to direct control installed image
podman commit "temp-$BASE_IMAGE-container" "$BASE_IMAGE-direct-control-installed"

# Now install the client in the same container
podman exec -w /workspace "temp-$BASE_IMAGE-container" sh -c "
  curl -k -sSL https://localhost:8080/download/client_installer.sh | sh -s https://localhost:8080 test-client &&
  curl -k -sSLO https://localhost:8080/download/client_installer.ps1; pwsh -ExecutionPolicy Bypass -File ./client_installer.ps1 https://localhost:8080 test-client &&
  echo 'Client installer completed, killing coordinator...' &&
  podman compose down
" || true

# Commit to client installed image
podman commit "temp-$BASE_IMAGE-container" "$BASE_IMAGE-client-installed"

# Clean up the container
podman rm --force -t 1 "temp-$BASE_IMAGE-container" >/dev/null 2>&1

do_diff

# Diff client files
process_diff "$BASE_IMAGE-client-installed" "$BASE_IMAGE-direct-control-installed" "./install-file-snapshots/client.toml"
