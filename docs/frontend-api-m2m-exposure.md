# Expose Frontend APIs for M2M Clients

**Status: Implemented**

## Context

Several endpoints currently live behind frontend cookie auth (`/api/*`,
`/api/push/*`, `/ws`) and have no machine-to-machine (m2m) counterpart
other than the existing `/api/m2m/*` namespace which only covers lease
actions and per-host status.

We want to allow m2m clients (e.g. Home Assistant) to call the remaining
frontend-only APIs without going through browser login.  At the same time
we keep the old `/api/m2m/*` endpoints unchanged for eternity — they are
the "designed for m2m" API with their own semantics (sync-wait, `?async`,
per-host status, strict HMAC action matching).

---

## Stability Guarantee (or Lack Thereof)

This mechanism is **deliberately a fallback, not a replacement**.

| Property | `/api/*` (unified path) | `/api/m2m/*` (old path) |
|---|---|---|
| Stability | **None**. The `/api/*` namespace is first and foremost a frontend UI backend. Endpoints, response shapes, and semantics change without notice. | **Stable**. This is the "designed for M2M" contract. |
| Versioning | Single coarse version string (`frontendendpointV1`). Breaking changes bump the constant, which **rejects all outdated clients** with a 403. | Per-action HMAC content. No global version. |
| Semantics | Fire-and-forget only. No `?async`, no sync-wait, no per-host status. | Full semantics: sync-wait, `?async`, per-host status. |
| Recommendation | Use for "it's ok if this breaks" integrations. | Use for production automation. |

The version string (`EXPECTED_FRONTEND_ENDPOINT_VERSION`) is an **unreliable
indicator** of compatibility — it serves only as a kill switch.  When a
breaking change lands in a frontend endpoint, the operator bumps the
constant.  Clients that still present the old version get a **403
Forbidden** with no grace period.  There is no mechanism to support
multiple concurrent versions.

---

## Approach: Unified Auth Middleware + Blocklist

Modify the existing `auth::require` middleware to accept **both** cookie
sessions (existing) **and** HMAC-SHA256 headers (new).  The same private
routes serve both browser and m2m clients.  A path-based blocklist
prevents m2m clients from hitting endpoints where m2m makes no sense.

### AuthInfo Extension Type

A new request extension inserted by the middleware:

```
enum AuthInfo {
    WebSession,
    M2MClient {
        client_id: String,
    },
}
```

Handlers extract `AuthInfo` to decide `LeaseSource` and for logging.

### HMAC Header Format (for the unified path)

Reuses the **same headers** as the existing `/api/m2m/*`:

- `X-Client-ID` — identifies the client (registered in `[clients]` config).
- `X-Request` — `timestamp|frontendpoint_version|hex_signature`

The `frontendpoint_version` string is a single static value per client
deployment, e.g. `frontendpointV1`.  It is **not** per-endpoint and is
**not** parsed into actions.  The middleware:

1. Validates the HMAC-SHA256 signature against the client's shared_secret.
2. Validates the timestamp is within the 30-second window.
3. **Matches** the `frontendpoint_version` against a hard-coded constant
   in the binary (e.g. `EXPECTED_FRONTEND_ENDPOINT_VERSION`).
     - Match → request proceeds.
     - Mismatch → **403 Forbidden** (the client is authenticated but its
       frontend-endpoint version is no longer accepted; the body explains
       that the client must be updated).
4. Logs the client_id, version string, and request path for observability.

The version constant is manually maintained — it is an unreliable indicator
of compatibility, not a hard contract.  When the frontend API changes in a
breaking way, the operator bumps the constant, and outdated clients stop
working.

### Client Contract

A client MUST:
- Construct `X-Request` as `{unix_timestamp}|{version_string}|{hex_hmac_sha256}`
- Compute HMAC-SHA256 over `{unix_timestamp}|{version_string}` using the
  shared secret from `[clients]` config.
- Ensure the timestamp is within 30 seconds of the server clock.
- Set the version string to the exact value in the server binary
  (`frontendpointV1` as of this writing).

If any of these are wrong, the server returns:
- **400 Bad Request** — missing or malformed headers
- **401 Unauthorized** — timestamp out of range or invalid HMAC
- **403 Forbidden** — unknown client, blocked path, or outdated version

The version string `frontendpointV1` is the initial value.  When the
constant changes to `frontendpointV2`, all clients still sending
`frontendpointV1` receive a **403** with body `"Outdated client version"`.

### Middleware Flow

1. **`Disabled` / `External` auth mode** → pass through (unchanged).
2. **HMAC headers present** (`X-Client-ID` + `X-Request`)?
     - Validate client identity, HMAC crypto, timestamp, version.
     - On failure → 400 / 401 / 403.
     - On success → check **blocklist**.  If blocked → 403.
     - Insert `AuthInfo::M2MClient` extension, call `next`.
3. **Cookie present**?
     - Existing cookie validation (Token or OIDC session).
     - Insert `AuthInfo::WebSession` extension, call `next`.
4. **Neither** → existing behaviour (401 for API, redirect to /login for HTML).

### Blocklist

Paths where m2m auth is rejected even with valid credentials:

```
const M2M_BLOCKED_PREFIXES: &[&str] = &[
    "/api/push/",   // browser VAPID push subscriptions — meaningless for m2m
    "/ws",          // WebSocket — would need separate m2m design
];
```

In the middleware: if `AuthInfo::M2MClient` and
`req.uri().path().starts_with(blocked_prefix)` → 403 Forbidden.

The SPA route `/` is implicitly handled: m2m clients won't send HMAC
headers to `/` (they have no reason to), so they'll fall to step 4 and
get a 401 (non-HTML request → no redirect).

---

## Handler Changes (`/api/*`)

All handlers in `coordinator/src/http/api.rs`:

| Handler | Change |
|---|---|
| `handle_lease_action` | Extract `AuthInfo`. `WebSession` → `LeaseSource::WebInterface`. `M2MClient { client_id }` → `LeaseSource::Client(client_id)`. Fire-and-forget response only (no `?async`). |
| `handle_reset_leases` | Extract `AuthInfo`. No additional verification — m2m clients can reset any client's leases, same as web UI. |
| `get_hosts_status` | Just check `AuthInfo` exists (any variant). |
| `serve_dependency_data` | Same — static data, no behavioural difference. |
| `get_latest_release` | Same — no behavioural difference. |

---

## What Stays Untouched (Forever)

- **`/api/m2m/*`** — the old, explicitly-designed-for-m2m API.  Unchanged.
  Full HMAC with strict action matching, `?async`, sync-wait, per-host
  status.  New m2m clients that need those features should use this, not
  the unified path.
- **`/api/m2m/test_wol`** — known to be unauthenticated.  Will be fixed
  separately.
- **`/api/push/*`, `/ws`** — blocked for m2m via middleware.

---

## File Change Summary

| File | Change |
|---|---|
| `coordinator/src/http/auth/auth_info.rs` | **New** — `AuthInfo` enum |
| `coordinator/src/http/auth/mod.rs` | Add `pub mod auth_info;`, `pub mod hmac;`, re-export `AuthInfo` |
| `coordinator/src/http/auth/hmac.rs` | **New** — `validate_hmac_identity(headers, state) -> Result<String, ...>`. Extracted from `m2m/validation.rs` but drops action matching. Returns only the verified `client_id`. Adds version constant check. |
| `coordinator/src/http/auth/middleware.rs` | Add HMAC branch + blocklist check. Insert `AuthInfo` extension. |
| `coordinator/src/http/api.rs` | Unify handlers: extract `AuthInfo`, use for `LeaseSource` decision. |
| `coordinator/src/http/m2m/validation.rs` | No change — still used by old `/api/m2m/*` with strict matching. |
| `coordinator/src/http/m2m/mod.rs` | No change. |
| `coordinator/src/http/server/router.rs` | Move auth `route_layer` into `create_app` (where `AppState` is available). Middleware now uses `State<AppState>` directly. |

---

## Route Structure (After Change)

```
public (no auth middleware):
  /login, /logout, /oidc/*
  static assets (/*.js, /*.css, ...)
  /download/*
  /api/m2m/*                              ← untouched, forever

private (unified auth middleware):
  /api/*                                   ← now accepts cookie OR HMAC
  /api/push/*                              ← blocked for m2m (403)
  /                                        ← cookies needed; m2m → 401
  /ws                                      ← blocked for m2m (403)
```

---

## Future / TODO

Two items to be prominently noted in the codebase (e.g. in module docs
or a tracking file):

```
TODO: Rate limiting — once the unified middleware carries m2m identity,
      per-client rate limits are a natural fit. Deferred.

TODO: SECURITY — /api/m2m/test_wol has zero authentication. Currently
      accessible without any credentials.  Needs HMAC validation or a
      move into the private router.  Tracked separately from this change.
```
