# Requirements

This document outlines the system requirements and platform support for ShutHost components.

## 🤖 Agent Requirements

For the requirements for the agent, see [Requirements to install the agent](frontend/assets/agent_install_requirements_gotchas.md).

## 🖥️ Coordinator Requirements

The coordinator must be run on a system that can reach the hosts you want to manage.

Assuming that the coordinator-host is on the same network as the hosts, with WOL broadcasts allowed, this requires additionally:
- 🔧 Running the coordinator as a **binary** on the coordinator-host, or
- 🐳 Running it in a **docker container** with the host network mode enabled

> ⚠️ **Important**: This does not work with the default network mode that docker uses on Windows and MacOS. It will also not work on WSL. On these Hosts, you will have to run the coordinator as a binary, or install a Linux VM with bridged networking to run docker.

❌ **Windows is currently not supported for coordinators or host agents**, even with the binary and/or WSL. You need a VM or a dedicated Linux machine for those components. However, **Windows clients are supported** via PowerShell scripts.

## Platform Support Matrix

| Component     | Linux                          | macOS                          | Windows                                      |
|---------------|--------------------------------|--------------------------------|----------------------------------------------|
| Web GUI       | ✅ (any modern browser)       | ✅ (any modern browser)       | ✅ (any modern browser)                       |
| Coordinator   | ✅ Binary<br>✅ Docker         | ✅ Binary<br>❌ Docker<br>✅ Linux VM (bridged networking) | ❌ Binary<br>❌ Docker<br>❌ WSL<br>✅ Linux VM (bridged networking) |
| Host Agent | ✅ Binary         | ✅ Binary         | ❌ Binary              |
| Client        | ✅ Shell<br>✅ Docker   | ✅ Shell<br>✅ Docker  | ✅ PowerShell<br>✅ Docker<br>✅ WSL (Shell)  |