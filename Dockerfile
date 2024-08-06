FROM oven/bun:1

WORKDIR /usr/src/app

RUN --mount=type=cache,target=/var/cache/apt \
    --mount=type=cache,target=/var/lib/apt \
    apt-get update && \
    apt-get install -y python3-pip ffmpeg && \
    pip3 install yt-dlp && \
    apt-get clean

COPY package.json bun.lockb ./

RUN --mount=type=cache,target=/root/.bun \
    bun install && \
    bun add -d typescript

COPY . .

RUN bunx tsc

RUN mkdir -p /usr/src/app/cache/compressed /usr/src/app/cache/lossless

EXPOSE 3000

CMD ["bun", "start"]