# ❓ FAQ

### 🔄 My host didn't shut down when I released the lease. What's wrong?

If your host missed the initial shutdown command, you'll need to perform a "full cycle" (release the lease, then take it again) to trigger another shutdown attempt.

**Solution:** This is a known limitation. We're considering adding regular state synchronization between the coordinator and hosts to prevent this issue. For now, simply release and re-acquire the lease to retry.

### 💾 The coordinator lost all my leases after restarting. How do I prevent this?

If you don't configure a database or don't persist it between restarts, the coordinator will lose its state.

**Solution:** Configure the `[db]` section in your config file and ensure the database file is persisted (e.g., keep the SQLite file on disk or mount the volume properly in Docker).

### 🌐 I can't access the coordinator WebUI from other Docker containers. What should I do?

Docker networking requires specific configuration for the coordinator to be accessible from other containers. By default, the coordinator only binds to the local network interface (localhost/127.0.0.1) for security reasons, preventing access from other containers and other hosts on the LAN.

**Solution:** See [WebUI Network Configuration](docs/examples/webui-network-config.md) for detailed setup instructions on configuring Docker networking to allow access from other containers.

### 🌐 WOL signals aren't reaching their target hosts when running the coordinator in Docker. What should I do?

Docker containers by default run a networking mode which prevents WOL (Wake-on-LAN) packets from reaching the physical network.

**Solution:** Use `network_mode: host` in your Docker configuration to allow the coordinator to send WOL packets directly to the network. Note that this is Linux-only and won't work with Docker Desktop on Mac/Windows.

### 🌐 The agent installation detected the wrong network interface. How do I fix it?

The installer chooses the default network interface to determine the IP address, MAC address, etc., which may not always be the correct interface for your setup.

**Solution:** Manually override the network interface in the agent configuration file after installation.

### 🐧 The coordinator binary fails with a glibc version error. What's the issue?

On certain distributions (e.g., Ubuntu 22.04), the default binary may be incompatible with your system's glibc version.

**Solution:** Use the **musl binary** instead, or run the coordinator in a **container**. For the agent, the install script will automatically recommend the musl binary and the corresponding command line invocation if the default one fails.

### 🔏 The agent/client install script fails when I use self-signed certificates. Why?

The install scripts cannot validate self-signed certificates without additional configuration.

**Solution:** Either proxy your self-signed certificates through a trusted endpoint, or use certificates from a trusted provider like Let's Encrypt.

### 🪪 OIDC login shows an error but works on the second try. Is this a bug?

Yes, OIDC login occasionally fails to revalidate the session and shows a generic error. Clicking "Login with SSO" again typically succeeds.

**Solution:** This issue is currently undiagnosed due to lack of data. As a workaround, simply click "Login with SSO" again to log in successfully. If you experience this issue, please consider reporting details to help us diagnose it.