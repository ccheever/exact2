// A stand-in for an app's Tamber server, answering canned `tamber/1` replies,
// for the tool's contract tests (`test` and `verify` run it where app.ts
// calls, app.json's `commands.server`). The real server is
// scripts/tamber-server.mjs; the tool's own drives use it
// (`bun exact.mjs agent web --url …`). A test adds an event as another client
// would with `request POST "/__stub/event" {…} as ada`, and restarts it
// with `/__stub/restart`. Its event log is kept
// in DB, a JSON file.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { API } from '../app.ts';
import DIALS from './dials.json';

const DB = process.env.DB ?? '.exact/tamber-stub.json';
const TOKEN = 'stub-token';
let EPOCH = 'stub-epoch', restarts = 0, writes = 0;
let FILES: Record<string, string> = { 'app.tamber': 'a'.repeat(64), 'brand.tamber': 'b'.repeat(64) };
type Event = Record<string, unknown> & { seq: number };
const log: Event[] = existsSync(DB) ? JSON.parse(readFileSync(DB, 'utf8') || '[]') : [];
const save = () => writeFileSync(DB, JSON.stringify(log));

// The tool's page is another origin under the test driver.
const CORS = { 'access-control-allow-origin': '*', 'access-control-allow-headers': 'content-type, x-exact-tamber-token, authorization', 'access-control-allow-methods': 'GET, POST' };
const send = (body: unknown, status = 200) => Response.json(body, { status, headers: CORS });
const preflight = () => new Response(null, { status: 204, headers: CORS });
const authorized = (req: Request) => req.headers.get('x-exact-tamber-token') === TOKEN;
// The test driver, as `request … as ada` sends it; no page can.
const driver = (req: Request) => req.headers.get('authorization') === 'Bearer ada';
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
      // stub/dials.tamber as the real server read it (stub/dials.json;
      // scripts/design-tool.test.mjs checks the two still agree).
      // Only app.tamber has a canned design; another file is refused, as a
      // server refuses a design that does not check.
      if (['describe', 'resolve', 'explain', 'propose'].includes(request.method) && (request.file ?? 'app.tamber') !== 'app.tamber') return send({ error: { message: `the stub has no design in ${request.file}` } }, 400);
      // Its digests are the stub's own, which hello and the events carry.
      if (request.method === 'describe') return send({ ok: { ...DIALS.describe, files: FILES } });
      if (request.method === 'resolve') return DIALS.resolve[request.token] ? send({ ok: DIALS.resolve[request.token] }) : send({ error: { message: `no token \`${request.token}\`` } }, 400);
      if (request.method === 'explain') return DIALS.explain[request.token] ? send({ ok: DIALS.explain[request.token] }) : send({ error: { message: `no token \`${request.token}\`` } }, 400);
      // A write is not applied: canned answers in its shape, enough for the
      // tool's own flow (its edits are tested on a real server).
      if (request.method === 'propose') return send({ ok: { files: FILES, class: 'live', changed: request.ops.map((o: { name?: string; from?: string }) => ({ file: 'app.tamber', token: o.name ?? o.from })), diff: `--- app.tamber\n+++ app.tamber\n+  ${JSON.stringify(request.ops[0])}\n` } });
      if (request.method === 'transaction') return send({ ok: { files: FILES, written: ['app.tamber'], client: request.client, inverse: `inv-stub-${++writes}`, changed: [], class: 'live', diff: '' } });
      if (request.method === 'undo') return send({ ok: { files: FILES, written: ['app.tamber'], client: request.client, inverse: null, changed: [], class: 'live', diff: '' } });
      return send({ error: { message: `the stub answers no ${request.method}` } }, 400);
    },
  },
  // Never held: the stub answers at once, so `clock data` lands it.
  '/__tamber/events': {
    OPTIONS: preflight,
    POST: async (req) => {
      if (!authorized(req)) return send({ message: 'Not found.' }, 404);
      const { after = 0, epoch } = await req.json().catch(() => ({}));
      const missed = epoch !== undefined && epoch !== EPOCH;
      return send({ events: missed ? [] : log.filter(e => e.seq > after), seq: seq(), epoch: EPOCH, missed, files: FILES, proposals: [] });
    },
  },
  // A new run of the server: a new epoch, its numbers from 0 again.
  '/__stub/restart': {
    POST: (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      EPOCH = `stub-epoch-${++restarts}`;
      log.length = 0;
      save();
      return send({ ok: true });
    },
  },
  '/__stub/event': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      const e = await req.json().catch(() => ({}));
      // An event's `files` are the digests from then on.
      if (e.files) FILES = { ...FILES, ...e.files };
      log.push({ event: 'changed', epoch: EPOCH, client: null, changed: [], edited: [], ...e, files: FILES, seq: seq() + 1 });
      save();
      return send({ ok: true });
    },
  },
} });
console.log(`tamber-stub: ${API}`);
