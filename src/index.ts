import express from 'express';
import ytdl from '@distube/ytdl-core';
import fs from 'fs';
import path from 'path';
import { Readable } from 'stream';
import { Writer } from 'wav';

const app = express();
const port = 3000;

app.use(express.json());

app.get('/stream', async (req, res) => {
  console.log('Received request for /stream');
  const { id } = req.query;
  if (!id || typeof id !== 'string') {
    console.log('Invalid or missing id parameter');
    return res.status(400).json({ error: 'Invalid or missing id parameter' });
  }

  const videoUrl = `https://www.youtube.com/watch?v=${id}`;
  const cacheDir = path.join('cache');
  const cacheFilePath = path.join(cacheDir, `${id}.wav`);

  try {
    if (fs.existsSync(cacheFilePath)) {
      console.log(`Serving cached file: ${cacheFilePath}`);
      return res.sendFile(cacheFilePath);
    }

    console.log(`Fetching video: ${videoUrl}`);
    const videoStream = ytdl(videoUrl, { quality: 'highestaudio' });
    const byteArray: number[] = [];

    videoStream.on('data', (chunk) => {
      for (let i = 0; i < chunk.length; i++) {
        byteArray.push(chunk[i]);
        console.log(byteArray);
      }
    });

    videoStream.on('end', () => {
      console.log('Video stream ended, processing audio');
      const audioBuffer = Buffer.from(byteArray);
      const audioStream = new Readable();
      audioStream.push(audioBuffer);
      audioStream.push(null);

      const writer = new Writer({
        channels: 2,
        sampleRate: 44100,
        bitDepth: 16
      });

      const outputStream = fs.createWriteStream(cacheFilePath);
      writer.pipe(outputStream);

      audioStream.pipe(writer);

      outputStream.on('finish', () => {
        console.log(`Audio processed and saved to ${cacheFilePath}`);
        res.sendFile(cacheFilePath);
      });
    });

    videoStream.on('error', (err) => {
      console.error('Error occurred while streaming video:', err);
      res.status(500).send('An error occurred while streaming the audio.');
    });
  } catch (err) {
    console.error('Error occurred:', err);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

app.listen(port, '0.0.0.0', () => {
  console.log(`Server running on port :${port}`);
});