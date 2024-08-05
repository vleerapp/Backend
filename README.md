# Backend

To run this instance yourself you can choose between hub.docker.com or github packages
```zsh
docker run -d -p 3000:3000 ghcr.io/vleerapp/backend:latest
```
```zsh
docker run -d -p 3000:3000 vleerapp/backend:latest
```



### To install dependencies:

```bash
bun install
```

To run:

```bash
bun run index.ts
```

This project was created using `bun init` in bun v1.1.15. [Bun](https://bun.sh) is a fast all-in-one JavaScript runtime.
