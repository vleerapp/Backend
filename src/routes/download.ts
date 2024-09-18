import express from 'express';
import YTDlpWrap from 'yt-dlp-wrap';
import fs from 'fs';
import path from 'path';
import { log } from '../index';

const router = express.Router();
const ytDlp = new YTDlpWrap();

router.get('/', async (req, res) => {
  const { id, quality } = req.query;
  if (!id || typeof id !== 'string' || !quality || (quality !== 'compressed' && quality !== 'lossless')) {
    log(`🚫 Invalid request: ${JSON.stringify({ id, quality })}`);
    return res.status(400).json({ error: 'Invalid or missing id or quality parameter' });
  }

  const videoUrl = `https://www.youtube.com/watch?v=${id}`;
  const cacheDir = path.resolve(process.cwd(), 'cache');
  const compressedDir = path.join(cacheDir, 'compressed');
  const losslessDir = path.join(cacheDir, 'lossless');
  const cacheFilePath = quality === 'compressed'
    ? path.join(compressedDir, `${id}.mp3`)
    : path.join(losslessDir, `${id}.flac`);

  try {
    if (fs.existsSync(cacheFilePath)) {
      const fileSize = fs.statSync(cacheFilePath).size / (1024 * 1024);
      log(`📦 Serving cached: ${path.basename(cacheFilePath)} | Size: ${fileSize.toFixed(2)} MB`);
      return res.sendFile(cacheFilePath);
    }

    if (!fs.existsSync(compressedDir)) {
      fs.mkdirSync(compressedDir, { recursive: true });
    }
    if (!fs.existsSync(losslessDir)) {
      fs.mkdirSync(losslessDir, { recursive: true });
    }

    log(`📥 Downloading: ${videoUrl}`);
    const startTime = Date.now();
    await new Promise<void>((resolve, reject) => {
      const args = [
        videoUrl,
        '-x',
        '-o', cacheFilePath,
        '--audio-format', quality === 'compressed' ? 'mp3' : 'flac'
      ];

      ytDlp.exec(args).on('close', () => {
        if (fs.existsSync(cacheFilePath)) {
          const endTime = Date.now();
          const fileSize = fs.statSync(cacheFilePath).size / (1024 * 1024);
          const duration = endTime - startTime;
          log(`✅ Download complete: ${path.basename(cacheFilePath)} | Size: ${fileSize.toFixed(2)} MB | Duration: ${duration} ms`);
          resolve();
        } else {
          reject(new Error('Audio file not found after download'));
        }
      }).on('error', reject);
    });

    res.sendFile(cacheFilePath);
  } catch (error) {
    log(`💥 Error: ${error instanceof Error ? error.message : String(error)}`);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

export default router;