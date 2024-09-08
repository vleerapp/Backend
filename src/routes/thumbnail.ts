import axios from 'axios';
import express from 'express';
import fs from 'fs';
import path from 'path';
import sharp from 'sharp';

const router = express.Router();

router.get('/', async (req, res) => {
  const { id } = req.query;
  if (!id || typeof id !== 'string') {
    return res.status(400).send('Invalid or missing id parameter');
  }

  const cacheDir = path.resolve(process.cwd(), 'cache', "thumbnails");
  const cacheFile = path.join(cacheDir, `${id}.webp`);

  try {
    if (fs.existsSync(cacheFile)) {
      return res.sendFile(cacheFile);
    }

    fs.mkdirSync(cacheDir, { recursive: true });

    const response = await axios({
      responseType: 'arraybuffer',
      url: `https://i3.ytimg.com/vi/${id}/maxresdefault.jpg`,
    });

    const image = await sharp(response.data)
      .metadata()
      .then((metadata: any) => {
        const size = Math.min(metadata.width, metadata.height);
        return sharp(response.data)
          .extract({ height: size, left: (metadata.width - size) / 2, top: (metadata.height - size) / 2, width: size })
          .toBuffer();
      });

    fs.writeFileSync(cacheFile, image);

    res.sendFile(cacheFile);
  } catch (error) {
    console.error('Error fetching thumbnail:', error);
    res.status(500).send('Failed to fetch thumbnail');
  }
});

export default router;