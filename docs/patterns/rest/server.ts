// The app's server: `bun server.ts` (bun:sqlite; a persona's token is its name). It listens where
// app.ts calls it, its API: the one place the address is. `test` and `verify` run it there
// themselves on an empty database, its path in DB; the default keeps data.db beside it.
import { Database } from 'bun:sqlite';
import { API } from './app.ts';

const db = new Database(process.env.DB ?? 'data.db');
db.run('CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, title TEXT NOT NULL, notes TEXT NOT NULL, by TEXT NOT NULL, locked INTEGER NOT NULL DEFAULT 0)');
const PEOPLE: Record<string, string> = { ada: 'Ada', grace: 'Grace' };
const person = (req: Request) => PEOPLE[req.headers.get('authorization')?.replace('Bearer ', '') ?? ''];
// The web app's page is another origin: every answer, and the preflight, allows it.
const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'authorization, content-type', 'access-control-allow-methods': 'GET, POST, PATCH, DELETE' };
const send = (body: unknown, status = 200) => Response.json(body, { status, headers: CORS });
const preflight = () => new Response(null, { status: 204, headers: CORS });
const signIn = () => send({ error: 'Sign in again.' }, 401);

const { hostname, port } = new URL(API);
Bun.serve({ hostname, port: Number(port), routes: {
  '/health': () => new Response('ok'),
  '/items': {
    OPTIONS: preflight,
    GET: (req) => person(req) ? send(db.query('SELECT id, title, notes, locked FROM items ORDER BY id DESC').all()) : signIn(),
    POST: async (req) => {
      const by = person(req), { title, notes } = await req.json().catch(() => ({}));
      if (!by) return signIn();
      if (typeof title !== 'string' || !title.trim()) return send({ error: 'Enter a title.' }, 400);
      const same = db.query('SELECT title FROM items WHERE lower(title) = lower(?)').get(title.trim()) as { title: string } | null;
      if (same) return send({ error: `There’s already an item called ${same.title}. Choose another title.` }, 409);
      db.query('INSERT INTO items (title, notes, by) VALUES (?, ?, ?)').run(title.trim(), typeof notes === 'string' ? notes : '', by);
      return send({ ok: true }, 201);
    },
  },
  '/items/:id': {
    OPTIONS: preflight,
    PATCH: async (req) => {
      const { locked } = await req.json().catch(() => ({}));
      if (!person(req)) return signIn();
      if (typeof locked !== 'boolean') return send({ error: 'Say whether the item is locked.' }, 400);
      db.query('UPDATE items SET locked = ? WHERE id = ?').run(locked ? 1 : 0, Number(req.params.id));
      return send({ ok: true });
    },
    DELETE: (req) => {
      if (!person(req)) return signIn();
      // The app disables Delete on a locked item; the server refuses it too, as it would another client.
      const it = db.query('SELECT title, locked FROM items WHERE id = ?').get(Number(req.params.id)) as { title: string; locked: number } | null;
      if (it?.locked) return send({ error: `Unlock ${it.title} to delete it.` }, 409);
      db.query('DELETE FROM items WHERE id = ?').run(Number(req.params.id));
      return send({ ok: true });
    },
  },
} });
console.log(`server.ts: ${API}`);
