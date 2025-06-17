# <img src="coordinator/assets/favicon.svg" alt="ShutHost" width="24" height="24"> ShutHost

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-active-success.svg)]()

> 🚀 A neat little (well, at one time it was) helper that manages the standby state of unix hosts with Wake-On-Lan (WOL) configured, with Web-GUI.

⚠️ **Note**: LARGE parts of this project were LLM generated. I checked over all of them before committing, but it is what it is.

---

## 📋 Table of Contents

- [🏗️ Architecture](#️-architecture)
- [📋 Requirements](#-requirements)
- [🔒 Security](#-security)
- [⚠️ Known Issues](#️-known-issues)
- [🚀 Potential Features](#-potential-features)

---

## 🏗️ Architecture

📖 See [Architecture Documentation](coordinator/assets/architecture.md)

---

## 📋 Requirements

### 🤖 Agent Requirements
For the requirements for the agent, see [Requirements to install the agent](coordinator/assets/agent_install_requirements_gotchas.md).

### 🖥️ Coordinator Requirements

The coordinator must be run on a host that can reach the hosts you want to manage.

This requires either:
- 🔧 Running the coordinator as a **binary** on the host, or
- 🐳 Running it in a **docker container** with the host network mode enabled

> ⚠️ **Important**: This does not work with the default network mode that docker uses on Windows and MacOS. It will also not work on WSL. On these Hosts, you will have to run the coordinator as a binary, or install a Linux VM with bridged networking.

❌ **Windows is currently not supported**, even with the binary and/or WSL.

### 🌐 Network Configuration

The coordinator exposes its server on `127.0.0.1` only by default - so on localhost, ipv4, without remote access. This is for security reasons.

#### 🐳 Docker Access
To access the binary from Docker, use the address:
```
http://host.containers.internal:<port>
```

Other container solutions might require additional configuration. On Podman, add the following to the container that wants to access the coordinator:
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

| Issue | Description | Impact |
|-------|-------------|---------|
| 🔄 **Missed Shutdown** | If the host misses the initial shutdown, a "full cycle" is required to send it again (release lease, take lease) | Medium |
| 💾 **State Loss** | The coordinator loses state on update | Low (currently only acts on state changes) |
| 🐳 **Docker Testing** | Docker is currently untested | Unknown |
| 🪟 **Windows Support** | Windows agent support currently not planned, due to large differences | N/A |
| 🌐 **Docker Connectivity** | Accessing the coordinator from Docker requires proper configuration | Medium |

> 💡 **Potential Solutions**: 
> - Considering regularly "syncing" states, maybe with explicit config on host (seems best) or coordinator-wide
> - State persistence could be fixed with e.g. sqlite. Should be considered before adding status syncing

---

## 🚀 Potential Features

### 🔐 Authentication & Authorization
- 🆔 **OIDC authorization** where I allow the required endpoints for all
  - Might consider enabling this by default
  - Show error if UI is shown without any authorization (detect by header presence)

### 🖥️ Platform Support
- 🐡 **BSD support** might happen
  - ⚠️ Requires using cross (won't do locally)
  - Means refactoring the GitHub pipeline
  - Would need to introduce features to build locally

### 🔧 Management Features
- 🗑️ **Uninstalls**
- 📝 **Self-registration endpoint** for host agents
  - ❓ Unclear how to deal with authorization:
    - Server secret?
    - Page is supposed to be behind reverse proxy...
    - Page is supposed to be behind reverse proxy...
