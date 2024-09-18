import axios from 'axios';
import { spawn } from 'child_process';
import express from 'express';
import fs from 'fs';
import path from 'path';
import { log } from '../index';

const router = express.Router();

async function convertAndStreamAudio(inputUrl: string, outputFormat: string, res: express.Response, cacheFilePath: string, retries = 3) {
  const startTime = Date.now();
  const userAgent = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36';

  try {
    const ffmpeg = spawn('ffmpeg', [
      '-user_agent', userAgent,
      '-i', inputUrl,
      '-c:a', outputFormat === 'mp3' ? 'libmp3lame' : 'flac',
      '-f', outputFormat,
      '-'
    ]);

    ffmpeg.stdout.pipe(res);

    const fileStream = fs.createWriteStream(cacheFilePath);
    ffmpeg.stdout.pipe(fileStream);

    let errorOutput = '';
    ffmpeg.stderr.on('data', (data) => {
      errorOutput += data.toString();
      log(`ffmpeg stderr: ${data}`);
    });

    const exitCode = await new Promise<number>((resolve) => {
      ffmpeg.on('close', resolve);
    });

    if (exitCode === 0) {
      const duration = Date.now() - startTime;
      log(`ffmpeg child process completed successfully. Duration: ${duration}ms`);
      fileStream.close();
      return;
    } else {
      log(`ffmpeg child process exited with code ${exitCode}.`);
      log(`Error output: ${errorOutput}`);
    }
  } catch (error) {
    log(`Error in convertAndStreamAudio: ${error instanceof Error ? error.message : String(error)}`);
    throw error;
  }
}

router.get('/', async (req, res) => {
  const routeStartTime = Date.now();
  const { id, quality } = req.query;
  if (!id || typeof id !== 'string' || !quality || (quality !== 'compressed' && quality !== 'lossless')) {
    log(`🚫 Invalid request: ${JSON.stringify({ id, quality })}`);
    return res.status(400).json({ error: 'Invalid or missing id or quality parameter' });
  }

  const cacheDir = path.resolve(process.cwd(), 'cache');
  const compressedDir = path.join(cacheDir, 'compressed');
  const losslessDir = path.join(cacheDir, 'lossless');
  const outputFormat = quality === 'compressed' ? 'mp3' : 'flac';
  const cacheFilePath = quality === 'compressed'
    ? path.join(compressedDir, `${id}.mp3`)
    : path.join(losslessDir, `${id}.flac`);

  try {
    if (fs.existsSync(cacheFilePath)) {
      const fileSize = fs.statSync(cacheFilePath).size / (1024 * 1024);
      const duration = Date.now() - routeStartTime;
      log(`📦 Serving cached: ${path.basename(cacheFilePath)} | Size: ${fileSize.toFixed(2)} MB | Duration: ${duration}ms`);
      return res.sendFile(cacheFilePath);
    }

    if (!fs.existsSync(compressedDir)) {
      fs.mkdirSync(compressedDir, { recursive: true });
    }
    if (!fs.existsSync(losslessDir)) {
      fs.mkdirSync(losslessDir, { recursive: true });
    }

    const apiStartTime = Date.now();
    const apiUrl = `https://pipedapi.wireway.ch/streams/${id}`;
    const response = await axios.get(apiUrl);
    const streamData = response.data;
    const apiDuration = Date.now() - apiStartTime;
    log(`API request completed in ${apiDuration}ms`);

    const audioStream = streamData.audioStreams.find((stream: any) => stream.itag === 251);
    if (!audioStream) {
      throw new Error('Audio stream with itag 251 not found');
    }

    log(`📥 Streaming and converting: ${audioStream.url}`);

    // Set headers for streaming
    res.setHeader('Content-Type', `audio/${outputFormat}`);
    res.setHeader('Content-Disposition', `attachment; filename="${id}.${outputFormat}"`);

    // Convert and stream the audio
    const streamStartTime = Date.now();
    convertAndStreamAudio(audioStream.url, outputFormat, res, cacheFilePath);

    // Wait for the client stream to finish
    await new Promise((resolve) => {
      res.on('finish', () => {
        const streamDuration = Date.now() - streamStartTime;
        log(`✅ Streaming complete for: ${id} | Duration: ${streamDuration}ms`);
        resolve(null);
      });
    });

    const totalDuration = Date.now() - routeStartTime;
    log(`Total route processing time: ${totalDuration}ms`);
  } catch (error) {
    const errorDuration = Date.now() - routeStartTime;
    log(`💥 Error: ${error instanceof Error ? error.message : String(error)} | Duration: ${errorDuration}ms`);
    if (axios.isAxiosError(error) && error.response) {
      log(`Response status: ${error.response.status}`);
      log(`Response data: ${JSON.stringify(error.response.data)}`);
    }
    if (!res.headersSent) {
      res.status(500).send('An error occurred while streaming the audio.');
    }
  }
});

export default router;