// The app's server: `bun server.ts` (bun:sqlite; a persona is a token). DB=:memory: gives
// a test run an empty database (test.mjs); the default keeps data.db beside it.
import { Database } from 'bun:sqlite';

const db = new Database(process.env.DB ?? 'data.db');
db.run('CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, title TEXT NOT NULL, notes TEXT NOT NULL, by TEXT NOT NULL)');
const PEOPLE: Record<string, string> = { 'token-ada': 'Ada', 'token-grace': 'Grace' };
const person = (req: Request) => PEOPLE[req.headers.get('authorization')?.replace('Bearer ', '') ?? ''];
// The web app's page is another origin: every answer, and the preflight, allows it.
const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'authorization, content-type', 'access-control-allow-methods': 'GET, POST, DELETE' };
const send = (body: unknown, status = 200) => Response.json(body, { status, headers: CORS });
const preflight = () => new Response(null, { status: 204, headers: CORS });
const signIn = () => send({ error: 'Sign in again.' }, 401);

Bun.serve({ hostname: '127.0.0.1', port: 8787, routes: {
  '/health': () => new Response('ok'),
  '/items': {
    OPTIONS: preflight,
    GET: (req) => person(req) ? send(db.query('SELECT id, title, notes FROM items ORDER BY id DESC').all()) : signIn(),
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
    DELETE: (req) => {
      if (!person(req)) return signIn();
      db.query('DELETE FROM items WHERE id = ?').run(Number(req.params.id));
      return send({ ok: true });
    },
  },
} });
console.log('server.ts: http://127.0.0.1:8787');
