# Use an official Rust image as the base
FROM rust:latest as builder

# Set the working directory
WORKDIR /usr/src/controller

# Copy the controller code and build it
COPY . .
RUN cargo build --release --bin controller

# Stage 2: Create the final image
FROM debian:stable-slim

# Create a directory to store the agent binary
RUN mkdir -p /web_root

# Copy the controller binary and agent binary
COPY --from=builder /usr/src/controller/target/release/controller /usr/local/bin/
COPY shuthost_agent /web_root
ENV AGENT_BINARIES_DIR=/web_root
ENV CONFIG_PATH=/config/controller_config.toml
VOLUME [ "/config" ]

# Expose the port for the HTTP server
EXPOSE 8081

# TODO: Doesn't work yet:
# * seems to need host networking to reach hosts with WOL per Mac (maybe look into what wol is doing with the -i option)
# * that means no reachability from bridge networking with reverse proxies, and also right now IDK at all how I reach the server from host (localhost:port doesn't work).
# * building is... annoying. I dont think I will be able to buid for all targets in docker at all... oh, wait, maybe with cross images?

# MY current best guess to the current best invocation:
# podman run -p 8081:8081 -v ./:/config --network host shuthost

# Set up the entry point for the controller
CMD ["controller"]
