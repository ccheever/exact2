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
import { allows, needs, refusal } from '../../../scripts/tamber-pairing.mjs';
import DIALS from './dials.json';

const DB = process.env.DB ?? '.exact/tamber-stub.json';
// A run's token: a restart issues another, as the server does.
let TOKEN = 'stub-token';
let EPOCH = 'stub-epoch', restarts = 0, writes = 0;
// The next commit's answer: refused, naming why, when a test asks.
let refuseCommit = '';
// A session with nothing to commit (`/__stub/refuse-commit` `{nothing: true}`).
let nothingToCommit = false;
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
// A state part's edit (`set-state`) rebuilds, as the server says.
// Why the server would refuse a `set-row` (LLP 1121 stage 9 D2), for the tests that ask.
const rowRefusal = (o: Record<string, unknown>) => o.op !== 'set-row' ? '' : o.sha256 === 'stale' ? `card.contract changed since it was compiled (sha256): select the view again` : o.value === 'not-a-colour' ? 'the app does not build with that: card.contract:3:9: `not-a-colour` is not a colour' : '';
// A move the server refuses (T5): a new param a component has already ("taken"), or a detach of
// Card.fill that leaves Card.fade unread, said by its diagnostic, as the compile says it.
const moveRefusal = (ops: Record<string, unknown>[]) => {
  if (ops.some(o => o.op === 'add-param' && o.name === 'taken')) return 'a param `taken` exists on `Card` already: give it another name';
  const gone = ops.filter(o => o.op === 'remove-param' && o.component === 'Card').map(o => o.name);
  // (Card.fade is read by another row too: removed alone, that row names no param, as the compile says it.)
  if (gone.includes('fade') && !gone.includes('fill')) return { message: 'refused', diagnostics: [{ file: 'app.contract', line: 14, id: 'tamber-param-unknown', message: '`param.fade`: `Card` declares no param `fade`' }] };
  return gone.includes('fill') && !gone.includes('fade') ? { message: 'refused', diagnostics: [{ file: 'app.contract', line: 12, id: 'tamber-param-unused', message: 'param `app.contract#Card.fade` is read by no row of `Card` (`param.fade`)' }] } : null;
};
const classOf = (ops: { name?: string; op?: string }[]) => ops.some(o => o.op === 'set-state' || o.op === 'set-row' || o.op === 'add-param' || o.op === 'remove-param' || REBUILDS.has(o.name ?? '') || (INLINE.has(o.name ?? '') && BUILDS.some(b => b.host === 'web-js'))) ? 'rebuild' : 'live';
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
  // (A paired client ended has no write left, a release waiting on its preview included.)
  allowed: (client: string) => !client.startsWith('remote:') || paired.some(p => p.client === client && Date.parse(p.expires) > Date.now()),
});
let scrubs = sessions();
const answered = (r: { status?: number; ok?: unknown; error?: unknown }) => r.ok ? send({ ok: r.ok }) : send({ error: r.error }, r.status ?? 409);
// The next poll says it missed events (more than the server keeps).
let miss = false;
// Held proposals, as a test holds them (`/__stub/hold`).
let held: Record<string, unknown>[] = [], holds = 0;
// Live copies (LLP 1122 T7), as a test turns them on (`/__stub/copies`):
// hello's page is this stub's own origin, whose root is a copy's page: it
// registers a build named by its frame (`copy-<name>`, a stable id, as a
// real page keeps its id under its frame's name), after the delay a test
// gives that name, and posts that id to the page framing it. A copy of a
// name in `noNavigate` can't navigate. Each navigate is kept, in order.
let copyDelays: Record<string, number> = {}, noNavigate: string[] = [], refuseLater: string[] = [];
// Copies whose pages are the variant gallery's (`page: 'gallery'`, no `navigate`), as a test asks.
let galleryCopies: string[] = [];
// Stage 12 D3's describe of library components (LLP 1139 D5), as a test turns it on (`/__stub/library`): per
// identity, its `parts` and `variants`, on describe's component entry (added when the app has none by that name).
let LIBRARY: Record<string, { name: string; parts: unknown[]; variants: unknown[] }> = {};
// A package's component with params on describe, and the app's own Contract sources in its files (a server lists
// them since 1.11), as a test turns them on (`/__stub/package-component`).
let PACKAGED: { components: Record<string, unknown>[]; files: Record<string, string>; tokens?: Record<string, string>; base?: Record<string, unknown>[]; resolve?: Record<string, unknown[]>; refuse?: string[] } | null = null, filesBefore: Record<string, string> | null = null;
const navs: { client: string; build: string; location: string }[] = [];
const COPY_PAGE = `<!doctype html><title>copy</title><p id="at"></p><script>(async () => {
  const r = await fetch('/__stub/copy', { method: 'POST', body: JSON.stringify({ name: window.name, location: location.pathname + location.search }) });
  const { id } = await r.json().catch(() => ({}));
  if (id && parent !== window) parent.postMessage({ type: 'exact-build', id }, '*');
  document.getElementById('at').textContent = id ?? 'no copy';
})();</script>`;
/** A copy's build as the server would list it: its facts its pin's (a page reports what it paints). */
const copyBuild = (id: string, name: string, location = '/') => ({ id, host: 'web', ...(galleryCopies.includes(name) ? { page: 'gallery' } : {}), capabilities: ['pin', 'inspect', 'hold', ...(noNavigate.includes(name) || galleryCopies.includes(name) ? [] : ['navigate'])], plan: 'g1', planDigest: 'c'.repeat(64), design: 'd1',
  status: { scheme: 'light', contrast: 'no-preference', mode: {}, rootFontSize: 16, revision: 0, location }, pin: null, pinRevision: 0, hold: null, held: null, unsupported: null });
const copyFacts = (b: any, pin: any) => ({ ...b.status, scheme: pin?.scheme ?? 'light', mode: pin?.mode ?? {}, rootFontSize: pin?.rootFontSize ?? 16 });
// What a page reports held when a refresh asks it again (`/__stub/held-answer`).
let heldAnswer: Record<string, unknown> | null = null;
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
// The test driver, as `request … as ada` sends it; no page can.
const driver = (req: Request) => req.headers.get('authorization') === 'Bearer ada';
const seq = () => log.at(-1)?.seq ?? 0;

// Pairing (LLP 1121 stage 10 A2, tamber/1.13), as the server answers it: the local token mints a
// code (`pair-code`), lists the clients (`clients`) and revokes one (`revoke`); a code presented at
// `/__tamber/pair` gives a client its own token, random each time and kept in memory only, at the
// code's scope, which A1's table checks on every request (scripts/tamber-pairing.mjs). A code is
// the next of a fixed series, so a test can type it. The remote doors a test opens
// (`/__stub/remote`) come with each code, as `{origin, fingerprint}`.
type Paired = { client: string; name: string; scope: string; peer: string; created: string; expires: string; lastSeen: string; token: string };
let paired: Paired[] = [], gone = new Set<string>(), refusePairing = new Set<string>(), canvasAt = '', forgeLocal = false, canvasCheck: unknown = null, shallowSelect = false, refuseSelect = false, pageRefusesSelect = false, codes = new Map<string, { scope: string; name: string; expires: number }>(), remote: unknown[] = [], minted = 0;
const GONE = 'this paired client was revoked or its token expired';
const tokenIn = (req: Request) => req.headers.get('x-exact-tamber-token') ?? '';
// A code as the server reads it: case, spaces, hyphens, I, L and O forgiven.
const normal = (c: unknown) => String(c ?? '').toUpperCase().replace(/[\s-]/g, '').replace(/[IL]/g, '1').replace(/O/g, '0');
/** Who a request is: the local client, a paired one (its token past its expiry ends it, as revoked), or no one (null). */
function who(req: Request) {
  // (Every token past its expiry ends, whoever asks, as the server's sweep ends it.)
  for (const p of paired.filter(c => Date.parse(c.expires) <= Date.now())) revoke(p.client, 'expired');
  if (tokenIn(req) === TOKEN) return { local: true, scope: 'write', client: null as string | null };
  const p = paired.find(c => c.token === tokenIn(req));
  if (p) p.lastSeen = new Date().toISOString();
  return p ? { local: false, scope: p.scope, client: p.client } : null;
}
/** A request no live token sent: told it was revoked if its token was, else not found. */
const nobody = (req: Request) => gone.has(tokenIn(req)) ? send({ error: { message: GONE } }, 401) : send({ message: 'Not found.' }, 404);
/** A code, as `pair-code` mints one (the next of the series). */
function mint(request: { scope?: unknown; name?: unknown }) {
  if (!['read', 'propose', 'write'].includes(String(request.scope))) return send({ error: { message: 'pair-code names its `scope`: read, propose or write' } }, 400);
  const n = ++minted, code = `7Q2KD-M4X${String(n % 100).padStart(2, '0')}`, expires = Date.now() + 120_000;
  codes.set(normal(code), { scope: String(request.scope), name: typeof request.name === 'string' ? request.name : '', expires });
  return send({ ok: { code, scope: request.scope, expires: new Date(expires).toISOString(), ...(remote.length ? { remote } : {}) } });
}
/** A paired client ended (revoked, or its token expired), as the server ends one: its token refused
 * from then on (401), its presence, inspect mode, pins and holds handed back, its held proposals kept
 * marked `revoked`, and a `revoked` event. */
function revoke(client: string, reason = 'revoked') {
  const p = paired.find(c => c.client === client);
  if (!p) return false;
  paired = paired.filter(c => c !== p);
  gone.add(p.token);
  present.delete(client);
  scrubs.absent((c: string) => present.has(c));
  held = held.map(h => h.client === client ? { ...h, revoked: true } : h);
  const inspected = BUILDS.filter(b => b.inspector === client).map(b => b.id);
  if (BUILDS.some(b => b.inspector === client || b.pinBy === client || b.holdBy === client)) {
    BUILDS = BUILDS.map(b => ({ ...b, ...(b.inspector === client ? { inspector: null } : {}), ...(b.pinBy === client ? { pin: null, pinBy: null } : {}), ...(b.holdBy === client ? { hold: null, holdBy: null } : {}) }));
    if (inspected.length) log.push({ event: 'inspect', epoch: EPOCH, client, on: false, builds: inspected, to: null, seq: seq() + 1 });
    log.push({ event: 'builds', epoch: EPOCH, builds: BUILDS, seq: seq() + 1 });
  }
  take({ event: 'revoked', client, reason });
  save();
  return true;
}

const { hostname, port } = new URL(API);
Bun.serve({ hostname, port: Number(port), routes: {
  '/health': () => new Response('ok'),
  '/__tamber/token': { GET: () => send({ token: TOKEN }) },
  '/__tamber': {
    OPTIONS: preflight,
    POST: async (req) => {
      const asker = who(req);
      if (!asker) return nobody(req);
      const request = await req.json().catch(() => ({}));
      // Revoked or expired while its body came: refused before anything it asks runs.
      if (asker.client && !who(req)) return nobody(req);
      // A paired client is the client its token names; its scope is checked first.
      if (asker.client) request.client = asker.client;
      const need = needs(request, (id: string) => (held.find(h => h.proposal === id)?.client as string | undefined) ?? null);
      if (need === null) return send({ error: { message: `\`${request.method}\` has no scope on this wire` } }, 400);
      if (!allows(asker.scope, need, asker.local)) { const r = refusal(request.method, asker.scope, need); return send({ error: r.error }, r.status); }
      // (A server that lies to a paired client, saying it is local with a page, as a test asks: `/__stub/forge-local`.)
      if (request.method === 'hello') return send({ ok: { protocol: 'tamber/1.13', format: 1, builds: BUILDS, app: '/stub/theme-fixture', files: FILES, epoch: EPOCH, design: 'app.tamber', page: asker.local || forgeLocal ? PAGE ?? (forgeLocal ? 'http://127.0.0.1:5173' : null) : null, live: LIVE, scope: asker.scope, local: asker.local || forgeLocal, ...(asker.client ? { client: asker.client } : {}) } });
      // (Refused once each, as a test asks: `/__stub/refuse-pairing`.)
      if ((request.method === 'pair-code' || request.method === 'clients') && refusePairing.delete(request.method)) return send({ error: { message: `${request.method}: refused, as the test asked` } }, 500);
      if (request.method === 'pair-code') return mint(request);
      // The gateway's grant (stage 10 C), for a paired client; its URL this stub's own, which has no gateway page.
      // (A test names the canvas origin a gateway would have, `/__stub/canvas`; by default the stub's host over
      // https, another port, which nothing serves: the frame is there, entered by nothing.)
      if (request.method === 'canvas') return asker.client ? send({ ok: { url: `${canvasAt || `https://${new URL(API).hostname}:${Number(new URL(API).port) + 1}`}/canvas/${encodeURIComponent(asker.client)}/`, grant: Buffer.from(crypto.getRandomValues(new Uint8Array(32))).toString('base64url'), expires: Date.now() + 30_000 } }) : send({ error: { message: '`canvas` is a paired client\'s: the local tool frames the dev loop itself' } }, 400);
      if (request.method === 'clients') return send({ ok: { clients: [{ client: 'local', local: true, scope: 'write' }, ...paired.map(({ token: _, ...c }) => c)] } });
      if (request.method === 'revoke') return revoke(request.client) ? send({ ok: { revoked: request.client } }) : send({ error: { message: `no paired client \`${request.client}\`` } }, 404);
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
      // A hold (stage 8 E): the server's record on the build, announced in a
      // builds event; the build's own `held` a test adds as an event.
      if (request.method === 'hold') {
        const b = BUILDS.find(x => x.id === request.build);
        if (!b || !(b.capabilities ?? []).includes('hold')) return send({ error: { message: `hold: no build \`${request.build}\` that holds states` } }, 409);
        BUILDS = BUILDS.map(x => x.id === request.build ? { ...x, hold: request.state ? { site: request.site, instance: request.instance ?? [], state: request.state } : null, held: null, holdBy: request.client } : x);
        log.push({ event: 'builds', epoch: EPOCH, builds: BUILDS, seq: seq() + 1 });
        save();
        return send({ ok: { build: request.build } });
      }
      // A navigation (LLP 1122 T7): one build that can, moved to the location, as its page then reports.
      if (request.method === 'navigate') {
        if (!present.has(request.client)) return send({ error: { message: `navigate: \`${request.client}\` is not present` } }, 409);
        const b = BUILDS.find(x => x.id === request.build);
        if (!b) return send({ error: { message: `no registered build \`${request.build}\`` } }, 400);
        if (!(b.capabilities ?? []).includes('navigate')) return send({ error: { message: `build \`${b.id}\` (web) cannot navigate`, unsupported: 'navigate' } }, 409);
        navs.push({ client: request.client, build: b.id, location: request.location });
        // (A page that takes the command and then can't route there: its refusal comes later, as an event to the sender alone.)
        if (refuseLater.includes(`exact-copy-${String(b.id).slice('copy-'.length)}`)) {
          log.push({ event: 'unsupported', epoch: EPOCH, build: b.id, what: 'navigate', detail: `${JSON.stringify(request.location)} is no route of this app`, only: request.client, seq: seq() + 1 });
          save();
          return send({ ok: { navigated: true, build: b.id, seq: navs.length } });
        }
        BUILDS = BUILDS.map(x => x.id === b.id ? { ...x, status: { ...(x.status ?? {}), location: request.location } } : x);
        log.push({ event: 'builds', epoch: EPOCH, builds: BUILDS, seq: seq() + 1 });
        save();
        return send({ ok: { navigated: true, build: b.id, seq: navs.length } });
      }
      if (request.method === 'inspect') {
        // Below write a refresh reads the selection each build last reported, asking no build again
        // and moving no inspect mode (ruling 5), as the server answers it.
        if (request.refresh === true && asker.scope !== 'write') return send({ ok: { selections: BUILDS.map(b => log.findLast(e => e.event === 'selection' && e.build === b.id)).filter(Boolean) } });
        if (!present.has(request.client)) return send({ error: { message: `inspect: \`${request.client}\` is not present` } }, 409);
        // `select: {up}` (tamber/1.16): the build's page selects that ancestor, reported as a click would be: the last
        // selection again, its source the call site `up` above and its chain what is left above that.
        if (request.select) {
          if (refuseSelect) return send({ error: { message: 'inspect is not on for this client on `web-1`: turn it on first' } }, 409);
          // (`/__stub/page-refuses-select`: sent, and the page answers it can't, as Apple's do.)
          if (pageRefusesSelect) { log.push({ event: 'unsupported', epoch: EPOCH, build: request.build, what: 'inspect', detail: 'this build cannot move its selection up (select): click the node instead', seq: seq() + 1 }); save(); return send({ ok: { builds: [request.build] } }); }
          const last = log.findLast(e => e.event === 'selection' && e.build === request.build) as { node?: number; source?: { chain?: unknown[] } } | undefined;
          // (`/__stub/shallow-select`: a page whose tree changed has fewer above, and lands at the root.)
          const chain = last?.source?.chain ?? [], up = shallowSelect ? chain.length : Math.min(Number(request.select.up), chain.length);
          if (!last || !up) return send({ error: { message: `\`${request.build}\` has no selection with a source map to move up from: select a node first` } }, 409);
          // (Another node, as the page's ancestor is: its own view id.)
          log.push({ ...last, node: Number(last.node ?? 0) + 100 * up, source: { ...(chain[up - 1] as object), chain: chain.slice(up) }, seq: seq() + 1 }); save();
          return send({ ok: { builds: [request.build] } });
        }
        // A refresh asks a build's page to report again, inspect mode as it is:
        // its last selection again, with the held look a test set (`/__stub/held-answer`).
        if (request.refresh && !('on' in request)) {
          const last = log.findLast(e => e.event === 'selection' && e.build === request.build);
          if (last && heldAnswer) { log.push({ ...last, held: heldAnswer, seq: seq() + 1 }); save(); }
          return send({ ok: { builds: [request.build] } });
        }
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
        BUILDS = BUILDS.map(b => ids.includes(b.id) ? { ...b, pin: request.method === 'preview' ? { mode: { [request.axis]: request.value } } : request.pin, pinRevision: pins, pinBy: request.client, ...(String(b.id).startsWith('copy-') ? { status: { ...copyFacts(b, request.pin), revision: pins } } : {}) } : b);
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
      if (request.method === 'describe') { const ir: Record<string, any> = { ...DIALS.describe.ir, ...(reversed ? { axes: [...DIALS.describe.ir.axes].reverse() } : {}) };
        if (PACKAGED) ir.components = [...ir.components, ...PACKAGED.components];
        // (A token a package's design defines: its describe row says the package.)
        if (PACKAGED?.tokens) ir.tokens = ir.tokens.map((t: { name: string }) => (PACKAGED!.tokens![t.name] ? { ...t, package: PACKAGED!.tokens![t.name] } : t));
        if (Object.keys(LIBRARY).length) { const known = ir.components.map((c: { identity: string }) => c.identity); ir.components = [...ir.components.map((c: { identity: string }) => (LIBRARY[c.identity] ? { ...c, ...LIBRARY[c.identity] } : c)), ...Object.entries(LIBRARY).filter(([id]) => !known.includes(id)).map(([identity, l]) => ({ identity, params: [], ...l }))]; } return send({ ok: { ...DIALS.describe, ir, files: FILES, ...(PACKAGED?.base ? { base: PACKAGED.base } : {}) } }); }
      // Reordered axes put each tuple at its index in the new order.
      const at = (modes: Record<string, string>) => [...DIALS.describe.ir.axes].reverse().reduce((n: number, a: { name: string; values: string[] }) => n * a.values.length + a.values.indexOf(modes[a.name]), 0);
      const order = (tuples: { modes: Record<string, string> }[]) => !reversed ? tuples : [...tuples].sort((x, y) => at(x.modes) - at(y.modes));
      // A package param's resolve, its tuples' winning level and name (stage 12 B2-levels), as the test gives them.
      // (Each of the design's tuples, by its modes as a token's resolve gives them, its level the test's in turn: one given
      // as a string is both sides', as `{light, dark}` each side's.)
      const sides = (g: Record<string, unknown>) => Object.fromEntries(Object.entries(g).map(([k, v]) => [k, (k === 'level' || k === 'name') && typeof v === 'string' ? { light: v, dark: v } : v]));
      const packagedParam = PACKAGED?.components.some(c => ((c.params ?? []) as { name: string }[]).some(p => p.name === request.token));
      if (request.method === 'resolve' && packagedParam && PACKAGED!.refuse?.includes(request.token)) return send({ error: { message: `the server could not resolve ${request.token}` } }, 400);
      if (request.method === 'resolve' && packagedParam) { const given = (PACKAGED!.resolve?.[request.token] ?? [{ level: 'default', name: request.token }]) as Record<string, unknown>[]; return send({ ok: { token: request.token, files: FILES, tuples: order(DIALS.resolve['space.gap'].tuples.map((u: Record<string, unknown>, i: number) => ({ modes: u.modes, ...sides(given[i % given.length]) }))) } }); }
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
      // A `set-row` the server refuses (T5), as a test names it: a file moved since the selection
      // (`sha256` "stale"), or a value the app does not build with ("not-a-colour").
      const refusedRow = ['propose', 'transaction'].includes(request.method) && ((request.ops ?? []).map(rowRefusal).find(Boolean) || moveRefusal(request.ops ?? []));
      if (refusedRow) return send({ error: typeof refusedRow === 'string' ? { message: refusedRow } : refusedRow }, 409);
      if (request.method === 'propose') return send({ ok: { files: FILES, class: classOf(request.ops), changed: request.ops.filter((o: { name?: string; from?: string }) => o.name ?? o.from).map((o: { name?: string; from?: string }) => ({ file: 'app.tamber', token: o.name ?? o.from })), diff: `--- app.tamber\n+++ app.tamber\n+  ${JSON.stringify(request.ops[0])}\n` } });
      if (request.method === 'transaction') return send({ ok: { files: FILES, written: ['app.tamber'], client: request.client, inverse: `inv-stub-${++writes}`, changed: [], class: 'live', diff: '' } });
      if (request.method === 'commit' && nothingToCommit) return send({ error: { message: 'nothing to commit: the session has written nothing' } }, 409);
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
          const review = p.review as { tokens?: number; offers?: { items?: number[] }[]; dials?: { items?: number[] }[]; pins?: { items?: number[] }[] } | undefined;
          const at = (i: number) => kept.findIndex(o => o.index === i);
          const renumber = (all: { items?: number[] }[]) => all.map(o => ({ ...o, items: (o.items ?? []).map(at).filter(i => i >= 0) }));
          // A re-import's dials and pins too, those whose operations it kept (stage 10b).
          const marks = review && Object.fromEntries((['dials', 'pins'] as const).filter(k => review[k]).map(k => [k, renumber(review[k]!).filter(m => m.items.length)]).filter(([, all]) => all.length));
          const remapped = review && { ...review, dials: undefined, pins: undefined, ...marks, tokens: kept.filter(o => o.index < (review.tokens ?? 0)).length, offers: renumber(review.offers ?? []) };
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
  // A code for a client's own token, once; a wrong one is 403, saying nothing of which exist.
  '/__tamber/pair': {
    OPTIONS: preflight,
    POST: async (req) => {
      const { code, name } = await req.json().catch(() => ({}));
      if (typeof code !== 'string' || code.length > 32) return send({ error: { message: '`code` is the pairing code, a string' } }, 400);
      if (name !== undefined && (typeof name !== 'string' || name.length > 40)) return send({ error: { message: '`name` is a string of up to 40 characters' } }, 400);
      const at = codes.get(normal(code));
      if (!at || at.expires <= Date.now()) return send({ error: { message: 'no such pairing code: it may have expired, been used, or been burned by failed attempts' } }, 403);
      codes.delete(normal(code));
      const label = (typeof name === 'string' && name) || at.name || 'client', now = new Date();
      const p: Paired = { client: `remote:${label}#${paired.length + gone.size + 1}`, name: label, scope: at.scope, peer: '127.0.0.1', created: now.toISOString(), expires: new Date(now.getTime() + 12 * 3600_000).toISOString(), lastSeen: now.toISOString(), token: crypto.randomUUID() };
      paired.push(p);
      take({ event: 'paired', client: p.client, scope: p.scope });
      save();
      return send({ ok: { token: p.token, client: p.client, scope: p.scope, expires: p.expires } });
    },
  },
  // The origin the next canvas grants name (a test's: another host refused by the tool).
  '/__stub/canvas': { POST: async (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); canvasAt = (await req.json().catch(() => ({}))).origin ?? ''; return send({ ok: true }); } },
  '/__stub/forge-local': { POST: (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); forgeLocal = true; return send({ ok: true }); } },
  // The next pair-code and clients refused, once each.
  '/__stub/refuse-pairing': { POST: (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); refusePairing = new Set(['pair-code', 'clients']); return send({ ok: true }); } },
  // A code or a revoke as the local tool would ask it, for a test whose tool is the paired one.
  '/__stub/pair-code': { POST: async (req) => (driver(req) ? mint(await req.json().catch(() => ({}))) : send({ message: 'Not found.' }, 404)) },
  '/__stub/revoke': { POST: async (req) => (!driver(req) ? send({ message: 'Not found.' }, 404) : revoke((await req.json().catch(() => ({}))).client) ? send({ ok: true }) : send({ error: { message: 'no such client' } }, 404)) },
  // The remote doors the next codes name (`{remote: [{origin, fingerprint}]}`), as `--remote` opens one.
  '/__stub/remote': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      remote = (await req.json().catch(() => ({}))).remote ?? [];
      return send({ ok: true });
    },
  },
  // Never held: the stub answers at once, so `clock data` lands it.
  '/__tamber/events': {
    OPTIONS: preflight,
    POST: async (req) => {
      const asker = who(req);
      if (!asker) return nobody(req);
      const { after = 0, epoch, client: named } = await req.json().catch(() => ({}));
      if (asker.client && !who(req)) return nobody(req);
      if (!allows(asker.scope, needs({ method: 'events' })!, asker.local)) { const r = refusal('events', asker.scope, 'read'); return send({ error: r.error }, r.status); }
      const client = asker.client ?? named;
      if (typeof client === 'string') present.add(client);
      const missed = (epoch !== undefined && epoch !== EPOCH) || miss;
      miss = false;
      return send({ events: missed ? [] : log.filter(e => e.seq > after), seq: seq(), epoch: EPOCH, missed, files: FILES, proposals: held });
    },
  },
  // A new run of the server: a new epoch, its numbers from 0 again.
  // The tool's canvas check as `tamber tool --via` answers it (scripts/tamber-frame.mjs), as a test sets it
  // (`/__stub/canvas-check`); not found by default, as a tool served without --via.
  '/__tamber/tool/canvas-check': { GET: () => (canvasCheck ? send(canvasCheck) : send({ message: 'Not found.' }, 404)) },
  '/__stub/page-refuses-select': { POST: (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); pageRefusesSelect = true; refuseSelect = false; return send({ ok: true }); } },
  '/__stub/refuse-select': { POST: (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); refuseSelect = true; return send({ ok: true }); } },
  '/__stub/shallow-select': { POST: (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); shallowSelect = true; return send({ ok: true }); } },
  '/__stub/canvas-check': { POST: async (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); canvasCheck = await req.json().catch(() => null); return send({ ok: true }); } },
  '/__stub/restart': {
    POST: (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      EPOCH = `stub-epoch-${++restarts}`;
      TOKEN = `stub-token-${restarts}`;
      log.length = 0;
      held = [];
      holds = 0;
      heldAnswer = null;
      BUILDS = [];
      reversed = false;
      refuseDescribe = '';
      pins = 0;
      present.clear();
      // (A restart revokes every paired client: the tokens were in memory.)
      paired = []; gone.clear(); codes.clear(); remote = []; minted = 0; refusePairing.clear(); canvasAt = ''; forgeLocal = false; canvasCheck = null; shallowSelect = false; refuseSelect = false; pageRefusesSelect = false;
      copyDelays = {}; noNavigate = []; galleryCopies = []; refuseLater = []; navs.length = 0; LIBRARY = {};
      // (The files as they were before the first package post: what any number of posts merged in, gone.)
      if (filesBefore) FILES = filesBefore;
      PACKAGED = null; filesBefore = null;
      scrubs.lost();
      scrubs = sessions();
      LIVE = false;
      save();
      return send({ ok: true });
    },
  },
  // (Its files join every reply's, as the server's own Contract sources are in each.)
  '/__stub/package-component': { POST: async (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); PACKAGED = await req.json(); filesBefore ??= FILES; FILES = { ...FILES, ...PACKAGED!.files }; return send({ ok: true }); } },
  '/__stub/library': { POST: async (req) => { if (!driver(req)) return send({ message: 'Not found.' }, 404); LIBRARY = await req.json(); return send({ ok: true }); } },
  '/__stub/hold': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      held.push(await req.json().catch(() => ({})));
      return send({ ok: true });
    },
  },
  '/__stub/held-answer': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      heldAnswer = (await req.json().catch(() => ({}))).held ?? null;
      return send({ ok: true });
    },
  },
  '/__stub/copies': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      const body = await req.json().catch(() => ({}));
      PAGE = `http://127.0.0.1:${copies.port}`;
      copyDelays = body.delays ?? {};
      noNavigate = body.noNavigate ?? [];
      galleryCopies = body.gallery ?? [];
      refuseLater = body.refuseLater ?? [];
      return send({ ok: true });
    },
  },
  // The person moving one copy (its page reports a new location).
  '/__stub/move': {
    POST: async (req) => {
      if (!driver(req)) return send({ message: 'Not found.' }, 404);
      const { build, location } = await req.json().catch(() => ({}));
      BUILDS = BUILDS.map(x => x.id === build ? { ...x, status: { ...(x.status ?? {}), location } } : x);
      log.push({ event: 'builds', epoch: EPOCH, builds: BUILDS, seq: seq() + 1 });
      save();
      return send({ ok: true });
    },
  },
  '/__stub/navs': { GET: (req) => (driver(req) ? send({ navs }) : send({ message: 'Not found.' }, 404)) },
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
      const body = await req.json().catch(() => ({}));
      refuseCommit = String(body.why ?? '');
      nothingToCommit = body.nothing === true;
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
// The copies' dev loop: its own origin (hello's page is never the tool's
// server's), whose root is a copy's page and which takes its registration.
const copies = Bun.serve({ hostname, port: 0, routes: {
  // (Every path: a copy reloaded at a place is the same page there.)
  '/*': { GET: () => new Response(COPY_PAGE, { headers: { 'content-type': 'text/html' } }) },
  '/__stub/copy': {
    POST: async (req) => {
      const body = await req.json().catch(() => ({})), name = String(body.name ?? ''), at = /^\/[\w/?=&-]*$/.test(String(body.location)) ? String(body.location) : '/';
      if (!/^exact-copy-\d{1,2}$/.test(name)) return send({ id: null });
      await new Promise(ok => setTimeout(ok, copyDelays[name] ?? 0));
      const id = `copy-${name.slice('exact-copy-'.length)}`;
      // (Registered again, a copy keeps the pin the server holds for it, as the server replays it.)
      const was = BUILDS.find(b => b.id === id);
      BUILDS = BUILDS.filter(b => b.id !== id).concat([{ ...copyBuild(id, name, at), ...(was?.pin ? { pin: was.pin, pinRevision: was.pinRevision, status: { ...copyFacts(was, was.pin), revision: was.pinRevision, location: at } } : {}) }]);
      log.push({ event: 'builds', epoch: EPOCH, builds: BUILDS, seq: seq() + 1 });
      save();
      return send({ id });
    },
  },
} });
console.log(`tamber-stub: ${API}`);
