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
    if (!fs.existsSync(compressedDir)) {
      fs.mkdirSync(compressedDir, { recursive: true });
    }
    if (!fs.existsSync(losslessDir)) {
      fs.mkdirSync(losslessDir, { recursive: true });
    }

    if (!fs.existsSync(cacheFilePath)) {
      log(`📥 Downloading: ${videoUrl}`);
      const startTime = Date.now();
      const args = [
        videoUrl,
        '-x',
        '-o', cacheFilePath,
        '--audio-format', quality === 'compressed' ? 'mp3' : 'flac'
      ];

      await new Promise<void>((resolve, reject) => {
        ytDlp.exec(args)
          .on('close', () => {
            const endTime = Date.now();
            const fileSize = fs.statSync(cacheFilePath).size / (1024 * 1024);
            const duration = endTime - startTime;
            log(`✅ Download complete: ${path.basename(cacheFilePath)} | Size: ${fileSize.toFixed(2)} MB | Duration: ${duration} ms`);
            resolve();
          })
          .on('error', reject);
      });
    }

    const stat = fs.statSync(cacheFilePath);
    const fileSize = stat.size;
    const range = req.headers.range;

    let start = 0;
    let end = fileSize - 1;
    const chunkSize = 500000;

    if (range) {
      const parts = range.replace(/bytes=/, "").split("-");
      start = parseInt(parts[0], 10);
      end = parts[1] ? parseInt(parts[1], 10) : Math.min(start + chunkSize - 1, fileSize - 1);

      if (isNaN(start) || isNaN(end) || start >= fileSize || end >= fileSize || start > end) {
        res.status(416).send('Requested range not satisfiable');
        return;
      }
    } else {
      end = Math.min(chunkSize - 1, fileSize - 1);
    }

    const contentLength = end - start + 1;

    const head = {
      'Content-Range': `bytes ${start}-${end}/${fileSize}`,
      'Accept-Ranges': 'bytes',
      'Content-Length': contentLength,
      'Content-Type': quality === 'compressed' ? 'audio/mpeg' : 'audio/flac',
    };

    res.writeHead(206, head);

    const fileStream = fs.createReadStream(cacheFilePath, { start, end });
    fileStream.pipe(res);

    log(`📦 Streaming: ${path.basename(cacheFilePath)} | Size: ${(fileSize / (1024 * 1024)).toFixed(2)} MB | Range: ${start}-${end}`);
  } catch (error) {
    log(`💥 Error: ${error instanceof Error ? error.message : String(error)}`);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

export default router;