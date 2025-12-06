FROM rust:alpine

RUN apk add --no-cache \
    npm \
    perl \
    make \
    openssl-dev \
    musl-dev \
    pkgconfig \
    gcc

WORKDIR /src