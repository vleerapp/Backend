import express from 'express';
import YTDlpWrap from 'yt-dlp-wrap';
import fs from 'fs';
import path from 'path';
import { getSelectedInstance } from './piped';
import axios from 'axios';

const instance = getSelectedInstance();
const ytDlp = new YTDlpWrap();
const app = express();
const port = 3000;

// init search cache
const CACHE_FILE = './cache/search_cache.json';
interface SearchCacheItem {
  results: any[];
  weight: number;
}
let searchCache: Record<string, SearchCacheItem> = {};

if (fs.existsSync(CACHE_FILE)) {
  searchCache = JSON.parse(fs.readFileSync(CACHE_FILE, 'utf-8'));
}

const saveCache = () => {
  fs.writeFileSync(CACHE_FILE, JSON.stringify(searchCache), 'utf-8');
};
/////////////////////

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
    : path.join(losslessDir, `${id}.flac`);

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

    console.log(`[${new Date().toLocaleString()}] 📥 Downloading: ${videoUrl}`);
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

app.get('/stream', async (req, res) => {
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
    : path.join(losslessDir, `${id}.flac`);

  try {
    if (!fs.existsSync(compressedDir)) {
      fs.mkdirSync(compressedDir, { recursive: true });
    }
    if (!fs.existsSync(losslessDir)) {
      fs.mkdirSync(losslessDir, { recursive: true });
    }

    if (!fs.existsSync(cacheFilePath)) {
      console.log(`[${new Date().toLocaleString()}] 📥 Downloading: ${videoUrl}`);
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
            console.log(`[${new Date().toLocaleString()}] ✅ Download complete: ${path.basename(cacheFilePath)} | Size: ${fileSize.toFixed(2)} MB | Duration: ${duration} ms`);
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

    console.log(`[${new Date().toLocaleString()}] 📦 Streaming: ${path.basename(cacheFilePath)} | Size: ${(fileSize / (1024 * 1024)).toFixed(2)} MB | Range: ${start}-${end}`);
  } catch (error) {
    console.error(`[${new Date().toLocaleString()}] 💥 Error: ${error instanceof Error ? error.message : String(error)}`);
    res.status(500).send('An error occurred while streaming the audio.');
  }
});

app.get('/search', async (req, res) => {
  const { query } = req.query;
  if (!query || typeof query !== 'string') {
    console.error(`[${new Date().toLocaleString()}] 🚫 Invalid search query: ${JSON.stringify(query)}`);
    res.status(400).json({ error: 'Invalid or missing query parameter' });
    return;
  }

  console.log(`[${new Date().toLocaleString()}] 🔍 Searching for: "${query}"`);
  const startTime = Date.now();

  try {
    let results;
    if (searchCache[query]) {
      results = searchCache[query].results;
      searchCache[query].weight += 1;
      console.log(`[${new Date().toLocaleString()}] 📦 Serving cached results for: "${query}"`);
    } else {
      const response = await axios.get(`${instance}/search`, {
        params: { q: query, filter: 'music_songs' }
      });
      results = response.data.items;
      searchCache[query] = { results, weight: 1 };
    }

    searchCache[query].weight += 1;
    saveCache();

    const flattenedResults = results.reduce((acc: Record<string, any>, song: any) => {
      const id = song.url.split('v=')[1];
      acc[id] = {
        id,
        title: song.title,
        artist: song.uploaderName,
        thumbnailUrl: song.thumbnail,
        duration: song.duration,
      };
      return acc;
    }, {});

    const sortedResults = Object.values(flattenedResults).sort((a: any, b: any) => b.weight - a.weight);

    const endTime = Date.now();
    const duration = endTime - startTime;
    console.log(`[${new Date().toLocaleString()}] ✅ Search complete: "${query}" | Results: ${sortedResults.length} | Duration: ${duration} ms`);

    res.json(sortedResults);
  } catch (error) {
    console.error(`[${new Date().toLocaleString()}] 💥 Search error for "${query}": ${error instanceof Error ? error.message : String(error)}`);
    res.status(500).json({ error: 'An error occurred while searching' });
  }
});

app.listen(port, '0.0.0.0', () => {
  console.log(`[${new Date().toLocaleString()}] 🚀 Server running on port :${port}`);
});