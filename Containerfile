FROM alpine:latest
# THIS IS NON-TESTED!

ARG TARGETARCH2="x86_64-unknown-linux-musl"

COPY target/${TARGETARCH2}/release/shuthost_controller /usr/local/bin/

ENV SHUTHOST_CONTROLLER_CONFIG_PATH=/config/controller_config.toml

# Declare the bind location for the config (note declaring like that is just for reference)
VOLUME [ "/config" ]

# Expose the port for the HTTP server (note exposing like that is just for reference, might be the wrong port depending on config)
EXPOSE 8080

# MY current best guess to the current best invocation:
# podman run -p 8080:8080 -v ./:/config --network host shuthost

# Set up the entry point for the controller
CMD ["shuthost_controller", "control-service"]
