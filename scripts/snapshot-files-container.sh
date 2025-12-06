#!/bin/sh
# Snapshot files created during shuthost installation using podman commands.
# Creates images at different steps and diffs filesystem changes.
# Requires podman and support for the x86_64-unknown-linux-gnu target to be built.

set -e

# Arguments: base_image install_deps_command base_output_dir host_binary restart_cmd
BASE_IMAGE="$1"
INSTALL_DEPS="$2"
OUTPUT_DIR="$3"
HOST_BINARY="$4"
RESTART_CMD="$5"

# Validate arguments
if [ -z "$BASE_IMAGE" ] || [ -z "$INSTALL_DEPS" ] || [ -z "$OUTPUT_DIR" ] || [ -z "$HOST_BINARY" ]; then
    echo "Usage: $0 <base_image> <install_deps_command> <output_dir> <host_binary> [restart_cmd]" >&2
    exit 1
fi
# Configuration
BASE_IMAGE_NAME="shuthost-base"
COORDINATOR_INSTALLED_NAME="shuthost-coordinator-installed"
AGENT_INSTALLED_NAME="shuthost-agent-installed"
CONTAINER_BINARY="/root/shuthost_coordinator"

# Cleanup function
cleanup() {
    echo "Cleaning up..."
    podman rm --force -t 1 temp-container >/dev/null 2>&1 || true
    podman rmi "$BASE_IMAGE_NAME" "$COORDINATOR_INSTALLED_NAME" "$AGENT_INSTALLED_NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Ensure output directory exists
mkdir -p "$OUTPUT_DIR"

# Run container from base image with privileged for systemd
podman run -d -t --rm --privileged --name temp-container "$BASE_IMAGE" sleep infinity

# Install curl in the container
podman exec temp-container sh -c "$INSTALL_DEPS"

# Copy the binary
podman cp "$HOST_BINARY" temp-container:"$CONTAINER_BINARY"

# Commit to base image
podman commit temp-container "$BASE_IMAGE_NAME"

# Install the coordinator
podman exec temp-container "$CONTAINER_BINARY" install root

# Enable TLS in the config
podman exec temp-container sed -i 's/# \[server\.tls\]/[server.tls]/' /home/root/.config/shuthost_coordinator/config.toml

# Restart the service if restart_cmd provided
if [ -n "$RESTART_CMD" ]; then
    podman exec temp-container sh -c "$RESTART_CMD" || true
    sleep 2
fi

# Commit to coordinator installed image
podman commit temp-container "$COORDINATOR_INSTALLED_NAME"

# Now install the agent in the same container
podman exec temp-container sh -c "
  curl -k -fsSL https://localhost:8080/download/host_agent_installer.sh | sh -s https://localhost:8080 &&
  echo 'Installer completed, killing coordinator...'
  pkill -f $CONTAINER_BINARY
" || true
# Commit to final installed image
podman commit temp-container "$AGENT_INSTALLED_NAME"

# Clean up the container
podman rm --force -t 1 temp-container >/dev/null 2>&1

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
            case \"\$path\" in /run/*) continue ;; esac
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

process_diff "$COORDINATOR_INSTALLED_NAME" "$BASE_IMAGE_NAME" "$OUTPUT_DIR/coordinator_files.toml"
process_diff "$AGENT_INSTALLED_NAME" "$COORDINATOR_INSTALLED_NAME" "$OUTPUT_DIR/agent_files.toml"

echo "Cleaned file lists with permissions and types saved to $OUTPUT_DIR/"
