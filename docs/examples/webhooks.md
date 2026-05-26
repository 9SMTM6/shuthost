# Webhook Notifications

ShutHost can POST a JSON payload to one or more HTTP endpoints whenever a host event occurs.
This lets you integrate with notification services (e.g. ntfy, Gotify, Slack, Home Assistant, n8n) or your own tooling.

Refer to the [example configuration](./example_config_webhooks.toml) for a ready-to-use TOML file.

## Events and Payload Shape

All payloads share a common envelope:

| Field | Type | Description |
|---|---|---|
| `host` | string | The host identifier from your config. |
| `at_unix` | integer | Unix timestamp (seconds) when the event was dispatched. |
| `event` | string | Event discriminator — one of the values below. |

Additional fields are event-specific:

### `unscheduled`

The host changed state without the coordinator initiating it.

```json
{ "host": "my-nas", "at_unix": 1748256000, "event": "unscheduled", "kind": "startup" }
{ "host": "my-nas", "at_unix": 1748256000, "event": "unscheduled", "kind": "shutdown" }
```

- `kind: "startup"` — host came online while no leases were held (unexpected boot).
- `kind: "shutdown"` — host went offline while leases were active (unexpected shutdown).

### `operation_failed`

A coordinator-initiated wake or shutdown did not complete within the timeout.

```json
{ "host": "my-nas", "at_unix": 1748256000, "event": "operation_failed", "kind": "startup", "is_repeat": false }
{ "host": "my-nas", "at_unix": 1748256000, "event": "operation_failed", "kind": "shutdown", "is_repeat": true }
```

- `kind` — the operation that failed (`"startup"` or `"shutdown"`).
- `is_repeat` — `true` when the same failure was already recorded (i.e. a retry). Webhooks always fire; PWA push notifications are suppressed for repeats.

### `online_for`

The host has been continuously online for a configured duration.

```json
{ "host": "my-nas", "at_unix": 1748256000, "event": "online_for", "online_for_secs": 3600 }
```

- `online_for_secs` — the duration (in seconds) that was configured in the filter.
- This event is **never** fired by default — it must be explicitly listed in `events`.

## Configuration

Webhooks are configured as an array of tables under `[[notifications.webhooks]]`.

### Fields

| Field | Required | Description |
|---|---|---|
| `url` | yes | The HTTP(S) endpoint to POST to. |
| `secret` | no | HMAC-SHA256 signing secret (see [Signature Verification](#signature-verification)). |
| `headers` | no | Inline table of extra HTTP headers (e.g. `Authorization`). |
| `events` | no | List of event filters (see [Event Filters](#event-filters)). |

### Minimal example

```toml
[[notifications.webhooks]]
url = "https://ntfy.sh/my-shuthost-topic"
```

Fires for all `unscheduled` and `operation_failed` events on all hosts. No `online_for`.

### With authentication headers and HMAC signing

```toml
[[notifications.webhooks]]
url = "https://ntfy.sh/my-shuthost-topic"
secret = "change-me"
headers = { Authorization = "Bearer my-ntfy-token" }
```

### Multiple webhooks

Multiple `[[notifications.webhooks]]` blocks are supported — each is evaluated independently.

```toml
[[notifications.webhooks]]
url = "https://ntfy.sh/shuthost-alerts"

[[notifications.webhooks]]
url = "https://hooks.example.com/shuthost"
secret = "change-me"
events = [
  { type = "operation_failed" },
]
```

## Event Filters

The `events` field controls which events fire a given webhook.

| Value | Behavior |
|---|---|
| Omitted | Fires for `unscheduled` and `operation_failed` on all hosts. `online_for` never fires by default. |
| `[]` (empty list) | Disables the webhook entirely. |
| Non-empty list | Fires only for the listed filters (see below). |

### String shorthands

String values match the named event on **all hosts**:

```toml
events = ["unscheduled", "operation_failed"]
```

Available shorthands: `"unscheduled"`, `"operation_failed"`.  
(`"online_for"` has no shorthand because it requires a `duration_secs`.)

### Inline-table filters

Use inline tables to scope by host or to configure `online_for`:

```toml
events = [
  # All hosts, unscheduled only
  "unscheduled",
  # Only my-nas, operation failures
  { type = "operation_failed", hosts = ["my-nas"] },
  # Fire after 5 minutes online (all hosts)
  { type = "online_for", duration_secs = 300 },
  # Fire after 1 hour online, only for my-nas
  { type = "online_for", duration_secs = 3600, hosts = ["my-nas"] },
]
```

The `hosts` field is optional on all structured filters — omit it to match all hosts.

## Signature Verification

When `secret` is set, each POST includes an `X-ShutHost-Signature` header:

```
X-ShutHost-Signature: sha256=<hex>
```

The signature is the HMAC-SHA256 of the raw JSON request body, keyed with the configured secret.

Verification example (Python):

```python
import hashlib, hmac

def verify(body: bytes, secret: str, header: str) -> bool:
    expected = "sha256=" + hmac.new(secret.encode(), body, hashlib.sha256).hexdigest()
    return hmac.compare_digest(expected, header)
```
