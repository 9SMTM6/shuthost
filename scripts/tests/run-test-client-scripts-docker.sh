#!/bin/bash

set -e

# This script runs the test-client-scripts-linux.sh in Docker containers for Ubuntu and Alpine
# It starts the coordinator on the host and connects from containers

# Find a free port
FREE_PORT=8080
while nc -z 127.0.0.1 $FREE_PORT 2>/dev/null; do
  FREE_PORT=$((FREE_PORT + 1))
done

# Find a free UDP port
FREE_UDP_PORT=8081
while nc -u -z 127.0.0.1 $FREE_UDP_PORT 2>/dev/null; do
  FREE_UDP_PORT=$((FREE_UDP_PORT + 1))
done

# Create config in temporary directory
TEMP_CONFIG_DIR=$(mktemp -d /tmp/shuthost_config.XXXXXX)
CONFIG_FILE="$TEMP_CONFIG_DIR/config.toml"
cat > "$CONFIG_FILE" << EOF
[server]
port = $FREE_PORT
bind = "0.0.0.0"
broadcast_port = $FREE_UDP_PORT

[hosts]

[clients]
EOF

# Start coordinator on host using cargo
cargo build --bin shuthost_coordinator
cargo run --bin shuthost_coordinator -- control-service --config "$CONFIG_FILE" &
COORD_PID=$!

# Wait for coordinator to be ready
for i in $(seq 1 30); do
  curl -fsS -o /dev/null http://localhost:$FREE_PORT/login && break || sleep 1
done
curl -fsS -o /dev/null http://localhost:$FREE_PORT/login || { echo "Controller service is not running"; kill $COORD_PID; exit 1; }

# Function to run test in container
run_test() {
  local distro=$1
  local image=$2
  local setup_cmd=$3

  echo "Testing $distro"
  docker run --rm --network host -v "$(pwd)":/workspace -v "$TEMP_CONFIG_DIR:/root/.config/shuthost_coordinator" -e PORT=$FREE_PORT -e CONFIG_PATH=/root/.config/shuthost_coordinator/config.toml "$image" /bin/sh -c "
$setup_cmd
cd /workspace && sh scripts/tests/test-client-scripts-linux.sh
"
}

# Test Ubuntu
run_test "Ubuntu" "ubuntu:24.04" "
apt update && apt install -y curl bsdmainutils
"

# Test Alpine
run_test "Alpine" "docker.io/heywoodlh/openrc:latest" "
apk update
apk add --no-cache curl openssl busybox-extras doas util-linux
echo 'permit nopass root' >> /etc/doas.conf
"

# Kill coordinator
kill $COORD_PID

# Cleanup
rm -rf "$TEMP_CONFIG_DIR"

echo "All tests passed"