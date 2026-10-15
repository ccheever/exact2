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
//   - `POST /__tamber/events` `{"after": n}`: the events after `n` (the last
//     256 kept), answered once there is one or after 25 s with none: for a
//     client whose `fetch` reads a body whole, as a JS-target data module's
//     does.
// - Every POST carries the token as `x-exact-tamber-token`, never in a URL.
//
// Mounted by both dev loops (host/web/dev.mjs, host/web-js/dev.mjs) when the
// app has a design; `bun exact.mjs tamber serve <app>` runs it alone and
// `bun exact.mjs tamber op <url> <request>` is its scripted client. Never in a
// release build or a bake. D13 names WebSocket; the dev loops serve
// server-sent events and no WebSocket, so stage 3 streams a POST's response.
import { spawn } from 'node:child_process';
import { randomBytes, timingSafeEqual } from 'node:crypto';
import { existsSync, readFileSync, watch } from 'node:fs';
import { createServer } from 'node:http';
import { basename, dirname, resolve } from 'node:path';
import { developmentGate, installBrowserOrigins } from './install-page.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
// The server's own line limit, less room for the id this adds.
const LINE_LIMIT = 1 << 20;
const BODY_LIMIT = LINE_LIMIT - 64;
const ANSWER_MS = 120_000, POLL_MS = 25_000, KEPT = 256;
const METHODS = new Set(['hello', 'describe', 'resolve', 'explain', 'propose', 'transaction', 'undo']);
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
  const waiting = new Map(), subscribers = new Set(), polls = new Set(), watchers = new Map(), events = [];
  // Each file's digest as last announced: what a later change is told from.
  const known = new Map();
  let next = 0, seq = 0, out = '', stderr = '', down = null, writing = 0, again = false, timer = null, first = true;
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
    if (edited.length || rest.client) send({ event: 'changed', seq: ++seq, ...rest, files: Object.fromEntries(known), edited });
  };

  // The server gone (never started, the lock another's, exited): every
  // request in flight, every subscriber and every later request told so.
  const fail = message => {
    if (down) return;
    down = message;
    log(`tamber: ${message}`);
    for (const reply of waiting.values()) reply({ error: { message } });
    waiting.clear();
    send({ event: 'down', seq: ++seq, message });
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

  const ask = request => new Promise(reply => {
    if (down) return reply({ error: { message: down } });
    const line = JSON.stringify({ ...request, id: ++next });
    if (Buffer.byteLength(line) >= LINE_LIMIT) return reply({ status: 413, error: { message: 'a request is at most 1 MiB' } });
    const id = next;
    const late = setTimeout(() => { waiting.delete(id); reply({ status: 504, error: { message: 'the design server did not answer' } }); }, ANSWER_MS);
    waiting.set(id, v => { clearTimeout(late); reply(v); });
    child.stdin.write(`${line}\n`);
  });

  // The design's files as the server has them now, announced where a
  // digest moved, and every directory holding one watched. A file that
  // appears (a new design, a checkout) is found by asking again.
  const refresh = async () => {
    const reply = await ask({ method: 'hello' });
    const baseline = first;
    first = false;
    // A failed read is said, and the next change reads again.
    if (!reply.ok) { log(`tamber: ${reply.error?.message}`); return; }
    // The first read is the baseline, not a change.
    if (baseline) for (const [path, d] of Object.entries(reply.ok.files)) known.set(path, d);
    else publish(reply.ok.files, { changed: null, client: null }, [...known.keys()].filter(path => !(path in reply.ok.files)));
    watchAll(Object.keys(reply.ok.files));
  };
  // The app's directory (where a new design beside it appears) and the
  // design's, whatever the first read gave, and any other holding a file.
  const watchAll = paths => {
    for (const dir of new Set([appDir, dirname(designPath(appDir) ?? resolve(appDir, design)), ...paths.map(path => dirname(resolve(appDir, path)))])) {
      if (watchers.has(dir)) continue;
      try {
        watchers.set(dir, watch(dir, (_, name) => {
          // A design file, or one that may be: `.tamber`, the design's own
          // name (app.json may name any), or a name the platform did not give.
          if (name && !String(name).endsWith('.tamber') && basename(String(name)) !== design) return;
          clearTimeout(timer); timer = setTimeout(rescan, settle);
        }));
      } catch (e) { log(`tamber: cannot watch ${dir}: ${e.message}`); }
    }
  };
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
      res.write(`data: ${JSON.stringify({ event: 'subscribed', seq, files: Object.fromEntries(known) })}\n\n`);
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
        const missed = after > seq || after < seq && (events[0]?.seq ?? seq + 1) > after + 1;
        json(res, 200, { events: after > seq ? events : since, seq, missed, files: Object.fromEntries(known) });
      };
      if (events.some(e => e.seq > after) || after > seq || down) { answer(); return true; }
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
    const write = WRITES.has(request.method);
    if (write && (typeof request.client !== 'string' || !request.client)) {
      json(res, 400, { error: { message: 'a write names its `client`' } });
      return true;
    }
    delete request.id;
    if (write) writing++;
    let reply;
    try {
      reply = await ask(request);
      if (write && typeof reply.ok?.inverse === 'string') {
        // What moved by hand since the last announcement, as the write read
        // it (the files it did not write, and the ones it did as they were
        // before it), is a hand edit, said first and apart.
        const written = new Set(reply.ok.written ?? []);
        const others = Object.fromEntries(Object.entries(reply.ok.files).filter(([path]) => !written.has(path)));
        publish({ ...others, ...reply.ok.before }, { changed: null, client: null });
        publish(reply.ok.files, { changed: reply.ok.changed, client: reply.ok.client });
      }
    } finally {
      if (write && --writing === 0 && again) { again = false; rescan(); }
    }
    const status = reply.ok !== undefined ? 200 : reply.status ?? (down ? 503 : reply.error?.conflict ? 409 : 400);
    json(res, status, reply.ok !== undefined ? { ok: reply.ok } : { error: reply.error });
    return true;
  }

  function close() {
    clearTimeout(timer);
    for (const w of watchers.values()) w.close();
    for (const res of subscribers) res.end();
    subscribers.clear();
    for (const poll of polls) poll();
    if (child.exitCode !== null || child.signalCode !== null || down) return Promise.resolve();
    const exited = new Promise(ok => child.once('exit', () => ok(true)));
    const signal = name => { try { process.kill(-child.pid, name); } catch { child.kill?.(name); } };
    const within = ms => Promise.race([exited, new Promise(ok => setTimeout(() => ok(false), ms))]);
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
