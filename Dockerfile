FROM rust:latest

RUN apt-get update && \
    apt-get install -y ffmpeg && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . .

RUN cargo build --release && \
    mv target/release/backend . && \
    rm -rf target && \
    rm -rf src Cargo.toml Cargo.lock .git .gitignore

EXPOSE 3000

CMD ["./backend"]