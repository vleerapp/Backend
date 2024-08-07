import express from 'express';
import downloadRouter from './routes/download';
import streamRouter from './routes/stream';
import searchRouter from './routes/search';
import searchSpotifyRouter from './routes/searchSpotify';

const app = express();
const port = 3000;

app.use(express.json());

app.get('/', (req, res) => {
  res.send('<h1>👋</h1>');
});

app.use('/download', downloadRouter);
app.use('/stream', streamRouter);
app.use('/search', searchRouter);
app.use('/searchSpotify', searchSpotifyRouter);

app.listen(port, '0.0.0.0', () => {
  console.log(`[${new Date().toLocaleString()}] 🚀 Server running on port :${port}`);
});