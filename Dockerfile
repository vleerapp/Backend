FROM --platform=$BUILDPLATFORM rust:latest AS builder

RUN apt-get update && \
    apt-get install -y ffmpeg && \
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

RUN rustup target add $(cat /tmp/target) && \
    cargo build --release --target $(cat /tmp/target) && \
    mv target/$(cat /tmp/target)/release/backend . && \
    rm -rf target && \
    rm -rf src Cargo.toml Cargo.lock .git .gitignore

FROM --platform=$TARGETPLATFORM debian:bullseye-slim

RUN apt-get update && \
    apt-get install -y ffmpeg && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/backend .

EXPOSE 3000

CMD ["./backend"]