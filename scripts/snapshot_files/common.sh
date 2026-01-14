#!/bin/sh

# Common functions for snapshot scripts

exec_with_coverage() {
    podman exec --env-file scripts/tests/coverage.env -w /workspace "temp-$BASE_IMAGE-container" "$@"
}

commit_snapshot() {
    podman commit "temp-$BASE_IMAGE-container" "$1"
}

# Use with trap cleanup EXIT
cleanup() {
    echo "Cleaning up..."
    podman rm --force -t 1 "temp-$BASE_IMAGE-container" >/dev/null 2>&1 || true
    # don't delete the built image to speed up repeated runs
    podman rmi "$BASE_IMAGE" "$BASE_IMAGE-coordinator-installed" "$BASE_IMAGE-agent-installed" "$BASE_IMAGE-direct-control-installed" "$BASE_IMAGE-client-installed" >/dev/null 2>&1 || true
}

do_snapshot() {
    # Expects CONTAINERFILE, RESTART_CMD, STOP_CMD, BASE_IMAGE, OUTPUT_DIR to be set

    # Ensure output directory exists
    mkdir -p "$OUTPUT_DIR"

    # Build the base image from Containerfile
    podman build -f "$CONTAINERFILE" -t "$BASE_IMAGE-built" .

    podman run -d -t --rm --privileged -v "$(pwd)":/repo --name "temp-$BASE_IMAGE-container" "$BASE_IMAGE-built" sleep infinity
    commit_snapshot "$BASE_IMAGE"

    # Install the coordinator
    exec_with_coverage //workspace/shuthost_coordinator install root

    # Set environment for the service
    if [ "$BASE_IMAGE" = "shuthost-systemd" ]; then
        exec_with_coverage sed -i '/\[Service\]/a Environment=LLVM_PROFILE_FILE=/repo/target/shuthost-%p-%16m.profraw' /etc/systemd/system/shuthost_coordinator.service
    elif [ "$BASE_IMAGE" = "shuthost-openrc" ]; then
        exec_with_coverage sed -i '2a export LLVM_PROFILE_FILE=/repo/target/shuthost-%p-%16m.profraw' /etc/init.d/shuthost_coordinator
    fi

    # Enable TLS in the config
    exec_with_coverage sed -i 's/# \[server\.tls\]/[server.tls]/' /home/root/.config/shuthost_coordinator/config.toml

    # Restart the service if restart_cmd provided
    if [ -n "$RESTART_CMD" ]; then
        exec_with_coverage sh -c "$RESTART_CMD" || true
        sleep 2
    fi

    # Commit to coordinator installed image
    commit_snapshot "$BASE_IMAGE-coordinator-installed"

    # Now install the agent in the same container
    exec_with_coverage sh -c "
      curl -k -fsSL https://localhost:8080/download/host_agent_installer.sh | sh -s https://localhost:8080 &&
      echo 'Installer completed, killing coordinator...'
      $STOP_CMD
    " || true

    # Commit to agent installed image
    commit_snapshot "$BASE_IMAGE-agent-installed"

    # Generate direct control script
    #  we need to specify the output path, otherwise it'll contain the randomly generated docker hostname
    exec_with_coverage shuthost_host_agent generate-direct-control --output /root/shuthost_direct_control

    # Commit to final installed image
    commit_snapshot "$BASE_IMAGE-direct-control-installed"

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
            case \"\$path\" in /run/*|/var/run/*|/var/cache/*|/tmp/*|/root/.cache/*|/var/log/journal/*|/*/.updated|*.profraw) continue ;; esac
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
    process_diff "$BASE_IMAGE-coordinator-installed" "$BASE_IMAGE" "$OUTPUT_DIR/coordinator.toml"
    process_diff "$BASE_IMAGE-agent-installed" "$BASE_IMAGE-coordinator-installed" "$OUTPUT_DIR/agent.toml"
    process_diff "$BASE_IMAGE-direct-control-installed" "$BASE_IMAGE-agent-installed" "$OUTPUT_DIR/direct_control.toml"
    echo "Cleaned file lists with permissions and types saved to $OUTPUT_DIR/"
}
