// Shelf's data: the library, kept as one JSON file under app:/data/shelf, and
// Open Library's subjects and ISBNs. The view (app.contract) says what it
// wants; this file answers.
import type { Answer, Result, Sources, Storage } from './app.contract.d.ts';
import { ndefRecords, write, type NDEFRecord, type NDEFRecordInit } from '@exact/nfc';

export const appId = 'com.exact.shelf';
export const grants = [
  'fs.read app:/data/shelf',
  'fs.write app:/data/shelf',
  'net.fetch https://openlibrary.org',
  'device.camera scanPurpose',
  'device.nfc nfcPurpose',
  'fs.read app:/tmp/picked',
].join('\n');

type Book = Result<'library'>['books'][number];
type Status = 'want' | 'reading' | 'done';

const DIR = 'app:/data/shelf';
const FILE = `${DIR}/library.json`;

const SEED: Book[] = [
  { id: 'seed-1', title: 'The Left Hand of Darkness', author: 'Ursula K. Le Guin', pages: 304, currentPage: 304, status: 'done', rating: 5, notes: 'Re-read every few years.', addedAt: 1700000000000, coverUrl: '', sourceKey: '', quotes: 0 },
  { id: 'seed-2', title: 'Piranesi', author: 'Susanna Clarke', pages: 272, currentPage: 120, status: 'reading', rating: 0, notes: '', addedAt: 1700000100000, coverUrl: '', sourceKey: '', quotes: 0 },
  { id: 'seed-3', title: 'The Dispossessed', author: 'Ursula K. Le Guin', pages: 387, currentPage: 0, status: 'want', rating: 0, notes: '', addedAt: 1700000200000, coverUrl: '', sourceKey: '/works/OL59863W', quotes: 0 },
];

type Saved = { books: Book[]; goal: number };
let state: Saved | null = null;
let loading: Promise<Saved> | null = null;

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
    quotes: Number(raw.quotes ?? 0) || 0,
  };
}

async function load(storage: Storage): Promise<Saved> {
  if (state) return state;
  if (!loading) {
    loading = (async () => {
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
    })();
    loading.catch(() => { loading = null; });
  }
  return loading;
}

// The module's storage runs one operation at a time, in the order issued
// (LLP 1097 D3), so both are issued here, in this answer's own turn: the
// directory is made before the write, and a later edit writes after this
// one. A write chained on another answer's would run after that answer
// replied, where the module counts it as storage no answer awaits.
async function persist(storage: Storage): Promise<void> {
  const snapshot = state;
  if (!snapshot) return;
  await Promise.all([
    storage.fs.mkdir(DIR).catch(() => { /* already there */ }),
    storage.fs.atomicWriteFile(FILE, encode(snapshot)),
  ]);
}

// An edit applies to the newest state, read and set with no await between:
// two edits that waited on the same first load would otherwise both start
// from its snapshot, and the later persist would drop the earlier edit.
async function change(storage: Storage, edit: (s: Saved) => Saved): Promise<Result<'removeBook'>> {
  try {
    await load(storage);
    state = edit(state!);
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

// Shelf's own NDEF record: an external type whose data is the book's id.
const BOOK_RECORD = 'com.exact.shelf:book';

/** A failed write's cause: the module's (D3 puts it after the error's name), else ours. */
function tagCause(e: unknown): string {
  const code = (e as { code?: string })?.code ?? '';
  const said = String((e as Error)?.message ?? '').split(': ').at(-1) ?? '';
  if (code === 'AbortError') return 'cancelled';
  if (code === 'NotSupportedError') return 'not-ndef';
  if (code === 'NotAllowedError') return 'blocked';
  if (code === 'NetworkError' && ['read-only', 'too-small', 'lost'].includes(said)) return said;
  return 'failed';
}

const sources: Sources = {
  widgetLibrary: async ([date], _store, storage) => {
    const { books } = await load(storage);
    const latest = [...books].sort((a, b) => b.addedAt - a.addedAt)[0]?.title ?? 'Add your first book';
    return [{ date, props: { count: books.length, latest } }];
  },

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
        quotes: 0,
      };
      return { ...s, books: [book, ...s.books] };
    }),

  removeBook: ([id], _store, storage) => change(storage, (s) => ({ ...s, books: s.books.filter((b) => b.id !== id) })),
  setStatus: ([id, status], _store, storage) => patchBook(storage, id, { status }),
  setPage: ([id, page], _store, storage) => patchBook(storage, id, { currentPage: page }),
  setRating: ([id, rating], _store, storage) => patchBook(storage, id, { rating }),
  setNotes: ([id, notes], _store, storage) => patchBook(storage, id, { notes }),
  // A quote goes after the notes as its own paragraph; `quotes` counts them,
  // so the view starts its draft again from the notes it now has.
  appendQuote: ([id, quote], _store, storage) =>
    change(storage, (s) => ({
      ...s,
      books: s.books.map((b) => (b.id === id ? { ...b, notes: b.notes.trim() === '' ? quote : `${b.notes.trimEnd()}\n\n${quote}`, quotes: b.quotes + 1 } : b)),
    })),
  setGoal: ([n], _store, storage) => change(storage, (s) => ({ ...s, goal: Math.max(1, Math.min(365, n)) })),

  // An ISBN's edition and its first author, from Open Library's Books API
  // (https://openlibrary.org/dev/docs/api/books): `/isbn/{isbn}.json`
  // redirects to the edition, whose `authors` are keys of author records.
  bookByIsbn: async ([isbn]) => {
    const none = { isbn, found: false, title: '', author: '', pages: 0 };
    if (isbn === '') return none;
    try {
      const res = await fetch(`https://openlibrary.org/isbn/${isbn}.json`);
      if (!res.ok) return none;
      const edition = (await res.json()) as { title?: string; number_of_pages?: number; authors?: Array<{ key?: string }>; by_statement?: string };
      let author = edition.by_statement ?? '';
      const key = edition.authors?.[0]?.key;
      if (key) {
        const who = await fetch(`https://openlibrary.org${key}.json`);
        if (who.ok) author = String(((await who.json()) as { name?: string }).name ?? author);
      }
      return { isbn, found: Boolean(edition.title), title: String(edition.title ?? ''), author: author || 'Unknown author', pages: Number(edition.number_of_pages ?? 0) || 0 };
    } catch (e) {
      if ((e as { code?: string }).code === 'bake') throw e;
      return none;
    }
  },

  // Tag this book (LLP 1133.035 §11): the book's Open Library page first, which
  // a phone without Shelf opens, then Shelf's own record naming this copy. On
  // iOS the sheet says how it went; elsewhere the view does, from `cause`.
  tagBook: async ([id], _store, storage) => {
    const { books } = await load(storage);
    const book = books.find((b) => b.id === id);
    if (!book) return { ok: false, cause: 'gone' };
    const records: NDEFRecordInit[] = [];
    if (book.sourceKey) records.push({ recordType: 'url', mediaType: null, id: null, encoding: null, lang: null, data: `https://openlibrary.org${book.sourceKey}` });
    records.push({ recordType: BOOK_RECORD, mediaType: null, id: null, encoding: null, lang: null, data: new TextEncoder().encode(book.id) });
    try {
      await write({
        message: { records },
        apple: {
          alertMessage: 'nfc.holdBook',
          doneMessage: 'nfc.tagged',
          errorMessages: { readOnly: 'nfc.locked', tooSmall: 'nfc.tooSmall', lost: 'nfc.holdStill', notNdef: 'nfc.noLink', failed: 'nfc.failed', unreadable: null },
        },
      });
      return { ok: true, cause: '' };
    } catch (e) {
      return { ok: false, cause: tagCause(e) };
    }
  },

  // The book a reading names: Shelf's own record, else the work key of its
  // Open Library link; `id` empty and `url` the link for a book not here.
  bookForTag: async ([reading], _store, storage) => {
    const { books } = await load(storage);
    let records: NDEFRecord[] = [];
    try {
      records = await ndefRecords({ reading });
    } catch {
      return { id: '', url: '' };
    }
    const own = records.find((r) => r.recordType === BOOK_RECORD);
    const ownId = own ? new TextDecoder().decode(own.data) : '';
    if (ownId && books.some((b) => b.id === ownId)) return { id: ownId, url: '' };
    const url = records.find((r) => r.recordType === 'url' || r.recordType === 'absolute-url' || r.recordType === 'smart-poster')?.text ?? '';
    const key = /^https?:\/\/(?:www\.)?openlibrary\.org(\/works\/OL\d+W)/.exec(url)?.[1] ?? '';
    const match = key ? books.find((b) => b.sourceKey === key) : undefined;
    return { id: match?.id ?? '', url };
  },

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
