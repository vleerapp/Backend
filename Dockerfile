FROM rust:alpine
WORKDIR /usr/src/app
RUN apt-get update && \
    apt-get install -y ffmpeg && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release
EXPOSE 3000
CMD ["./target/release/backend"]