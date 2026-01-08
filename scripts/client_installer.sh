#!/bin/sh

# This script installs the shuthost client by downloading the client template
# from the coordinator, generating a client ID and shared secret,
# filling in the template, and installing it locally.
# It connects the client to the coordinator it downloaded from.

set -e

# Default values
INSTALL_DIR="$HOME/.local/bin"
REMOTE_URL="${1:-http://localhost:8080}"

# Determine if we should accept self-signed certificates (for localhost/testing)
HOST=$(echo "$REMOTE_URL" | sed -e 's|^https*://||' -e 's|/.*$||' -e 's|:.*$||')
if [ "$HOST" = "localhost" ] || echo "$HOST" | grep -q '^127\.'; then
    CURL_OPTS="-k"
else
    CURL_OPTS=""
fi

# Ensure the installation directory exists
if [ ! -d "$INSTALL_DIR" ]; then
  echo "Creating installation directory: $INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"
fi

# Check if the installation directory is in PATH
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  echo "Warning: $INSTALL_DIR is not in your PATH."
  echo "You may need to add it to your PATH to use the installed script easily."
  echo "To do so, add the following line to your shell configuration file:"
  echo "export PATH=\$PATH:$INSTALL_DIR"
fi

# Word lists for generating readable client IDs
ADJECTIVES="red blue swift calm bold wise kind brave"
NOUNS="fox bird wolf bear lion deer hawk eagle"

# Generate a random client ID using word lists
RANDOM_PART=$(echo "$ADJECTIVES" | tr ' ' '\n' | awk 'BEGIN{srand()}{print rand() "\t" $0}' | sort -n | cut -f2- | head -n1)_$(echo $NOUNS | tr ' ' '\n' | awk 'BEGIN{srand()}{print rand() "\t" $0}' | sort -n | cut -f2- | head -n1)

if [ -n "$2" ]; then
    BASE_ID="$2"
else
    BASE_ID="$RANDOM_PART"
fi

SUBDOMAIN=$(hostname | cut -d. -f1)
CLIENT_ID="${SUBDOMAIN}_${BASE_ID}"

CLIENT_SCRIPT_NAME="shuthost_client_${BASE_ID}"

################## Boring setup complete ------------- Interesting stuff is starting here

# Download the client script template
echo "Downloading client script template..."
set -v
echo "$REMOTE_URL"

echo "$CLIENT_ID"


curl --compressed -L --fail-with-body $CURL_OPTS "${REMOTE_URL}/download/shuthost_client.sh" -o "/tmp/$CLIENT_SCRIPT_NAME.tmpl"

# Generate a random shared secret using openssl
SHARED_SECRET=$(openssl rand -hex 16)

# Replace placeholders in the script:
cat "/tmp/$CLIENT_SCRIPT_NAME.tmpl" | \
  awk -v client_id="$CLIENT_ID" -v shared_secret="$SHARED_SECRET" -v remote_url="$REMOTE_URL" \
  '{gsub("{client_id}", client_id); gsub("{shared_secret}", shared_secret); gsub("{embedded_remote_url}", remote_url); print}' > "/tmp/$CLIENT_SCRIPT_NAME"

mv "/tmp/$CLIENT_SCRIPT_NAME" "$INSTALL_DIR/$CLIENT_SCRIPT_NAME"
# Make script executable, readable and writeable for you, but noone else 
chmod 700 "$INSTALL_DIR/$CLIENT_SCRIPT_NAME"

set +v
################## Aaand done -----------------------------------------------------

# Print the configuration line for the coordinator
echo "Installation complete!"
echo "Add the following line to your coordinator config under [clients]:"
echo ""
echo "\"$CLIENT_ID\" = { shared_secret = \"$SHARED_SECRET\" }"
echo ""
echo "Afterwards you can use the client script with the following command:"
echo "$INSTALL_DIR/$CLIENT_SCRIPT_NAME <take|release> <host> [remote_url]"