import axios from 'axios';
import { log } from './index';

interface PipedInstance {
  name: string;
  api_url: string;
}

let selectedInstance: string | null = null;

async function pingInstance(instance: PipedInstance): Promise<number> {
  const start = Date.now();
  try {
    await axios.get(`${instance.api_url}/healthcheck`, { timeout: 5000 });
    return Date.now() - start;
  } catch {
    return Infinity;
  }
}

export async function selectBestPipedInstance(): Promise<void> {
  try {
    const response = await axios.get('https://piped-instances.kavin.rocks/', { timeout: 5000 });
    const instances: PipedInstance[] = response.data.filter((instance: PipedInstance) => instance.name !== 'phoenixthrush.com' && instance.name !== 'adminforge.de' && instance.name !== 'piped.yt' && instance.name !== 'ehwurscht.at' && instance.name !== 'ggtyler.dev' && instance.name !== 'private.coffee' && instance.name !== 'projectsegfau.lt' && instance.name !== 'privacydev.net');
    instances.push({ name: 'wireway.ch', api_url: 'https://pipedapi.wireway.ch' });

    const pingResults = await Promise.all(
      instances.map(async (instance) => {
        const pingTime = await pingInstance(instance);
        log(`🏓 Ping test for ${instance.name}: ${pingTime}ms`);
        return { instance, pingTime };
      })
    );

    const bestInstance = pingResults.reduce((best, current) =>
      current.pingTime < best.pingTime ? current : best
    );

    selectedInstance = bestInstance.instance.api_url;
    log(`🌐 Selected Piped instance: ${selectedInstance} (${bestInstance.pingTime}ms)`);
  } catch (error) {
    log(`💥 Error selecting Piped instance: ${error instanceof Error ? error.message : String(error)}`);
    selectedInstance = 'https://pipedapi.kavin.rocks';
    log(`⚠️ Fallback to default Piped instance: ${selectedInstance}`);
  }
}

export function getSelectedInstance(): string {
  return selectedInstance || 'https://pipedapi.kavin.rocks';
}