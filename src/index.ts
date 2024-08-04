process.env.YTDL_NO_UPDATE = '1';

import express from 'express';
import ytdl from '@distube/ytdl-core';
import fs from 'fs';
import path from 'path';
import ffmpeg from 'fluent-ffmpeg';

const app = express();
const port = 3000;
const cacheDir = path.resolve('cache');

// Ensure cache directory exists
if (!fs.existsSync(cacheDir)) {
  fs.mkdirSync(cacheDir, { recursive: true });
}

app.use(express.json());

app.get('/stream', async (req, res) => {
  const { id } = req.query;
  if (!id || typeof id !== 'string') {
    return res.status(400).json({ error: 'Invalid or missing id parameter' });
  }

  const videoUrl = `https://www.youtube.com/watch?v=${id}`;
  const cacheFilePath = path.resolve(cacheDir, `${id}.wav`);

  try {
    if (fs.existsSync(cacheFilePath)) {
      console.log(`Cached file found for ${id}, sending file`);
      return res.sendFile(cacheFilePath);
    }

    console.log(`Fetching video info for ${videoUrl}`);
    const info = await ytdl.getInfo(videoUrl);
    console.log(`Video info fetched successfully for ${id}`);

    console.log(`Starting download and conversion for ${id}`);
    
    const audioStream = ytdl(videoUrl, { 
      quality: 'highestaudio',
      filter: 'audioonly'
    });

    let dataReceived = false;

    audioStream.on('data', (chunk) => {
      if (!dataReceived) {
        console.log(`Received first chunk of data for ${id}`);
        dataReceived = true;
      }
    });

    audioStream.on('end', () => {
      console.log(`YouTube stream ended for ${id}`);
    });

    ffmpeg(audioStream)
      .audioCodec('pcm_s16le')
      .audioFrequency(44100)
      .format('wav')
      .on('start', (commandLine) => {
        console.log('FFmpeg started with command:', commandLine);
      })
      .on('progress', (progress) => {
        console.log(`Processing ${id}: ${progress.percent ? progress.percent.toFixed(2) : 'N/A'}% done`);
      })
      .on('error', (err) => {
        console.error(`FFmpeg error for ${id}:`, err);
        res.status(500).send('An error occurred during audio conversion.');
      })
      .on('end', () => {
        console.log(`Conversion complete for ${id}`);
        if (fs.existsSync(cacheFilePath)) {
          res.sendFile(cacheFilePath, (err) => {
            if (err) {
              console.error('Error sending file:', err);
              res.status(500).send('An error occurred while sending the file.');
            } else {
              console.log(`File sent successfully for ${id}`);
            }
          });
        } else {
          console.error(`File not found after conversion: ${cacheFilePath}`);
          res.status(500).send('An error occurred while saving the converted file.');
        }
      })
      .save(cacheFilePath);

  } catch (err) {
    console.error(`Error occurred for ${id}:`, err);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

app.listen(port, '0.0.0.0', () => {
  console.log(`Server running on port :${port}`);
});