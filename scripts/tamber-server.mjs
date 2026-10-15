// The design server's wire (LLP 1121 D13, stage 3): HTTP to clients, JSON
// lines to `contract tamber serve --stdio`, which owns the files (digests,
// the lock, the journal, every write). This writes no design file.
//
// - `GET /__tamber/token`: the token, minted in memory at start, only to a
//   request the development gate calls local (loopback peer and host, no
//   forwarding header); any other gets the install endpoint's not-found.
// - `POST /__tamber`: one request (`hello`, `describe`, `resolve`, `explain`,
//   `propose`, `transaction`, `undo`), its reply as the server gave it.
// - `POST /__tamber/subscribe`: a streamed response, one event a change,
//   whoever made it: a client's write (with its client), a hand edit or a
//   `git checkout` (seen by the watcher, `client` null). The server's own
//   writes are known by digest and announced once.
// - Both POSTs carry the token as `x-exact-tamber-token`, never in a URL.
//
// Mounted by both dev loops (host/web/dev.mjs, host/web-js/dev.mjs) when the
// app has a design; `bun exact.mjs tamber serve <app>` runs it alone and
// `bun exact.mjs tamber op <url> <request>` is its scripted client. Never in a
// release build or a bake. D13 names WebSocket; the dev loops serve
// server-sent events and no WebSocket, so stage 3 streams a POST's response.
import { spawn } from 'node:child_process';
import { createHash, randomBytes, timingSafeEqual } from 'node:crypto';
import { existsSync, readFileSync, watch } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, resolve } from 'node:path';
import { developmentGate, installBrowserOrigins } from './install-page.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const BODY_LIMIT = 1 << 20;
const METHODS = new Set(['hello', 'describe', 'resolve', 'explain', 'propose', 'transaction', 'undo']);
const WRITES = new Set(['transaction', 'undo']);

const json = (res, status, body) => {
  res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
  res.end(JSON.stringify(body));
};
const notFound = res => json(res, 404, { message: 'Not found.' });
const digest = path => { try { return createHash('sha256').update(readFileSync(path)).digest('hex'); } catch { return ''; } };

/** The app's design, as the compiler finds it: the file `app.json`'s
 * `"tamber"` names, else `app.tamber`; null when there is none. */
export function designPath(appDir) {
  let named = null;
  try { named = JSON.parse(readFileSync(resolve(appDir, 'app.json'), 'utf8')).tamber ?? null; } catch { /* no manifest */ }
  const path = resolve(appDir, typeof named === 'string' ? named : 'app.tamber');
  return existsSync(path) ? path : null;
}

/** `contract tamber serve --stdio <app>`, through cargo as the dev loops run
 * the compiler. */
function spawnContract(appDir) {
  return spawn('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'serve', '--stdio', appDir], { cwd: ROOT, stdio: ['pipe', 'pipe', 'pipe'] });
}

/** The wire for one app: `handle(req, res, url, access)` answers the
 * `/__tamber` paths (true when it did), `close()` stops the server. */
export function tamberServer({ appDir, spawnServe = spawnContract, log = console.log, settle = 50 }) {
  const token = randomBytes(32).toString('hex');
  const child = spawnServe(appDir);
  const waiting = new Map(), subscribers = new Set(), watchers = [];
  // Each file's digest as last announced: what a later change is told from.
  const known = new Map();
  let next = 0, out = '', stderr = '', exited = null, writing = 0, again = false, timer = null;

  const publish = event => {
    const edited = Object.entries(event.files ?? {}).filter(([path, d]) => known.get(path) !== d).map(([path]) => path);
    for (const [path, d] of Object.entries(event.files ?? {})) known.set(path, d);
    const line = `data: ${JSON.stringify({ ...event, edited })}\n\n`;
    for (const res of subscribers) res.write(line);
  };
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', chunk => {
    out += chunk;
    const lines = out.split('\n'); out = lines.pop();
    for (const text of lines) {
      if (!text.trim()) continue;
      let v; try { v = JSON.parse(text); } catch { continue; }
      // A write's event is published from its reply (below), in order.
      if (v.event) continue;
      const reply = waiting.get(v.id);
      waiting.delete(v.id);
      reply?.(v);
    }
  });
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-4000); });
  child.on('exit', code => {
    exited = stderr.trim().split('\n').pop() || `the design server exited ${code}`;
    log(`tamber: ${exited}`);
    for (const reply of waiting.values()) reply({ error: { message: exited } });
    waiting.clear();
  });
  child.stdin.on('error', () => {});

  const ask = request => new Promise(reply => {
    if (exited) return reply({ error: { message: exited } });
    const id = ++next;
    waiting.set(id, reply);
    child.stdin.write(`${JSON.stringify({ ...request, id })}\n`);
  });

  // A hand edit: the files read again once they settle, and announced when
  // a digest moved. While a write is in flight its own renames are not one.
  const rescan = () => {
    timer = null;
    if (writing) { again = true; return; }
    const files = Object.fromEntries([...known.keys()].map(path => [path, digest(resolve(appDir, path))]));
    if (Object.entries(files).some(([path, d]) => known.get(path) !== d)) publish({ event: 'changed', files, changed: null, client: null });
  };
  const ready = ask({ method: 'hello' }).then(reply => {
    for (const [path, d] of Object.entries(reply.ok?.files ?? {})) known.set(path, d);
    const dirs = new Set([...known.keys()].map(path => dirname(resolve(appDir, path))));
    for (const dir of dirs) {
      try {
        watchers.push(watch(dir, (_, name) => {
          if (name && !String(name).endsWith('.tamber')) return;
          clearTimeout(timer); timer = setTimeout(rescan, settle);
        }));
      } catch (e) { log(`tamber: cannot watch ${dir}: ${e.message}`); }
    }
  });

  const tokenOf = req => {
    const given = Buffer.from(String(req.headers['x-exact-tamber-token'] ?? ''));
    const mine = Buffer.from(token);
    return given.length === mine.length && timingSafeEqual(given, mine);
  };
  const body = req => new Promise(done => {
    const chunks = []; let size = 0, over = false;
    req.on('data', chunk => { size += chunk.length; if (size > BODY_LIMIT) over = true; else chunks.push(chunk); });
    req.on('end', () => done(over ? null : Buffer.concat(chunks).toString('utf8')));
    req.on('error', () => done(null));
  });

  async function handle(req, res, url, access) {
    if (url.pathname === '/__tamber/token') {
      if (req.method !== 'GET' || !access.local) notFound(res);
      else json(res, 200, { token });
      return true;
    }
    if (url.pathname !== '/__tamber' && url.pathname !== '/__tamber/subscribe') return false;
    if (req.method !== 'POST' || !tokenOf(req)) { notFound(res); return true; }
    await ready;
    if (url.pathname === '/__tamber/subscribe') {
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-store', connection: 'keep-alive' });
      res.write(`data: ${JSON.stringify({ event: 'subscribed', files: Object.fromEntries(known) })}\n\n`);
      subscribers.add(res);
      req.on('close', () => subscribers.delete(res));
      return true;
    }
    const text = await body(req);
    if (text == null) { json(res, 413, { error: { message: 'a request is at most 1 MiB' } }); return true; }
    let request;
    try { request = JSON.parse(text); } catch { request = null; }
    if (!request || typeof request !== 'object' || Array.isArray(request) || !METHODS.has(request.method)) {
      json(res, 400, { error: { message: `a request is an object whose method is one of ${[...METHODS].join(', ')}` } });
      return true;
    }
    delete request.id;
    const write = WRITES.has(request.method);
    if (write) writing++;
    let reply;
    try {
      reply = await ask(request);
      if (write && typeof reply.ok?.inverse === 'string') publish({ event: 'changed', files: reply.ok.files, changed: reply.ok.changed, client: reply.ok.client });
    } finally {
      if (write && --writing === 0 && again) { again = false; rescan(); }
    }
    const status = reply.ok !== undefined ? 200 : exited ? 503 : reply.error?.conflict ? 409 : 400;
    json(res, status, reply.ok !== undefined ? { ok: reply.ok } : { error: reply.error });
    return true;
  }

  function close() {
    clearTimeout(timer);
    for (const w of watchers) w.close();
    for (const res of subscribers) res.end();
    subscribers.clear();
    child.stdin.end();
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM');
  }

  return { handle, close, ready, get token() { return token; } };
}

/** `bun exact.mjs tamber serve <app> [--port <n>] [--lan]`: the wire alone,
 * on loopback unless `--lan` (whose clients still cannot have the token). */
export async function serveAlone(argv) {
  const at = name => { const i = argv.indexOf(name); return i < 0 ? undefined : argv[i + 1]; };
  const appDir = resolve(argv.find((a, i) => !a.startsWith('-') && !['--port'].includes(argv[i - 1])) ?? '.');
  const dir = appDir.endsWith('.contract') ? dirname(appDir) : appDir;
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
  const stop = () => { tamber.close(); server.close(); process.exit(0); };
  process.on('SIGINT', stop); process.on('SIGTERM', stop);
  console.log(origins.map(o => `${o.origin}/__tamber`).join('\n'));
  console.log(`  (the design server for ${dir}; the token at GET /__tamber/token from this machine only; ctrl-c to stop)`);
  await new Promise(() => {});
}

/** `bun exact.mjs tamber op <url> <request JSON>|subscribe`: the scripted
 * client. It takes the token as any local client does, then prints the
 * reply, or each event as it comes. */
export async function op(argv) {
  const [base, what] = argv;
  if (!base || !what) { console.error('usage: bun exact.mjs tamber op <server url> <request JSON>|subscribe'); return 2; }
  const origin = new URL(base).origin;
  const got = await fetch(`${origin}/__tamber/token`);
  if (!got.ok) { console.error(`tamber op: ${origin} gave no token (${got.status}); it answers this machine's loopback only`); return 1; }
  const { token } = await got.json();
  const headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  if (what === 'subscribe') {
    const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers });
    const decoder = new TextDecoder();
    for await (const chunk of res.body) process.stdout.write(decoder.decode(chunk, { stream: true }).replace(/^data: /gm, '').replace(/\n\n/g, '\n'));
    return 0;
  }
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: what });
  console.log(await res.text());
  return res.ok ? 0 : 1;
}
