FROM --platform=$BUILDPLATFORM rust:latest AS builder

RUN apt-get update && \
    apt-get install -y \
    ffmpeg \
    pkg-config \
    libssl-dev \
    gcc-aarch64-linux-gnu \
    libc6-dev-arm64-cross && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . .

ARG TARGETARCH
RUN case "$TARGETARCH" in \
        "amd64") echo "x86_64-unknown-linux-gnu" > /tmp/target ;; \
        "arm64") echo "aarch64-unknown-linux-gnu" > /tmp/target ;; \
        *) echo "Unsupported architecture: $TARGETARCH" && exit 1 ;; \
    esac

ENV PKG_CONFIG_ALLOW_CROSS=1

RUN rustup target add $(cat /tmp/target)

RUN if [ "$TARGETARCH" = "arm64" ]; then \
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc && \
        export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc && \
        export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++ && \
        export OPENSSL_DIR=/usr/include/aarch64-linux-gnu && \
        export OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu && \
        export OPENSSL_INCLUDE_DIR=/usr/include/aarch64-linux-gnu; \
    else \
        export OPENSSL_DIR=/usr/include && \
        export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu && \
        export OPENSSL_INCLUDE_DIR=/usr/include; \
    fi && \
    cargo build --release --target $(cat /tmp/target) && \
    mv target/$(cat /tmp/target)/release/backend .

FROM --platform=$TARGETPLATFORM debian:bullseye-slim

RUN apt-get update && \
    apt-get install -y ffmpeg libssl-dev && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/backend .

EXPOSE 3000

CMD ["./backend"]