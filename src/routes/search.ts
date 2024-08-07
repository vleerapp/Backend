import express from 'express';
import axios from 'axios';
import fs from 'fs';
import { getSelectedInstance } from '../piped';

const router = express.Router();
const instance = getSelectedInstance();

const CACHE_FILE = './cache/search_cache.json';
const SEARCH_WEIGHTS_FILE = './cache/search_weights.json';

interface SearchCacheItem {
  results: any[];
  weight: number;
}

interface SearchResult {
  url: string;
}

let searchCache: Record<string, SearchCacheItem> = {};
let searchWeights: Record<string, Record<string, number>> = {};

if (fs.existsSync(CACHE_FILE)) {
  searchCache = JSON.parse(fs.readFileSync(CACHE_FILE, 'utf-8'));
}

if (fs.existsSync(SEARCH_WEIGHTS_FILE)) {
  searchWeights = JSON.parse(fs.readFileSync(SEARCH_WEIGHTS_FILE, 'utf-8'));
}

const saveCache = () => {
  fs.writeFileSync(CACHE_FILE, JSON.stringify(searchCache), 'utf-8');
};

const saveSearchWeights = () => {
  fs.writeFileSync(SEARCH_WEIGHTS_FILE, JSON.stringify(searchWeights), 'utf-8');
};

const updateSearchWeight = (query: string, selectedId: string) => {
  if (!searchWeights[query]) {
    searchWeights[query] = {};
  }
  searchWeights[query][selectedId] = (searchWeights[query][selectedId] || 0) + 1;
  saveSearchWeights();
};

router.get('/', async (req, res) => {
  const { query } = req.query;
  if (!query || typeof query !== 'string') {
    console.error(`[${new Date().toLocaleString()}] 🚫 Invalid search query: ${JSON.stringify(query)}`);
    res.status(400).json({ error: 'Invalid or missing query parameter' });
    return;
  }

  const startTime = Date.now();

  try {
    let results: SearchResult[];
    if (searchCache[query]) {
      results = searchCache[query].results;
    } else {
      const response = await axios.get(`${instance}/search`, {
        params: { q: query, filter: 'music_songs' }
      });
      results = response.data.items;
      searchCache[query] = { results, weight: 1 };
    }

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

    const getWeight = (id: string, title: string, artist: string) => {
      let weight = 0;
      const lowerQuery = query.toLowerCase();
      const lowerTitle = title.toLowerCase();
      const lowerArtist = artist.toLowerCase();

      Object.keys(searchWeights).forEach(savedQuery => {
        if (lowerQuery.includes(savedQuery.toLowerCase()) || savedQuery.toLowerCase().includes(lowerQuery)) {
          weight += searchWeights[savedQuery][id] || 0;
        }
      });

      if (lowerTitle.includes(lowerQuery) || lowerArtist.includes(lowerQuery)) {
        weight += 1;
      }

      return weight;
    };

    const sortedResults = Object.entries(flattenedResults)
      .sort(([idA, a]: [string, any], [idB, b]: [string, any]) => {
        const weightA = getWeight(idA, a.title, a.artist);
        const weightB = getWeight(idB, b.title, b.artist);

        if (weightA === weightB) {
          return results.findIndex((item: SearchResult) => item.url.includes(idA)) - results.findIndex((item: SearchResult) => item.url.includes(idB));
        }
        return weightB - weightA;
      })
      .reduce((acc, [id, value]) => {
        acc[id] = value;
        return acc;
      }, {} as Record<string, any>);

    const endTime = Date.now();
    const duration = endTime - startTime;
    console.log(`[${new Date().toLocaleString()}] ✅ Search complete: "${query}" | Duration: ${duration} ms`);

    res.json(sortedResults);
  } catch (error) {
    console.error(`[${new Date().toLocaleString()}] 💥 Search error for "${query}": ${error instanceof Error ? error.message : String(error)}`);
    res.status(500).json({ error: 'An error occurred while searching' });
  }
});

router.post('/update-weight', (req, res) => {
  const { query, selectedId } = req.body;
  if (!query || typeof query !== 'string' || !selectedId || typeof selectedId !== 'string') {
    res.status(400).json({ error: 'Invalid or missing query or selectedId' });
    return;
  }

  updateSearchWeight(query, selectedId);
  res.json({ success: true });
});

export default router;