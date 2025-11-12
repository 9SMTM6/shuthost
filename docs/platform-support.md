# Platform Support Matrix

This document outlines the platform support for different ShutHost components across various operating systems.

## Requirements Context

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

## Component Explanations

- **Web GUI**: The web-based user interface that works in any modern browser
- **Coordinator**: The central server component that manages hosts and clients
- **Host Agent**: Lightweight agents installed on managed hosts for power control
- **Client**: Command-line tools for machine-to-machine communication (e.g., backup scripts)

## Support Legend

- ✅ **Supported**: Fully supported and tested
- ❌ **Not Supported**: Not available on this platform
- **Binary**: Native executable for the platform
- **Docker**: Container deployment option
- **Linux VM**: Requires a Linux virtual machine with bridged networking
- **Shell**: POSIX shell scripts
- **PowerShell**: Windows PowerShell scripts
- **WSL**: Windows Subsystem for Linux compatibility
