# Understanding the `enforce_state` Behavior

Refer to the [example configuration](./example_config.toml).

The `enforce_state` field in the ShutHost configuration allows the coordinator to periodically enforce the desired state of a host, even when no lease changes occur. This feature is useful in scenarios where external factors might alter the host's state, and you want to ensure the host remains in the desired state.

## How It Works

When `enforce_state` is set to `true` for a host, the coordinator will:

1. Periodically check the current state of the host.
2. Compare the current state with the desired state.
3. Send wake or shutdown commands as needed to align the host's state with the desired state.

If `enforce_state` is set to `false`, the coordinator will only send commands when a lease change occurs (edge-triggered behavior).

## Situations Where `enforce_state=false` May Not Wake/Shutdown a Host

When `enforce_state` is set to `false` - which is the default -, the coordinator will not take action in the following scenarios:

- **Manual State Changes**: If a host is manually powered on or off without a corresponding lease change, the coordinator will not intervene.
- **External Interference**: If an external system or user modifies the host's state (e.g., through a hardware button or another management tool), the coordinator will not correct the state.
- **Network Disruptions**: If the coordinator temporarily loses connectivity to the host, it will not attempt to re-enforce the state once connectivity is restored unless a lease change occurs.
