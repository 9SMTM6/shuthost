FROM rust:alpine

RUN rustup component add llvm-tools

RUN apk add --no-cache \
    npm \
    perl \
    make \
    openssl-dev \
    musl-dev \
    pkgconfig \
    gcc \
    curl \
    llvm \
    tar

WORKDIR /src

# Get cargo-llvm-cov to allow for code coverage builds
RUN curl --proto '=https' --tlsv1.2 -fsSLO "https://github.com/taiki-e/cargo-llvm-cov/releases/latest/download/cargo-llvm-cov-x86_64-unknown-linux-musl.tar.gz"
RUN tar xzf cargo-llvm-cov-x86_64-unknown-linux-musl.tar.gz -C /usr/local/bin
RUN chmod +x /usr/local/bin/cargo-llvm-cov
