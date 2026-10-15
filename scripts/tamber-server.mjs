// The design server's wire (LLP 1121 D13, stage 3): HTTP to clients, JSON
// lines to `contract tamber serve --stdio`, which owns the files (digests,
// the lock, the journal, every write). This writes no design file.
//
// - `GET /__tamber/token`: the token, minted in memory at start, only to a
//   request the development gate calls local (loopback peer and host, no
//   forwarding header); any other gets the install endpoint's not-found.
// - `POST /__tamber`: one request (`hello`, `describe`, `resolve`, `explain`,
//   `propose`, `transaction`, `undo`), its reply as the server gave it. A
//   write names its `client`: the clients share one server, so a hello's
//   name is no write's.
// - The change stream, an event per change whoever made it: a client's
//   write (its client, the files it wrote), a hand edit or a `git checkout`
//   (seen by the watcher, `client` null). Each event has a sequence number;
//   the server's own writes are known by digest and announced once.
//   - `POST /__tamber/subscribe`: a streamed response, one `data:` line an
//     event.
//   - `POST /__tamber/events` `{"after": n, "epoch": e}`: the events after
//     `n` (the last 256 kept), answered once there is one or after 25 s with
//     none: for a client whose `fetch` reads a body whole, as a JS-target
//     data module's does. A number this run did not give, or one older than
//     the events kept, is `missed`: no events, and the files to start from.
// - The wire's own requests (LLP 1122 D9, D10), beside the server's:
//   - `commit` `{client, message?}`: the files this session wrote, committed
//     to git as scripts/tamber-commit.mjs guards it; never pushed.
//   - `propose` with `hold: true` and a `client`: kept 15 min for review
//     (64 at most) and announced (`proposal`, its operations by index).
//     `accept` `{proposal, client, items?}` applies it against the digests
//     it was planned with; a file moved since, or only some items,
//     recomputes it as a new proposal for a fresh accept. `drop` `{proposal,
//     client}` forgets it. The ones held now come with `subscribed` and
//     each long poll's answer. A `review` object rides along for the tool
//     (an import's offers by item, its unmapped report and count).
//   - A conflict names who last changed each file that moved (`by`: a
//     client, or "changed on disk").
// - Every POST carries the token as `x-exact-tamber-token`, never in a URL.
//
// Mounted by both dev loops (host/web/dev.mjs, host/web-js/dev.mjs) when the
// app has a design; `bun exact.mjs tamber serve <app>` runs it alone and
// `bun exact.mjs tamber op <url> <request>` is its scripted client. Never in a
// release build or a bake. D13 names WebSocket; the dev loops serve
// server-sent events and no WebSocket, so stage 3 streams a POST's response.
import { spawn, spawnSync } from 'node:child_process';
import { randomBytes, timingSafeEqual } from 'node:crypto';
import { existsSync, readFileSync, statSync, watch } from 'node:fs';
import { createServer } from 'node:http';
import { basename, dirname, relative, resolve, sep } from 'node:path';
import { developmentGate, installBrowserOrigins } from './install-page.mjs';
import { designSession } from './tamber-commit.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
// The server's own line limit, less room for the id this adds.
const LINE_LIMIT = 1 << 20;
const BODY_LIMIT = LINE_LIMIT - 64;
const ANSWER_MS = 120_000, POLL_MS = 25_000, KEPT = 256, HOLD_MS = 15 * 60_000, HELD = 64;
const METHODS = new Set(['hello', 'describe', 'resolve', 'explain', 'propose', 'transaction', 'undo', 'commit', 'accept', 'drop']);
const WRITES = new Set(['transaction', 'undo']);

const json = (res, status, body) => {
  res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
  res.end(JSON.stringify(body));
};
const notFound = res => json(res, 404, { message: 'Not found.' });

/** The app's design, as the compiler finds it: the file `app.json`'s
 * `"tamber"` names, else `app.tamber`; null when there is none. */
export function designPath(appDir) {
  let named = null;
  try { named = JSON.parse(readFileSync(resolve(appDir, 'app.json'), 'utf8')).tamber ?? null; } catch { /* no manifest */ }
  const path = resolve(appDir, typeof named === 'string' ? named : 'app.tamber');
  return existsSync(path) ? path : null;
}

/** `contract tamber serve --stdio <app>`, through cargo as the dev loops run
 * the compiler, in a process group of its own so stopping it stops the
 * compiler cargo starts. */
function spawnContract(appDir) {
  return spawn('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'serve', '--stdio', appDir], { cwd: ROOT, stdio: ['pipe', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
}

/** The wire for one app: `handle(req, res, url, access)` answers the
 * `/__tamber` paths (true when it did); `close()` stops the server and
 * resolves once it has let go of the design's lock. */
export function tamberServer({ appDir, spawnServe = spawnContract, log = console.log, settle = 50 }) {
  const token = randomBytes(32).toString('hex');
  // This run's name on every event: a number from another run means nothing.
  const epoch = randomBytes(8).toString('hex');
  const waiting = new Map(), subscribers = new Set(), polls = new Set(), watchers = new Map(), events = [];
  // Each file's digest as last announced: what a later change is told from.
  const known = new Map();
  let next = 0, seq = 0, out = '', stderr = '', down = null, writing = 0, again = false, timer = null, first = true, proposalCount = 0;
  // What `commit` needs of the session, and who last changed each file.
  const session = designSession(appDir), lastBy = new Map();
  const design = basename(designPath(appDir) ?? 'app.tamber');

  const send = event => {
    events.push(event);
    if (events.length > KEPT) events.shift();
    const line = `data: ${JSON.stringify(event)}\n\n`;
    for (const res of subscribers) res.write(line);
    for (const poll of polls) poll();
  };
  /** Announce `files` (path → digest): the paths whose digest moved, and
   * any `gone`. */
  const publish = (files, rest, gone = []) => {
    const edited = Object.entries(files).filter(([path, d]) => known.get(path) !== d).map(([path]) => path);
    for (const [path, d] of Object.entries(files)) known.set(path, d);
    for (const path of gone) known.delete(path);
    edited.push(...gone);
    for (const path of edited) lastBy.set(path, rest.client ?? 'changed on disk');
    if (!rest.client) session.seen(Object.fromEntries(edited.map(path => [path, files[path] ?? ''])));
    if (edited.length || rest.client) send({ event: 'changed', seq: ++seq, epoch, ...rest, files: Object.fromEntries(known), edited });
  };

  // The server gone (never started, the lock another's, exited): every
  // request in flight, every subscriber and every later request told so.
  const fail = message => {
    if (down) return;
    down = message;
    log(`tamber: ${message}`);
    for (const reply of waiting.values()) reply({ error: { message } });
    waiting.clear();
    send({ event: 'down', seq: ++seq, epoch, message });
    for (const res of subscribers) res.end();
    subscribers.clear();
  };
  const child = spawnServe(appDir);
  child.on('error', e => fail(`the design server did not start: ${e.message}`));
  child.on('exit', code => fail(stderr.trim().split('\n').pop() || `the design server exited ${code}`));
  child.stdin.on('error', () => {});
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-4000); });
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', chunk => {
    out += chunk;
    const lines = out.split('\n'); out = lines.pop();
    for (const text of lines) {
      if (!text.trim()) continue;
      let v; try { v = JSON.parse(text); } catch { continue; }
      // A write's event is published from its reply (below), in order.
      if (v.event) continue;
      // The server answers in order: a reply it could not tie to a request
      // (a line it could not read) is the oldest one's.
      const id = v.id ?? waiting.keys().next().value;
      const reply = waiting.get(id);
      waiting.delete(id);
      reply?.(v);
    }
  });

  /** One request's reply. `done`, when given, sees the server's own reply
   * even after the client was told it timed out (`wait` ms; none for the
   * first read, which may wait on a cold build of the compiler). */
  const ask = (request, { wait = ANSWER_MS, done = () => {} } = {}) => new Promise(reply => {
    if (down) { done(null); return reply({ error: { message: down } }); }
    const line = JSON.stringify({ ...request, id: ++next });
    if (Buffer.byteLength(line) >= LINE_LIMIT) { done(null); return reply({ status: 413, error: { message: 'a request is at most 1 MiB' } }); }
    const id = next;
    const late = wait && setTimeout(() => reply({ status: 504, error: { message: 'the design server did not answer' } }), wait);
    waiting.set(id, v => { clearTimeout(late); done(v); reply(v); });
    child.stdin.write(`${line}\n`);
  });

  // The design's files as the server has them now, announced where a
  // digest moved, and every directory holding one watched. A file that
  // appears (a new design, a checkout) is found by asking again.
  // Taken as the server answers, in order with its writes' replies, so an
  // older read never lands after a newer write.
  const read = reply => {
    // A failed read is said, and read again at the next change; until the
    // first succeeds, every 2 s too.
    if (!reply?.ok) {
      log(`tamber: ${reply?.error?.message ?? 'no answer'}`);
      if (first && !down) setTimeout(rescan, 2000).unref?.();
      return;
    }
    // The first read is the baseline, not a change.
    // The first read is the baseline only while nothing has been announced.
    if (first && known.size === 0) { first = false; for (const [path, d] of Object.entries(reply.ok.files)) known.set(path, d); }
    else { first = false; publish(reply.ok.files, { changed: null, client: null }, [...known.keys()].filter(path => !(path in reply.ok.files))); }
    watchAll(Object.keys(reply.ok.files));
  };
  const refresh = () => ask({ method: 'hello' }, { wait: first ? 0 : ANSWER_MS, done: read });
  // The app's directory (where a new design beside it appears) and the
  // design's, whatever the first read gave, and any other holding a file.
  // A directory removed and made again is watched again (by its inode).
  const watchAll = paths => {
    for (const dir of new Set([appDir, dirname(designPath(appDir) ?? resolve(appDir, design)), ...paths.map(path => dirname(resolve(appDir, path)))])) {
      let ino = null;
      try { ino = statSync(dir).ino; } catch { /* not there now */ }
      const held = watchers.get(dir);
      if (held && held.ino === ino) continue;
      held?.watcher.close();
      watchers.delete(dir);
      if (ino === null) continue;
      try {
        watchers.set(dir, { ino, watcher: watch(dir, (_, name) => {
          // A design file, or one that may be: `.tamber`, the design's own
          // name (app.json may name any), a directory a design is in, or a
          // name the platform did not give.
          const n = name && basename(String(name));
          if (n && !n.endsWith('.tamber') && n !== design && !holders.has(n)) return;
          clearTimeout(timer); timer = setTimeout(rescan, settle);
        }) });
      } catch (e) { log(`tamber: cannot watch ${dir}: ${e.message}`); }
    }
  };
  // The directories between the app's and each design file's, by name.
  const holders = new Set(relative(appDir, dirname(designPath(appDir) ?? resolve(appDir, design))).split(sep).filter(Boolean));
  // While a write is in flight its own renames are not a hand edit.
  const rescan = () => {
    timer = null;
    if (writing) { again = true; return; }
    void refresh();
  };
  watchAll([]);
  const ready = refresh();

  const tokenOf = req => {
    const given = Buffer.from(String(req.headers['x-exact-tamber-token'] ?? ''));
    const mine = Buffer.from(token);
    return given.length === mine.length && timingSafeEqual(given, mine);
  };
  /** The body as text; null when over the limit (the connection is cut)
   * or when the client left. */
  const body = req => new Promise(done => {
    if (req.destroyed) return done(null);
    if (Number(req.headers['content-length']) > BODY_LIMIT) { req.destroy?.(); return done(null); }
    const chunks = []; let size = 0, settled = false;
    const finish = v => { if (!settled) { settled = true; done(v); } };
    req.on('data', chunk => {
      size += chunk.length;
      if (size > BODY_LIMIT) { finish(null); req.destroy?.(); } else chunks.push(chunk);
    });
    req.on('end', () => finish(Buffer.concat(chunks).toString('utf8')));
    req.on('error', () => finish(null));
    req.on('close', () => finish(null));
  });
  const parse = text => {
    try { const v = JSON.parse(text); return v && typeof v === 'object' && !Array.isArray(v) ? v : null; } catch { return null; }
  };

  async function handle(req, res, url, access) {
    if (url.pathname === '/__tamber/token') {
      if (req.method !== 'GET' || !access.local) notFound(res);
      else json(res, 200, { token });
      return true;
    }
    if (!['/__tamber', '/__tamber/subscribe', '/__tamber/events'].includes(url.pathname)) return false;
    if (req.method !== 'POST' || !tokenOf(req)) { notFound(res); return true; }
    await ready;
    // The client left while the server started.
    if (res.destroyed || res.writableEnded) return true;
    if (url.pathname === '/__tamber/subscribe') {
      if (down) { json(res, 503, { error: { message: down } }); return true; }
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
      res.write(`data: ${JSON.stringify({ event: 'subscribed', seq, epoch, files: Object.fromEntries(known), proposals: held() })}\n\n`);
      subscribers.add(res);
      res.on('close', () => subscribers.delete(res));
      return true;
    }
    const text = await body(req);
    if (text == null) { json(res, 413, { error: { message: 'a request is at most 1 MiB' } }); return true; }
    const request = parse(text);
    if (url.pathname === '/__tamber/events') {
      const after = request?.after ?? seq;
      if (!Number.isSafeInteger(after) || after < 0) { json(res, 400, { error: { message: '`after` is a sequence number' } }); return true; }
      const answer = () => {
        const since = events.filter(e => e.seq > after);
        // Fewer kept than asked for, or a number from before this server
        // started: the client missed some, and reads again.
        const missed = (request?.epoch !== undefined && request.epoch !== epoch) || after > seq || after < seq && (events[0]?.seq ?? seq + 1) > after + 1;
        // Missed: no events to apply, the files to start again from.
        json(res, 200, { events: missed ? [] : since, seq, epoch, missed, files: Object.fromEntries(known), proposals: held() });
      };
      if (events.some(e => e.seq > after) || after > seq || (request?.epoch !== undefined && request.epoch !== epoch) || down) { answer(); return true; }
      await new Promise(ok => {
        const poll = () => { polls.delete(poll); clearTimeout(wait); ok(); };
        const wait = setTimeout(poll, POLL_MS);
        polls.add(poll);
        res.on('close', poll);
      });
      if (!res.writableEnded && !res.destroyed) answer();
      return true;
    }
    if (!request || !METHODS.has(request.method)) {
      json(res, 400, { error: { message: `a request is an object whose method is one of ${[...METHODS].join(', ')}` } });
      return true;
    }
    delete request.id;
    const reply = await dispatch(request);
    const status = reply.ok !== undefined ? 200 : reply.status ?? (down ? 503 : reply.error?.conflict ? 409 : 400);
    json(res, status, reply.ok !== undefined ? { ok: reply.ok } : { error: reply.error });
    return true;
  }

  // What the wire itself reads of a review: the offers' items, the tokens' count.
  const reviewShaped = r => typeof r === 'object' && r !== null && !Array.isArray(r)
    && (r.offers === undefined || Array.isArray(r.offers) && r.offers.every(o => typeof o === 'object' && o !== null && (o.items === undefined || Array.isArray(o.items) && o.items.every(Number.isInteger))))
    && (r.tokens === undefined || Number.isInteger(r.tokens));
  const needsClient = request => typeof request.client === 'string' && request.client ? null : { status: 400, error: { message: `${request.method} names its \`client\`` } };

  /** One request: the server's, or one of the wire's own (`commit`, a held
   * proposal's `accept` and `drop`). */
  async function dispatch(request) {
    if (WRITES.has(request.method)) return needsClient(request) ?? write(request);
    if (request.method === 'commit') {
      const refused = needsClient(request);
      if (refused) return refused;
      if (request.message !== undefined && typeof request.message !== 'string') return { status: 400, error: { message: '`message` is a string' } };
      // A write in flight would land beside the commit's reading of the files.
      if (writing) return { status: 409, error: { message: 'a write is in flight; commit again once it lands' } };
      const done = session.commit({ client: request.client, message: request.message });
      if (done.ok) send({ event: 'committed', seq: ++seq, epoch, client: request.client, ...done.ok });
      return done;
    }
    if (request.method === 'propose' && request.hold !== undefined) {
      if (request.hold !== true) return { status: 400, error: { message: '`hold` is true when given' } };
      if (request.review !== undefined && !reviewShaped(request.review)) return { status: 400, error: { message: '`review` is an object; its `offers` a list of objects with `items` (indices), its `tokens` a count' } };
      return needsClient(request) ?? hold(request.client, request.ops, request.files, undefined, request.review);
    }
    if (request.method === 'accept') return needsClient(request) ?? accept(request);
    if (request.method === 'drop') {
      const refused = needsClient(request);
      if (refused) return refused;
      if (!proposals.has(request.proposal)) return { status: 404, error: { message: `no held proposal \`${request.proposal}\`` } };
      forget(request.proposal, { client: request.client, reason: 'dropped' });
      return { ok: { dropped: request.proposal } };
    }
    return ask(request);
  }

  /** A write, announced from the server's reply, even one that comes after
   * the client was told it timed out; the watcher waits for it. A conflict
   * names who last changed each file that moved. */
  async function write(request, extra = {}, settled = () => {}) {
    // The git facts of each file it may touch, before it does (commit's
    // baseline); this write's hold on them ends with its reply.
    const designFile = relative(appDir, designPath(appDir) ?? resolve(appDir, design));
    let holds;
    try {
      holds = session.prepare(request.method === 'undo' ? [...known.keys()] : (Array.isArray(request.ops) ? request.ops : []).map(o => (typeof o?.file === 'string' ? o.file : designFile)));
    } catch (e) {
      return { status: 500, error: { message: `reading git before the write: ${e.message}` } };
    }
    // Taken in the server's reply order, so a later reply has not yet moved
    // what this one is read against.
    const announce = v => {
      const written = new Set(v?.ok?.written ?? []);
      if (typeof v?.ok?.inverse === 'string') {
        // What moved by hand since the last announcement, as the write read
        // it (the files it did not write, the ones it did as they were
        // before it, and any gone), is a hand edit, said first and apart.
        const others = Object.fromEntries(Object.entries(v.ok.files).filter(([path]) => !written.has(path)));
        publish({ ...others, ...v.ok.before }, { changed: null, client: null }, [...known.keys()].filter(path => !(path in v.ok.files)));
        for (const path of written) {
          session.wrote(path, { before: v.ok.before[path], after: v.ok.files[path], client: v.ok.client, tokens: (v.ok.changed ?? []).filter(c => c.file === path).map(c => c.token) });
        }
        publish(v.ok.files, { changed: v.ok.changed, client: v.ok.client, ...extra });
      }
      session.release(holds.filter(path => !written.has(path)));
      if (v?.error?.conflict) {
        // A file whose digest is not the last announced moved by hand,
        // though the watcher has not said so yet.
        v.error.by = Object.fromEntries((v.error.stale ?? []).map(path => [path, v.error.files?.[path] === known.get(path) ? lastBy.get(path) ?? 'changed on disk' : 'changed on disk']));
      }
      settled(v);
      if (--writing === 0 && again) { again = false; rescan(); }
    };
    writing++;
    return ask(request, { done: announce });
  }
  const proposals = new Map();
  const forget = (id, why) => { proposals.delete(id); send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, ...why }); };
  const expire = () => { for (const [id, p] of proposals) if (p.expires < Date.now()) forget(id, { client: null, reason: 'expired' }); };
  const expiry = setInterval(expire, 30_000);
  expiry.unref?.();
  /** A proposal held again (an accept that did not apply it), announced,
   * within the cap. */
  const restore = (id, p) => {
    if (p.expires <= Date.now()) { send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: null, reason: 'expired' }); return; }
    proposals.set(id, p);
    while (proposals.size > HELD) forget(proposals.keys().next().value, { client: null, reason: 'too many held' });
    send({ event: 'proposal', seq: ++seq, epoch, ...p.shown });
  };
  /** The proposals held now, as a reconnecting client reads them. */
  const held = () => [...proposals].map(([id, p]) => p.shown);
  async function hold(client, ops, files, replaces, review) {
    expire();
    const planned = await ask({ method: 'propose', ops, ...(files === undefined ? {} : { files }) });
    if (!planned.ok) return planned;
    const id = `prop-${epoch}-${++proposalCount}`;
    const touched = new Set((Array.isArray(ops) ? ops : []).map(o => o?.file ?? relative(appDir, designPath(appDir) ?? resolve(appDir, design))));
    const base = Object.fromEntries(Object.entries(planned.ok.files).filter(([path]) => touched.has(path)));
    const expires = Date.now() + HOLD_MS;
    // The operations by index, as `accept`'s items name them.
    const shown = { proposal: id, client, diff: planned.ok.diff, changed: planned.ok.changed, class: planned.ok.class, expires: new Date(expires).toISOString(), ops: ops.map((op, index) => ({ index, op })), ...(replaces ? { replaces } : {}), ...(review ? { review } : {}) };
    proposals.set(id, { client, ops, base, expires, shown });
    // At most 64 held: the oldest goes.
    while (proposals.size > HELD) forget(proposals.keys().next().value, { client: null, reason: 'too many held' });
    send({ event: 'proposal', seq: ++seq, epoch, ...shown });
    return { ok: { ...planned.ok, ...shown } };
  }
  /** Accept a held proposal: applied against the digests it was reviewed
   * with; a file moved since, or only some `items` accepted, recomputes it
   * on the current files as a new proposal, for a fresh accept. */
  async function accept(request) {
    expire();
    const id = request.proposal, p = proposals.get(id);
    if (!p) return { status: 404, error: { message: `no held proposal \`${id}\` (accepted, dropped, being accepted or expired)` } };
    const items = request.items;
    if (items !== undefined && (!Array.isArray(items) || items.some(i => !Number.isInteger(i) || i < 0 || i >= p.ops.length))) {
      return { status: 400, error: { message: `\`items\` lists indices of the proposal's ${p.ops.length} operations` } };
    }
    // Taken out before anything waits: a second accept or a drop meanwhile
    // finds nothing. Every way out is announced: accepted, replaced, or
    // held again.
    proposals.delete(id);
    // The review rides along, its offers' items renumbered to the
    // operations kept.
    const recompute = async (ops, kept = ops.map((_, i) => i)) => {
      const old = p.shown.review;
      const review = old && {
        ...old,
        offers: (old.offers ?? []).map(o => ({ ...o, items: (o.items ?? []).map(i => kept.indexOf(i)).filter(i => i >= 0) })),
        ...(old.tokens === undefined ? {} : { tokens: kept.filter(i => i < old.tokens).length }),
      };
      let fresh;
      try { fresh = await hold(p.client, ops, undefined, id, review); } catch (e) { restore(id, p); throw e; }
      if (!fresh.ok) { restore(id, p); return fresh; }
      send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: request.client, reason: 'replaced', replacedBy: fresh.ok.proposal });
      return { ok: { recomputed: true, replaces: id, ...fresh.ok } };
    };
    const some = items && new Set(items).size < p.ops.length;
    if (some) {
      const kept = [...new Set(items)].sort((a, b) => a - b);
      return recompute(kept.map(i => p.ops[i]), kept);
    }
    // The write's own end, whenever the server answers (a client told it
    // timed out included): accepted, or held again unless it went stale.
    let waiting = true;
    const settled = v => {
      if (v?.ok) send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: request.client, reason: 'accepted' });
      else if (!v?.error?.conflict) restore(id, p);
      else if (!waiting) send({ event: 'proposal-dropped', seq: ++seq, epoch, proposal: id, client: null, reason: 'stale' });
    };
    const done = await write({ method: 'transaction', client: p.client, files: p.base, ops: p.ops }, { acceptedBy: request.client, proposal: id }, settled);
    waiting = false;
    if (done.error?.conflict) return recompute(p.ops);
    return done;
  }

  function close() {
    clearTimeout(timer);
    clearInterval(expiry);
    for (const w of watchers.values()) w.watcher.close();
    for (const res of subscribers) res.end();
    subscribers.clear();
    for (const poll of polls) poll();
    if (child.exitCode !== null || child.signalCode !== null || down) return Promise.resolve();
    const exited = new Promise(ok => child.once('exit', () => ok(true)));
    const signal = name => { try { process.kill(-child.pid, name); } catch { child.kill?.(name); } };
    const within = ms => Promise.race([exited, new Promise(ok => setTimeout(() => ok(false), ms).unref?.())]);
    child.stdin.end();
    signal('SIGTERM');
    // Not gone in 5 s: killed, and said if even that does not end it.
    return within(5000).then(gone => gone || (signal('SIGKILL'), within(2000))).then(gone => {
      if (!gone) log('tamber: the design server did not exit; its lock may still be held');
    });
  }

  return { handle, close, ready, get token() { return token; } };
}

/** `bun exact.mjs tamber serve <app> [--port <n>] [--lan]`: the wire alone,
 * on loopback unless `--lan` (whose clients still cannot have the token). */
export async function serveAlone(argv) {
  const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const given = resolve(argv.find((a, i) => !a.startsWith('-') && argv[i - 1] !== '--port') ?? '.');
  const dir = /\.(contract|tamber)$/.test(given) ? dirname(given) : given;
  if (!designPath(dir)) { console.error(`tamber serve: ${dir} has no design (app.tamber, or the file app.json's "tamber" names)`); return 1; }
  const lan = argv.includes('--lan');
  const host = lan ? '0.0.0.0' : '127.0.0.1';
  const server = createServer();
  await new Promise((ok, fail) => { server.once('error', fail); server.listen(Number(at('--port') ?? 0), host, ok); });
  const port = server.address().port;
  const origins = installBrowserOrigins({ host, port });
  const gate = developmentGate(origins, port);
  const tamber = tamberServer({ appDir: dir });
  server.on('request', async (req, res) => {
    const access = gate.check(req);
    if (!access.allowed) { res.writeHead(421, { 'cache-control': 'no-store' }); res.end(); return; }
    let url; try { url = new URL(req.url, 'http://localhost'); } catch { notFound(res); return; }
    if (!(await tamber.handle(req, res, url, access))) notFound(res);
  });
  const stop = async () => { await tamber.close(); server.close(); process.exit(0); };
  process.on('SIGINT', stop); process.on('SIGTERM', stop); process.on('SIGHUP', stop);
  // Its launcher (bun exact.mjs) gone, even by SIGKILL: stop too.
  const launcher = Number(process.env.EXACT_LAUNCHER_PID);
  if (Number.isInteger(launcher) && launcher > 1) setInterval(() => { try { process.kill(launcher, 0); } catch (e) { if (e.code === 'ESRCH') stop(); } }, 1000).unref();
  console.log(origins.map(o => `${o.origin}/__tamber`).join('\n'));
  console.log(`  (the design server for ${dir}; the token at GET /__tamber/token from this machine only; ctrl-c to stop)`);
  await new Promise(() => {});
}

/** `bun exact.mjs tamber op <url> <request JSON>|subscribe`: the scripted
 * client. It takes the token as any local client does, from that origin and
 * no other (no redirect is followed), then prints the reply, or each event
 * as it comes. */
export async function op(argv) {
  const [base, what] = argv;
  if (!base || !what) { console.error('usage: bun exact.mjs tamber op <server url> <request JSON>|subscribe'); return 2; }
  const origin = new URL(base).origin;
  const got = await fetch(`${origin}/__tamber/token`, { redirect: 'error' });
  if (!got.ok) { console.error(`tamber op: ${origin} gave no token (${got.status}); it answers this machine's loopback only`); return 1; }
  const { token } = await got.json();
  const headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  if (what === 'subscribe') {
    const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers, redirect: 'error' });
    if (!res.ok) { console.error(`tamber op: ${await res.text()}`); return 1; }
    const decoder = new TextDecoder();
    let buf = '';
    for await (const chunk of res.body) {
      buf += decoder.decode(chunk, { stream: true });
      const parts = buf.split('\n\n'); buf = parts.pop();
      for (const part of parts) if (part.startsWith('data: ')) console.log(part.slice(6));
    }
    return 0;
  }
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: what, redirect: 'error' });
  console.log(await res.text());
  return res.ok ? 0 : 1;
}

/** `bun exact.mjs tamber import <kind> <source> <server url> [--app <dir>]
 * [--prefix <p>]`: another system's theme (LLP 1121 D14) as held proposals
 * on the design server: its transaction as one, each role override it
 * offers as its own, for a person to accept or decline. Prints the count
 * and the unmapped report. */
export async function importTheme(argv) {
  const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const words = argv.filter((a, i) => !a.startsWith('-') && !['--app', '--prefix'].includes(argv[i - 1]));
  const [kind, source, base] = words;
  if (!kind || !source || !base) { console.error('usage: bun exact.mjs tamber import <kind> <source> <server url> [--app <dir>] [--prefix <prefix>]'); return 2; }
  const args = ['run', '-q', '-p', 'contract', '--', 'tamber', 'import', kind, resolve(source), '--app', resolve(at('--app') ?? '.'), ...(at('--prefix') ? ['--prefix', at('--prefix')] : [])];
  const r = spawnSync('cargo', args, { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20 });
  if (r.error) { console.error(`tamber import: cargo: ${r.error.message}`); return 1; }
  if (r.status !== 0) { process.stderr.write(r.stderr ?? ''); return r.status ?? 1; }
  const imported = JSON.parse(r.stdout);
  const origin = new URL(base).origin;
  const got = await fetch(`${origin}/__tamber/token`, { redirect: 'error' });
  if (!got.ok) { console.error(`tamber import: ${origin} gave no token (${got.status}); it answers this machine's loopback only`); return 1; }
  const { token } = await got.json();
  const client = `import:${kind}`;
  const { source: n, mapped, reported } = imported.counted;
  console.log(`${kind}: ${n} source values, ${mapped} mapped, ${reported} reported`);
  for (const u of imported.unmapped) console.log(`  unmapped ${u.block} ${u.name}: ${u.value || '(empty)'} (${u.why}${u.token ? `; would be ${u.token}` : ''})`);
  // One proposal: the tokens, then each role override as its own items, for
  // the review to leave off until ticked (accepting some items recomputes).
  const ops = [...imported.ops];
  const offers = imported.offers.map(o => {
    const items = o.ops.map((op, k) => ops.length + k);
    ops.push(...o.ops);
    return { role: o.role, token: o.token, items, note: o.note };
  });
  const offered = offers.flatMap(o => o.items);
  for (const o of offers) console.log(`  offer ${o.role} → ${o.token}${o.items.length ? `: items ${o.items.join(', ')}` : `: ${o.note}`}`);
  // The source as the design has it already: nothing to propose; a role
  // override declined before is not offered again on its own.
  if (!imported.ops.length) { console.log('nothing to propose: the design has every mapped value already'); return 0; }
  const review = { importer: kind, counted: imported.counted, unmapped: imported.unmapped, offers, tokens: imported.ops.length };
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', redirect: 'error', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: JSON.stringify({ method: 'propose', hold: true, client, ops, review }) });
  const held = await res.json();
  if (!held.ok) { console.error(`tamber import: ${held.error?.message}`); return 1; }
  console.log(`proposal ${held.ok.proposal}: ${ops.length} operations (${imported.ops.length} tokens and axes, ${offered.length} offered), held for review`);
  return 0;
}
