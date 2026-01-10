#!/bin/sh
# Snapshot files created during shuthost installation using docker-compose in podman-in-podman (rootful).
# Creates images at different steps and diffs filesystem changes.
# Requires podman.

set -e

. ./scripts/snapshot_files/common.sh
. ./scripts/helpers.sh

if [ -n "$1" ]; then
    HOST_BINARY="$1"
else
    build_musl
    HOST_BINARY="./target/x86_64-unknown-linux-musl/release/shuthost_coordinator"
fi

# Configuration
BASE_IMAGE="quay.io/podman/stable"
INSTALL_DEPS="rpm -Uvh https://packages.microsoft.com/config/rhel/9/packages-microsoft-prod.rpm || true; dnf install -y podman-compose curl hostname file openssl powershell"
OUTPUT_DIR="./install-file-snapshots/docker-compose"
BASE_IMAGE_NAME="shuthost-compose-base"
COORDINATOR_INSTALLED_NAME="shuthost-compose-coordinator-installed"
AGENT_INSTALLED_NAME="shuthost-compose-agent-installed"
CLIENT_INSTALLED_NAME="shuthost-compose-client-installed"

trap cleanup EXIT

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Run container from base image with privileged for podman-in-podman
podman run -d -t --privileged --name temp-container "$BASE_IMAGE" sleep infinity

# Install dependencies in the container
podman exec temp-container sh -c "$INSTALL_DEPS"

# Create workspace directory with relative structure
podman exec temp-container mkdir -p /workspace/coordinator_config /workspace/data /workspace/target/x86_64-unknown-linux-musl/release

# Copy the binary
podman cp "$HOST_BINARY" temp-container:/workspace/target/x86_64-unknown-linux-musl/release/shuthost_coordinator

# Copy Containerfile
podman cp Containerfile temp-container:/workspace/Containerfile

# Commit to base image
podman commit temp-container "$BASE_IMAGE_NAME"

# Copy config file and modify it
podman cp docs/examples/example_config.toml temp-container:/workspace/coordinator_config/config.toml
podman exec -w /workspace temp-container sed -i 's/# \[server\.tls\]/[server.tls]/' coordinator_config/config.toml
podman exec -w /workspace temp-container chmod 600 coordinator_config/config.toml

# Copy docker-compose file and modify it to build locally
podman cp docs/examples/docker-compose.yml temp-container:/workspace/docker-compose.yml
podman exec -w /workspace temp-container sed -i 's|image: ghcr.io/9smtm6/shuthost/shuthost-coordinator:latest|build: .|' docker-compose.yml

# Run podman-compose up
podman exec -w /workspace temp-container sh -c "podman compose up -d"

# Wait a bit for the service to start
sleep 5

# Commit to coordinator image
podman commit temp-container "$COORDINATOR_INSTALLED_NAME"

# Now install the agent in the same container
# This will end up installing the serviceless agent, since it can't detect an init system in this container.
# We override the OS to linux-musl since the coordinator we built only contains that agent.
podman exec -w /workspace temp-container sh -c "
  curl -k -fsSL https://localhost:8080/download/host_agent_installer.sh | sh -s https://localhost:8080 --os linux-musl
" || true

# Commit to final installed image
podman commit temp-container "$AGENT_INSTALLED_NAME"

# Now install the client in the same container
podman exec -w /workspace temp-container sh -c "
  curl -k -sSL https://localhost:8080/download/client_installer.sh | sh -s https://localhost:8080 test-client &&
  curl -k -sSLO https://localhost:8080/download/client_installer.ps1; pwsh -ExecutionPolicy Bypass -File ./client_installer.ps1 https://localhost:8080 test-client &&
  echo 'Client installer completed, killing coordinator...' &&
  podman compose down
" || true

# Commit to client installed image
podman commit temp-container "$CLIENT_INSTALLED_NAME"

# Clean up the container
podman rm --force -t 1 temp-container >/dev/null 2>&1

do_diff "$OUTPUT_DIR"

# Diff client files
process_diff "$CLIENT_INSTALLED_NAME" "$AGENT_INSTALLED_NAME" "./install-file-snapshots/client_files.toml"
