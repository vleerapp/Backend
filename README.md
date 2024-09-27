# Backend

To run this instance yourself you can choose between hub.docker.com or github packages
```zsh
docker run -d --name vleer-backend -p 3000:3000 -v vleer-cache:/usr/src/app/cache ghcr.io/vleerapp/backend:latest
```
```zsh
docker run -d --name vleer-backend -p 3000:3000 -v vleer-cache:/usr/src/app/cache vleerapp/backend:latest
```
