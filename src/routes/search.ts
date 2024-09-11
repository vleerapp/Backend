import axios from 'axios';
import express from 'express';
import fs from 'fs';
import { log } from '../index';
import { selectBestPipedInstance, getSelectedInstance } from '../piped';
import { Album, Playlist, Song } from '../types/types';

const router = express.Router();
selectBestPipedInstance()

const CACHE_FILE = './cache/search_cache.json';
const SEARCH_WEIGHTS_FILE = './cache/search_weights.json';

interface SearchCacheItem {
  results: SearchResponse;
  timestamp: number;
}

interface SearchResponse {
  albums: Record<string, Album>;
  playlists: Record<string, Playlist>;
  songs: Record<string, Song>;
}

let searchCache: Record<string, SearchCacheItem> = {};
let searchWeights: Record<string, Record<string, number>> = {};

const initializeCache = () => {
  if (fs.existsSync(CACHE_FILE)) {
    searchCache = JSON.parse(fs.readFileSync(CACHE_FILE, 'utf-8'));
  }

  if (fs.existsSync(SEARCH_WEIGHTS_FILE)) {
    searchWeights = JSON.parse(fs.readFileSync(SEARCH_WEIGHTS_FILE, 'utf-8'));
  }
};

initializeCache();

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

type FilterType = 'songs' | 'albums' | 'playlists';

router.get('/', async (req, res) => {
  const { filter, query } = req.query;
  if (!query || typeof query !== 'string') {
    log(`🚫 Invalid search query: ${JSON.stringify(query)}`);
    res.status(400).json({ error: 'Invalid or missing query parameter' });
    return;
  }

  const instance = getSelectedInstance();
  const startTime = Date.now();
  const filters: Record<FilterType, string> = {
    albums: 'music_albums',
    playlists: 'music_playlists',
    songs: 'music_songs',
  };

  try {
    let results: SearchResponse = {
      albums: {},
      playlists: {},
      songs: {},
    };
    let isCached = false;
    const filtersToSearch: FilterType[] = filter ? [filter as FilterType] : ['albums', 'playlists', 'songs'];

    if (searchCache[query] && filtersToSearch.every(f => Object.keys(searchCache[query].results[f]).length > 0)) {
      results = searchCache[query].results;
      isCached = true;
    } else {
      const searchPromises = filtersToSearch.map(f => 
        axios.get(`${instance}/search`, {
          params: { q: query, filter: filters[f], _internalType: f }
        })
      );

      const responses = await Promise.all(searchPromises);
      const rawResults = responses.flatMap(response => {
        const internalType = response.config.params._internalType;
        return response.data.items.map((item: any) => ({ ...item, _internalType: internalType }));
      });

      const fetchPromises: Promise<void>[] = [];

      rawResults.forEach(item => {
        let id = '';
        if (item.url) {
          if (item.url.includes('list=')) {
            id = item.url.split('list=')[1];
          } else if (item.url.includes('v=')) {
            id = item.url.split('v=')[1];
          } else {
            const urlParts = item.url.split('/');
            id = urlParts[urlParts.length - 1];
          }
        }
        if (!id) return;

        switch (item._internalType) {
          case 'songs':
            results.songs[id] = {
              id,
              title: item.title,
              artist: item.uploaderName,
              album: '',
              cover: item.thumbnail,
              duration: item.duration,
            };
            break;
          case 'albums':
            results.albums[id] = {
              id,
              name: item.name,
              author: item.uploaderName,
              cover: item.thumbnail,
              songs: [],
            };
            fetchPromises.push(fetchSongs(instance, id, 'album', results.albums[id]));
            break;
          case 'playlists':
            results.playlists[id] = {
              id,
              name: item.name,
              author: item.uploaderName,
              cover: item.thumbnail,
              songs: [],
            };
            fetchPromises.push(fetchSongs(instance, id, 'playlist', results.playlists[id]));
            break;
        }
      });

      await Promise.all(fetchPromises);

      if (!searchCache[query]) {
        searchCache[query] = {
          results: {
            albums: {},
            playlists: {},
            songs: {},
          },
          timestamp: Date.now(),
        };
      }
      filtersToSearch.forEach(f => {
        (searchCache[query].results[f] as Record<string, Album | Playlist | Song>) = results[f];
      });
      searchCache[query].timestamp = Date.now();
      saveCache();
    }
    
    if (filter) {
      const filteredResults = {
        albums: {},
        playlists: {},
        songs: {},
      };
      filteredResults[filter as FilterType] = results[filter as FilterType];
      results = filteredResults;
    }

    const endTime = Date.now();
    const duration = endTime - startTime;
    if (isCached) {
      log(`✅ Search (cached): "${query}" | Filter: ${filter || 'all'} | Duration: ${duration} ms`);
    } else {
      log(`✅ Search: "${query}" | Filters: ${filtersToSearch.join(', ')} | Duration: ${duration} ms`);
    }

    res.json(results);

  } catch (error) {
    log(`💥 Search error for "${query}": ${error instanceof Error ? error.message : String(error)}`);
    res.status(500).json({ error: 'An error occurred while searching' });
  }
});

async function fetchSongs(instance: string, id: string, type: 'playlist' | 'album', container: Album | Playlist) {
  try {
    const response = await axios.get(`${instance}/playlists/${id}`);
    const relatedStreams = response.data.relatedStreams || [];
    container.songs = relatedStreams.map((stream: any) => ({
      id: stream.url?.split('v=')[1] || '',
      title: stream.title,
      artist: stream.uploaderName,
      album: type === 'album' ? response.data.name : '',
      cover: stream.thumbnail,
      duration: stream.duration,
    }));
  } catch (error) {
    log(`💥 Error fetching songs for ${type} ${id}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

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