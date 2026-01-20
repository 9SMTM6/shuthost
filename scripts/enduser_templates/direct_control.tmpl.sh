#!/bin/sh

# This may be a template containing placeholders like {host_ip}, {port}, {shared_secret}, {mac_address}, and {hostname}
# that must be replaced with actual values before use.

set -eu

print_help() {
        cat <<EOF
Usage: $0 <status|shutdown|wake>

Generated for host: {hostname}

Requires: openssl, date, hexdump, printf, nc

Arguments:
    <status|shutdown|wake>   Action to perform (required)

Options:
    -h, --help               Show this help message and exit

Examples:
    $0 status
    $0 shutdown
    $0 wake
EOF
}

# Check for help flag (prioritized above other args)
for arg in "$@"; do
    case "$arg" in
        -h|--help)
            print_help
            exit 0
            ;;
    esac
done

if [ $# -ne 1 ]; then
    echo "Error: Exactly one argument required." >&2
    print_help
    exit 1
fi

ACTION="$1"
HOST_IP="{host_ip}"
PORT="{port}"
SECRET="{shared_secret}"
MAC_ADDRESS="{mac_address}"
BROADCAST_IP="255.255.255.255"

################## Boring setup complete ------------- Interesting stuff is starting here

case "$ACTION" in
    status|shutdown)
        # Get current timestamp (UTC)
        TIMESTAMP=$(date -u +%s)

        # Build the message and signature
        MESSAGE="${TIMESTAMP}|${ACTION}"
        SIGNATURE=$(printf "%s" "$MESSAGE" | openssl dgst -sha256 -hmac "$SECRET" -binary | hexdump -ve '/1 "%02x"')

        # Combine into final message
        FINAL_MESSAGE="${TIMESTAMP}|${ACTION}|${SIGNATURE}"

        set -v

        # Send the message via TCP and print response
        printf "%s" "$FINAL_MESSAGE" | nc "$HOST_IP" "$PORT"
        ;;
    wake)
        echo "WOL via this script is in testing and may not work reliable across all platforms. Please report issues."
        # Construct magic packet
        # 6 bytes of FF
        PACKET='\xff\xff\xff\xff\xff\xff'
        # 16 repetitions of MAC address
        MAC_BYTES=$(printf '%s' "$MAC_ADDRESS" | sed 's/:/ /g')
        for _ in $(seq 1 16); do
            for byte in $MAC_BYTES; do
                PACKET="${PACKET}\\x${byte}"
            done
        done

        set -v

        # Send magic packet via UDP
        if [ "$(uname)" = "Darwin" ]; then
            # macOS: use Python 3 for reliable broadcast
            python3 -c "
import socket
packet = b'$PACKET'
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
for _ in range(3):
    s.sendto(packet, ('$BROADCAST_IP', 9))
"
        else
            # Linux: use nc with broadcast option
            for i in 1 2 3; do
                printf '%b' "$PACKET" | nc -u -b -w0 "$BROADCAST_IP" 9
            done
        fi
        ;;
    *)
        echo "Error: Invalid action '$ACTION'. Must be status, shutdown, or wake." >&2
        exit 1
        ;;
esac
