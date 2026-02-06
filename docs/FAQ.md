# ❓ FAQ

### 🔄 My host didn't shut down when I released the lease. What's wrong?

If your host missed the initial shutdown command, you'll need to perform a "full cycle" (release the lease, then take it again) to trigger another shutdown attempt.

**Solution:** This is a known limitation. We're considering adding regular state synchronization between the coordinator and hosts to prevent this issue. For now, simply release and re-acquire the lease to retry.

### 💾 The coordinator lost all my leases after restarting. How do I prevent this?

If you don't configure a database or don't persist it between restarts, the coordinator will lose its state.

**Solution:** Configure the `[db]` section in your config file and ensure the database file is persisted (e.g., keep the SQLite file on disk or mount the volume properly in Docker).

### 🌐 I can't access the coordinator WebUI from (other, if deployed as container) Docker containers. What should I do?

Docker networking requires specific configuration for services bound to loopback (aka 127.0.0.1, not bound to 0.0.0.0 aka all interfaces) to be accessible from other containers. By default, the coordinator only binds to loopback for security reasons, preventing access from and other hosts on the LAN as well as containers.
Because of the `network_mode: host` setting required by the container, this applies both to the binary as well as the docker deployment.

**Solution:** See [WebUI Network Configuration](./examples/webui-network-config.md) for detailed setup instructions on configuring Docker networking to allow access from other containers.

### 🌐 WOL signals aren't reaching their target hosts when running the coordinator in Docker. What should I do?

Docker containers by default run a networking mode which prevents WOL (Wake-on-LAN) packets from reaching the physical network.

**Solution:** Use `network_mode: host` in your Docker configuration to allow the coordinator to send WOL packets directly to the network. Note that this is Linux-only and won't work with Docker Desktop on Mac/Windows.

### 🌐 The agent installation detected the wrong network interface. How do I fix it?

The installer chooses the default network interface to determine the IP address, MAC address, etc., which may not always be the correct interface for your setup.

**Solution:** Manually override the network interface in the agent configuration file after installation.

### 🔏 The agent/client install script fails when I use self-signed certificates. Why?

The install scripts cannot validate self-signed certificates without additional configuration.

**Solution:** Either proxy your self-signed certificates through a trusted endpoint, or use certificates from a trusted provider like Let's Encrypt. For the agent specifically, you can also install the agent directly from GitHub  - see [Agent-only Installation](./examples/agent-installation.md), no need to generate the direct control script, just continue like with a normal install.

### 🌐 Must be served on a (sub)domain, not a subpath

Shuthost must be served from a dedicated domain or subdomain (for example
`coordinator.example.com`). Serving it from a URL path such as
`ex.ample.com/shuthost` is not supported and will break important features.
