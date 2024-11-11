FROM rust:1.82.0 AS builder

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    USER=root cargo new --bin backend
WORKDIR /backend

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/backend/target \
    cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/backend/target \
    rm ./target/release/deps/backend* && \
    cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ffmpeg pkg-config libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /backend/target/release/backend /usr/local/bin/

EXPOSE 3001
CMD ["backend"]