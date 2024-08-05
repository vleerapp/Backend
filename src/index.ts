import express from 'express';
import YTDlpWrap from 'yt-dlp-wrap';
import fs from 'fs';
import path from 'path';

const app = express();
const port = 3000;

app.use(express.json());

app.get('/', (req, res) => {
  res.send('<h1>👋</h1>');
});

app.get('/download', async (req, res) => {
  const { id, quality } = req.query;
  if (!id || typeof id !== 'string' || !quality || (quality !== 'compressed' && quality !== 'lossless')) {
    console.log('Invalid or missing id or quality parameter');
    return res.status(400).json({ error: 'Invalid or missing id or quality parameter' });
  }

  const videoUrl = `https://www.youtube.com/watch?v=${id}`;
  const cacheDir = path.resolve(process.cwd(), 'cache');
  const compressedDir = path.join(cacheDir, 'compressed');
  const losslessDir = path.join(cacheDir, 'lossless');
  const cacheFilePath = quality === 'compressed' 
    ? path.join(compressedDir, `${id}.mp3`)
    : path.join(losslessDir, `${id}.wav`);

  try {
    if (fs.existsSync(cacheFilePath)) {
      console.log(`Cached file found for id: ${id}, quality: ${quality}`);
      return res.sendFile(cacheFilePath);
    }

    console.log(`Cache miss for id: ${id}, quality: ${quality}, downloading...`);
    if (!fs.existsSync(compressedDir)) {
      console.log('Creating compressed cache directory');
      fs.mkdirSync(compressedDir, { recursive: true });
    }
    if (!fs.existsSync(losslessDir)) {
      console.log('Creating lossless cache directory');
      fs.mkdirSync(losslessDir, { recursive: true });
    }

    const ytDlp = new YTDlpWrap();

    await new Promise<void>((resolve, reject) => {
      const args = [
        videoUrl,
        '-x',
        '-o', cacheFilePath,
      ];

      if (quality === 'compressed') {
        args.push('--audio-format', 'mp3');
      } else {
        args.push('--audio-format', 'wav');
      }

      ytDlp.exec(args).on('close', () => {
        if (fs.existsSync(cacheFilePath)) {
          console.log(`Download completed for id: ${id}, quality: ${quality}`);
          resolve();
        } else {
          console.error(`Audio file not found after download for id: ${id}, quality: ${quality}`);
          reject(new Error('Audio file not found after download'));
        }
      }).on('error', (error) => {
        console.error(`Error during download for id: ${id}, quality: ${quality}`, error);
        reject(error);
      });
    });

    console.log(`Sending file for id: ${id}, quality: ${quality}`);
    res.sendFile(cacheFilePath);
  } catch (error) {
    console.error(`An error occurred for id: ${id}, quality: ${quality}:`, error);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

app.listen(port, '0.0.0.0', () => {
  console.log(`Server running on port :${port}`);
});