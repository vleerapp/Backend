FROM rust:latest

WORKDIR /usr/src/app

RUN apt-get clean && \
    apt-get update && \
    apt-get install -y ffmpeg

COPY Cargo.lock Cargo.toml ./

RUN cargo fetch

COPY . .

RUN cargo build --release

RUN mkdir -p /usr/src/app/cache/compressed /usr/src/app/cache/lossless

EXPOSE 3000

CMD ["./target/release/Backend"]