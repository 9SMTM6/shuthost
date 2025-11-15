# <img src="frontend/assets/favicon.svg" alt="ShutHost" width="24" height="24"> ShutHost

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)]()
[![Status](https://img.shields.io/badge/status-active-success.svg)]()
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/9SMTM6/shuthost/main.yaml?label=build%20%26%20test)](https://github.com/9SMTM6/shuthost/actions/workflows/main.yaml)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/9SMTM6/shuthost/qa.yml?label=QA)](https://github.com/9SMTM6/shuthost/actions/workflows/qa.yaml)

A neat helper that manages the standby state of unix hosts with Wake-On-Lan (WOL) configured, with Web-GUI.

> Note: LARGE parts of this project were LLM generated. None were blindly committed, but it is what it is.

[![Live demo: PWA controlling NAS aka old PC (2x speed)](docs/shuthost_live_demo_2x.webp)](./docs/shuthost_live_demo_2x.webp)
> played at 2x speed, using the WebUI installed as PWA

⚠️ **Note**: the short demo clip shown above is slightly out of date with respect to theming and layout. Check the [live demo](https://9SMTM6.github.io/shuthost/) or [screenshots below](#-ui-screenshots) for the current UI.

## 🌐 Live Demo

You can try a demo of the ShutHost WebUI (no backend, simulated data) via GitHub Pages:

- [Live Demo](https://9SMTM6.github.io/shuthost/)

This demo runs entirely in your browser and does not control any real hosts. It is useful for previewing the UI and features without installing anything.
Note that the theme (light/dark) is selected based on your system preference.

---

## ✨ Features

- Manage standby state of Unix hosts with Wake-On-Lan (WOL) and lightweight agents
- Web-based GUI for easy management 
  - Light/Dark theme are selected based on system preference (with CSS media queries)
  - installable as [PWA](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Guides/Installing#installing_and_uninstalling_pwas)
    - this allows behavior similar to an native app on e.g. Android
- API for machine-to-machine control (e.g. backups)
- Should support extension (e.g. Home Assistant)
- Docker and simple binary deployment options (Docker has some strict requirements though)
- Convenience scripts for simple agent/client installation
- An attempt at extensive documentation

---

## 📋 Table of Contents

- [💿 Installation](#-installation)
- [📷 UI screenshots](#-ui-screenshots)
- [🔒 Security](#-security)
- [❓ FAQ](#-faq)
- [🚀 Potential Features](#-potential-features)

## 📚 Documentation

- [📚 Examples](docs/examples/)
- [📋 Requirements](docs/requirements.md)
- [🖥️ Platform Support](frontend/assets/partials/platform_support.md)
- [� WebUI Network Configuration](docs/examples/webui-network-config.md)
- [⚙️ Full Configuration Example](docs/examples/example_config.toml)
- [�🏗️ Architecture](https://9smtm6.github.io/shuthost/#architecture)
- [📖 API Documentation](docs/API.md)
- [🤝 Contributing](docs/CONTRIBUTING.md)

---

## 💿 Installation

Choose either the binary (recommended for reliability and WOL support) or the container (Linux only) installation.

Binary (recommended)
- Download the latest release from:
  - https://github.com/9SMTM6/shuthost/releases/latest
- Example (adjust filename for the asset you downloaded):
  ```bash
  curl -L -o shuthost_coordinator "https://github.com/9SMTM6/shuthost/releases/download/latest/shuthost_coordinator-x86_64-unknown-linux-gnu"
  chmod +x shuthost_coordinator
  ```
- Install as a system service (binary supports systemd/openrc/launchd)
  - Install command (runs the chosen platform service installer, creates config with correct permissions and enables start-on-boot):
    ```bash
    # Linux (recommended run with sudo)
    sudo ./shuthost install <optional user>

    # macOS (user is required on macOS)
    sudo ./shuthost install your-username
    ```
  - Notes:
    - On Linux the installer infers the target user from SUDO_USER if you run under sudo.
    - The installer will create service units for systemd or openrc where appropriate and set config file ownership/permissions.

Docker (Linux only)
- docker-compose example:
  ```yaml
  version: "3.8"
  services:
    shuthost:
      image: ghcr.io/9smtm6/shuthost/shuthost-coordinator:latest
      network_mode: "host"      # required for WOL
      restart: unless-stopped
      volumes:
        - ./coordinator_config/:/config/:ro
        - ./data/:/data/        # persist DB and generated certs here, optional
      # no ports, since network_mode: host
  ```
- CLI example:
  ```bash
  docker run --rm --network host \
    -v ./coordinator_config.toml:/config/coordinator_config.toml:ro \
    -v ./data/:/data/ \
    ghcr.io/9smtm6/shuthost/shuthost-coordinator:latest
  ```
-  Both with a config file (see [example_config.toml](docs/examples/example_config.toml), ensure restrictive permissions with `chmod 600 $(whoami) <config location>`)
- Notes:
  - `--network host` is Linux-only and will not work properly on Docker Desktop for Mac/Windows. Use the binary there or run on a Linux VM with bridged networking.


### Agent / Client installation
- To install a host-agent (controls the hosts): open the web UI, open "Install Host Agent" and follow the instructions shown.
- To install a client (M2M, e.g., backup scripts): switch to the Clients tab, open "Install Client" and follow the instructions shown.

---

## 📷 UI screenshots

More screenshots can be found in the [frontend/tests/visual-regression.spec.ts-snapshots](frontend/tests/visual-regression.spec.ts-snapshots) and the [frontend/tests/mobile-navigation.spec.ts-snapshots](frontend/tests/mobile-navigation.spec.ts-snapshots) folders.
These are generated or validated automatically as part of the test suite, and thus are guaranteed to be up-to-date (if the tests pass).

<table>
  <tr>
    <td><img src="frontend/tests/visual-regression.spec.ts-snapshots/at-hosts-Desktop-Dark.png" alt="Hosts — desktop dark" width="540"></td>
    <td><img src="frontend/tests/visual-regression.spec.ts-snapshots/at-hosts-Mobile-Dark.png" alt="Hosts — mobile dark" width="220"></td>
  </tr>
  <tr>
    <td><img src="frontend/tests/visual-regression.spec.ts-snapshots/at-hosts-expanded-install-Desktop-Light.png" alt="Hosts expanded — desktop light" width="540"></td>
    <td><img src="frontend/tests/visual-regression.spec.ts-snapshots/at-hosts-expanded-install-Mobile-Dark.png" alt="Hosts expanded — mobile dark" width="220"></td>
  </tr>
</table>

---

## 🔒 Security

### 🌐 WebUI Security
> ⚠️ **Warning**: You should enable the built-in authentication or use a reverse proxy that provides authentication.

#### Built-in Authentication (optional)
ShutHost can also enforce simple auth on its own, either with a static token or with OIDC login. If you enable this, you don't need external auth.

See the generated config file (a current version is also at [example_config.toml](docs/examples/example_config.toml)) for details.

See [OIDC Authentication with Kanidm](docs/examples/oidc-kanidm.md) for an example setup of OIDC with Kanidm.

For external auth, you need to add the following exceptions. The WebUI will show you convenience configs for some auth providers if you set `exceptions_version=0`.

Public endpoints (bypass):
- `/download/*`, `/manifest.json`, `/favicon.svg`, `/architecture*.svg`
- `/api/m2m/*` (M2M API, e.g. for clients)

All other routes should be protected by your external auth.

#### TLS configuration
See the generated config file (a current version is also at [example_config.toml](docs/examples/example_config.toml)) for details on how to enable TLS in the built-in server.

If you proxy unencrypted traffic with an external proxy (so the unencrypted traffic can be intercepted), this will not be detected, and poses a security risk, as well as a potential source for issues. Such a setup is neither recommended nor supported.

### 🛡️ Agent Security
- ✅ Host agents are secured with **HMAC signatures** and **timestamps** against replay attacks
- ✅ Only the coordinator that knows these (shared) secrets can use them
> ⚠️ **Warning**: All traffic between the coordinator and agents is **unencrypted** and only secured with HMAC signatures. This means that while status checks and commands are protected from tampering, anyone on the same LAN can observe the traffic and infer host statuses.

### 🔐 Client Security
- ✅ The client is secured in the same way as agents are
- ✅ The coordinator only accepts requests from **registered clients**

### 🔧 Reverse Proxy Configuration
To use the convenience scripts suggested by the WebUI, you will have to configure exceptions in the authorization of your reverse proxy, so that the requests from the host agents and clients are not blocked. These are detailed [above](#built-in-authentication-optional).

The WebUI will show you the required exceptions, alongside convenience configs for:
- 🔑 **Authelia**
- 🌐 **NGINX Proxy Manager** 
- 🚦 **Generic forward-auth in traefik**

---

## ❓ FAQ

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

---

## 🚀 Potential Features

### 🎯 Core Features
- 🔌 **Custom Wakers**: Support for alternative wake mechanisms beyond WOL, such as smart plugs or custom scripts (e.g., via API integrations). This would allow hosts without WOL support to be managed through external devices or services.
- 🔔 **Notifications about host state changes through the PWA**
- 📊 **Host state tracking for statistics**

### 🖥️ Platform Support
- 🐡 **BSD support** might happen
  - ⚠️ Requires using more advanced cross compilation
  - I have no ability to test these practically myself.

### 🔧 Management Features
- 🗑️ **Uninstalls**
- 📝 **Self-registration endpoint** for host agents
  - ❓ Unclear how to deal with authorization:
    - Server secret?
