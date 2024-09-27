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
RUN apt-get update && \
    apt-get install -y gcc-aarch64-linux-gnu && \
    rustup target add aarch64-unknown-linux-gnu && \
    cargo build --release --target aarch64-unknown-linux-gnu && \
    mv target/aarch64-unknown-linux-gnu/release/backend /app/backend

FROM --platform=$TARGETPLATFORM debian:bullseye-slim AS runtime
RUN apt-get update && \
    apt-get install -y ffmpeg libssl-dev && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder-${TARGETARCH} /app/backend .
EXPOSE 3000
CMD ["./backend"]