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
    console.error(`[${new Date().toLocaleString()}] 🚫 Invalid request: ${JSON.stringify({ id, quality })}`);
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
      const fileSize = fs.statSync(cacheFilePath).size / (1024 * 1024);
      console.log(`[${new Date().toLocaleString()}] 📦 Serving cached: ${path.basename(cacheFilePath)} | Size: ${fileSize.toFixed(2)} MB`);
      return res.sendFile(cacheFilePath);
    }

    if (!fs.existsSync(compressedDir)) {
      fs.mkdirSync(compressedDir, { recursive: true });
    }
    if (!fs.existsSync(losslessDir)) {
      fs.mkdirSync(losslessDir, { recursive: true });
    }

    const ytDlp = new YTDlpWrap();

    console.log(`[${new Date().toLocaleString()}] 📥 Downloading: ${videoUrl}`);
    const startTime = Date.now();
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
          const endTime = Date.now();
          const fileSize = fs.statSync(cacheFilePath).size / (1024 * 1024);
          const duration = endTime - startTime;
          console.log(`[${new Date().toLocaleString()}] ✅ Download complete: ${path.basename(cacheFilePath)} | Size: ${fileSize.toFixed(2)} MB | Duration: ${duration} ms`);
          resolve();
        } else {
          reject(new Error('Audio file not found after download'));
        }
      }).on('error', reject);
    });

    res.sendFile(cacheFilePath);
  } catch (error) {
    console.error(`[${new Date().toLocaleString()}] 💥 Error: ${error instanceof Error ? error.message : String(error)}`);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

app.listen(port, '0.0.0.0', () => {
  console.log(`[${new Date().toLocaleString()}] 🚀 Server running on port :${port}`);
});