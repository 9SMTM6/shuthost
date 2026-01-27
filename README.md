# <img src="frontend/assets/favicon.svg" alt="ShutHost" width="24" height="24"> ShutHost

[![License: GPL-2.0-only](https://img.shields.io/badge/license-GPL--2.0-blue.svg)]()
[![Status](https://img.shields.io/badge/status-active-success.svg)]()
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/9SMTM6/shuthost/main.yaml?label=build%20%26%20test)](https://github.com/9SMTM6/shuthost/actions/workflows/main.yaml)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/9SMTM6/shuthost/qa.yml?label=QA)](https://github.com/9SMTM6/shuthost/actions/workflows/qa.yml)

A neat helper that manages the standby state of unix and windows hosts with Wake-On-Lan (WOL) configured, with Web-GUI.

> Note: LARGE parts of this project were LLM generated. None were blindly committed, but it is what it is.

[![Live demo: PWA controlling NAS aka old PC (2x speed)](docs/shuthost_live_demo_2x.webp)](./docs/shuthost_live_demo_2x.webp)
> played at 2x speed, using the WebUI installed as PWA

⚠️ **Note**: the short demo clip shown above is slightly out of date with respect to theming and layout. Check the [live demo](https://9SMTM6.github.io/shuthost/) or [screenshots below](#-ui-screenshots) for the current UI.

## 🌐 Live Demo

You can try a demo of the ShutHost WebUI (no backend, simulated data) via GitHub Pages:

<p style="text-align: center;"><a href="https://9SMTM6.github.io/shuthost/" style="display: inline-block; padding: 7px 15px; background-color: #007bff; color: white; text-decoration: none; border-radius: 6px; font-weight: bold; font-size: 16px;">🚀 Live Demo</a></p>

This demo runs entirely in your browser and does not control any real hosts. It is useful for previewing the UI and some of the features without installing anything.
Note that the theme (light/dark) is selected based on your system preference.

---

## ✨ Features

- Manage standby state of Linux, MacOS and Windows hosts with Wake-On-Lan (WOL) and lightweight agents
- Web-based GUI for easy management
  - Light/Dark theme are selected based on system preference (with CSS media queries)
  - installable as [PWA](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Guides/Installing#installing_and_uninstalling_pwas)
    - this allows behavior similar to an native app on e.g. Android
- API for machine-to-machine control (e.g. backups)
- customizable, e.g. custom shutdown/sleep command per host
- Should support extension (e.g. Home Assistant)
- Convenience scripts for simple installation of agents, clients and coordinator
- Docker and simple binary deployment options for the coordinator (due to how Wake-on-LAN works docker only supports Linux though)
- simplified agent-only deployment doesn't require a coordinator on an always-on host
- security wasn't an afterthought

---

## 📚 Documentation & Resources

Extended documentation, examples, and additional resources to help you get the most out of ShutHost:

- [🧭 ShutHost Design & Operation](#-shuthost-design--operation)
- [💿 Installation](#-installation)
- [⚡ Agent-only Install](#-agent-only-install)
- [📚 Examples](docs/examples/)
- [📋 Requirements](docs/requirements.md)
- [🔒 Security Considerations](docs/security_considerations.md)
- [❓ FAQ](docs/FAQ.md)
- [📷 UI screenshots](#-ui-screenshots)
- [🖥️ Platform Support](frontend/assets/partials/platform_support.md)
- [🏗️ Architecture](https://9smtm6.github.io/shuthost/#architecture)
- [🚀 Potential Future Features](#-potential-future-features)
- [🤝 Contributing](docs/CONTRIBUTING.md)

---
## 🧭 ShutHost Design & Operation

ShutHost began from a simple observation: Wake-on-LAN (WOL) is reasonably standardized for starting machines on a LAN, but there is no well-established, safe equivalent for remotely shutting down running systems. Some projects try to solve this—for example, [sleep-on-lan](https://github.com/SR-G/sleep-on-lan) and snippets/guides that log in via SSH and shut down the computer that way—but those approaches commonly enlarge the attack surface, are difficult to deploy, and lack usability.

ShutHost addresses these challenges through three key design decisions:

- **Authorization & safety:** Remote shutdown commands pose risks of accidental or malicious denial-of-service. To mitigate this, ShutHost requires authenticated requests: shutdowns are authorized using HMAC-signed messages with timestamps to prevent replay attacks and avoid sending plaintext credentials over the network.
- **Privilege & init integration:** Performing a shutdown usually requires elevated privileges and must persist across reboots. ShutHost provides lightweight host agents that integrate with common service managers so the shutdown capability is available after restarts. Supported integrations include `systemd` (the dominant init on most mainstream Linux distributions), `openrc` (used by distributions like Alpine and Gentoo), and `launchd` (macOS). A "self-extracting" mode is also available for custom or manual setups where users handle init integration themselves (see [Deploying the Self-Extracting Agent on Unraid](docs/examples/unraid-self-extracting-agent-deployment.md) for an example).
- **Network reachability & central control:** Wake-on-LAN only operates on the local broadcast domain. To manage hosts from outside the LAN, ShutHost includes a coordinator component: a single LAN-hosted coordinator provides a web GUI (installable as a PWA) and an API. The coordinator sends WOL packets to start machines locally and forwards authenticated shutdown requests to host agents over IP.

Host agents are intentionally minimal and designed for security. They use IP-addressed, authenticated requests and avoid running full-featured HTTP servers. This reduces the attack surface for components that typically run with elevated privileges. The `host_agent` performs the actual shutdown and registers with the host's service manager so the capability survives reboots. The `host_agent` can also be used standalone with direct control scripts (see [Agent-only Installation](docs/examples/agent-installation.md)); its API is documented in [docs/API.md](docs/API.md). The `host_agent` supports custom shutdown commands, allowing users to define how their systems should be powered down or put to sleep—this can also be seen in the [Unraid example](docs/examples/unraid-self-extracting-agent-deployment.md).

![Host-Agent Interaction](frontend/assets/generated/host_agent_interaction.svg)

The coordinator glues the pieces together and provides usability features:

- A web UI and API make it easy to start/stop machines and integrate with other services.
- The coordinator doesn't require elevated privileges to run.
- The coordinator offers an installer and convenience scripts that simplify deploying `host_agent`s on the LAN and clients over the internet.
- A lease system prevents hosts from being shut down while a client holds an active lease (for instance, while a backup job is running).
  > This safety depends on all starts and stops going through the coordinator (either the UI or a client using the coordinator API); actions performed outside the coordinator are outside its control.

![Lease system explainer](frontend/assets/generated/client_controller_interaction.svg)

## 💿 Installation

Choose either the binary or the container (Linux only) installation.
Windows [isn't supported by the coordinator](frontend/assets/partials/platform_support.md); use a Linux VM or install the agent only (see [Agent-only Install](#-agent-only-install)).

#### Binary
- Use the [automated installation script](scripts/enduser_installers/coordinator.sh):
  ```bash
  curl -fsSL https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator_installer.sh | sh
  ```
  This script will automatically detect your platform, download the appropriate binary, print the checksum, and install the coordinator as a system service.

- Or follow the [manual steps](docs/examples/manual_install.md).

#### Docker (Linux only)
-  Download the [example_config.toml](docs/examples/example_config.toml) and [docker-compose.yml](docs/examples/docker-compose.yml) from Github and run the service:
    ```bash
    # Create config directory and download the example config from GitHub
    mkdir -p coordinator_config data
    curl -L -o coordinator_config/config.toml \
      https://raw.githubusercontent.com/9SMTM6/shuthost/main/docs/examples/example_config.toml
    
    # Set restrictive permissions (readable/writable by owner only)
    chmod 600 coordinator_config/config.toml
    # Download the docker-compose file
    curl -L -o docker-compose.yml \
      https://raw.githubusercontent.com/9SMTM6/shuthost/main/docs/examples/docker-compose.yml
    
    # Run the service in the background
    docker-compose up -d shuthost
    
    # Access the WebUI at http://localhost:8080
    ```
- Notes:
  - Uses `network_mode: host` to reach the hosts with the Wake-on-LAN packet. This setting is Linux-only and will not work properly on Docker Desktop for Mac/Windows. Use the binary on Mac or run on a Linux VM with bridged networking on Mac or Windows.

![Deployment Possibilities](frontend/assets/generated/deployment.svg)

### Agent / Client installation
- To install a host-agent (controls the hosts): open the web UI, open "Install Host Agent" and follow the instructions shown.
- To install a client (M2M, e.g., backup scripts): switch to the Clients tab, open "Install Client" and follow the instructions shown.

## ⚡ Agent-only Install

Lightweight option: install the host agent only (no coordinator). This does not require an always-on coordinator or a domain; it is easy to deploy but has limitations — the control scripts work only on the same LAN. See the detailed example in [docs/examples/agent-installation.md](docs/examples/agent-installation.md).

![Direct Control Comparison with LAN Limitation](frontend/assets/generated/direct_control_comparison.svg)

> **Note for Windows users:** Windows agents are only available as self-extracting archives. You must manually configure the agent to start on boot using a service manager like [NSSM](https://nssm.cc/).
>
> ⚠️ **Warning:** Windows agents currently have known issues in most deployments, which are being debugged.

Install the released agent installer and generate a direct-control script:

<!-- Note: If you update these invocations update the corresponding invocations in tests! See .github/workflows/main.yaml and scripts/tests/install-and-run-direct-control.sh -->
```bash
# Install the agent:
curl -fsSL https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_host_agent_installer.sh | sh
# Generate a direct-control script (run on the machine where the agent binary is installed):
# If the agent is in your PATH (it should be by default):
sudo shuthost_host_agent generate-direct-control
```
```powershell
# For Windows (PowerShell):
curl.exe -fLO "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_host_agent_installer.ps1"
powershell -ExecutionPolicy Bypass -File .\shuthost_host_agent_installer.ps1
# Then generate direct-control script from the self-extracting script:
powershell -ExecutionPolicy Bypass -File .\shuthost_host_agent_self_extracting.ps1 generate-direct-control
```
```bash
# Move the script to the device you want to use as the controller (same LAN).
# copy via scp, USB, etc.
# Ensure its executable (on Unix):
chmod +x shuthost_direct_control_<hostname>
```

After moving the direct-control script to the controller device, you can run `./shuthost_direct_control_<hostname> wake`, `./shuthost_direct_control_<hostname> status` or `./shuthost_direct_control_<hostname> shutdown` while on the same LAN. See the example document for tradeoffs and security notes.


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

## 🚀 Potential Future Features

### 🎯 Core Features
- 🔌 **Custom Wakers**: Support for alternative wake mechanisms beyond WOL, such as smart plugs or custom scripts (e.g., via API integrations). This would allow hosts without WOL support to be managed through external devices or services.
- 🔔 **Notifications about host state changes through the PWA**
- 📊 **Host state tracking for statistics**
- 🛡️ **Rate limiting of requests by shuthost clients**

### 🖥️ Platform Support
- 🐡 **BSD support** might happen
  - ⚠️ Requires using more advanced cross compilation
  - I have no ability to test these practically myself.

### 🔧 Management Features
- 🗑️ **Uninstalls**
- 📝 **Self-registration endpoint** for host agents
  - ❓ Unclear how to deal with authorization:
    - Server secret?

<!-- see https://crates.io/crates/ceviche https://crates.io/crates/windows-service https://crates.io/crates/planif -->
<!-- 
TODO: actually add bare metal tests for windows variants, validate workflow triggers

todo: add a bunch of pwsh scripts for windows agent
 * add tests to CI
 * consider running tests on metal generally and for windows specifically.


* todo: port test-client-scripts to run locally as well
-->
