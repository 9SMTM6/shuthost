# Use an official Rust image as the base
FROM rust:latest as builder

# Set the working directory
WORKDIR /usr/src/controller

# Copy the controller code and build it
COPY . .
RUN cargo build --release

# Stage 2: Create the final image
FROM debian:bullseye-slim

# Install any necessary dependencies
RUN apt-get update && apt-get install -y \
    curl \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a directory to store the agent binary
RUN mkdir -p /opt/agent

# Copy the controller binary and agent binary
COPY --from=builder /usr/src/controller/target/release/controller /usr/local/bin/
COPY agent_binary /opt/agent/agent

# Expose the port for the HTTP server
EXPOSE 8080

# Set up the entry point for the controller
CMD ["controller"]
