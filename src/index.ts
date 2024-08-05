import express from 'express';
import YTDlpWrap from 'yt-dlp-wrap';
import fs from 'fs';
import path from 'path';

const app = express();
const port = 3000;

app.use(express.json());

app.get('/', (req, res) => {
  res.send('👋');
});

app.get('/stream', async (req, res) => {
  const { id } = req.query;
  if (!id || typeof id !== 'string') {
    console.log('Invalid or missing id parameter');
    return res.status(400).json({ error: 'Invalid or missing id parameter' });
  }

  const videoUrl = `https://www.youtube.com/watch?v=${id}`;
  const cacheDir = path.resolve(process.cwd(), 'cache');
  const cacheFilePath = path.join(cacheDir, `${id}.wav`);

  try {
    if (fs.existsSync(cacheFilePath)) {
      console.log(`Cached file found for id: ${id}`);
      return res.sendFile(cacheFilePath);
    }

    console.log(`Cache miss for id: ${id}, downloading...`);
    if (!fs.existsSync(cacheDir)) {
      console.log('Creating cache directory');
      fs.mkdirSync(cacheDir, { recursive: true });
    }

    const ytDlp = new YTDlpWrap();

    await new Promise<void>((resolve, reject) => {
      ytDlp.exec([
        videoUrl,
        '-x',
        '--audio-format', 'wav',
        '--audio-quality', '265K',
        '-o', cacheFilePath,
      ]).on('close', () => {
        if (fs.existsSync(cacheFilePath)) {
          console.log(`Download completed for id: ${id}`);
          resolve();
        } else {
          console.error(`Wav file not found after download for id: ${id}`);
          reject(new Error('Wav file not found after download'));
        }
      }).on('error', (error) => {
        console.error(`Error during download for id: ${id}`, error);
        reject(error);
      });
    });

    console.log(`Sending file for id: ${id}`);
    res.sendFile(cacheFilePath);
  } catch (error) {
    console.error(`An error occurred for id: ${id}:`, error);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

app.listen(port, '0.0.0.0', () => {
  console.log(`Server running on port :${port}`);
});