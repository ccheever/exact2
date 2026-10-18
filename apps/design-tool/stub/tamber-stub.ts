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
import { scrubSessions } from '../../../scripts/tamber-scrub.mjs';
import DIALS from './dials.json';

const DB = process.env.DB ?? '.exact/tamber-stub.json';
// A run's token: a restart issues another, as the server does.
let TOKEN = 'stub-token';
let EPOCH = 'stub-epoch', restarts = 0, writes = 0;
// The next commit's answer: refused, naming why, when a test asks.
let refuseCommit = '';
// The dev loop's page hello names, as a test sets it (`/__stub/page`): none
// by default, as `tamber tool` alone.
let PAGE: string | null = null;
// The builds hello lists, as a test's `builds` event sets them, and the
// pins' revision: a pin or preview (which replaces the build's pin, as the
// server's does) is announced as a `builds` event, each pinned build naming
// its `pinRevision`, and answered with it; its status is the test's to send.
// The clients present: those whose long poll named them.
let BUILDS: Record<string, any>[] = [], pins = 0;
const present = new Set<string>();
// A dev loop serving the wire (hello's `live`), as a test sets it
// (`/__stub/live`). Scrubs are the server's own sessions
// (scripts/tamber-scrub.mjs), planned and written here in canned shapes:
// a preview takes `previewMs` (`/__stub/slow-preview`), and a release a
// test refuses as moved (`/__stub/refuse-release`) names who moved it. An
// edit of the warmth dial is rebuild-class (as if it fed a role override),
// both proposed and scrubbed; so is a motion token's, as the server says.
const REBUILDS = new Set(['warmth', 'duration.fast', 'easing.standard', 'app.contract#Card.fade', 'Card.fade', 'app.tamber#MapCard.pan', 'MapCard.pan']);
// A token the JS loop's pages write inline: rebuild-class while a web-js
// build runs, as the serving loop says (LLP 1121 stage 6).
const INLINE = new Set(['opacity.muted']);
const classOf = (ops: { name?: string }[]) => ops.some(o => REBUILDS.has(o.name ?? '') || (INLINE.has(o.name ?? '') && BUILDS.some(b => b.host === 'web-js'))) ? 'rebuild' : 'live';
// The design's axes in the other order, as a test reorders them
// (`/__stub/reverse-axes`): describe says so from then on. `describe`
// refused (an event's `refuseDescribe`), so a test sees whether the tool
// reads the design again.
let LIVE = false, refuseRelease = '', previewMs = 0, reversed = false, refuseDescribe = '', describePasses = 0;
const changedOf = (ops: { name?: string }[]) => ops.map(o => ({ file: 'app.tamber', token: o.name }));
// A run's sessions (a restart starts their ids again, as a new server's do).
const sessions = () => scrubSessions({
  ask: async (r: { ops: { name?: string }[] }) => ({ ok: { class: r.ops.some(o => REBUILDS.has(o.name ?? '')) ? 'rebuild' : 'live', texts: {}, changed: changedOf(r.ops), synthetic: [], files: FILES } }),
  write: async (r: { client: string; ops: { name?: string }[] }) => {
    if (refuseRelease) { const by = refuseRelease; refuseRelease = ''; return { status: 409, error: { message: 'stale digests: app.tamber', conflict: true, by: { 'app.tamber': by } } }; }
    // The digest after a scrub's write names the value written, so a test
    // reads what was kept (`digest-app.tamber`).
    ++writes;
    FILES = { ...FILES, 'app.tamber': String((r.ops[0] as { value?: string })?.value ?? writes).padEnd(64, '0') };
    log.push({ event: 'changed', epoch: EPOCH, client: r.client, changed: changedOf(r.ops), edited: ['app.tamber'], files: FILES, seq: seq() + 1 });
    save();
    return { ok: { files: FILES, written: ['app.tamber'], client: r.client, inverse: `inv-scrub-${writes}`, changed: [], class: 'live', diff: '' } };
  },
  preview: async () => { if (previewMs) await Bun.sleep(previewMs); return {}; },
  restore: () => {},
  opened: (e: Record<string, unknown>) => { log.push({ event: 'scrub', epoch: EPOCH, ...e, reason: 'opened', seq: seq() + 1 }); save(); },
  ended: (e: Record<string, unknown>) => { log.push({ event: 'scrub', epoch: EPOCH, ...e, seq: seq() + 1 }); save(); },
});
let scrubs = sessions();
const answered = (r: { status?: number; ok?: unknown; error?: unknown }) => r.ok ? send({ ok: r.ok }) : send({ error: r.error }, r.status ?? 409);
// The next poll says it missed events (more than the server keeps).
let miss = false;
// Held proposals, as a test holds them (`/__stub/hold`).
let held: Record<string, unknown>[] = [], holds = 0;
let FILES: Record<string, string> = { 'app.tamber': 'a'.repeat(64), 'brand.tamber': 'b'.repeat(64) };
type Event = Record<string, unknown> & { seq: number };
const log: Event[] = existsSync(DB) ? JSON.parse(readFileSync(DB, 'utf8') || '[]') : [];
const save = () => writeFileSync(DB, JSON.stringify(log));

/** One event as a test adds it: logged, and the state it moves. */
function take(e: Record<string, any>) {
  // An event's `files` are the digests from then on.
  if (e.files) FILES = { ...FILES, ...e.files };
  // A `builds` event is the list hello gives from then on, and a
  // `build-status` one that build's entry.
  // (A record is the server's: its last refusal kept on it.)
  if (e.event === 'builds') BUILDS = (e.builds ?? []).map((b: Record<string, unknown>) => ({ ...b }));
  if (e.event === 'build-status') BUILDS = BUILDS.map(b => b.id === e.build?.id ? { ...b, ...e.build } : b);
  if (e.event === 'unsupported') BUILDS = BUILDS.map(b => b.id === e.build ? { ...b, unsupported: { what: e.what, detail: e.detail ?? null } } : b);
  log.push({ event: 'changed', epoch: EPOCH, client: null, changed: [], edited: [], ...e, files: FILES, seq: seq() + 1 });
}

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
      if (request.method === 'hello') return send({ ok: { protocol: 'tamber/1.1', format: 1, builds: BUILDS, app: '/stub/theme-fixture', files: FILES, epoch: EPOCH, design: 'app.tamber', page: PAGE, live: LIVE } });
      // Scrubs, as the server takes them: a dev loop and a present client.
      if (request.method?.startsWith('scrub')) {
        if (!LIVE) return send({ error: { message: 'no builds preview here: a scrub needs the dev loop\'s design server' } }, 501);
        if (request.method === 'scrub-cancel') return answered(scrubs.cancel(request));
        if (request.method === 'scrub-release') return answered(await scrubs.release(request));
        if (!present.has(request.client)) return send({ error: { message: `scrub: \`${request.client}\` is not present` } }, 409);
        return answered(await scrubs.update(request));
      }
      // Any other write ends a scrub first, as the server's do.
      if (['transaction', 'undo', 'accept'].includes(request.method)) scrubs.compete();
      // Inspect goes to the builds that say they can (LLP 1122 D4); `uses`
      // is canned for surface.card; `outline` answers with its sites.
      if (request.method === 'inspect') {
        if (!present.has(request.client)) return send({ error: { message: `inspect: \`${request.client}\` is not present` } }, 409);
        const can = BUILDS.filter(b => (b.capabilities ?? []).includes('inspect')).map(b => b.id);
        // Its owners move, said as the server says them (tamber/1.5).
        const was = new Map(BUILDS.map(b => [b.id, b.inspector ?? null]));
        BUILDS = BUILDS.map(b => request.on ? (can.includes(b.id) ? { ...b, inspector: request.client } : b) : (b.inspector === request.client ? { ...b, inspector: null } : b));
        const lost = new Map<string, string[]>(), took: string[] = [];
        for (const b of BUILDS) { const before = was.get(b.id); if (before !== (b.inspector ?? null)) { if (before) lost.set(before, [...(lost.get(before) ?? []), b.id]); if (b.inspector) took.push(b.id); } }
        for (const [owner, builds] of lost) log.push({ event: 'inspect', epoch: EPOCH, client: owner, on: false, builds, to: request.on ? request.client : null, seq: seq() + 1 });
        if (took.length) log.push({ event: 'inspect', epoch: EPOCH, client: request.client, on: true, builds: took, seq: seq() + 1 });
        save();
        return send({ ok: { builds: can, unsupported: BUILDS.map(b => b.id).filter(id => !can.includes(id)) } });
      }
      if (request.method === 'uses') return send({ ok: { token: request.token, plan: 'p1', sites: request.token === 'surface.card' ? [
        { site: 3, row: 'background_color', file: '/stub/app.contract', line: 12, col: 5, end_col: 9, component: 'App' },
        { site: 7, row: 'background_color', file: '/stub/card.contract', line: 4, col: 7, end_col: 11, component: 'Card', via: { token: 'surface.raised' } },
      ] : [], aliases: [], files: FILES } });
      if (request.method === 'outline') return send({ ok: { sites: (request.at ?? []).map((_: unknown, i: number) => i), builds: BUILDS.filter(b => (b.capabilities ?? []).includes('inspect')).map(b => b.id) } });
      // A pin names a present client, and a build the server lists.
      if (request.method === 'pin' || request.method === 'preview') {
        if (!present.has(request.client)) return send({ error: { message: `\`${request.client}\` is not present: a client's pins live while its subscription or long poll does` } }, 409);
        if (request.build && !BUILDS.some(b => b.id === request.build)) return send({ error: { message: `no registered build \`${request.build}\`` } }, 400);
        const ids = request.build ? [request.build] : BUILDS.map(b => b.id);
        pins += 1;
        // Each record replaced, never edited: an event logged before keeps
        // what it said.
        BUILDS = BUILDS.map(b => ids.includes(b.id) ? { ...b, pin: request.method === 'preview' ? { mode: { [request.axis]: request.value } } : request.pin, pinRevision: pins } : b);
        log.push({ event: 'builds', epoch: EPOCH, builds: BUILDS, seq: seq() + 1 });
        save();
        return send({ ok: { builds: ids, revision: pins } });
      }
      // stub/dials.tamber as the real server read it (stub/dials.json;
      // scripts/design-tool.test.mjs checks the two still agree).
      // Only app.tamber has a canned design; another file is refused, as a
      // server refuses a design that does not check.
      if (['describe', 'resolve', 'explain', 'propose'].includes(request.method) && (request.file ?? 'app.tamber') !== 'app.tamber') return send({ error: { message: `the stub has no design in ${request.file}` } }, 400);
      // Its digests are the stub's own, which hello and the events carry.
      if (request.method === 'describe' && refuseDescribe && describePasses-- <= 0) return send({ error: { message: refuseDescribe } }, 400);
      if (request.method === 'describe') return send({ ok: { ...DIALS.describe, ...(reversed ? { ir: { ...DIALS.describe.ir, axes: [...DIALS.describe.ir.axes].reverse() } } : {}), files: FILES } });
      // Reordered axes put each tuple at its index in the new order.
      const at = (modes: Record<string, string>) => [...DIALS.describe.ir.axes].reverse().reduce((n: number, a: { name: string; values: string[] }) => n * a.values.length + a.values.indexOf(modes[a.name]), 0);
      const order = (tuples: { modes: Record<string, string> }[]) => !reversed ? tuples : [...tuples].sort((x, y) => at(x.modes) - at(y.modes));
      // surface.card's dark side moves once app.tamber does (an event's
      // `files`), so a change to it has cells that moved.
      if (request.method === 'resolve' && request.token === 'surface.card' && FILES['app.tamber'] !== 'a'.repeat(64)) {
        const card = DIALS.resolve['surface.card'];
        return send({ ok: { ...card, files: FILES, tuples: order(card.tuples).map((t: Record<string, unknown>) => ({ ...t, dark: `#${FILES['app.tamber'].slice(0, 6)}ff` })) } });
      }
      if (request.method === 'resolve') return DIALS.resolve[request.token] ? send({ ok: { ...DIALS.resolve[request.token], tuples: order(DIALS.resolve[request.token].tuples), files: FILES } }) : send({ error: { message: `no token \`${request.token}\`` } }, 400);
      if (request.method === 'explain') return DIALS.explain[request.token] ? send({ ok: { ...DIALS.explain[request.token], files: FILES } }) : send({ error: { message: `no token \`${request.token}\`` } }, 400);
      // A write is not applied: canned answers in its shape, enough for the
      // tool's own flow (its edits are tested on a real server).
      // Held: kept for the inbox, as the server keeps it.
      if (request.method === 'propose' && request.hold) {
        const id = `held-${++holds}`;
        held.push({ proposal: id, client: request.client, diff: '', changed: [], ops: request.ops.map((op: unknown, index: number) => ({ index, op })), review: request.review });
        return send({ ok: { proposal: id, client: request.client } });
      }
      if (request.method === 'propose') return send({ ok: { files: FILES, class: classOf(request.ops), changed: request.ops.map((o: { name?: string; from?: string }) => ({ file: 'app.tamber', token: o.name ?? o.from })), diff: `--- app.tamber\n+++ app.tamber\n+  ${JSON.stringify(request.ops[0])}\n` } });
      if (request.method === 'transaction') return send({ ok: { files: FILES, written: ['app.tamber'], client: request.client, inverse: `inv-stub-${++writes}`, changed: [], class: 'live', diff: '' } });
      if (request.method === 'commit') return refuseCommit
        ? send({ error: { message: `commit refused: app.tamber: ${refuseCommit}`, refused: [{ file: 'app.tamber', why: refuseCommit }] } }, 409)
        : send({ ok: { ...(request.dry ? { scope: 'stub-scope' } : { commit: 'c0ffee0123456789abcdef0123456789abcdef01' }), files: ['app.tamber'], edits: [{ file: 'app.tamber', client: 'agent:codex', tokens: ['surface.card'] }] } });
      // Two pages: the second, past `skip`, one older commit.
      if (request.method === 'history' && request.skip) return send({ ok: { more: false, commits: [{ commit: 'feed0123456789abcdef0123456789abcdef0123', parent: null, author: 'Grace', email: 'grace@example.test', date: '2026-10-01T12:00:00Z', message: 'design: the first', files: [] }] } });
      if (request.method === 'history') return send({ ok: { more: true, commits: [{ commit: 'beef0123456789abcdef0123456789abcdef0123', parent: null, author: 'Ada', email: 'ada@example.test', date: '2026-10-15T12:00:00Z', message: 'design: a warmer card\n\n- design-tool:Ada', files: [
        { file: 'app.tamber', status: 'M', repoPath: 'app.tamber', edits: [{ what: 'token', name: 'surface.card', when: [], before: 'light-dark(#f1f3f5, #16181c)', after: 'light-dark(#f3f1ef, #16181c)' }, { what: 'dial', name: 'hue', when: [], before: '200', after: '210' }] },
      ] }] } });
      if (request.method === 'revert') return send({ ok: { proposal: 'prop-stub-1', client: request.client, diff: '', changed: [], class: 'live', ops: [] } });
      if (request.method === 'accept') {
        const p = held.find(h => h.proposal === request.proposal);
        if (!p) return send({ error: { message: `no held proposal \`${request.proposal}\`` } }, 404);
        held = held.filter(h => h !== p);
        // Some of it: held again as a new proposal of those items.
        if (request.items) {
          // Its own diff and tokens: the items kept, as the server plans them.
          const kept = (p.ops as { index: number; op: { name?: string } }[]).filter(o => request.items.includes(o.index));
          // Its review renumbered to the items kept, as the server's is.
          const review = p.review as { tokens?: number; offers?: { items?: number[] }[] } | undefined;
          const at = (i: number) => kept.findIndex(o => o.index === i);
          const remapped = review && { ...review, tokens: kept.filter(o => o.index < (review.tokens ?? 0)).length, offers: (review.offers ?? []).map(o => ({ ...o, items: (o.items ?? []).map(at).filter(i => i >= 0) })) };
          const fresh = { ...p, proposal: `${p.proposal}-r`, replaces: p.proposal, ops: kept.map((o, index) => ({ index, op: o.op })), ...(remapped ? { review: remapped } : {}), diff: `--- app.tamber\n+++ app.tamber\n${kept.map(o => `+  ${o.op.name} (kept)`).join('\n')}\n`, changed: kept.map(o => ({ file: 'app.tamber', token: o.op.name })) };
          held.push(fresh);
          return send({ ok: { recomputed: true, replaces: p.proposal, proposal: fresh.proposal } });
        }
        return send({ ok: { files: FILES, written: ['app.tamber'], client: p.client, inverse: `inv-stub-${++writes}`, changed: [], class: 'live', diff: '' } });
      }
      if (request.method === 'drop') { held = held.filter(h => h.proposal !== request.proposal); return send({ ok: { dropped: request.proposal } }); }
      if (request.method === 'undo') return send({ ok: { files: FILES, written: ['app.tamber'], client: request.client, inverse: null, changed: [], class: 'live', diff: '' } });
      return send({ error: { message: `the stub answers no ${request.method}` } }, 400);
    },
  },
  // Never held: the stub answers at once, so `clock data` lands it.
  '/__tamber/events': {
    OPTIONS: preflight,
    POST: async (req) => {
      if (!authorized(req)) return send({ message: 'Not found.' }, 404);
      const { after = 0, epoch, client } = await req.json().catch(() => ({}));
      if (typeof client === 'string') present.add(client);
      const missed = (epoch !== undefined && epoch !== EPOCH) || miss;
      miss = false;
      return send({ events: missed ? [] : log.filter(e => e.seq > after), seq: seq(), epoch: EPOCH, missed, files: FILES, proposals: held });
    },
  },
  // A new run of the server: a new epoch, its numbers from 0 again.
  '/__stub/restart': {
    POST: (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      EPOCH = `stub-epoch-${++restarts}`;
      TOKEN = `stub-token-${restarts}`;
      log.length = 0;
      held = [];
      holds = 0;
      BUILDS = [];
      reversed = false;
      refuseDescribe = '';
      pins = 0;
      present.clear();
      scrubs.lost();
      scrubs = sessions();
      LIVE = false;
      save();
      return send({ ok: true });
    },
  },
  '/__stub/hold': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      held.push(await req.json().catch(() => ({})));
      return send({ ok: true });
    },
  },
  '/__stub/page': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      PAGE = (await req.json().catch(() => ({}))).page ?? null;
      return send({ ok: true });
    },
  },
  '/__stub/live': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      LIVE = (await req.json().catch(() => ({}))).live === true;
      return send({ ok: true });
    },
  },
  '/__stub/refuse-release': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      refuseRelease = String((await req.json().catch(() => ({}))).by ?? '');
      return send({ ok: true });
    },
  },
  // The server ending the scrub open: a competing write, or the server lost.
  '/__stub/end-scrub': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      const reason = String((await req.json().catch(() => ({}))).reason ?? 'competing write');
      if (reason === 'server lost') scrubs.lost(); else scrubs.compete();
      return send({ ok: true });
    },
  },
  '/__stub/slow-preview': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      previewMs = Number((await req.json().catch(() => ({}))).ms ?? 0);
      return send({ ok: true });
    },
  },
  '/__stub/reverse-axes': {
    POST: (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      reversed = true;
      return send({ ok: true });
    },
  },
  '/__stub/miss': {
    POST: (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      miss = true;
      return send({ ok: true });
    },
  },
  '/__stub/refuse-commit': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      refuseCommit = String((await req.json().catch(() => ({}))).why ?? '');
      return send({ ok: true });
    },
  },
  '/__stub/event': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      const body = await req.json().catch(() => ({}));
      // `refuseDescribe`: describe refused from then on (after `pass` more
      // answered), with the events, so a test sees whether they read again.
      if (typeof body.refuseDescribe === 'string') { refuseDescribe = body.refuseDescribe; describePasses = Number(body.pass ?? 0); }
      for (const e of body.events ?? [body]) take(e);
      save();
      return send({ ok: true });
    },
  },
} });
console.log(`tamber-stub: ${API}`);
