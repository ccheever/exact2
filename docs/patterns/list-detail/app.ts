import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

const FILE = 'app:/data/items.json';
export const grants = [`fs.read ${FILE}`, `fs.write ${FILE}`].join('\n');

type Item = Result<'loadItems'>[number];
let items: Item[] | null = null; // memory; the file is what survives a launch

async function load(storage: Storage): Promise<Item[]> {
  if (!items) items = await storage.fs.readFile(FILE).then(
    (bytes) => JSON.parse(new TextDecoder().decode(bytes)) as Item[],
    (e) => { if (e.code === 'ENOENT') return []; throw e; }); // no file yet; 'bake' (the build) passes through
  return items;
}
async function save(storage: Storage, next: Item[]): Promise<Result<'addItem'>> {
  items = next;
  await storage.fs.atomicWriteFile(FILE, new TextEncoder().encode(JSON.stringify(next)));
  return { ok: true, error: '' };
}

const sources: Sources = {
  loadItems: async (_args, _store, storage) => [...(await load(storage))].reverse(), // newest first
  addItem: async ([title, notes], _store, storage) => {
    const all = await load(storage);
    const same = all.find((i) => i.title.toLowerCase() === title.toLowerCase());
    if (same) return { ok: false, error: `There’s already an item called ${same.title}. Choose another title.` }; // a refusal is a reply too
    const id = String(Math.max(0, ...all.map((i) => Number(i.id))) + 1);
    return save(storage, [...all, { id, title, notes, locked: false }]);
  },
  lockItem: async ([id, locked], _store, storage) => save(storage, (await load(storage)).map((i) => i.id === id ? { ...i, locked } : i)),
  removeItem: async ([id], _store, storage) => save(storage, (await load(storage)).filter((i) => i.id !== id)),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
