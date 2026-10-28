import type { Answer, Sources } from './app.contract.d.ts';
import { connection, getBattery, powerState } from '@exact/device';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
// The device module asks no grant (LLP 1133.031 D7).
export const grants = '';

/** One read's answer as text, or its error's name. */
async function line<T>(read: () => Promise<T>, text: (v: T) => string): Promise<string> {
  try {
    return text(await read());
  } catch (e) {
    return (e as { name?: string }).name ?? 'Error';
  }
}

/** An answer's platform words (D9), by platform, as JSON: what the suites check is each platform's own. */
const platformWords: Record<string, Record<string, unknown>> = {};
function keep(source: string, answer: object) {
  const all = answer as Record<string, unknown>;
  platformWords[source] = Object.fromEntries(['apple', 'android', 'linux', 'windows'].filter((k) => all[k] != null).map((k) => [k, all[k]]));
}

const sources: Sources = {
  facts: async ([reading]) => {
    if (!reading) return { battery: 'off', network: 'off', power: 'off', words: '' };
    const [battery, network, power] = await Promise.all([
      line(() => getBattery({}), (b) => (keep('battery', b), `${b.level} ${b.charging}`)),
      line(() => connection({}), (c) => (keep('network', c), `${c.type} ${c.saveData}`)),
      line(() => powerState({}), (p) => (keep('power', p), `${p.lowPowerMode} ${p.thermalState ?? 'none'}`)),
    ]);
    return { battery, network, power, words: JSON.stringify(platformWords) };
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
