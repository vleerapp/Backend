# Backend

A custom backend for Vleer that will be available opensource for everyone to host their own instance.

This comes with following features:

- [x] YouTube Music Streaming
- [x] YouTube Music Download
- [x] YouTube Music Search
- [x] YouTube Music Thumbnails
- [x] YouTube Music Download (compressed/lossless)
- [ ] Spotify Podcast
- [ ] ~~Apple Music lossless Download~~

To run this instance yourself you can choose between hub.docker.com or github packages
```zsh
docker run -d --name vleer-backend -p 3000:3000 -v vleer-cache:/usr/src/app/cache ghcr.io/vleerapp/backend:latest
```
```zsh
docker run -d --name vleer-backend -p 3000:3000 -v vleer-cache:/usr/src/app/cache vleerapp/backend:latest
```
