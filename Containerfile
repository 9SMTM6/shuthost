FROM alpine:latest
# THIS IS NON-TESTED!

# Use TARGETARCH as set by buildx
ARG TARGETARCH

# Map TARGETARCH to Rust arch
ENV RUST_ARCH="$(if [ "$TARGETARCH" = "amd64" ]; then echo x86_64; elif [ "$TARGETARCH" = "arm64" ]; then echo aarch64; else echo x86_64; fi)"
ENV RUST_MUSL_TARGET="$RUST_ARCH-unknown-linux-musl"

# Copy the correct binary based on the mapped target
COPY target/${RUST_MUSL_TARGET}/release/shuthost_coordinator /usr/sbin/

ENV SHUTHOST_CONTROLLER_CONFIG_PATH=/config/coordinator_config.toml

# Declare the bind location for the config (note declaring like that is just for reference)
VOLUME [ "/config" ]

# Expose the port for the HTTP server (note exposing like that is just for reference, might be the wrong port depending on config)
EXPOSE 8080

# MY current best guess to the current best invocation:
# podman run -p 8080:8080 -v ./:/config --network host shuthost

# Set up the entry point for the coordinator
CMD ["shuthost_coordinator", "control-service"]
