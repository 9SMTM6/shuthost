#!/bin/sh

# Common functions for snapshot scripts

# Use with trap cleanup EXIT
cleanup() {
    echo "Cleaning up..."
    podman rm --force -t 1 "temp-$BASE_IMAGE-container" >/dev/null 2>&1 || true
    # don't delete the built image to speed up repeated runs
    podman rmi "$BASE_IMAGE" "$BASE_IMAGE-coordinator-installed" "$BASE_IMAGE-agent-installed" "$BASE_IMAGE-client-installed" >/dev/null 2>&1 || true
}

do_snapshot() {
    # Expects CONTAINERFILE, RESTART_CMD, STOP_CMD, BASE_IMAGE, OUTPUT_DIR to be set

    # Ensure output directory exists
    mkdir -p "$OUTPUT_DIR"

    # Build the base image from Containerfile
    podman build -f "$CONTAINERFILE" -t "$BASE_IMAGE-built" .

    podman run -d -t --rm --privileged --name "temp-$BASE_IMAGE-container" "$BASE_IMAGE-built" sleep infinity
    podman commit "temp-$BASE_IMAGE-container" "$BASE_IMAGE"

    # Install the coordinator
    podman exec "temp-$BASE_IMAGE-container" //workspace/shuthost_coordinator install root

    # Enable TLS in the config
    podman exec "temp-$BASE_IMAGE-container" sed -i 's/# \[server\.tls\]/[server.tls]/' /home/root/.config/shuthost_coordinator/config.toml

    # Restart the service if restart_cmd provided
    if [ -n "$RESTART_CMD" ]; then
        podman exec "temp-$BASE_IMAGE-container" sh -c "$RESTART_CMD" || true
        sleep 2
    fi

    # Commit to coordinator installed image
    podman commit "temp-$BASE_IMAGE-container" "$BASE_IMAGE-coordinator-installed"

    # Now install the agent in the same container
    podman exec "temp-$BASE_IMAGE-container" sh -c "
      curl -k -fsSL https://localhost:8080/download/host_agent_installer.sh | sh -s https://localhost:8080 &&
      echo 'Installer completed, killing coordinator...'
      $STOP_CMD
    " || true
    # Commit to final installed image
    podman commit "temp-$BASE_IMAGE-container" "$BASE_IMAGE-agent-installed"

    # Clean up the container
    podman rm --force -t 1 "temp-$BASE_IMAGE-container" >/dev/null 2>&1
}

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
            case \"\$path\" in /run/*|/var/run/*|/var/cache/*|/tmp/*|/root/.cache/*|/var/log/journal/*|/*/.updated) continue ;; esac
            if [ -f \"\$path\" ]; then
                perms=\$(stat -c '%a' \"\$path\")
                ftype=\$(file -b \"\$path\" | cut -d, -f1)
                # Handle known SQLite files that may appear empty
                case \"\$path\" in
                    */shuthost.db-wal)
                        if [ \"\$ftype\" = \"empty\" ]; then
                            ftype=\"SQLite Write-Ahead Log\"
                        fi
                        ;;
                    */shuthost.db-shm)
                        if [ \"\$ftype\" = \"empty\" ]; then
                            ftype=\"SQLite Write-Ahead Log shared memory\"
                        fi
                        ;;
                esac
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

do_diff() {
    echo "Diffing filesystem changes..."
    process_diff "$BASE_IMAGE-coordinator-installed" "$BASE_IMAGE" "$OUTPUT_DIR/coordinator_files.toml"
    process_diff "$BASE_IMAGE-agent-installed" "$BASE_IMAGE-coordinator-installed" "$OUTPUT_DIR/agent_files.toml"
    echo "Cleaned file lists with permissions and types saved to $OUTPUT_DIR/"
}
