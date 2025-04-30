# THIS IS NON-FUNCTIONAL!

# Stage 2: Create the final image
FROM debian:stable-slim

COPY target/aarch64-unknown-linux-gnu/release/controller /usr/local/bin/
# COPY ./target/x86_64-unknown-linux-gnu/release/controller /usr/local/bin/
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
