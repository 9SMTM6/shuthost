# 🔒 Security Considerations

## 🌐 WebUI Security
> ⚠️ **Warning**: You should enable the built-in authentication or use a reverse proxy that provides authentication.

### Built-in Authentication (optional)
ShutHost can also enforce simple auth on its own, either with a static token or with OIDC login. If you enable this, you don't need external auth.

See the generated config file (a current version is also at [example_config.toml](docs/examples/example_config.toml)) for details.

See [OIDC Authentication with Kanidm](docs/examples/oidc-kanidm.md) for an example setup of OIDC with Kanidm.

For external auth, you need to add the following exceptions. The WebUI will show you convenience configs for some auth providers if you set `exceptions_version=0`.

Public endpoints (bypass):
- `/download/*`, `/manifest.json`, `/favicon.*.svg`, `/architecture*.svg`
- `/api/m2m/*` (M2M API, e.g. for clients)

All other routes should be protected by your external auth.

### TLS configuration
See the generated config file (a current version is also at [example_config.toml](docs/examples/example_config.toml)) for details on how to enable TLS in the built-in server.

If you proxy unencrypted traffic with an external proxy (so the unencrypted traffic can be intercepted), this will not be detected, and poses a security risk, as well as a potential source for issues. Such a setup is neither recommended nor supported.

## 🛡️ Agent Security
- ✅ Host agents are secured with **HMAC signatures** and **timestamps** against replay attacks
- ✅ Only the coordinator that knows these (shared) secrets can use them
> ⚠️ **Warning**: All traffic between the coordinator and agents is **unencrypted** and only secured with HMAC signatures. This means that while status checks and commands are protected from tampering, anyone on the same LAN can observe the traffic and infer host statuses.

## 🔐 Client Security
- ✅ The client is secured in the same way as agents are
- ✅ The coordinator only accepts requests from **registered clients**

## 🔧 Reverse Proxy Configuration
To use the convenience scripts suggested by the WebUI, you will have to configure exceptions in the authorization of your reverse proxy, so that the requests from the host agents and clients are not blocked. These are detailed [above](#built-in-authentication-optional).

The WebUI will show you the required exceptions, alongside convenience configs for:
- 🔑 **Authelia**
- 🌐 **NGINX Proxy Manager**
- 🚦 **Generic forward-auth in traefik**