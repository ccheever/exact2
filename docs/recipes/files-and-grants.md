# Recipe: files, SQLite and grants

Where an app keeps what it keeps, and the line of `app.ts`'s `grants` that reaches it:

| What | Where | Grant |
| --- | --- | --- |
| A setting | `state … persist` ([persisted-setting](persisted-setting.md)) | none |
| A small collection | a JSON file, `storage.fs` (start-here's list) | `fs.read app:/data/items.json`, `fs.write …` |
| A collection that grows, or is queried | SQLite, `storage.sqlite` | `sqlite.open app:/data/journal.db` |
| A secret: a token | `store.get`, `store.set` | `secret.keep token` |
| A server | `fetch` ([rest-backend](rest-backend.md)) | `net.fetch https://api.example.com` |

A journal in SQLite (its field and Add are start-here's):

```contract
shape Entry
  id: string
  text: string

shape Reply
  ok: bool
  error: string

component Journal
  resource entries = loadEntries() as shape Entry[] else empty()
  mutation added as shape Reply refreshes entries
  action add(text: string)
    send added = addEntry(text)
  view
    column
      for e in entries key=e.id
        text e.text
```

```ts
import type { Answer, Database, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.example.journal';
const DB = 'app:/data/journal.db';
export const grants = `sqlite.open ${DB}`;

async function use<T>(storage: Storage, work: (db: Database) => Promise<T>): Promise<T> {
  const db = await storage.sqlite.open(DB);
  try {
    await db.execute('CREATE TABLE IF NOT EXISTS entries (id INTEGER PRIMARY KEY, text TEXT NOT NULL)');
    return await work(db);
  } finally { await db.close(); }
}
const sources: Sources = {
  loadEntries: (_args, _store, storage) => use(storage, async (db) =>
    (await db.query('SELECT id, text FROM entries ORDER BY id DESC')).rows
      .map(([id, text]) => ({ id: String(id), text: String(text) }))), // an INTEGER is a bigint
  addEntry: ([text], _store, storage) => use(storage, async (db) => {
    await db.execute('INSERT INTO entries (text) VALUES (?)', [text]);
    return { ok: true, error: '' };
  }),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

- A grant names a path or a prefix (`fs.write app:/data` covers its tree); anything else
  is refused, code `denied`. `app:/data` is kept, `app:/cache` can be fetched again,
  `app:/tmp` is scratch.
- A file is bytes (`readFile`, `atomicWriteFile`); a missing one is code `ENOENT`: answer
  the empty collection, and branch on `e.code`, never the message.
- SQLite: `query` reads, `execute` writes, `transaction([...])` writes several or none.
- At build there is no storage (code `bake`): let it through.
