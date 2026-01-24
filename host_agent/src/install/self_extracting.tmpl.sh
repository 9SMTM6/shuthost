#!/bin/sh

# { description }

export SHUTHOST_SHARED_SECRET="{ secret }"
export PORT="{ port }"
export SHUTDOWN_COMMAND="{ shutdown_command }"

OUT=$(mktemp /tmp/selfbin.XXXXXX)
BINARY_PAYLOAD="{ encoded }"
echo "$BINARY_PAYLOAD" | base64 -d > "$OUT"
chmod +x "$OUT"
if [ "$#" -gt 0 ] && [ "${1#-}" = "$1" ]; then
    if [ "$1" = "generate-direct-control" ] || [ "$1" = "registration" ]; then
        "$OUT" "$@" --script-path "$0" --init-system self-extracting-shell
    else
        "$OUT" "$@"
    fi
else
    nohup "$OUT" service --port="$PORT" --shutdown-command="$SHUTDOWN_COMMAND" "$@" >"$OUT.log" 2>&1 &
fi
exit 0
