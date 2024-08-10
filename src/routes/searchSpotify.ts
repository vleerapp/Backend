import express from 'express';
import axios from 'axios';
import fs from 'fs';

const router = express.Router();

let accessToken: string | null = null;
let clientId: string | null = null;
let tokenExpirationTime: number | null = null;

const CACHE_FILE = './cache/spotify_search_cache.json';
let searchCache: Record<string, Record<string, MinifiedTrack>> = {};

if (fs.existsSync(CACHE_FILE)) {
    searchCache = JSON.parse(fs.readFileSync(CACHE_FILE, 'utf-8'));
}

const saveCache = () => {
    fs.writeFileSync(CACHE_FILE, JSON.stringify(searchCache), 'utf-8');
};

interface SpotifySession {
    accessToken: string;
    clientId: string;
}

async function auth(): Promise<boolean> {
    if (accessToken && tokenExpirationTime && Date.now() < tokenExpirationTime) {
        return true;
    }

    const re = /<script id="session" data-testid="session" type="application\/json"\>({.*})<\/script>/;
    try {
        const response = await axios.get("https://open.spotify.com/search");
        const match = response.data.match(re);
        if (match) {
            const json: SpotifySession = JSON.parse(match[1]);
            accessToken = json.accessToken;
            clientId = json.clientId;
            tokenExpirationTime = Date.now() + 3600000;
            return true;
        }
    } catch (err) {
        console.error(err);
    }
    console.error("Failed to get access token");
    return false;
}

interface SpotifyTrack {
    id: string;
    name: string;
    artists: { name: string }[];
    album: {
        name: string;
        images: { url: string }[];
    };
    duration_ms: number;
    preview_url: string;
}

interface MinifiedTrack {
    id: string;
    title: string;
    artist: string;
    thumbnailUrl: string;
    duration: number;
}

router.get('/', async (req: express.Request, res: express.Response) => {
    const { query } = req.query;
    if (!query || typeof query !== 'string') {
        console.error(`[${new Date().toLocaleString()}] 🚫 Invalid Spotify search query: ${JSON.stringify(query)}`);
        return res.status(400).json({ error: 'Search query is required' });
    }

    const startTime = Date.now();

    if (searchCache[query]) {
        const endTime = Date.now();
        const duration = endTime - startTime;
        console.log(`[${new Date().toLocaleString()}] ✅ Spotify search (cached): "${query}" | Duration: ${duration} ms`);
        return res.json(searchCache[query]);
    }

    if (!(await auth())) {
        console.error(`[${new Date().toLocaleString()}] 💥 Spotify authentication failed for query: "${query}"`);
        return res.status(500).json({ error: 'Failed to authenticate' });
    }

    try {
        const url = `https://api.spotify.com/v1/search?q=${encodeURIComponent(query)}&type=track`;
        const response = await axios.get<{ tracks: { items: SpotifyTrack[] } }>(url, {
            headers: {
                Authorization: `Bearer ${accessToken}`,
            }
        });
        const data = response.data;
        const minifiedResults: Record<string, MinifiedTrack> = {};
        data.tracks.items.forEach(track => {
            minifiedResults[track.id] = {
                id: track.id,
                title: track.name,
                artist: track.artists[0].name,
                thumbnailUrl: track.album.images[0].url,
                duration: Math.round(track.duration_ms / 1000)
            };
        });
        searchCache[query] = minifiedResults;
        saveCache();

        const endTime = Date.now();
        const duration = endTime - startTime;
        console.log(`[${new Date().toLocaleString()}] ✅ Spotify search: "${query}" | Duration: ${duration} ms`);

        res.json(minifiedResults);
    } catch (error) {
        console.error(`[${new Date().toLocaleString()}] 💥 Spotify search error for "${query}": ${error instanceof Error ? error.message : String(error)}`);
        res.status(500).json({ error: 'Failed to fetch search results' });
    }
});

export default router;