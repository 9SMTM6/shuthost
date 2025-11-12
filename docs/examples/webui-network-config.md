# WebUI Network Configuration

This example shows how to configure network access for the ShutHost WebUI, particularly when running the coordinator in different network setups.

## Default Configuration

The coordinator binary exposes its server on `127.0.0.1` only by default - so on localhost, IPv4, without remote access. This is for security reasons.

## Docker Access

To access the WebUI served by the coordinator from Docker containers (e.g. your reverse proxy, like NGINX), use the address:
```
http://host.containers.internal:<port>
```

### Alternative Container Solutions

Container solutions other than Docker (e.g. Podman) might require additional configuration.

**For Podman**, add the following to the container that wants to access the coordinator:
```yaml
extra_hosts:
  - "host.docker.internal:host-gateway"
```

## Custom Binding

Alternatively, you can set the address the coordinator binds to in the configuration file by modifying the `bind` setting in your `shuthost_coordinator.toml`:

```toml
[server]
bind = "0.0.0.0:8080"  # Listen on all interfaces
# or
bind = "192.168.1.100:8080"  # Listen on specific IP
```

> **⚠️ Security Warning**: Binding to addresses other than localhost (`127.0.0.1`) exposes the WebUI to your network. Ensure you have proper authentication and TLS configured before doing this.