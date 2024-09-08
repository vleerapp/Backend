import express from 'express';
import downloadRouter from './routes/download';
import streamRouter from './routes/stream';
import searchRouter from './routes/search';
import searchSpotifyRouter from './routes/searchSpotify';
import thumbnailRouter from './routes/thumbnail';

const app = express();
const port = 3000;

const isDevelopment = process.env.NODE_ENV === 'development';

export const log = (message: string) => {
  if (isDevelopment) {
    console.log(`[${new Date().toLocaleString()}] ${message}`);
  }
};

app.use(express.json());

app.use((req, res, next) => {
  res.setHeader('Access-Control-Allow-Origin', '*');
  next();
});

app.get('/', (req, res) => {
  res.send('<h1>👋</h1>');
});

app.use('/download', downloadRouter);
app.use('/stream', streamRouter);
app.use('/search', searchRouter);
app.use('/searchSpotify', searchSpotifyRouter);
app.use('/thumbnail', thumbnailRouter);

app.listen(port, '0.0.0.0', () => {
  log(`🚀 Server running on port :${port}`);
});
