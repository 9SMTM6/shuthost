# Manual Installation

- Download the latest release from: https://github.com/9SMTM6/shuthost/releases/latest
    ```bash
    uname -m
    # Possible outputs: x86_64 => Intel/AMD, aarch64 => ARM/Apple Silicon
    ```
    ```bash
    # Linux on Intel/AMD
    curl -fL -o shuthost_coordinator.tar.gz "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator-x86_64-unknown-linux-musl.tar.gz"
    # There are also gnu binaries available, but the musl variants have wider compatibility for users that dont know which version of libc they have.
    ```
    ```bash
    # Linux on ARM
    curl -fL -o shuthost_coordinator.tar.gz "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator-aarch64-unknown-linux-musl.tar.gz"
    ```
    ```bash
    # macOS on Apple Silicon
    curl -fL -o shuthost_coordinator.tar.gz "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator-aarch64-apple-darwin.tar.gz"
    ```
    ```bash
    # macOS on Intel
    curl -fL -o shuthost_coordinator.tar.gz "https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_coordinator-x86_64-apple-darwin.tar.gz"
    ```
    ```bash
    # Optionally verify the checksum against the one provided on the releases page
    shasum -a 256 shuthost_coordinator.tar.gz
    # Extract the downloaded archive
    tar -xzf shuthost_coordinator.tar.gz
    rm shuthost_coordinator.tar.gz
    ```
- Install as a system service (binary supports systemd/openrc/launchd)
  ```bash
  # Run the installer (installs binary, creates a config with restrictive permissions and enables start-on-boot)
  # optionally specify the user as first argument (inferred from SUDO_USER if run under sudo, otherwise that argument is required)
  sudo ./shuthost_coordinator install
  # Remove the binary (it'll have been copied to the appropriate location by the installer)
  rm shuthost_coordinator
  # Access the WebUI at http://localhost:8080
  ```
- Notes:
  - The installer will create service units for systemd or openrc where appropriate and set config file ownership/permissions.