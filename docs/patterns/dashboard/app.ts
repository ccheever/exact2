import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.example.starter';
const FILE = 'app:/data/expenses.json';
export const grants = `fs.read ${FILE}\nfs.write ${FILE}`; // a server's data: `net.fetch <origin>` here, and fetch in the source

type Expense = Result<'loadSummary'>['recent'][number];
// A budget per category, in cents; the file keeps an expense's category by id.
const categories = [
  { id: 'groceries', name: 'Groceries', budget: 45000 },
  { id: 'dining', name: 'Eating Out', budget: 20000 },
  { id: 'transport', name: 'Transport', budget: 15000 },
];
const sum = (rows: { amount: number }[]) => rows.reduce((total, e) => total + e.amount, 0);
const load = (storage: Storage): Promise<Expense[]> => storage.fs.readFile(FILE).then(
  (bytes) => JSON.parse(new TextDecoder().decode(bytes)),
  (e) => { if (e.code === 'ENOENT') return []; throw e; }); // no file yet ('bake', the build, passes through)
const save = async (storage: Storage, all: Expense[]) => {
  await storage.fs.atomicWriteFile(FILE, new TextEncoder().encode(JSON.stringify(all)));
  return { ok: true, error: '' };
};

const sources: Sources = {
  // The view has no reduce or sort: the data module adds and orders; the view shows. It has no
  // clock: `month` ("2026-03") is an argument, and each expense carries the `day` and the instant `at` the view read.
  loadSummary: async ([month], _store, storage) => {
    const rows = (await load(storage)).filter((e) => e.day.startsWith(month));
    const name = (id: string) => categories.find((c) => c.id === id)?.name ?? id;
    return {
      spent: sum(rows),
      budget: categories.reduce((total, c) => total + c.budget, 0),
      categories: categories.map((c) => ({ ...c, spent: sum(rows.filter((e) => e.category === c.id)) })),
      recent: rows.slice(-5).reverse().map((e) => ({ ...e, category: name(e.category) })),
    };
  },
  addExpense: async ([title, category, amount, day, at], _store, storage) => {
    const all = await load(storage);
    return save(storage, [...all, { id: String(Math.max(0, ...all.map((e) => Number(e.id))) + 1), title, category, amount, day, at }]);
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
