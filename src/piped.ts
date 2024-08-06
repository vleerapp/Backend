import axios from 'axios';

interface PipedInstance {
  name: string;
  api_url: string;
}

let selectedInstance: string | null = null;

async function pingInstance(instance: PipedInstance): Promise<number> {
  const start = Date.now();
  try {
    await axios.get(`${instance.api_url}/healthcheck`);
    return Date.now() - start;
  } catch {
    return Infinity;
  }
}

export async function selectBestPipedInstance(): Promise<void> {
  try {
    const response = await axios.get('https://piped-instances.kavin.rocks/');
    const instances: PipedInstance[] = response.data;
    instances.push({ name: 'wireway.ch', api_url: 'https://pipedapi.wireway.ch' });

    const pingResults = await Promise.all(
      instances.map(async (instance) => {
        const pingTime = await pingInstance(instance);
        console.log(`[${new Date().toLocaleString()}] 🏓 Ping test for ${instance.name}: ${pingTime}ms`);
        return { instance, pingTime };
      })
    );

    const bestInstance = pingResults.reduce((best, current) =>
      current.pingTime < best.pingTime ? current : best
    );

    selectedInstance = bestInstance.instance.api_url;
    console.log(`[${new Date().toLocaleString()}] 🌐 Selected Piped instance: ${selectedInstance} (${bestInstance.pingTime}ms)`);
  } catch (error) {
    console.error(`[${new Date().toLocaleString()}] 💥 Error selecting Piped instance: ${error instanceof Error ? error.message : String(error)}`);
    selectedInstance = 'https://pipedapi.kavin.rocks';
    console.log(`[${new Date().toLocaleString()}] ⚠️ Fallback to default Piped instance: ${selectedInstance}`);
  }
}

export function getSelectedInstance(): string {
  return selectedInstance || 'https://pipedapi.kavin.rocks';
}