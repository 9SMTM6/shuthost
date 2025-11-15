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

## 📚 Documentation & Resources

Extended documentation, examples, and additional resources to help you get the most out of ShutHost:

- [💿 Installation](#-installation)
- [📚 Examples](docs/examples/)
- [📋 Requirements](docs/requirements.md)
- [🔒 Security Considerations](docs/security_considerations.md)
- [❓ FAQ](docs/FAQ.md)
- [📷 UI screenshots](#-ui-screenshots)
- [🖥️ Platform Support](frontend/assets/partials/platform_support.md)
- [🛜 WebUI Network Configuration](docs/examples/webui-network-config.md)
- [⚙️ Full Configuration Example](docs/examples/example_config.toml)
- [🏗️ Architecture](https://9smtm6.github.io/shuthost/#architecture)
- [🚀 Potential Features](#-potential-features)
- [📖 API Documentation](docs/API.md)
- [🤝 Contributing](docs/CONTRIBUTING.md)

---

## 💿 Installation

Choose either the binary (recommended for reliability and WOL support) or the container (Linux only) installation.

Binary (recommended)
- Download the latest release from: https://github.com/9SMTM6/shuthost/releases/latest
    ```bash
    uname -m
    # Possible outputs: x86_64 => Intel/AMD, aarch64 => ARM
    # Linux on Intel/AMD
    curl -L -o shuthost_coordinator "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator-x86_64-unknown-linux-musl"
    # There are also gnu binaries available, but the musl variants have wider compatibility for users that dont know which version they have.
    # Linux on ARM
    curl -L -o shuthost_coordinator "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator-aarch64-unknown-linux-musl"
    # macOS on Apple Silicon
    curl -L -o shuthost_coordinator "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator-aarch64-apple-darwin"
    # macOS on Intel
    curl -L -o shuthost_coordinator "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator-x86_64-apple-darwin"
    ```
- Install as a system service (binary supports systemd/openrc/launchd)
  - Install command (installs binary, creates a config with restrictive permissions and enables start-on-boot):
    ```bash
    # Make the binary executable
    chmod +x shuthost_coordinator
    # Run the installer (installs binary, creates a config with restrictive permissions and enables start-on-boot)
    sudo ./shuthost_coordinator install #<optional user>
    # Remove the binary (it'll have been copied to the appropriate location by the installer)
    rm shuthost_coordinator
    # Access the WebUI at http://localhost:8080
    ```
  - Notes:
    - The installer infers the target user from SUDO_USER if you run under sudo, otherwise the user is required to be specified.
    - The installer will create service units for systemd or openrc where appropriate and set config file ownership/permissions.

Docker (Linux only)
-  Download the [example_config.toml](docs/examples/example_config.toml) and [docker-compose.yml](docs/examples/docker-compose.yml) from Github and run the service:
    ```bash
    # Create config directory and download the example config from GitHub
    mkdir -p coordinator_config data
    curl -L -o coordinator_config/coordinator_config.toml \
      https://raw.githubusercontent.com/9SMTM6/shuthost/main/docs/examples/example_config.toml

    # Set restrictive permissions (readable/writable by owner only)
    chmod 600 coordinator_config/coordinator_config.toml
    # Download the docker-compose file
    curl -L -o docker-compose.yml \
      https://raw.githubusercontent.com/9SMTM6/shuthost/main/docs/examples/docker-compose.yml

    # Run the service in the background
    docker-compose up -d shuthost

    # Access the WebUI at http://localhost:8080
    ```
- Notes:
  - Uses `network_mode: host` to reach the hosts with the Wake-on-LAN packet. This setting is Linux-only and will not work properly on Docker Desktop for Mac/Windows. Use the binary on Mac or run on a Linux VM with bridged networking on Mac or Windows.

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

## 🚀 Potential Future Features

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
