FROM alpine:latest
# THIS IS NON-TESTED!

ARG RUSTC_TARGET=x86_64-unknown-linux-musl

# Copy the correct binary based on the provided target
COPY target/${RUSTC_TARGET}/release/shuthost_coordinator /usr/sbin/

ENV SHUTHOST_CONTROLLER_CONFIG_PATH=/config/coordinator_config.toml

# Declare the bind location for the config (note declaring like that is just for reference)
VOLUME [ "/config" ]

# Expose the port for the HTTP server (note exposing like that is just for reference, might be the wrong port depending on config)
EXPOSE 8080

# MY current best guess to the current best invocation:
# podman run -p 8080:8080 -v ./:/config --network host shuthost

# Set up the entry point for the coordinator
CMD ["shuthost_coordinator", "control-service"]
