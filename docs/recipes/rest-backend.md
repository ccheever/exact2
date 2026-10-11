# Recipe: your own REST server

A list shared through a small Bun server: `fetch` with a bearer token, polling with a
`task`, every failure answered as words, and a fresh database for each test run.

**`server.ts`**, started with `bun server.ts` (`bun:sqlite`; a persona is a token):

```typescript
import { Database } from 'bun:sqlite';

const db = new Database(process.env.DB ?? 'data.db'); // DB=:memory: for a test run
db.run('CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, title TEXT NOT NULL, by TEXT NOT NULL)');
const PEOPLE: Record<string, string> = { 'token-ada': 'Ada', 'token-grace': 'Grace' };
const person = (req: Request) => PEOPLE[req.headers.get('authorization')?.replace('Bearer ', '') ?? ''];
// The web app's page is another origin: every answer, and the preflight, allows it.
const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'authorization, content-type' };
const send = (body: unknown, status = 200) => Response.json(body, { status, headers: CORS });

Bun.serve({ hostname: '127.0.0.1', port: 8787, routes: {
  '/health': () => new Response('ok'),
  '/items': {
    OPTIONS: () => new Response(null, { status: 204, headers: CORS }),
    GET: (req) => person(req) ? send(db.query('SELECT id, title, by FROM items ORDER BY id').all())
      : send({ error: 'Sign in again.' }, 401),
    POST: async (req) => {
      const by = person(req), { title } = await req.json().catch(() => ({}));
      if (!by) return send({ error: 'Sign in again.' }, 401);
      if (typeof title !== 'string' || !title.trim()) return send({ error: 'Enter a title.' }, 400);
      db.query('INSERT INTO items (title, by) VALUES (?, ?)').run(title.trim(), by);
      return send({ ok: true }, 201);
    },
  },
} });
```

**The app**: the list is read again every 4 seconds, and after each write.

```contract
routes nav
  home "/"

shape Item
  id: string
  title: string
  by: string

shape Items
  ok: bool
  error: string
  items: list<Item>

shape Reply
  ok: bool
  error: string

component Shared
  state draft = ""
  resource items = loadItems() as shape Items else empty(ok=true)
  mutation added as shape Reply queue refreshes items
  derive addError = match added { case some(r) => r.error, case none => "" }
  task poll mount
    every(4000, pull)
  action pull
    refresh items
  action edit(v: string)
    draft = v
  action add
    if trim(draft) != ""
      send added = addItem(trim(draft))
      draft = ""
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" testId="app" width="100%" height="100%"
      each e in stack(nav) key=e.id
        column navigationKey=`${e.id}` navigationScroll="list" position="absolute" inset=0
          display="flex" flex-direction="column"
          header display="flex" align-items="center" padding="8px 16px"
            text "Shared List" role="heading" aria-level=1
          list id="list" appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
            section
              row
                input appearance="none" value=draft input=edit submit=add placeholder="Add an item"
                  aria-label="New item" testId="draft" flex=1
              footer
                text (items.ok ? addError : items.error) testId="error"
            section
              each i in items.items key=i.id
                row
                  text i.title testId=`item-${i.id}`
                  text i.by
```

```ts
import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.shared'; // exact new writes this line
const API = 'http://127.0.0.1:8787';
export const grants = `net.fetch ${API}`;
const TOKEN = 'token-ada'; // a real sign-in keeps its token with `secret.keep token` and store.set

type Answered = { ok: true; body: unknown } | { ok: false; error: string };
async function call(path: string, init: RequestInit = {}): Promise<Answered> {
  try {
    const res = await fetch(`${API}${path}`, { ...init, exactTimeout: 10000,
      headers: { authorization: `Bearer ${TOKEN}`, 'content-type': 'application/json' } });
    const body = (await res.json().catch(() => ({}))) as { error?: unknown };
    if (res.ok) return { ok: true, body };
    return { ok: false, error: typeof body.error === 'string' ? body.error : 'The server had a problem. Try again.' };
  } catch (e) {
    if ((e as { code?: string }).code === 'bake') throw e; // no server at build
    return { ok: false, error: 'Can’t reach the server. Your list will update when it’s back.' };
  }
}

let last: Result<'loadItems'>['items'] = []; // a failed poll keeps the list on screen
const sources: Sources = {
  loadItems: async () => {
    const r = await call('/items');
    if (!r.ok) return { ok: false, error: r.error, items: last };
    last = (r.body as Array<{ id: number; title: string; by: string }>).map((i) => ({ id: String(i.id), title: i.title, by: i.by }));
    return { ok: true, error: '', items: last };
  },
  addItem: async ([title]) => {
    const r = await call('/items', { method: 'POST', body: JSON.stringify({ title }) });
    return r.ok ? { ok: true, error: '' } : { ok: false, error: r.error };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

```contract-test
test "an item is added for everyone; a lost server is said in words"
  type "draft" "Milk"
  type "draft" key "Enter"
  clock data
  expect text "item-1" == "Milk"
  fail fetch "http://127.0.0.1:8787"
  clock +4000
  clock data
  expect text "error" == "Can’t reach the server. Your list will update when it’s back."
  expect text "item-1" == "Milk"
```

**`test.mjs`** gives every test run its own server and an empty database, so a test
never meets rows the dev server or another run left. Name it in `app.json` (`"commands":
{"api-test": ["bun", "test.mjs"]}`) and run `bun exact.mjs api-test web`:

```js
const up = () => fetch('http://127.0.0.1:8787/health').then((r) => r.ok, () => false);
if (await up()) throw new Error('port 8787 is in use: stop the dev server first, so the tests get an empty database');
const server = Bun.spawn(['bun', 'server.ts'], { env: { ...process.env, DB: ':memory:' }, stdio: ['ignore', 'inherit', 'inherit'] });
for (let i = 0; !(await up()); i++) { if (i > 50) throw new Error('server.ts did not start'); await Bun.sleep(100); }
const tests = Bun.spawn(['bun', 'exact.mjs', 'test', ...process.argv.slice(2)], { stdio: ['inherit', 'inherit', 'inherit'] });
const code = await tests.exited;
server.kill();
process.exit(code);
```

- Every failure is an answer the view shows in words: the server's own `error`, or a
  sentence for a lost connection. Never a status code, `Refused: E_INPUT`, or a
  "Synced" line.
- `task poll mount` with `every(4000, pull)` polls; `queue refreshes items` reads again
  after each write. A failed poll keeps the last list (`last`) under its message.
- The grant names the server's origin, `http://127.0.0.1:8787`. On the web the page is
  another origin, so the server answers CORS (the `OPTIONS` preflight too). A phone
  reaches the Mac by its LAN address or HTTPS, granted instead.
- Before calling it done, drive each write on the real clock against the dev server:
  [tests](tests.md), "Before you call it done".
