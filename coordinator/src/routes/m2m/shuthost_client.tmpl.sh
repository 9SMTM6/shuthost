#!/bin/sh

# Usage: ./shuthost_client_<client_id>.sh <take|release> <host> [remote_url]
# Requires: curl, openssl, date

set -eu

ACTION="$1"
TARGET_HOST="$2"
REMOTE_URL="${3:-"{embedded_remote_url}"}"
COORDINATOR_URL="${REMOTE_URL}/api/m2m/lease/${TARGET_HOST}/${ACTION}"

CLIENT_ID="{client_id}"
SECRET="{shared_secret}"

# Get current timestamp (UTC)
TIMESTAMP=$(date -u +%s)

# Build the message and signature
MESSAGE="${TIMESTAMP}|${ACTION}"
SIGNATURE=$(printf "%s" "$MESSAGE" | openssl dgst -sha256 -hmac "$SECRET" -binary | hexdump -ve '/1 "%02x"')

# Combine into final X-Request header
X_REQUEST="${TIMESTAMP}|${ACTION}|${SIGNATURE}"

# Make the request
curl -sS --fail-with-body -X POST "$COORDINATOR_URL" \
  -H "X-Client-ID: $CLIENT_ID" \
  -H "X-Request: $X_REQUEST"
