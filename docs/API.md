# ShutHost API Documentation

This document describes the APIs for communication between different components in the ShutHost system:
- **Coordinator M2M API**: Machine-to-machine lease management and control
- **Agent Protocol**: Host management commands and status checking

This documentation is intended to help with third-party integrations, including custom scripts and systems like Home Assistant.

> **⚠️ API Stability Notice**: An effort is taken to version breaking changes appropriately, but no guarantee is given. Only successful response status codes (2xx) are subject to stability efforts - error responses and message content may change without notice. APIs not covered in this documentation are not subject to any stability efforts.

> **💡 Implementation Examples**: For concrete code examples, examine the source code of the provided convenience scripts (`shuthost_client`, `direct_control`) and the ShutHost coordinator/agent components.

## Table of Contents

1. [Coordinator M2M API](#coordinator-m2m-api)
2. [Agent Protocol](#agent-protocol)
3. [Authentication & Security](#authentication--security)
4. [Examples](#examples)
   - [Client Scripts](#client-scripts)
   - [Direct Control Scripts](#direct-control-scripts)

---

## Coordinator M2M API

The coordinator provides HTTP REST APIs for clients to manage host leases.

> **🔒 Security Note**: The coordinator is expected to be served with HTTPS and behind authentication (e.g., Authelia). The M2M endpoints (`/api/m2m/*`) are exempt from web authentication as they use HMAC-based authentication.

### Base URL Format
```
https://{coordinator_host}:{port}/api
```

### M2M Lease Management

**Endpoint:** `POST /api/m2m/lease/{hostname}/{action}`

**Description:** Take or release a lease on a host (machine-to-machine)

**Path Parameters:**
- `hostname` (string): Target host identifier
- `action` (string): Either `take` or `release`

**Query Parameters:**
- `async` (boolean, optional): 
  - `false` (default): Synchronous operation - waits for host to reach desired state
  - `true`: Asynchronous operation - returns immediately after triggering state change

**Headers:**
- `X-Client-ID` (required): Client identifier
- `X-Request` (required): HMAC-signed request in format `{timestamp}|{action}|{signature}`

**Request Body:** None

**Response:**
- **200 OK**: Lease operation successful
  - Sync mode: `"Lease taken, host is online"` or `"Lease released, host is offline"`
  - Async mode: `"Lease taken (async)"` or `"Lease released (async)"`
- **400 Bad Request**: Invalid request format or parameters
- **401 Unauthorized**: Invalid HMAC signature or timestamp
- **403 Forbidden**: Unknown client ID
- **500 Internal Server Error**: Host operation failed

---

## Agent Protocol

The host agent accepts TCP connections for status checks and shutdown commands. This protocol can be used by the coordinator or any other system that needs to communicate with the agent.

### Protocol Details

**Transport:** TCP  
**Port:** Configurable per host (default: 9090, `shuthost_common::DEFAULT_AGENT_TCP_PORT`).
The agent also emits a UDP startup broadcast; that port defaults to `shuthost_common::DEFAULT_COORDINATOR_BROADCAST_PORT` (5757) and may be set separately with `--broadcast-port`.

> **Note:** When you copy the host‑agent install command from the coordinator web UI, it
> will automatically include `--broadcast-port <value>` reflecting the port
> configured for that coordinator instance, so no manual adjustment is
> needed.
**Message Format:** Text-based with HMAC authentication

### Message Structure

All messages sent to the agent follow this format:
```
{timestamp}|{command}|{hmac_signature}
```

**Components:**
- `timestamp`: Unix timestamp (UTC seconds)
- `command`: Command string (`status` or `shutdown`)
- `hmac_signature`: Hex-encoded HMAC-SHA256 signature

### Commands

#### 1. Status Check

**Command:** `status`

**Purpose:** Check if the host agent is online and responsive

**Example Message:**
```
1674567890|status|a1b2c3d4e5f6789...
```

**Agent Response:**
```
OK: status
```

#### 2. Shutdown Request

**Command:** `shutdown`

**Purpose:** Request the host to execute its configured shutdown command

**Example Message:**
```
1674567890|shutdown|a1b2c3d4e5f6789...
```

**Agent Response:**
```
Now executing command: {shutdown_command}. Hopefully goodbye.
```

### Agent Response Format

**Success Responses:**
- `OK: status` - Status check successful
- `Now executing command: {command}. Hopefully goodbye.` - Shutdown initiated

**Error Responses:**
- `ERROR: Invalid UTF-8` - Message contains invalid UTF-8
- `ERROR: Invalid request format` - Message doesn't follow expected format
- `ERROR: Timestamp out of range` - Timestamp outside allowed window (±30 seconds)
- `ERROR: Invalid HMAC signature` - HMAC verification failed
- `ERROR: Invalid command` - Unknown command received

### Connection Handling

**Timeout:** 2 seconds for TCP operations
**Buffer Size:** 1024 bytes for responses  
**Connection Model:** One request per connection (connection closed after response)

---

## Authentication & Security

### HMAC Authentication

Both coordinator M2M API and agent protocol communications use HMAC-SHA256 for message authentication.

#### HMAC Message Creation

1. **Create Message:** `{timestamp}|{command}`
2. **Generate Signature:** `HMAC-SHA256(message, secret_key)`
3. **Encode Signature:** Convert to hexadecimal string
4. **Final Format:** `{timestamp}|{command}|{signature}`

#### Timestamp Validation

- **Window:** ±30 seconds from current UTC time
- **Purpose:** Prevents replay attacks
- **Format:** Unix timestamp (seconds since epoch)

#### Example HMAC Generation (Shell)

```bash
# Generate timestamp
TIMESTAMP=$(date -u +%s)

# Create message
MESSAGE="${TIMESTAMP}|${ACTION}"

# Generate signature
SIGNATURE=$(printf "%s" "$MESSAGE" | openssl dgst -sha256 -hmac "$SECRET" -binary | hexdump -ve '/1 "%02x"')

# Combine into final message
FINAL_MESSAGE="${TIMESTAMP}|${ACTION}|${SIGNATURE}"
```

### Client Authentication

**M2M Endpoints:** Use HMAC-based authentication as described above. These endpoints bypass web authentication.

**Web Interface:** Protected by external authentication (e.g., Authelia) and served over HTTPS.

**Client Registration:** Clients must be registered in coordinator configuration with unique ID and shared secret

**Configuration Format:**
```toml
[clients]
"client_name" = { shared_secret = "hex_encoded_secret" }
```

### Host Agent Security

**Shared Secrets:** Each host has individual shared secret for HMAC authentication
**Environment Variable:** `SHUTHOST_SHARED_SECRET` must be set on host (is taken care off in the service files).
**Third-party Integration:** Any system can communicate with agents using the TCP protocol and proper HMAC authentication.

---

## Examples

### Client Scripts

Convenience scripts are provided for M2M lease management via the coordinator API:

```bash
# Take lease on host (synchronous)
./shuthost_client_myclient.sh take myhost

# Release lease on host (asynchronous)  
./shuthost_client_myclient.sh release myhost https://coordinator.example.com --async

# Take lease with custom coordinator URL
./shuthost_client_myclient.sh take myhost https://coordinator.example.com

# Take lease asynchronously with custom coordinator URL
./shuthost_client_myclient.sh take myhost https://coordinator.example.com --async
```

### Direct Control Scripts

For direct communication with host agents (bypassing the coordinator):

```bash
# Generate direct-control script (Linux/macOS)
sudo shuthost_host_agent generate-direct-control
./shuthost_direct_control_<hostname> status
./shuthost_direct_control_<hostname> shutdown
```

```powershell
# Generate direct-control script (Windows)
powershell -ExecutionPolicy Bypass -File .\shuthost_host_agent_self_extracting.ps1 generate-direct-control
.\shuthost_direct_control_<hostname>.ps1 status
.\shuthost_direct_control_<hostname>.ps1 shutdown
```

See [Agent-only Installation](docs/examples/agent-installation.md) for details.

### cURL Examples

#### Take Lease (M2M)

```bash
# Generate HMAC-signed request
TIMESTAMP=$(date -u +%s)
MESSAGE="${TIMESTAMP}|take"
SIGNATURE=$(printf "%s" "$MESSAGE" | openssl dgst -sha256 -hmac "$SECRET" -binary | hexdump -ve '/1 "%02x"')
X_REQUEST="${TIMESTAMP}|take|${SIGNATURE}"

# Make request
curl -X POST "https://coordinator.example.com/api/m2m/lease/myhost/take" \
  -H "X-Client-ID: myclient" \
  -H "X-Request: $X_REQUEST"
```

### Configuration Example

```toml
# coordinator config
[hosts]
"myhost" = { 
  ip = "192.168.1.100", 
  mac = "aa:bb:cc:dd:ee:ff", 
  port = 9090,
  shared_secret = "secret123" 
}

[clients]  
"myclient" = { shared_secret = "clientsecret456" }
```
