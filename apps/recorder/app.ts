// The recorder's data (LLP 1133.009 §11): the takes are the files
// @exact/recording wrote to app:/data/takes; each one's length and level,
// which the Contract passes when it files a take, are kept beside them in
// index.json.
import type { Answer, Storage } from './app.contract.d.ts';

export const appId = 'com.exact.recorder';
const DIR = 'app:/data/takes';
const INDEX = `${DIR}/index.json`;
export const grants = `device.microphone purpose.microphone\nfs.read ${DIR}\nfs.write ${DIR}`;

type Kept = { name: string; seconds: number; level: number };

async function index(storage: Storage): Promise<Kept[]> {
  try {
    return JSON.parse(new TextDecoder().decode(await storage.fs.readFile(INDEX))) as Kept[];
  } catch {
    return [];
  }
}

const nameOf = (path: string) => path.slice(path.lastIndexOf('/') + 1).replace(/\.[^.]*$/, '');
const label = (k: Kept) => `${k.name.replace('take-', 'Take ')} · ${k.seconds.toFixed(1)} s · level ${k.level}%`;

// Typed here rather than by `Sources`: the smoke builds this file against fixture plans that read neither source.
const sources: Record<string, (args: any, store: unknown, storage: Storage) => Promise<unknown>> = {
  takes: async (_args, _store, storage) => {
    let files: string[] = [];
    try {
      files = await storage.fs.readdir(DIR);
    } catch {
      // No takes yet: the directory is made by the first.
    }
    const kept = await index(storage);
    const takes = files.filter((f) => f !== 'index.json').map((f) => {
      const name = nameOf(f), k = kept.find((t) => t.name === name);
      return { name, label: k ? label(k) : name.replace('take-', 'Take '), path: `${DIR}/${f}` };
    }).sort((a, b) => Number(a.name.slice(5)) - Number(b.name.slice(5)));
    return { takes, next: takes.reduce((n, t) => Math.max(n, Number(t.name.slice(5)) || 0), 0) + 1 };
  },
  keepTake: async ([path, duration, level], _store, storage) => {
    const take: Kept = { name: nameOf(path), seconds: Math.round(duration / 100) / 10, level: Math.round(level * 100) };
    const kept = (await index(storage)).filter((t) => t.name !== take.name);
    await storage.fs.mkdir(DIR);
    await storage.fs.writeFile(INDEX, new TextEncoder().encode(JSON.stringify([...kept, take])));
    return { name: take.name, message: `Saved ${label(take)}` };
  },
};

export const answer = ((source: string, args: unknown, store: unknown, storage: Storage) => sources[source](args, store, storage)) as unknown as Answer;
