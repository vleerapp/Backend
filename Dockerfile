FROM rust:1.82.0 AS builder

WORKDIR /backend
COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/backend/target,id=rust_target \
    cargo build --release && \
    cp target/release/backend /backend/backend

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ffmpeg pkg-config libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /backend/backend /usr/local/bin/

EXPOSE 3001
CMD ["backend"]