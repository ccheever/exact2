// Shelf's data: the library, kept as one JSON file under app:/data/shelf, and
// Open Library's subjects. The view (app.contract) says what it wants; this
// file answers.
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.exact.shelf';
export const grants = [
  'fs.read app:/data/shelf',
  'fs.write app:/data/shelf',
  'net.fetch https://openlibrary.org',
].join('\n');

type Book = Result<'library'>['books'][number];
type Status = 'want' | 'reading' | 'done';

const DIR = 'app:/data/shelf';
const FILE = `${DIR}/library.json`;

const SEED: Book[] = [
  { id: 'seed-1', title: 'The Left Hand of Darkness', author: 'Ursula K. Le Guin', pages: 304, currentPage: 304, status: 'done', rating: 5, notes: 'Re-read every few years.', addedAt: 1700000000000, coverUrl: '', sourceKey: '' },
  { id: 'seed-2', title: 'Piranesi', author: 'Susanna Clarke', pages: 272, currentPage: 120, status: 'reading', rating: 0, notes: '', addedAt: 1700000100000, coverUrl: '', sourceKey: '' },
  { id: 'seed-3', title: 'The Dispossessed', author: 'Ursula K. Le Guin', pages: 387, currentPage: 0, status: 'want', rating: 0, notes: '', addedAt: 1700000200000, coverUrl: '', sourceKey: '' },
];

type Saved = { books: Book[]; goal: number };
let state: Saved | null = null;
let loading: Promise<Saved> | null = null;

// Storage runs one step at a time, in the order the answers asked.
let tail: Promise<unknown> = Promise.resolve();
function serial<T>(work: () => Promise<T>): Promise<T> {
  const run = tail.then(work);
  tail = run.catch(() => {});
  return run;
}

const encode = (v: unknown) => new TextEncoder().encode(JSON.stringify(v));
const decode = (b: ArrayBuffer) => new TextDecoder().decode(new Uint8Array(b));

function normalize(raw: Partial<Book>): Book {
  return {
    id: String(raw.id ?? ''),
    title: String(raw.title ?? ''),
    author: String(raw.author ?? 'Unknown author'),
    pages: Number(raw.pages ?? 0) || 0,
    currentPage: Number(raw.currentPage ?? 0) || 0,
    status: (['want', 'reading', 'done'].includes(String(raw.status)) ? raw.status : 'want') as Status,
    rating: Number(raw.rating ?? 0) || 0,
    notes: String(raw.notes ?? ''),
    addedAt: Number(raw.addedAt ?? 0) || 0,
    coverUrl: String(raw.coverUrl ?? ''),
    sourceKey: String(raw.sourceKey ?? ''),
  };
}

async function load(storage: Storage): Promise<Saved> {
  if (state) return state;
  if (!loading) {
    loading = serial(async () => {
      let text: string | null = null;
      try {
        text = decode(await storage.fs.readFile(FILE));
      } catch (error) {
        if ((error as { code?: string }).code === 'bake') throw error;
        text = null; // first launch: no file yet
      }
      let saved: Saved = { books: SEED.map((b) => ({ ...b })), goal: 12 };
      if (text) {
        try {
          const parsed = JSON.parse(text) as Partial<Saved>;
          saved = {
            books: Array.isArray(parsed.books) ? parsed.books.map(normalize) : saved.books,
            goal: Number(parsed.goal) || 12,
          };
        } catch {
          // A damaged file: start from the seed, as a first launch does.
        }
      }
      state = saved;
      return saved;
    });
    loading.catch(() => { loading = null; });
  }
  return loading;
}

async function persist(storage: Storage): Promise<void> {
  const snapshot = state;
  if (!snapshot) return;
  await serial(async () => {
    try { await storage.fs.mkdir(DIR); } catch { /* already there */ }
    await storage.fs.atomicWriteFile(FILE, encode(snapshot));
  });
}

async function change(storage: Storage, edit: (s: Saved) => Saved): Promise<Result<'removeBook'>> {
  try {
    const current = await load(storage);
    state = edit(current);
    await persist(storage);
    return { ok: true };
  } catch (error) {
    console.log(`shelf: save failed: ${String(error)}`);
    return { ok: false };
  }
}

// The store's rules: progress follows status and status follows progress.
function update(b: Book, patch: Partial<Book>): Book {
  const next = { ...b, ...patch };
  if (patch.status === 'done') next.currentPage = next.pages;
  if (patch.currentPage !== undefined) {
    next.currentPage = Math.max(0, Math.min(next.pages, patch.currentPage));
    if (next.currentPage === next.pages && next.pages > 0) next.status = 'done';
    else if (next.currentPage > 0 && next.status === 'want') next.status = 'reading';
  }
  return next;
}

function patchBook(storage: Storage, id: string, patch: Partial<Book>) {
  return change(storage, (s) => ({ ...s, books: s.books.map((b) => (b.id === id ? update(b, patch) : b)) }));
}

const sources: Sources = {
  library: async (_args, _store, storage) => {
    try {
      const { books, goal } = await load(storage);
      const sorted = [...books].sort((a, b) => b.addedAt - a.addedAt);
      const rated = books.filter((b) => b.status === 'done' && b.rating > 0);
      const avg = rated.length ? rated.reduce((s, b) => s + b.rating, 0) / rated.length : null;
      return {
        ready: true,
        books: sorted,
        goal,
        pagesRead: books.reduce((sum, b) => sum + b.currentPage, 0),
        avgText: avg === null ? '—' : `${avg.toFixed(1)} ★`,
      };
    } catch (error) {
      if ((error as { code?: string }).code !== 'bake') console.log(`shelf: load failed: ${String(error)}`);
      return { ready: false, books: [], goal: 12, pagesRead: 0, avgText: '—' };
    }
  },

  addBook: ([title, author, pages, status, coverUrl, sourceKey, at], _store, storage) =>
    change(storage, (s) => {
      if (sourceKey && s.books.some((b) => b.sourceKey === sourceKey)) return s;
      const book: Book = {
        id: `${at}${crypto.randomUUID().slice(0, 4)}`,
        title,
        author,
        pages,
        currentPage: status === 'done' ? pages : 0,
        status,
        rating: 0,
        notes: '',
        addedAt: at,
        coverUrl,
        sourceKey,
      };
      return { ...s, books: [book, ...s.books] };
    }),

  removeBook: ([id], _store, storage) => change(storage, (s) => ({ ...s, books: s.books.filter((b) => b.id !== id) })),
  setStatus: ([id, status], _store, storage) => patchBook(storage, id, { status }),
  setPage: ([id, page], _store, storage) => patchBook(storage, id, { currentPage: page }),
  setRating: ([id, rating], _store, storage) => patchBook(storage, id, { rating }),
  setNotes: ([id, notes], _store, storage) => patchBook(storage, id, { notes }),
  setGoal: ([n], _store, storage) => change(storage, (s) => ({ ...s, goal: Math.max(1, Math.min(365, n)) })),

  subjectWorks: async ([subject]) => {
    try {
      const res = await fetch(`https://openlibrary.org/subjects/${subject}.json?limit=20`);
      if (!res.ok) throw new Error(`Open Library returned ${res.status}`);
      const json = (await res.json()) as { works?: Array<{ key: string; title: string; authors?: Array<{ name?: string }>; cover_id?: number }> };
      return {
        ok: true,
        error: '',
        works: (json.works ?? []).map((w) => ({
          key: String(w.key),
          title: String(w.title),
          author: w.authors?.[0]?.name ?? 'Unknown author',
          pages: 300, // the subjects endpoint has no page counts; default and let the user edit
          coverUrl: w.cover_id ? `https://covers.openlibrary.org/b/id/${w.cover_id}-M.jpg` : '',
        })),
      };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e; // no network at build: leave it unbaked
      return { ok: false, error: (e as Error)?.message ?? 'Something went wrong', works: [] };
    }
  },
};

export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
