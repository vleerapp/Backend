FROM rust:1.82.0 AS builder

RUN USER=root cargo new --bin backend
WORKDIR /backend

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/backend*
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y ffmpeg pkg-config libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /backend/target/release/backend /usr/local/bin/

EXPOSE 3001
CMD ["backend"]