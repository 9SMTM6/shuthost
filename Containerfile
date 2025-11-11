FROM alpine:latest

ARG RUSTC_TARGET=x86_64-unknown-linux-musl

# Copy the correct binary based on the provided target
# Yes, you need to build the binary before doing docker build.
# Since the agents for multiple targets are required for a complete coordinator,
# and macOS can't be built in docker,
# we need to build these outside of the container anyways.
# 
# This containerfile is mostly to be used in the CI pipeline.
COPY target/${RUSTC_TARGET}/release/shuthost_coordinator /usr/sbin/

ENV SHUTHOST_CONTROLLER_CONFIG_PATH=/config/coordinator_config.toml

# Declare the bind location for the config (note declaring like that is just for reference)
VOLUME [ "/config" ]

# Expose the port for the HTTP server (note exposing like that is just for reference, might be the wrong port depending on config)
EXPOSE 8080

# Set up the entry point for the coordinator
CMD ["shuthost_coordinator", "control-service"]
