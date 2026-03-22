#!/bin/sh

set -e

# Assume coordinator is already running on localhost:$PORT
PORT=${PORT:-8080}
# Wait for coordinator to be ready (timeout 30s)
for i in $(seq 1 30); do
  curl -fsS -o /dev/null http://localhost:$PORT/login && break || sleep 1
done
curl -fsS -o /dev/null http://localhost:$PORT/login || { echo "Controller service is not running"; exit 1; }

# Determine config path (assuming root)
if [ -z "$CONFIG_PATH" ]; then
  echo "CONFIG_PATH not set"
  exit 1
fi

# Install and run client
CLIENT_INSTALL_OUT=$(curl -fsSL http://localhost:$PORT/download/client_installer.sh | sh -s http://localhost:$PORT 2>&1 || true)
echo "--- installer output start ---"
echo "$CLIENT_INSTALL_OUT"
CLIENT_PATH=$(echo "$CLIENT_INSTALL_OUT" | grep -Eo '/[^ ]*shuthost_client_[^ ]*' | head -n1)
echo "Detected client path: $CLIENT_PATH"
if [ -z "$CLIENT_PATH" ]; then
  echo "Client path not found in installer output"; exit 1
fi

# Parse the client config line from installer output and add to config
clientConfigLine=$(echo "$CLIENT_INSTALL_OUT" | grep '^".*" = { shared_secret = ".*" }$')
if [ -n "$clientConfigLine" ]; then
  echo "Config file before modification:"
  cat "$CONFIG_PATH"
  # Add the host if not present
  if ! grep -q 'testhost' "$CONFIG_PATH"; then
    sed -i.bak '/^\[hosts\]$/a testhost = { ip = "127.0.0.1", mac = "disableWOL", port = 9000, shared_secret = "testsecret" }' "$CONFIG_PATH" && rm "$CONFIG_PATH.bak"
  fi
  # Add the client under [clients]
  sed -i.bak '/^\[clients\]$/a '"$clientConfigLine" "$CONFIG_PATH" && rm "$CONFIG_PATH.bak"
  echo "Added client to config: $clientConfigLine"
  echo "Config file after modification:"
  cat "$CONFIG_PATH"
  # Wait for config reload
  sleep 5
else
  echo "Could not parse client config line from installer output"; exit 1
fi

# Run the client and capture output.
# The client is now registered, so the request should succeed.
# We want to test that the request is sent correctly and succeeds
OUTPUT=$($CLIENT_PATH take testhost --async 2>&1)
echo "--- client output start ---"
echo "$OUTPUT"
echo "--- client output end ---"

# Ensure the request succeeded (no error messages)
if echo "$OUTPUT" | grep -iq "error\|failed\|not found"; then
  echo 'Client request failed!';
  exit 1
fi

echo "Test passed"