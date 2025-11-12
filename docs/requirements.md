# Requirements

This document outlines the system requirements and platform support for ShutHost components.

## 🤖 Agent Requirements

For the requirements for the agent, see [Requirements to install the agent](../frontend/assets/agent_install_requirements_gotchas.md).

## 🖥️ Coordinator Requirements

The coordinator must be run on a system that can reach the hosts you want to manage.

Assuming that the coordinator-host is on the same network as the hosts, with WOL broadcasts allowed, this requires additionally:
- 🔧 Running the coordinator as a **binary** on the coordinator-host, or
- 🐳 Running it in a **docker container** with the host network mode enabled

> ⚠️ **Important**: This does not work with the default network mode that docker uses on Windows and MacOS. It will also not work on WSL. See [Platform Support](../frontend/assets/partials/platform_support.md).
