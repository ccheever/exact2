// A stand-in for an app's Tamber server, answering canned `tamber/1` replies,
// for the tool's contract tests (`test` and `verify` run it where app.ts
// calls, app.json's `commands.server`). The real server is
// scripts/tamber-server.mjs; the tool's own drives use it
// (`bun exact.mjs agent web --url …`). A test adds an event as another client
// would with `request POST "/__stub/event" {…} as ada`. Its event log is kept
// in DB, a JSON file.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { API } from '../app.ts';

const DB = process.env.DB ?? '.exact/tamber-stub.json';
const TOKEN = 'stub-token';
const EPOCH = 'stub-epoch';
const FILES = { 'app.tamber': 'a'.repeat(64), 'brand.tamber': 'b'.repeat(64) };
type Event = Record<string, unknown> & { seq: number };
const log: Event[] = existsSync(DB) ? JSON.parse(readFileSync(DB, 'utf8') || '[]') : [];
const save = () => writeFileSync(DB, JSON.stringify(log));

// The tool's page is another origin under the test driver.
const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type, x-exact-tamber-token, authorization', 'access-control-allow-methods': 'GET, POST' };
const send = (body: unknown, status = 200) => Response.json(body, { status, headers: CORS });
const preflight = () => new Response(null, { status: 204, headers: CORS });
const authorized = (req: Request) => req.headers.get('x-exact-tamber-token') === TOKEN;
const seq = () => log.at(-1)?.seq ?? 0;

const { hostname, port } = new URL(API);
Bun.serve({ hostname, port: Number(port), routes: {
  '/health': () => new Response('ok'),
  '/__tamber/token': { GET: () => send({ token: TOKEN }) },
  '/__tamber': {
    OPTIONS: preflight,
    POST: async (req) => {
      if (!authorized(req)) return send({ message: 'Not found.' }, 404);
      const request = await req.json().catch(() => ({}));
      if (request.method === 'hello') return send({ ok: { protocol: 'tamber/1.1', format: 1, builds: [], app: '/stub/theme-fixture', files: FILES } });
      return send({ error: { message: `the stub answers no ${request.method}` } }, 400);
    },
  },
  // Never held: the stub answers at once, so `clock data` lands it.
  '/__tamber/events': {
    OPTIONS: preflight,
    POST: async (req) => {
      if (!authorized(req)) return send({ message: 'Not found.' }, 404);
      const { after = 0, epoch } = await req.json().catch(() => ({}));
      const missed = epoch !== EPOCH;
      return send({ events: missed ? [] : log.filter(e => e.seq > after), seq: seq(), epoch: EPOCH, missed, files: FILES, proposals: [] });
    },
  },
  '/__stub/event': {
    OPTIONS: preflight,
    POST: async (req) => {
      const e = await req.json().catch(() => ({}));
      log.push({ event: 'changed', epoch: EPOCH, client: null, changed: [], files: FILES, edited: [], ...e, seq: seq() + 1 });
      save();
      return send({ ok: true });
    },
  },
} });
console.log(`tamber-stub: ${API}`);
