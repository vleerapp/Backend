FROM --platform=$BUILDPLATFORM rust:latest AS builder-base
RUN apt-get update && \
    apt-get install -y ffmpeg pkg-config libssl-dev && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .

FROM builder-base AS builder-amd64
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc
RUN rustup target add x86_64-unknown-linux-gnu && \
    cargo build --release --target x86_64-unknown-linux-gnu && \
    mv target/x86_64-unknown-linux-gnu/release/backend /app/backend

FROM builder-base AS builder-arm64
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
RUN dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install -y \
        gcc-aarch64-linux-gnu \
        libssl-dev:arm64 \
        pkg-config \
        crossbuild-essential-arm64 && \
    rustup target add aarch64-unknown-linux-gnu && \
    PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig \
    PKG_CONFIG_SYSROOT_DIR=/ \
    OPENSSL_DIR=/usr/lib/aarch64-linux-gnu \
    OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu \
    OPENSSL_INCLUDE_DIR=/usr/include/aarch64-linux-gnu \
    AARCH64_UNKNOWN_LINUX_GNU_OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu \
    AARCH64_UNKNOWN_LINUX_GNU_OPENSSL_INCLUDE_DIR=/usr/include/aarch64-linux-gnu \
    cargo build --release --target aarch64-unknown-linux-gnu && \
    mv target/aarch64-unknown-linux-gnu/release/backend /app/backend

FROM ubuntu:22.04 AS runtime
RUN apt-get update && \
    apt-get install -y ffmpeg libssl3 && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app

FROM runtime AS runtime-amd64
COPY --from=builder-amd64 /app/backend .

FROM runtime AS runtime-arm64
COPY --from=builder-arm64 /app/backend .

FROM runtime-$TARGETARCH
EXPOSE 3001
CMD ["./backend"]