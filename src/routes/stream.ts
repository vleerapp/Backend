import express from 'express';
import ytdl from '@distube/ytdl-core';
import { log } from '../index';

const router = express.Router();

const CHUNK_SIZE = 1000 * 1024; // 500 KB chunks

router.get('/', async (req, res) => {
  const { id, quality } = req.query;
  if (!id || typeof id !== 'string' || !quality || (quality !== 'compressed' && quality !== 'lossless')) {
    log(`🚫 Invalid request: ${JSON.stringify({ id, quality })}`);
    return res.status(400).json({ error: 'Invalid or missing id or quality parameter' });
  }

  const videoUrl = `https://www.youtube.com/watch?v=${id}`;

  try {
    log(`🎵 Streaming: ${videoUrl}`);

    const info = await ytdl.getInfo(videoUrl);
    const audioFormat = ytdl.chooseFormat(info.formats, {
      quality: quality === 'compressed' ? 'lowestaudio' : 'highestaudio',
    });

    if (!audioFormat) {
      throw new Error('No suitable audio format found');
    }

    const totalLength = parseInt(audioFormat.contentLength, 10);
    const range = req.headers.range;

    let start = 0;
    let end = totalLength - 1;

    if (range) {
      const parts = range.replace(/bytes=/, '').split('-');
      start = parseInt(parts[0], 10);
      end = parts[1] ? parseInt(parts[1], 10) : Math.min(start + CHUNK_SIZE - 1, totalLength - 1);

      if (isNaN(start) || isNaN(end) || start >= totalLength || end >= totalLength || start > end) {
        res.status(416).send('Requested range not satisfiable');
        return;
      }
    } else {
      end = Math.min(CHUNK_SIZE - 1, totalLength - 1);
    }

    const contentLength = end - start + 1;

    res.writeHead(206, {
      'Content-Range': `bytes ${start}-${end}/${totalLength}`,
      'Accept-Ranges': 'bytes',
      'Content-Length': contentLength,
      'Content-Type': audioFormat.mimeType,
    });

    const stream = ytdl(videoUrl, {
      format: audioFormat,
      range: { start, end },
    });

    let bytesSent = 0;

    stream.on('data', (chunk) => {
      if (bytesSent + chunk.length <= contentLength) {
        res.write(chunk);
        bytesSent += chunk.length;
      } else {
        const remainingBytes = contentLength - bytesSent;
        res.write(chunk.slice(0, remainingBytes));
        bytesSent += remainingBytes;
        stream.destroy();
        res.end();
      }
    });

    stream.on('end', () => {
      res.end();
    });

    stream.on('error', (error) => {
      log(`💥 Streaming error: ${error.message}`);
      res.status(500).end();
    });

    res.on('close', () => {
      stream.destroy();
    });

    log(`📦 Streaming: ${info.videoDetails.title} | Format: ${audioFormat.container} | Range: ${start}-${end}/${contentLength}`);
  } catch (error) {
    log(`💥 Error: ${error instanceof Error ? error.message : String(error)}`);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

export default router;