// The design server's wire (LLP 1121 D13, stage 3 commit 4): the token only
// to a local request, every read and write behind it, two clients' writes to
// one token where one conflicts, a hand edit and a `git checkout` reaching a
// subscriber, a write announced once, and the dev loop answering `hello`.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { PassThrough } from 'node:stream';
import { EventEmitter } from 'node:events';
import { developmentGate } from './install-page.mjs';
import { tamberServer } from './tamber-server.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const DESIGN = 'tamber t\n\ntheme\n  ink = #000000\n  edge = #101010\n';
const BRAND = 'tamber brand\n\ntheme\n  brand.ink = #202020\n';
const sha = text => createHash('sha256').update(text).digest('hex');

/** A stand-in for `contract tamber serve` that answers `hello`. */
function fakeServe() {
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null, kill() {} });
  let buf = '';
  child.stdin.on('data', chunk => {
    buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
    for (const line of lines) child.stdout.write(`${JSON.stringify({ id: JSON.parse(line).id, ok: { files: {} } })}\n`);
  });
  return child;
}

/** A request as the gate and the handler see it, and what it was answered. */
async function ask(tamber, gate, { path, method = 'GET', headers = {}, peer = '127.0.0.1', body = '' }) {
  const req = Object.assign(new PassThrough(), { method, headers, socket: { remoteAddress: peer } });
  let status = 0, text = '';
  const res = { writeHead(s) { status = s; }, write(t) { text += t; }, end(t) { text += t ?? ''; } };
  queueMicrotask(() => req.end(body));
  const handled = await tamber.handle(req, res, new URL(path, 'http://x'), gate.check(req));
  return { handled, status, body: text ? JSON.parse(text) : null };
}

test('the token goes only to a local request, and every request needs it', async () => {
  const port = 8800;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }, { origin: `http://192.168.1.20:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const local = { host: `127.0.0.1:${port}` };
  const given = await ask(tamber, gate, { path: '/__tamber/token', headers: local });
  expect(given.status).toBe(200);
  expect(given.body.token).toBe(tamber.token);
  // A forwarded request, a `--lan` host, a peer off this machine: the install endpoint's not-found.
  for (const refused of [
    { headers: { ...local, 'x-forwarded-for': '203.0.113.9' } },
    { headers: { ...local, forwarded: 'for=203.0.113.9' } },
    { headers: { host: `192.168.1.20:${port}` }, peer: '192.168.1.30' },
    { headers: local, peer: '192.168.1.30' },
  ]) {
    const answer = await ask(tamber, gate, { path: '/__tamber/token', ...refused });
    expect(answer.status).toBe(404);
    expect(answer.body).toEqual({ message: 'Not found.' });
  }
  // A read or a write without the token, or with another.
  for (const headers of [local, { ...local, 'x-exact-tamber-token': 'f'.repeat(64) }]) {
    const answer = await ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: '{"method":"hello"}' });
    expect(answer.status).toBe(404);
  }
  const hello = await ask(tamber, gate, { path: '/__tamber', method: 'POST', headers: { ...local, 'x-exact-tamber-token': tamber.token }, body: '{"method":"hello"}' });
  expect(hello.status).toBe(200);
  // A path that is not the wire's is left to the dev loop.
  expect((await ask(tamber, gate, { path: '/__dev', headers: local })).handled).toBe(false);
  tamber.close();
});

// The real server (`contract tamber serve`, built once by cargo) behind an
// HTTP listener and the development gate, as a dev loop mounts it.
let dir, server, tamber, origin, headers;
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-tamber-wire-'));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "a"\n');
  writeFileSync(join(dir, 'app.tamber'), DESIGN);
  writeFileSync(join(dir, 'brand.tamber'), BRAND);
  const git = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: dir });
  git('init', '-q'); git('add', '.'); git('commit', '-q', '-m', 'design');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  tamber = tamberServer({ appDir: dir, log: () => {} });
  server.on('request', async (req, res) => {
    if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  origin = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${origin}/__tamber/token`)).json();
  headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  await tamber.ready;
}, 600_000); // a cold checkout builds the compiler first
afterAll(() => {
  tamber?.close();
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});

const post = async request => {
  const res = await fetch(`${origin}/__tamber`, { method: 'POST', headers, body: JSON.stringify(request) });
  return { status: res.status, ...(await res.json()) };
};

/** The change stream's events as they come. */
async function subscribe() {
  const res = await fetch(`${origin}/__tamber/subscribe`, { method: 'POST', headers });
  const reader = res.body.getReader(), decoder = new TextDecoder(), events = [];
  let buf = '';
  (async () => {
    for (;;) {
      const { value, done } = await reader.read().catch(() => ({ done: true }));
      if (done) return;
      buf += decoder.decode(value, { stream: true });
      const parts = buf.split('\n\n'); buf = parts.pop();
      for (const part of parts) if (part.startsWith('data: ')) events.push(JSON.parse(part.slice(6)));
    }
  })();
  const until = async (pass, ms = 5000) => {
    const end = Date.now() + ms;
    while (!pass(events)) { if (Date.now() > end) throw new Error(`no such event in ${JSON.stringify(events)}`); await Bun.sleep(20); }
  };
  return { events, until, close: () => reader.cancel() };
}

const disk = file => sha(readFileSync(join(dir, file)));

test('two clients writing one token: the second conflicts, and the write is announced once', async () => {
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  const seen = { 'app.tamber': disk('app.tamber') };
  const first = await post({ method: 'transaction', client: 'one', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#111111' }] });
  expect(first.status).toBe(200);
  const second = await post({ method: 'transaction', client: 'two', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#222222' }] });
  expect(second.status).toBe(409);
  expect(second.error.conflict).toBe(true);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('ink = #111111');
  await stream.until(e => e.some(x => x.client === 'one'));
  // The watcher sees the server's own rename and says nothing more.
  await Bun.sleep(400);
  const changes = stream.events.filter(x => x.event === 'changed');
  expect(changes.length).toBe(1);
  expect(changes[0]).toMatchObject({ client: 'one', edited: ['app.tamber'], changed: [{ file: 'app.tamber', token: 'ink' }] });
  const undone = await post({ method: 'undo', client: 'one', inverse: first.ok.inverse });
  expect(undone.status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toBe(DESIGN);
  stream.close();
}, 30_000);

test('a hand edit and a git checkout reach a subscriber', async () => {
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#303030'));
  await stream.until(e => e.some(x => x.event === 'changed' && x.client === null && x.edited.includes('brand.tamber')));
  writeFileSync(join(dir, 'app.tamber'), DESIGN.replace('#101010', '#121212'));
  await Bun.sleep(300);
  const before = stream.events.length;
  // A checkout puts both files back: one change, or two, each a hand edit.
  spawnSync('git', ['checkout', '--', 'app.tamber', 'brand.tamber'], { cwd: dir });
  await stream.until(e => {
    const last = e.at(-1);
    return e.length > before && last.files['app.tamber'] === sha(DESIGN) && last.files['brand.tamber'] === sha(BRAND);
  });
  expect(stream.events.slice(before).every(x => x.client === null)).toBe(true);
  stream.close();
}, 30_000);

test("the dev loop mounts the wire, and the theme fixture's server answers hello", async () => {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-tamber-dev-'));
  const dev = spawn(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', 'theme-fixture', '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    let out = '';
    await new Promise((ok, fail) => {
      dev.stdout.on('data', d => { out += d; if (out.includes(`127.0.0.1:${port}/`)) ok(); });
      dev.on('exit', code => fail(new Error(`the dev loop exited ${code}: ${out}`)));
    });
    const base = `http://127.0.0.1:${port}`;
    const { token } = await (await fetch(`${base}/__tamber/token`)).json();
    const hello = await (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    expect(hello.ok.protocol).toBe('tamber/1.0');
    expect(Object.keys(hello.ok.files)).toContain('app.tamber');
  } finally {
    const exit = new Promise(ok => dev.once('exit', ok));
    dev.kill('SIGTERM');
    await exit;
    rmSync(dist, { recursive: true, force: true });
  }
}, 600_000);
