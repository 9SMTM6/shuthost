#!/bin/sh
# Snapshot files created during shuthost installation using docker-compose in podman-in-podman (rootful).
# Creates images at different steps and diffs filesystem changes.
# Requires podman.

set -e

# Configuration
BASE_IMAGE="quay.io/podman/stable"
INSTALL_DEPS="dnf install -y podman-compose curl hostname file"
OUTPUT_DIR="./install-file-snapshots/docker-compose"
BASE_IMAGE_NAME="shuthost-compose-base"
COORDINATOR_RUNNING_NAME="shuthost-compose-coordinator-running"
AGENT_INSTALLED_NAME="shuthost-compose-agent-installed"
CONTAINER_NAME="temp-container"

# Cleanup function
cleanup() {
    echo "Cleaning up..."
    podman rm --force -t 1 "$CONTAINER_NAME" >/dev/null 2>&1 || true
    podman rmi "$BASE_IMAGE_NAME" "$COORDINATOR_RUNNING_NAME" "$AGENT_INSTALLED_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Run container from base image with privileged for podman-in-podman
podman run -d -t --privileged --name "$CONTAINER_NAME" "$BASE_IMAGE" sleep infinity

# Install dependencies in the container
podman exec "$CONTAINER_NAME" sh -c "$INSTALL_DEPS"

# Create workspace directory with relative structure
podman exec "$CONTAINER_NAME" mkdir -p /workspace/coordinator_config /workspace/data /workspace/target/x86_64-unknown-linux-musl/release

# Copy the binary
podman cp target/x86_64-unknown-linux-musl/release/shuthost_coordinator "$CONTAINER_NAME":/workspace/target/x86_64-unknown-linux-musl/release/shuthost_coordinator

# Copy Containerfile
podman cp Containerfile "$CONTAINER_NAME":/workspace/Containerfile

# Commit to base image
podman commit "$CONTAINER_NAME" "$BASE_IMAGE_NAME"

# Copy config file and modify it
podman cp docs/examples/example_config.toml "$CONTAINER_NAME":/workspace/coordinator_config/config.toml
podman exec -w /workspace "$CONTAINER_NAME" sed -i 's/# \[server\.tls\]/[server.tls]/' coordinator_config/config.toml
podman exec -w /workspace "$CONTAINER_NAME" chmod 600 coordinator_config/config.toml

# Copy docker-compose file and modify it to build locally
podman cp docs/examples/docker-compose.yml "$CONTAINER_NAME":/workspace/docker-compose.yml
podman exec -w /workspace "$CONTAINER_NAME" sed -i 's|image: ghcr.io/9smtm6/shuthost/shuthost-coordinator:latest|build: .|' docker-compose.yml

# Run podman-compose up
podman exec -w /workspace "$CONTAINER_NAME" sh -c "podman compose up -d"

# Wait a bit for the service to start
sleep 5

# Commit to coordinator running image
podman commit "$CONTAINER_NAME" "$COORDINATOR_RUNNING_NAME"

# Now install the agent in the same container (this will end up installing the serviceless agent, since it cant detect an init system in this container)
podman exec -w /workspace "$CONTAINER_NAME" sh -c "
  curl -k -fsSL https://localhost:8080/download/host_agent_installer.sh | sh -s https://localhost:8080 --os linux-musl &&
  echo 'Installer completed, killing coordinator...'
  podman compose down
" || true

# Commit to final installed image
podman commit "$CONTAINER_NAME" "$AGENT_INSTALLED_NAME"

# Clean up the container
podman rm --force -t 1 "$CONTAINER_NAME" >/dev/null 2>&1

echo "Diffing filesystem changes..."

# Get diff output
process_diff() {
    image="$1"
    base_image="$2"
    output_file="$3"
    temp_file=$(mktemp)
    
    # Get diff plain text and extract added paths
    podman image diff "$image" "$base_image" | grep '^A' | sed 's/^A //' | sort > "$temp_file"
    
    # Mount temp file and get metadata
    podman run --rm -v "$temp_file":/tmp/paths:ro --entrypoint /bin/sh "$image" -c "
        while read -r path; do
            case \"\$path\" in /run/*|/var/run/*|/var/cache/*|/tmp/*) continue ;; esac
            if [ -f \"\$path\" ]; then
                perms=\$(stat -c '%a' \"\$path\")
                ftype=\$(file -b \"\$path\" | cut -d, -f1)
                echo '[[files]]'
                echo \"path = \\\"\$path\\\"\"
                echo \"perms = \\\"\$perms\\\"\"
                echo \"type = \\\"\$ftype\\\"\"
                echo ''
            fi
        done < /tmp/paths
    " > "$output_file"
    
    rm "$temp_file"
}

process_diff "$COORDINATOR_RUNNING_NAME" "$BASE_IMAGE_NAME" "$OUTPUT_DIR/compose_coordinator_files.toml"
process_diff "$AGENT_INSTALLED_NAME" "$COORDINATOR_RUNNING_NAME" "$OUTPUT_DIR/compose_agent_files.toml"

echo "Cleaned file lists with permissions and types saved to $OUTPUT_DIR/"