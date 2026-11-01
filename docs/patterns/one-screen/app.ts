import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

const FILE = 'app:/data/log.json';
export const grants = `fs.read ${FILE}\nfs.write ${FILE}`;

type Drink = Result<'loadLog'>['entries'][number];
const load = (storage: Storage): Promise<Drink[]> => storage.fs.readFile(FILE).then(
  (bytes) => JSON.parse(new TextDecoder().decode(bytes)),
  (e) => { if (e.code === 'ENOENT') return []; throw e; }); // no file yet ('bake', the build, passes through)
const save = async (storage: Storage, all: Drink[]) => {
  await storage.fs.atomicWriteFile(FILE, new TextEncoder().encode(JSON.stringify(all)));
  return { ok: true, error: '' };
};

// This module has no clock: the view passes the day and the instant, and the module adds and orders for it.
const sources: Sources = {
  loadLog: async ([day], _store, storage) => {
    const all = await load(storage);
    return { total: all.filter((e) => e.day === day).reduce((sum, e) => sum + e.ml, 0), entries: all.slice(-20).reverse() };
  },
  addEntry: async ([ml, day, at], _store, storage) => {
    const all = await load(storage);
    return save(storage, [...all, { id: String(Math.max(0, ...all.map((e) => Number(e.id))) + 1), ml, day, at }]);
  },
  undoEntry: async ([day], _store, storage) => {
    const all = await load(storage), last = all.map((e) => e.day).lastIndexOf(day);
    return last < 0 ? { ok: true, error: '' } : save(storage, all.filter((_, i) => i !== last));
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
