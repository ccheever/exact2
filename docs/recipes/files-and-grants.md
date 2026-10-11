# Recipe: files, SQLite and grants

Where an app keeps what it keeps, and the grant line (in `app.ts`'s `grants`, one per
line) that lets the data module reach it:

| What | Where | Grant |
| --- | --- | --- |
| A setting: units, a tip % | `state … persist` ([persisted-setting](persisted-setting.md)) | none |
| A small collection | a JSON file, `storage.fs` ([start-here](../start-here.md)'s list) | `fs.read app:/data/items.json`, `fs.write …` |
| A collection that grows or is queried | SQLite, `storage.sqlite` (below) | `sqlite.open app:/data/journal.db` |
| A secret: a session token | `store.get`, `store.set` | `secret.keep token` |
| A server | `fetch` ([rest-backend](rest-backend.md)) | `net.fetch https://api.example.com` |

```contract
shape Entry
  id: string
  text: string

shape Reply
  ok: bool
  error: string

component Journal
  state draft = ""
  resource entries = loadEntries() as shape list<Entry> else empty()
  mutation added as shape Reply refreshes entries then afterAdd
  derive addError = match added { case some(r) => r.error, case none => "" }
  action edit(v: string)
    draft = v
  action add
    if trim(draft) != "" and not pending(added)
      send added = addEntry(trim(draft))
  action afterAdd
    if addError == ""
      draft = ""
  view
    list appearance="auto" listStyle="inset-grouped" height="100%" testId="app"
      section
        row
          column flex=1 min-width=0 gap=4
            input appearance="none" value=draft input=edit submit=add placeholder="Today I…"
              aria-label="New entry" enterkeyhint="done" testId="draft"
            when addError != "" and not pending(added)
              text addError font="-exact-footnote" color="-exact-system-red" testId="error"
      section
        each e in entries key=e.id
          row
            text e.text testId=`entry-${e.id}`
```

```ts
import type { Answer, Database, Sources, Storage } from './app.contract.d.ts';

export const appId = 'com.example.journal'; // exact new writes this line
const DB = 'app:/data/journal.db';
export const grants = `sqlite.open ${DB}`;

let made = false; // the table, once a launch
async function use<T>(storage: Storage, work: (db: Database) => Promise<T>): Promise<T> {
  const db = await storage.sqlite.open(DB);
  try {
    if (!made) await db.execute('CREATE TABLE IF NOT EXISTS entries (id INTEGER PRIMARY KEY, text TEXT NOT NULL)');
    made = true;
    return await work(db);
  } finally { await db.close(); }
}
const sources: Sources = {
  loadEntries: (_args, _store, storage) => use(storage, async (db) =>
    (await db.query('SELECT id, text FROM entries ORDER BY id DESC')).rows
      .map(([id, text]) => ({ id: String(id), text: String(text) }))), // an INTEGER is a bigint
  addEntry: ([text], _store, storage) => use(storage, async (db) => {
    if (text.length > 280) return { ok: false, error: 'Keep an entry to 280 characters.' };
    await db.execute('INSERT INTO entries (text) VALUES (?)', [text]);
    return { ok: true, error: '' };
  }),
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "an entry is kept across a relaunch"
  type "draft" "Planted the tomatoes"
  type "draft" key "Enter"
  clock data
  reload
  expect text "entry-1" == "Planted the tomatoes"
```

- A grant names a path or a prefix (`fs.write app:/data` covers its tree). Anything
  outside the grants is refused with code `denied`. `app:/data` is kept, `app:/cache`
  holds what can be fetched again, `app:/tmp` is scratch.
- A file is bytes (`readFile`, `atomicWriteFile`); a missing one is code `ENOENT`:
  answer the empty collection, and branch on `e.code`, never the message.
- SQLite: `query` reads, `execute` writes, `transaction([...])` writes several or none;
  an integer comes back as a `bigint`.
- At build there is no storage (code `bake`): let it through, and the view shows the
  placeholder until the app runs. A drive needs `--storage <name>` to keep anything.
