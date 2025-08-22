# <img src="coordinator/assets/favicon.svg" alt="ShutHost" width="24" height="24"> ShutHost

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)]()
[![Status](https://img.shields.io/badge/status-active-success.svg)]()
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/9SMTM6/shuthost/main.yaml?label=build%20%26%20test)](https://github.com/9SMTM6/shuthost/actions/workflows/main.yaml)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/9SMTM6/shuthost/qa.yml?label=QA)](https://github.com/9SMTM6/shuthost/actions/workflows/qa.yaml)

> A neat little (well, at one time it was) helper that manages the standby state of unix hosts with Wake-On-Lan (WOL) configured, with Web-GUI.

⚠️ **Note**: LARGE parts of this project were LLM generated. I checked over all of them before committing, but it is what it is.

[![Live demo: PWA controlling NAS (2x speed)](docs/shuthost_live_demo_2x.webp)](./docs/shuthost_live_demo_2x.webp)
> played at 2x speed, using the WebUI installed as PWA

## 🌐 Live Demo

You can try a static demo of the ShutHost WebUI (no backend, simulated data) via GitHub Pages:

- [Live Demo](https://9SMTM6.github.io/shuthost/)

This demo runs entirely in your browser and does not control any real hosts. It is useful for previewing the UI and features without installing anything.

---

## ✨ Features

- Manage standby state of Unix hosts with Wake-On-Lan (WOL) and lightweight agents
- Web-based GUI for easy management 
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
- [🏗️ Architecture](#️-architecture)
- [📖 API Documentation](#-api-documentation)
- [📋 Requirements](#-requirements)
- [🔒 Security](#-security)
- [⚠️ Known Issues](#️-known-issues)
- [🚀 Potential Features](#-potential-features)

---

## 💿 Installation

Choose either the binary (recommended for reliability and WOL support) or the container (Linux only — host network required).

Release (binary, recommended)
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

Docker (Linux only — host network mode required for WOL)
- Run with the host network so broadcasts and LAN reachability work:
  ```bash
  docker run --rm --network host \
    -v ./coordinator_config.toml:/config/coordinator_config.toml:ro \
    ghcr.io/9SMTM6/shuthost:latest
  ```
- docker-compose example:
  ```yaml
  version: "3.8"
  services:
    shuthost:
      image: ghcr.io/9SMTM6/shuthost:latest
      network_mode: "host"      # required for WOL
      restart: unless-stopped
      volumes:
        - ./coordinator_config.toml:/config/coordinator_config.toml:ro
      # no ports, since network-mode: host
  ```
-  Both with config file
  ```toml
  [server]
  port = 8080 # change accordingly
  bind = "127.0.0.1" # forward to this with your reverse proxy, INCLUDING AUTHORIZATION! With exceptions as detailed in the WebUI.

  [hosts]

  [clients]
  ```
- Notes:
  - --network host is Linux-only and will not work properly on Docker Desktop for Mac/Windows. Use the binary there or run on a Linux VM with bridged networking.

Quick links & notes
- Release: https://github.com/9SMTM6/shuthost/releases/latest
- Homebrew / distro packages: Might be provided if there is community interest and/or support — please file an issue or react to the appropriate.

Agent / Client installation
- To install a host-agent (controls the hosts): open the web UI, open "Install Host Agent" and follow the instructions shown.
- To install a client (M2M, e.g., backup scripts): switch to the Clients tab, open "Install Client" and follow the instructions shown.

---

## 🏗️ Architecture

📖 See [Architecture Documentation](coordinator/assets/architecture.md)

## 📖 API Documentation

📚 See [API Documentation](docs/API.md) for details on:
- **Coordinator M2M API**: Machine-to-machine lease management and control
- **Agent Protocol**: Host management commands and status checking

This documentation is intended to help with third-party integrations, including custom scripts and systems like Home Assistant.

## 📋 Requirements

### 🤖 Agent Requirements
For the requirements for the agent, see [Requirements to install the agent](coordinator/assets/agent_install_requirements_gotchas.md).

### 🖥️ Coordinator Requirements

The coordinator must be run on a system that can reach the hosts you want to manage.

Assuming that the coordinator-host is on the same network as the hosts, with WOL broadcasts allowed, this requires additionally:
- 🔧 Running the coordinator as a **binary** on the coordinator-host, or
- 🐳 Running it in a **docker container** with the host network mode enabled

> ⚠️ **Important**: This does not work with the default network mode that docker uses on Windows and MacOS. It will also not work on WSL. On these Hosts, you will have to run the coordinator as a binary, or install a Linux VM with bridged networking to run docker.

❌ **Windows is currently not supported**, even with the binary and/or WSL. You need a VM or a dedicated Linux machine.

### 🌐 WebUI Network Configuration

The coordinator binary exposes its server on `127.0.0.1` only by default - so on localhost, ipv4, without remote access. This is for security reasons.

#### 🐳 Docker Access
To access the WebUI served by the binary from Docker containers (e.g. NGINX), use the address:
```
http://host.containers.internal:<port>
```

Container solutions other than Docker (e.g. Podman) might require additional configuration.
On Podman, add the following to the container that wants to access the coordinator:
```yaml
extra_hosts:
  - "host.docker.internal:host-gateway"
```

Alternatively, you can set the address the coordinator binds to in the configuration file.

---

## 🔒 Security

### 🌐 WebUI Security
> ⚠️ **Warning**: The WebUI is **not secured**, so you should run it behind a reverse proxy that provides TLS and authentication.

### 🛡️ Agent Security
- ✅ Host agents are secured with **HMAC signatures** and **timestamps** against replay attacks
- ✅ Only the coordinator that knows these (shared) secrets can use them
> ⚠️ **Warning**: All traffic between the coordinator and agents is **unencrypted** and only secured with HMAC signatures. This means that while status checks and commands are protected from tampering, anyone on the same LAN can observe the traffic and infer host statuses.

### 🔐 Client Security
- ✅ The client is secured in the same way
- ✅ The coordinator only accepts requests from **registered clients**

### 🔧 Reverse Proxy Configuration
To use the convenience scripts suggested by the WebUI, you will have to configure exceptions in the authorization of your reverse proxy, so that the requests from the host agents and clients are not blocked. 

The WebUI will show you the required exceptions, alongside convenience configs for:
- 🔑 **Authelia**
- 🌐 **NGINX Proxy Manager** 
- 🚦 **Generic forward-auth in traefik**

---

## ⚠️ Known Issues

| Issue | Description | Impact | Solution |
|-------|-------------|--------|----------|
| 🔄 **Missed Shutdown** | If the host misses the initial shutdown, a "full cycle" is required to send it again (release lease, take lease) | Medium | [APP-SIDE] Regularly "syncing" states, either with explicit config on the host or coordinator-wide |
| 💾 **State Loss** | The coordinator loses state on restart (including updates) | Low (currently only acts on state changes) | [APP-SIDE] Considering state persistence with e.g. sqlite or explicit syncing |
| 🐳 **Docker Testing** | Docker is currently not well tested | Unknown | N/A |
| 🪟 **Windows Support** | Windows agent support currently not planned, due to large differences in the way services are implemented | N/A | N/A |
| 🌐 **Docker Connectivity** | Accessing the coordinator from Docker requires proper configuration | Medium | Ensure proper Docker network configuration |
| 🌐 **Default Network Interface Selection** | The agent installation chooses the default network interface to determine the IP, MAC, etc. for the config, which may not always be correct | Medium | Manually override the network interface in the configuration |
| 🐧 **glibc Version Errors** | On certain distributions (e.g., Ubuntu 22.04), the coordinator binary may fail due to incompatible glibc versions | Medium | Use the **musl binary** or the **container** for the coordinator. For the agent the install script will recommend the correct override to get the musl binary if the original binary fails |

---

## 🚀 Potential Features

### 🔐 Authentication & Authorization
- 🆔 **OIDC authorization** where I allow the required endpoints for all
  - alternative to putting the GUI behind external Authorization
  - Might consider enabling this by default
  - Show error if UI is shown without any authorization (detected e.g. by header presence)

### 🖥️ Platform Support
- 🐡 **BSD support** might happen
  - ⚠️ Requires using cross
  - I have no ability to test these.

### 🔧 Management Features
- 🗑️ **Uninstalls**
- 📝 **Self-registration endpoint** for host agents
  - ❓ Unclear how to deal with authorization:
    - Server secret?
