// The design server's wire (LLP 1121 D13, stage 3 commit 4): the token only
// to a local request, every read and write behind it, two clients' writes to
// one token where one conflicts, a hand edit, a new file and a `git checkout`
// reaching a subscriber and a long poll, a write announced once, a server
// whose lock is another's answering 503, and the dev loop answering `hello`.
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
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
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough(), stderr: new PassThrough(), exitCode: null, signalCode: null });
  child.kill = () => { child.exitCode = 0; child.emit('exit', 0); };
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

// LLP 1121 stage 4: a dev loop's wire says its builds take a value-only edit
// live; a write is held until the server answers, however long (a read
// still times out), so a write never lands after its client was told it
// failed.
test('hello says live, and a slow write is answered, not timed out', async () => {
  const port = 8803;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  // Answers a transaction after 300 ms, a describe never.
  const slowServe = () => {
    const child = fakeServe();
    child.stdin.removeAllListeners('data');
    let buf = '';
    child.stdin.on('data', chunk => {
      buf += chunk; const lines = buf.split('\n'); buf = lines.pop();
      for (const line of lines) {
        const r = JSON.parse(line);
        const answer = () => child.stdout.write(`${JSON.stringify({ id: r.id, ok: { files: {}, written: [], changed: [] } })}\n`);
        if (r.method === 'transaction') setTimeout(answer, 300);
        else if (r.method !== 'describe') answer();
      }
    });
    return child;
  };
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: slowServe, log: () => {}, live: true, answerMs: 100 });
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  const post = body => ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: JSON.stringify(body) });
  expect((await post({ method: 'hello' })).body.ok.live).toBe(true);
  const write = await post({ method: 'transaction', client: 'agent', files: {}, ops: [] });
  expect(write.status).toBe(200);
  expect((await post({ method: 'describe' })).status).toBe(504);
  tamber.close();
});

// LLP 1122 D4: the dev loop's builds in `hello`, and a pin a client may
// set only while it subscribes by name, cleared on every build when it goes.
test('a pin is its client\'s while it is present, and builds are in hello', async () => {
  const port = 8804;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {} });
  const headers = { host: `127.0.0.1:${port}`, 'x-exact-tamber-token': tamber.token };
  const post = body => ask(tamber, gate, { path: '/__tamber', method: 'POST', headers, body: JSON.stringify(body) });
  const frames = [];
  const build = tamber.channel.open(f => frames.push(f));
  tamber.channel.report(JSON.stringify({ nonce: build.nonce, kind: 'register', id: 'web-1', host: 'web', capabilities: ['pin'] }));
  expect((await post({ method: 'hello' })).body.ok.builds.map(b => b.id)).toEqual(['web-1']);
  // Not present: refused.
  expect((await post({ method: 'pin', client: 'tool', build: 'web-1', pin: { scheme: 'dark' } })).status).toBe(409);
  // A long poll naming the client makes it present for 35 s after it is answered.
  const polled = await ask(tamber, gate, { path: '/__tamber/events', method: 'POST', headers, body: JSON.stringify({ wait: false, client: 'poller' }) });
  expect(polled.status).toBe(200);
  expect((await post({ method: 'pin', client: 'poller', build: 'web-1', pin: { scheme: 'light' } })).status).toBe(200);
  // 35 s on with no poll: absent, and its pin handed back.
  const realNow = Date.now;
  Date.now = () => realNow() + 36_000;
  try { await post({ method: 'hello' }); } finally { Date.now = realNow; }
  expect(frames.at(-1)).toMatchObject({ kind: 'pin', pin: null });
  // Subscribed by name: taken, and a frame goes to that build.
  const req = Object.assign(new PassThrough(), { method: 'POST', headers, socket: { remoteAddress: '127.0.0.1' } });
  const res = Object.assign(new EventEmitter(), { destroyed: false, writableEnded: false, writeHead() {}, write() {}, end() {} });
  queueMicrotask(() => req.end(JSON.stringify({ client: 'tool' })));
  await tamber.handle(req, res, new URL('/__tamber/subscribe', 'http://x'), gate.check(req));
  const taken = await post({ method: 'preview', client: 'tool', build: 'web-1', axis: 'dark-style', value: 'dim' });
  expect(taken.status).toBe(200);
  expect(frames.at(-1)).toMatchObject({ kind: 'pin', pin: { mode: { 'dark-style': 'dim' } } });
  // The client goes: its pin is handed back.
  res.emit('close');
  expect(frames.at(-1)).toMatchObject({ kind: 'pin', pin: null });
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
  git('init', '-q'); git('config', 'user.email', 't@example.test'); git('config', 'user.name', 't'); git('add', '.'); git('commit', '-q', '-m', 'design');
  server = createServer();
  await new Promise(ok => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  // The watcher settles slower than a request lands, so a write's own
  // reply, not the watcher, is what tells a hand edit before it apart.
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 250 });
  server.on('request', async (req, res) => {
    if (!(await tamber.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  origin = `http://127.0.0.1:${port}`;
  const { token } = await (await fetch(`${origin}/__tamber/token`)).json();
  headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
  await tamber.ready;
}, 600_000); // a cold checkout builds the compiler first
afterAll(async () => {
  await tamber?.close();
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});

/** The wire stopped and started again: a new session, a new token. */
async function freshWire() {
  await tamber.close();
  tamber = tamberServer({ appDir: dir, log: () => {}, settle: 250 });
  await tamber.ready;
  const { token } = await (await fetch(`${origin}/__tamber/token`)).json();
  headers = { 'content-type': 'application/json', 'x-exact-tamber-token': token };
}

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
  const anonymous = await post({ method: 'transaction', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#111111' }] });
  expect(anonymous.status).toBe(400);
  const first = await post({ method: 'transaction', client: 'one', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#111111' }] });
  expect(first.status).toBe(200);
  expect(first.ok.written).toEqual(['app.tamber']);
  const second = await post({ method: 'transaction', client: 'two', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#222222' }] });
  expect(second.status).toBe(409);
  expect(second.error.conflict).toBe(true);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('ink = #111111');
  // A hand edit after it: the watcher's next read, which would also carry
  // any second announcement of the write.
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#212121'));
  await stream.until(e => e.some(x => x.client === null && x.edited?.includes('brand.tamber')));
  const changes = stream.events.filter(x => x.event === 'changed');
  expect(changes.map(x => [x.client, x.edited])).toEqual([['one', ['app.tamber']], [null, ['brand.tamber']]]);
  expect(changes[0].changed).toEqual([{ file: 'app.tamber', token: 'ink' }]);
  const undone = await post({ method: 'undo', client: 'one', inverse: first.ok.inverse });
  expect(undone.status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toBe(DESIGN);
  writeFileSync(join(dir, 'brand.tamber'), BRAND);
  await stream.until(e => e.at(-1)?.files['brand.tamber'] === sha(BRAND));
  stream.close();
}, 30_000);

test('a hand edit made before a write is its own event, not the writer\'s', async () => {
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  const at = stream.events.length;
  // Too soon for the watcher: the write's reply carries the moved digest.
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#232323'));
  const done = await post({ method: 'transaction', client: 'one', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'edge', value: '#131313' }] });
  expect(done.status).toBe(200);
  await stream.until(e => e.slice(at).some(x => x.client === 'one'));
  const mine = stream.events.slice(at).find(x => x.client === 'one');
  expect(mine.edited).toEqual(['app.tamber']);
  await stream.until(e => e.slice(at).some(x => x.client === null && x.edited.includes('brand.tamber')));
  await post({ method: 'undo', client: 'one', inverse: done.ok.inverse });
  writeFileSync(join(dir, 'brand.tamber'), BRAND);
  await stream.until(e => e.at(-1)?.files['brand.tamber'] === sha(BRAND) && e.at(-1)?.files['app.tamber'] === sha(DESIGN));
  stream.close();
}, 30_000);

test('a hand edit and a git checkout reach a subscriber', async () => {
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#303030'));
  await stream.until(e => e.some(x => x.event === 'changed' && x.client === null && x.edited.includes('brand.tamber')));
  writeFileSync(join(dir, 'app.tamber'), DESIGN.replace('#101010', '#121212'));
  await stream.until(e => e.at(-1)?.files['app.tamber'] === sha(DESIGN.replace('#101010', '#121212')));
  const before = stream.events.length;
  // A checkout puts both files back: one change, or two, each a hand edit.
  spawnSync('git', ['checkout', '--', 'app.tamber', 'brand.tamber'], { cwd: dir });
  await stream.until(e => {
    const last = e.at(-1);
    return e.length > before && last.files['app.tamber'] === sha(DESIGN) && last.files['brand.tamber'] === sha(BRAND);
  });
  expect(stream.events.slice(before).every(x => x.client === null)).toBe(true);
  // A design file that appears is found, and a long poll hears of it.
  const { seq } = await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: '{"after":0}' })).json();
  // `wait: false` answers at once, with nothing new.
  const now = await (await fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: JSON.stringify({ after: seq, wait: false }), signal: AbortSignal.timeout(5000) })).json();
  expect(now).toMatchObject({ events: [], seq, missed: false });
  const polled = fetch(`${origin}/__tamber/events`, { method: 'POST', headers, body: JSON.stringify({ after: seq }) }).then(r => r.json());
  writeFileSync(join(dir, 'late.tamber'), 'tamber late\n\ntheme\n  late.ink = #404040\n');
  await stream.until(e => e.some(x => x.edited?.includes('late.tamber')));
  const answer = await polled;
  expect(answer.events.some(x => x.edited.includes('late.tamber'))).toBe(true);
  const late = await post({ method: 'propose', ops: [{ op: 'set-token', file: 'late.tamber', name: 'late.ink', value: '#414141' }] });
  expect(late.status).toBe(200);
  rmSync(join(dir, 'late.tamber'));
  await stream.until(e => !('late.tamber' in e.at(-1)?.files));
  // The design itself removed and checked out again.
  rmSync(join(dir, 'app.tamber'));
  await stream.until(e => !('app.tamber' in e.at(-1)?.files) && e.at(-1)?.edited.includes('app.tamber'));
  spawnSync('git', ['checkout', '--', 'app.tamber'], { cwd: dir });
  await stream.until(e => e.at(-1)?.files['app.tamber'] === sha(DESIGN));
  stream.close();
}, 30_000);

test('a second wire for the app, whose lock is the first\'s, answers 503', async () => {
  const second = tamberServer({ appDir: dir, log: () => {} });
  await second.ready;
  const gate = developmentGate([{ origin: 'http://127.0.0.1:8801' }], 8801);
  const local = { host: '127.0.0.1:8801', 'x-exact-tamber-token': second.token };
  const subscribed = await ask(second, gate, { path: '/__tamber/subscribe', method: 'POST', headers: local });
  expect(subscribed.status).toBe(503);
  expect(subscribed.body.error.message).toContain('another tamber server');
  const hello = await ask(second, gate, { path: '/__tamber', method: 'POST', headers: local, body: '{"method":"hello"}' });
  expect(hello.status).toBe(503);
  // A long poll gets the `down` event once, then is told so, never
  // answered empty to ask again at once.
  const first = await ask(second, gate, { path: '/__tamber/events', method: 'POST', headers: local, body: '{"after":0}' });
  expect(first.body.events.map(e => e.event)).toEqual(['down']);
  const polled = await ask(second, gate, { path: '/__tamber/events', method: 'POST', headers: local, body: JSON.stringify({ after: first.body.seq }) });
  expect(polled.status).toBe(503);
  await second.close();
}, 600_000);

test("the dev loop mounts the wire, and the theme fixture's server answers hello", async () => {
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-tamber-dev-'));
  const dev = spawn(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', 'theme-fixture', '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    let out = '';
    let err = '';
    dev.stderr.on('data', d => { err = (err + d).slice(-4000); });
    await new Promise((ok, fail) => {
      dev.stdout.on('data', d => { out += d; if (out.includes(`127.0.0.1:${port}/`)) ok(); });
      dev.on('error', fail);
      dev.on('exit', code => fail(new Error(`the dev loop exited ${code}: ${out}${err}`)));
    });
    const base = `http://127.0.0.1:${port}`;
    const { token } = await (await fetch(`${base}/__tamber/token`)).json();
    const hello = await (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    expect(hello.ok.protocol).toBe('tamber/1.2');
    expect(Object.keys(hello.ok.files)).toContain('app.tamber');
  } finally {
    if (dev.exitCode === null && dev.signalCode === null) {
      const exit = new Promise(ok => dev.once('exit', ok));
      dev.kill('SIGTERM');
      if (!(await Promise.race([exit.then(() => true), new Promise(ok => setTimeout(() => ok(false), 30_000).unref())]))) dev.kill('SIGKILL');
    }
    rmSync(dist, { recursive: true, force: true });
  }
}, 600_000);

// LLP 1122 D9 and D10 on the wire (stage 3 commit 5): who changed a file,
// `commit`'s guard, held proposals.
const gitIn = (...args) => spawnSync('git', ['-c', 'user.email=t@example.test', '-c', 'user.name=t', ...args], { cwd: dir, encoding: 'utf8' });
const head = () => gitIn('rev-parse', 'HEAD').stdout.trim();

test('a conflict names who last changed the file', async () => {
  const seen = { 'app.tamber': disk('app.tamber') };
  const first = await post({ method: 'transaction', client: 'agent', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#151515' }] });
  const stale = await post({ method: 'transaction', client: 'person', files: seen, ops: [{ op: 'set-token', name: 'ink', value: '#161616' }] });
  expect(stale.status).toBe(409);
  expect(stale.error.by).toEqual({ 'app.tamber': 'agent' });
  // A hand edit the watcher has not yet said: the conflict says it.
  writeFileSync(join(dir, 'app.tamber'), DESIGN);
  const again = await post({ method: 'undo', client: 'agent', inverse: first.ok.inverse });
  expect(again.status).toBe(409);
  expect(again.error.by['app.tamber']).toBe('changed on disk');
  const stream = await subscribe();
  await stream.until(e => e.at(-1)?.files?.['app.tamber'] === sha(DESIGN));
  stream.close();
}, 30_000);

test("commit takes the session's files only, keeps other staging, and never pushes", async () => {
  // Every file as HEAD has it, a remote, and something else staged.
  gitIn('checkout', '--', '.');
  const bare = mkdtempSync(join(tmpdir(), 'exact-tamber-remote-'));
  spawnSync('git', ['init', '-q', '--bare', bare]);
  gitIn('remote', 'add', 'origin', bare);
  writeFileSync(join(dir, 'notes.txt'), 'staged elsewhere\n');
  gitIn('add', 'notes.txt');
  // Earlier tests wrote these files through this wire: a session begins
  // with a wire of its own.
  await freshWire();
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  const before = head();
  const first = await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'edge', value: '#161616' }] });
  expect(first.status).toBe(200);
  // A scope read before a further write no longer holds.
  const early = await post({ method: 'commit', client: 'person', dry: true });
  const done = await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'edge', value: '#171717' }] });
  expect(done.status).toBe(200);
  const stale = await post({ method: 'commit', client: 'person', scope: early.ok.scope });
  expect([stale.status, stale.error.stale]).toEqual([409, true]);
  // A dry run: what the commit would take, each edit with its write's
  // number, and nothing written.
  const dry = await post({ method: 'commit', client: 'person', dry: true });
  expect(dry.status, JSON.stringify(dry)).toBe(200);
  expect(dry.ok).toMatchObject({ files: ['app.tamber'], edits: [{ file: 'app.tamber', client: 'agent', tokens: ['edge'], write: first.ok.write }, { file: 'app.tamber', client: 'agent', tokens: ['edge'], write: done.ok.write }] });
  expect(head()).toBe(before);
  const committed = await post({ method: 'commit', client: 'person', message: 'design: a darker edge', scope: dry.ok.scope });
  expect(committed.status, JSON.stringify(committed)).toBe(200);
  expect(committed.ok.files).toEqual(['app.tamber']);
  expect(committed.ok.edits).toEqual(dry.ok.edits);
  expect(gitIn('rev-parse', 'HEAD~1').stdout.trim()).toBe(before);
  const message = gitIn('log', '-1', '--format=%B').stdout;
  expect(message).toContain('design: a darker edge');
  expect(message).toContain('- agent: edge (app.tamber)');
  expect(gitIn('show', '--name-only', '--format=', 'HEAD').stdout.trim()).toBe(`${''}${gitIn('ls-files', '--full-name', 'app.tamber').stdout.trim()}`);
  // notes.txt is still staged, and not in the commit; app.tamber is clean.
  expect(gitIn('diff', '--cached', '--name-only').stdout.trim()).toBe('notes.txt');
  expect(gitIn('status', '--porcelain', 'app.tamber').stdout).toBe('');
  expect(spawnSync('git', ['for-each-ref'], { cwd: bare, encoding: 'utf8' }).stdout).toBe('');
  await stream.until(e => e.some(x => x.event === 'committed'));
  stream.close();
  rmSync(bare, { recursive: true, force: true });
}, 30_000);

test('commit refuses a file dirty at its baseline, and one changed by hand between writes', async () => {
  // brand.tamber differs from HEAD before the session writes it.
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#242424'));
  const stream = await subscribe();
  await stream.until(e => e.at(-1)?.files?.['brand.tamber'] === sha(BRAND.replace('#202020', '#242424')));
  await post({ method: 'transaction', client: 'agent', files: { 'brand.tamber': disk('brand.tamber') }, ops: [{ op: 'set-token', file: 'brand.tamber', name: 'brand.ink', value: '#252525' }] });
  // app.tamber: written, edited by hand, written again.
  const at = head();
  await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'ink', value: '#181818' }] });
  writeFileSync(join(dir, 'app.tamber'), readFileSync(join(dir, 'app.tamber'), 'utf8').replace('#181818', '#191919'));
  await stream.until(e => e.at(-1)?.client === null && e.at(-1)?.edited.includes('app.tamber'));
  await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'ink', value: '#1a1a1a' }] });
  const refused = await post({ method: 'commit', client: 'person' });
  expect(refused.status).toBe(400);
  expect(refused.error.refused.map(r => r.file).sort()).toEqual(['app.tamber', 'brand.tamber']);
  expect(refused.error.refused.find(r => r.file === 'brand.tamber').why).toContain('differed from HEAD');
  expect(refused.error.refused.find(r => r.file === 'app.tamber').why).toContain('someone other than this server');
  expect(head()).toBe(at);
  stream.close();
}, 30_000);

test('a held proposal is accepted, and one gone stale or taken in part is recomputed', async () => {
  gitIn('checkout', '--', 'app.tamber');
  const stream = await subscribe();
  await stream.until(e => e.at(-1)?.files?.['app.tamber'] === disk('app.tamber'));
  const held = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'ink', value: '#1b1b1b' }] });
  expect(held.status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).not.toContain('#1b1b1b');
  await stream.until(e => e.some(x => x.event === 'proposal' && x.proposal === held.ok.proposal));
  const taken = await post({ method: 'accept', client: 'person', proposal: held.ok.proposal });
  expect(taken.status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('ink = #1b1b1b');
  await stream.until(e => e.some(x => x.client === 'agent' && x.acceptedBy === 'person'));
  await stream.until(e => e.some(x => x.event === 'proposal-dropped' && x.proposal === held.ok.proposal && x.reason === 'accepted'));
  expect((await post({ method: 'accept', client: 'person', proposal: held.ok.proposal })).status).toBe(404);
  // Stale: the file moves after review; the accept gives a new diff to accept.
  const second = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'edge', value: '#1c1c1c' }] });
  writeFileSync(join(dir, 'app.tamber'), readFileSync(join(dir, 'app.tamber'), 'utf8').replace('#1b1b1b', '#1d1d1d'));
  await stream.until(e => e.at(-1)?.client === null && e.at(-1)?.edited.includes('app.tamber'));
  const stale = await post({ method: 'accept', client: 'person', proposal: second.ok.proposal });
  expect(stale.ok.recomputed).toBe(true);
  expect(stale.ok.proposal).not.toBe(second.ok.proposal);
  await stream.until(e => e.some(x => x.event === 'proposal-dropped' && x.proposal === second.ok.proposal && x.reason === 'replaced' && x.replacedBy === stale.ok.proposal));
  expect(stream.events.find(x => x.event === 'proposal' && x.proposal === stale.ok.proposal).replaces).toBe(second.ok.proposal);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).not.toContain('#1c1c1c');
  expect((await post({ method: 'accept', client: 'person', proposal: stale.ok.proposal })).status).toBe(200);
  expect(readFileSync(join(dir, 'app.tamber'), 'utf8')).toContain('edge = #1c1c1c');
  // A held proposal checks the files its client requires, as a transaction
  // does, and so does a part of it accepted later.
  const guarded = await post({ method: 'propose', hold: true, client: 'agent', requires: { 'brand.tamber': 'f'.repeat(64) }, ops: [{ op: 'set-token', name: 'ink', value: '#1e1e1e' }] });
  expect(guarded.status).toBe(409);
  const brand = disk('brand.tamber');
  // A `requires` digest and a `files` one for the same file must agree.
  const clash = await post({ method: 'propose', hold: true, client: 'agent', files: { 'brand.tamber': brand }, requires: { 'brand.tamber': 'f'.repeat(64) }, ops: [{ op: 'set-token', name: 'ink', value: '#1e1e1e' }] });
  expect(clash.status).toBe(400);
  const kept = await post({ method: 'propose', hold: true, client: 'agent', requires: { 'brand.tamber': brand }, ops: [{ op: 'set-token', name: 'ink', value: '#1e1e1e' }, { op: 'set-token', name: 'edge', value: '#1f1f1f' }] });
  expect(kept.status).toBe(200);
  const before = readFileSync(join(dir, 'brand.tamber'), 'utf8');
  writeFileSync(join(dir, 'brand.tamber'), `${before}// moved\n`);
  await stream.until(e => e.some(x => x.client === null && x.edited?.includes('brand.tamber') && x.files['brand.tamber'] !== brand));
  expect((await post({ method: 'accept', client: 'person', proposal: kept.ok.proposal, items: [1] })).status).toBe(409);
  writeFileSync(join(dir, 'brand.tamber'), before);
  await stream.until(e => e.at(-1)?.files?.['brand.tamber'] === brand);
  // In part: the one item kept is a new proposal.
  const two = await post({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'ink', value: '#1e1e1e' }, { op: 'set-token', name: 'edge', value: '#1f1f1f' }] });
  const part = await post({ method: 'accept', client: 'person', proposal: two.ok.proposal, items: [1] });
  expect(part.ok.recomputed).toBe(true);
  expect(part.ok.ops).toEqual([{ index: 0, op: { op: 'set-token', name: 'edge', value: '#1f1f1f' } }]);
  expect(part.ok.diff).toContain('#1f1f1f');
  expect(part.ok.diff).not.toContain('#1e1e1e');
  expect((await post({ method: 'drop', proposal: part.ok.proposal })).status).toBe(400);
  expect((await post({ method: 'drop', client: 'person', proposal: part.ok.proposal })).status).toBe(200);
  await stream.until(e => e.some(x => x.event === 'proposal-dropped' && x.proposal === part.ok.proposal && x.client === 'person'));
  stream.close();
}, 30_000);

test('commit refuses a file staged at its baseline, and one whose HEAD moved; a locked index moves nothing', async () => {
  gitIn('checkout', '--', 'app.tamber', 'brand.tamber');
  gitIn('reset', '-q');
  await freshWire();
  // brand.tamber staged (and the working tree back as HEAD has it).
  writeFileSync(join(dir, 'brand.tamber'), BRAND.replace('#202020', '#262626'));
  gitIn('add', 'brand.tamber');
  spawnSync('sh', ['-c', 'git show HEAD:brand.tamber > brand.tamber'], { cwd: dir });
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  await post({ method: 'transaction', client: 'agent', files: { 'brand.tamber': disk('brand.tamber') }, ops: [{ op: 'set-token', file: 'brand.tamber', name: 'brand.ink', value: '#272727' }] });
  const staged = await post({ method: 'commit', client: 'person' });
  expect(staged.error.refused).toEqual([{ file: 'brand.tamber', why: 'it was staged when the session first wrote it' }]);
  // HEAD moves under the session.
  gitIn('reset', '-q');
  spawnSync('sh', ['-c', 'git show HEAD:brand.tamber > brand.tamber'], { cwd: dir });
  await freshWire();
  await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'ink', value: '#282828' }] });
  gitIn('commit', '-q', '--allow-empty', '-m', 'elsewhere');
  const moved = await post({ method: 'commit', client: 'person' });
  expect(moved.error.refused).toEqual([{ file: 'app.tamber', why: 'HEAD has moved since the session first wrote it' }]);
  // A locked index: the commit fails whole, HEAD where it was.
  gitIn('checkout', '--', 'app.tamber');
  await freshWire();
  await post({ method: 'transaction', client: 'agent', files: { 'app.tamber': disk('app.tamber') }, ops: [{ op: 'set-token', name: 'ink', value: '#292929' }] });
  const at = head();
  const lock = join(dir, '.git', 'index.lock');
  writeFileSync(lock, '');
  const locked = await post({ method: 'commit', client: 'person' });
  rmSync(lock);
  expect(locked.status, JSON.stringify(locked)).toBe(400);
  expect(head()).toBe(at);
  const retried = await post({ method: 'commit', client: 'person' });
  expect(retried.status, JSON.stringify(retried)).toBe(200);
  expect(gitIn('status', '--porcelain', 'app.tamber').stdout).toBe('');
  stream.close();
}, 30_000);

test("an import is held as proposals: the tokens as one, each role override apart; again, it proposes nothing", async () => {
  const { importTheme } = await import('./tamber-server.mjs');
  gitIn('checkout', '--', 'app.tamber');
  const stream = await subscribe();
  await stream.until(e => e[0]?.event === 'subscribed');
  const css = resolve(ROOT, 'tamber/tests/import/shadcn-neutral.css');
  const log = console.log;
  const said = [];
  console.log = line => said.push(line);
  try {
    expect(await importTheme(['shadcn', css, origin, '--app', dir])).toBe(0);
  } finally { console.log = log; }
  expect(said[0]).toBe('shadcn: 47 source values, 46 mapped, 1 reported');
  await stream.until(e => e.some(x => x.event === 'proposal' && x.client === 'import:shadcn'));
  const held = stream.events.find(x => x.event === 'proposal' && x.client === 'import:shadcn');
  expect(held.ops.length).toBe(28);
  expect(held.review.tokens).toBe(23);
  expect(held.review.offers.map(o => [o.role, o.items])).toEqual([['-exact-background', [23]], ['-exact-label', [24]], ['-exact-secondary-background', [25]], ['-exact-separator', [26]], ['AccentColor', [27]]]);
  expect(held.review.unmapped.map(u => u.name)).toContain('--radius');
  // The overrides left off: the tokens alone, recomputed, then accepted.
  const part = await post({ method: 'accept', client: 'person', proposal: held.proposal, items: [...Array(23).keys()] });
  expect(part.ok.recomputed).toBe(true);
  // Its review kept, the overrides' items gone with them.
  expect(part.ok.review.unmapped.map(u => u.name)).toContain('--radius');
  expect(part.ok.review.offers.every(o => o.items.length === 0)).toBe(true);
  expect((await post({ method: 'accept', client: 'person', proposal: part.ok.proposal })).status).toBe(200);
  const text = readFileSync(join(dir, 'app.tamber'), 'utf8');
  expect(text).toContain('shadcn.background = light-dark(oklch(1 0 0), oklch(0.145 0 0))');
  expect(text).not.toContain('-exact-background');
  said.length = 0;
  console.log = line => said.push(line);
  try {
    expect(await importTheme(['shadcn', css, origin, '--app', dir])).toBe(0);
  } finally { console.log = log; }
  expect(said).toContain('nothing to propose: the design has every mapped value already');
  stream.close();
}, 120_000);

// LLP 1122 T1's server additions (`tamber/1.1`): `history`, `revert`,
// `import`, and the design tool's files.
test('the design tool is served to a local request only, its base at its path', async () => {
  const port = 8802;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }, { origin: `http://192.168.1.20:${port}` }], port);
  const toolDir = mkdtempSync(join(tmpdir(), 'exact-tamber-tool-'));
  const unbuilt = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, toolDir: join(toolDir, 'none') });
  const local = { host: `127.0.0.1:${port}` };
  const missing = await ask(unbuilt, gate, { path: '/__tamber/tool/', headers: local });
  expect(missing.status).toBe(404);
  expect(missing.body.message).toContain('bun exact.mjs tamber tool');
  unbuilt.close();
  writeFileSync(join(toolDir, 'index.html'), '<!doctype html>\n<base href="/">\n<script type="module" src="./app.js"></script>\n');
  writeFileSync(join(toolDir, 'app.js'), 'export {};\n');
  const tamber = tamberServer({ appDir: '/nowhere', spawnServe: fakeServe, log: () => {}, toolDir });
  const raw = async (path, extra = {}) => {
    const req = Object.assign(new PassThrough(), { method: 'GET', headers: local, socket: { remoteAddress: '127.0.0.1' }, ...extra });
    let status = 0, head = {}, text = '';
    const res = { writeHead(s, h = {}) { status = s; head = h; }, write(t) { text += t; }, end(t) { text += t ?? ''; } };
    req.end();
    await tamber.handle(req, res, new URL(path, 'http://x'), gate.check(req));
    return { status, head, text };
  };
  expect((await raw('/__tamber/tool')).head.location).toBe('/__tamber/tool/');
  const page = await raw('/__tamber/tool/');
  expect(page.status).toBe(200);
  expect(page.head['content-type']).toContain('text/html');
  expect(page.text).toContain('<base href="/__tamber/tool/">');
  expect((await raw('/__tamber/tool/app.js')).head['content-type']).toBe('text/javascript');
  // No dot path, no way out, no write, and nothing to a request from off this machine.
  // A link out of the build, or to a dot file in it, serves nothing.
  writeFileSync(join(toolDir, '.secret'), 'dot\n');
  symlinkSync(join(ROOT, 'package.json'), join(toolDir, 'out.json'));
  symlinkSync(join(toolDir, '.secret'), join(toolDir, 'alias.txt'));
  for (const path of ['/__tamber/tool/.exact-build.json', '/__tamber/tool/a%2f..%2f..%2fsecret', '/__tamber/tool/nope.js', '/__tamber/tool/out.json', '/__tamber/tool/alias.txt']) expect((await raw(path)).status, path).toBe(404);
  expect((await raw('/__tamber/tool/', { method: 'POST' })).status).toBe(404);
  expect((await raw('/__tamber/tool/', { headers: { host: `192.168.1.20:${port}` }, socket: { remoteAddress: '192.168.1.30' } })).status).toBe(404);
  tamber.close();
  rmSync(toolDir, { recursive: true, force: true });
});

test('history reads three design commits as token edits; a revert applies, is recomputed on moved files, and is refused', async () => {
  const V1 = 'tamber t\n\ntheme\n  ink = #000000\n  edge = #101010\n';
  const V2 = V1.replace('#101010', '#202020');
  const V3 = V2.replace('#000000', '#111111') + '  card = #303030\n';
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-history-'));
  const ada = { GIT_AUTHOR_NAME: 'Ada', GIT_AUTHOR_EMAIL: 'ada@example.test', GIT_COMMITTER_NAME: 'Ada', GIT_COMMITTER_EMAIL: 'ada@example.test' };
  const g = (...args) => spawnSync('git', args, { cwd: repo, encoding: 'utf8', env: { ...process.env, ...ada } });
  writeFileSync(join(repo, 'app.contract'), 'component App\n  view\n    text "a"\n');
  g('init', '-q');
  for (const [text, message] of [[V1, 'a design'], [V2, 'a darker edge'], [V3, 'ink and a card']]) {
    writeFileSync(join(repo, 'app.tamber'), text);
    g('add', '.'); g('commit', '-q', '-m', message);
  }
  // A commit that touches no design file is not the design's history.
  writeFileSync(join(repo, 'notes.txt'), 'n\n'); g('add', '.'); g('commit', '-q', '-m', 'notes');
  const sha = rev => g('rev-parse', rev).stdout.trim();
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  try {
    await wire.ready;
    const at = `http://127.0.0.1:${port}`;
    const { token } = await (await fetch(`${at}/__tamber/token`)).json();
    const call = async request => {
      const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) });
      return { status: res.status, ...(await res.json()) };
    };
    const log = await call({ method: 'history' });
    expect(log.status, JSON.stringify(log)).toBe(200);
    expect(log.ok.commits.map(c => c.message.split('\n')[0])).toEqual(['ink and a card', 'a darker edge', 'a design']);
    expect(log.ok.more).toBe(false);
    const [third, second, first] = log.ok.commits;
    expect(second).toMatchObject({ commit: sha('HEAD~2'), author: 'Ada', email: 'ada@example.test' });
    expect(second.files).toHaveLength(1);
    expect(second.files[0]).toMatchObject({ file: 'app.tamber', status: 'M', edits: [{ what: 'token', name: 'edge', when: [], before: '#101010', after: '#202020' }] });
    expect(second.files[0].diff).toContain('+  edge = #202020');
    expect(third.files[0].edits.map(e => [e.name, e.before, e.after])).toEqual([['ink', '#000000', '#111111'], ['card', null, '#303030']]);
    expect(first.files[0].status).toBe('A');
    expect(first.files[0].edits.map(e => [e.what, e.name])).toEqual([['other', 'name'], ['token', 'ink'], ['token', 'edge']]);
    const page = await call({ method: 'history', limit: 1, skip: 1 });
    expect(page.ok.commits.map(c => c.message.split('\n')[0])).toEqual(['a darker edge']);
    expect(page.ok.more).toBe(true);
    expect((await call({ method: 'history', limit: 0 })).status).toBe(400);
    // Refused: the file moved on the commit's own edit.
    const digest = () => createHash('sha256').update(readFileSync(join(repo, 'app.tamber'))).digest('hex');
    const moved = await call({ method: 'transaction', client: 'agent', files: { 'app.tamber': digest() }, ops: [{ op: 'set-token', name: 'edge', value: '#2a2a2a' }] });
    expect(moved.status).toBe(200);
    expect((await call({ method: 'revert', commit: second.commit })).status).toBe(400);
    const refused = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    expect(refused.status).toBe(409);
    expect(refused.error.message).toContain('edge');
    expect(refused.error.refused).toEqual([{ file: 'app.tamber', edit: { what: 'token', name: 'edge', when: [], before: '#101010', after: '#202020' }, now: '#2a2a2a', why: 'changed since' }]);
    // Recomputed: the third commit reverted on a file moved since, the later edit kept.
    const held = await call({ method: 'revert', client: 'design-tool:ada', commit: third.commit });
    expect(held.status, JSON.stringify(held)).toBe(200);
    expect(held.ok.review.revert).toMatchObject({ commit: third.commit, subject: 'ink and a card', moved: ['app.tamber'] });
    expect(held.ok.ops.map(o => o.op.op)).toEqual(['set-token', 'remove-token']);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toContain('card');
    expect((await call({ method: 'accept', client: 'design-tool:ada', proposal: held.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(V1.replace('#101010', '#2a2a2a'));
    // Applied: the edit put back as the commit left it, then reverted.
    await call({ method: 'transaction', client: 'agent', files: { 'app.tamber': digest() }, ops: [{ op: 'set-token', name: 'edge', value: '#202020' }] });
    const again = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    expect(again.status, JSON.stringify(again)).toBe(200);
    expect((await call({ method: 'accept', client: 'design-tool:ada', proposal: again.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(V1);
    const nothing = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    expect(nothing.status).toBe(409);
    expect(nothing.error.message).toContain('nothing to revert');
    expect((await call({ method: 'revert', client: 'design-tool:ada', commit: sha('HEAD') })).error.message).toContain('changed no design file');
    // Held, then the files move: on the revert's own edit, the accept is
    // refused; elsewhere, it is planned again from the commit.
    const setEdge = value => call({ method: 'transaction', client: 'agent', files: { 'app.tamber': digest() }, ops: [{ op: 'set-token', name: 'edge', value }] });
    await setEdge('#202020');
    const h1 = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    await setEdge('#303030');
    const stale = await call({ method: 'accept', client: 'design-tool:ada', proposal: h1.ok.proposal });
    expect(stale.status).toBe(409);
    expect(stale.error.refused[0]).toMatchObject({ edit: { name: 'edge' }, now: '#303030' });
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toContain('edge = #303030');
    await setEdge('#202020');
    const h2 = await call({ method: 'revert', client: 'design-tool:ada', commit: second.commit });
    await call({ method: 'transaction', client: 'agent', files: { 'app.tamber': digest() }, ops: [{ op: 'set-token', name: 'ink', value: '#121212' }] });
    const replanned = await call({ method: 'accept', client: 'design-tool:ada', proposal: h2.ok.proposal });
    expect(replanned.ok?.recomputed, JSON.stringify(replanned)).toBe(true);
    expect(replanned.ok.ops).toEqual([{ index: 0, op: { op: 'set-token', file: 'app.tamber', name: 'edge', when: [], value: '#101010', comment: null } }]);
    expect((await call({ method: 'accept', client: 'design-tool:ada', proposal: replanned.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(V1.replace('#000000', '#121212'));
    expect((await call({ method: 'revert', client: 'design-tool:ada', commit: '--all' })).status).toBe(400);
    expect((await call({ method: 'revert', client: 'design-tool:ada', commit: 'f'.repeat(40) })).status).toBe(404);
    // An import the tool starts from a file's text, held with its review.
    const css = readFileSync(resolve(ROOT, 'tamber/tests/import/shadcn-neutral.css'), 'utf8');
    expect((await call({ method: 'import', kind: 'shadcn', source: css })).status).toBe(400);
    const imported = await call({ method: 'import', client: 'design-tool:ada', kind: 'shadcn', source: css, name: 'globals.css' });
    expect(imported.status, JSON.stringify(imported)).toBe(200);
    expect(imported.ok.client).toBe('design-tool:ada');
    expect(imported.ok.ops).toHaveLength(28);
    expect(imported.ok.review).toMatchObject({ importer: 'shadcn', counted: { source: 47, mapped: 46, reported: 1 }, tokens: 23 });
    expect(imported.ok.review.offers.map(o => o.role)).toEqual(['-exact-background', '-exact-label', '-exact-secondary-background', '-exact-separator', 'AccentColor']);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(V1.replace('#000000', '#121212'));
    // A stylesheet's importer takes one sheet, never silently the first of two.
    const two = await call({ method: 'import', client: 'design-tool:ada', kind: 'shadcn', sources: [{ name: 'a.css', text: css }, { name: 'b.css', text: css }] });
    expect(two.status).toBe(400);
    expect(two.error.message).toContain('imports one stylesheet');
    // Several DTCG files at once: a Figma export's modes, named as picked.
    const figma = resolve(ROOT, 'tamber/tests/import/figma');
    const sources = readdirSync(figma).map(name => ({ name, text: readFileSync(join(figma, name), 'utf8') }));
    const modes = await call({ method: 'import', client: 'design-tool:ada', kind: 'dtcg', sources });
    expect(modes.status, JSON.stringify(modes).slice(0, 400)).toBe(200);
    expect(modes.ok.review.counted).toEqual({ source: 104, mapped: 40, reported: 64 });
    // A design file whose name git quotes still has its diff.
    writeFileSync(join(repo, 'ünï.tamber'), 'tamber u\n\ntheme\n  u.ink = #010101\n');
    g('add', 'ünï.tamber'); g('commit', '-q', '-m', 'a named file');
    const named = await call({ method: 'history', limit: 1 });
    expect(named.ok.commits[0].files).toHaveLength(1);
    expect(named.ok.commits[0].files[0]).toMatchObject({ file: 'ünï.tamber', status: 'A' });
    expect(named.ok.commits[0].files[0].diff).toContain('+  u.ink = #010101');
    const astryx = await call({ method: 'import', client: 'design-tool:ada', kind: 'astryx', source: '{}' });
    expect(astryx.status).toBe(400);
    expect(astryx.error.message).toContain('bun exact.mjs tamber import astryx');
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 600_000);

test("a held revert keeps its untouched files' digests and a person's selection when it is planned again", async () => {
  const A1 = 'tamber t\n\ntheme\n  ink = #000000\n  edge = #101010\n';
  const B1 = 'tamber b\n\ntheme\n  b.ink = #010101\n';
  const repo = mkdtempSync(join(tmpdir(), 'exact-tamber-revert-'));
  const env = { ...process.env, GIT_AUTHOR_NAME: 'Ada', GIT_AUTHOR_EMAIL: 'ada@example.test', GIT_COMMITTER_NAME: 'Ada', GIT_COMMITTER_EMAIL: 'ada@example.test' };
  const g = (...args) => spawnSync('git', args, { cwd: repo, encoding: 'utf8', env });
  writeFileSync(join(repo, 'app.contract'), 'component App\n  view\n    text "a"\n');
  g('init', '-q');
  for (const [a, b] of [[A1, B1], [A1.replace('#000000', '#222222').replace('#101010', '#202020'), B1.replace('#010101', '#020202')]]) {
    writeFileSync(join(repo, 'app.tamber'), a); writeFileSync(join(repo, 'b.tamber'), b);
    g('add', '.'); g('commit', '-q', '-m', 'design');
  }
  const commit = g('rev-parse', 'HEAD').stdout.trim();
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  const gate = developmentGate([{ origin: `http://127.0.0.1:${port}` }], port);
  const wire = tamberServer({ appDir: repo, log: () => {}, settle: 250 });
  listener.on('request', async (req, res) => {
    if (!(await wire.handle(req, res, new URL(req.url, 'http://x'), gate.check(req)))) { res.writeHead(404); res.end(); }
  });
  try {
    await wire.ready;
    const at = `http://127.0.0.1:${port}`;
    const { token } = await (await fetch(`${at}/__tamber/token`)).json();
    const call = async request => {
      const res = await fetch(`${at}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: JSON.stringify(request) });
      return { status: res.status, ...(await res.json()) };
    };
    const digest = f => createHash('sha256').update(readFileSync(join(repo, f))).digest('hex');
    const set = (file, name, value) => call({ method: 'transaction', client: 'agent', files: { [file]: digest(file) }, ops: [{ op: 'set-token', file, name, value }] });
    // b.tamber already as before: no operation there, but its digest guards the plan.
    await set('b.tamber', 'b.ink', '#010101');
    const held = await call({ method: 'revert', client: 'ada', commit });
    expect(held.ok.ops.map(o => o.op.file)).toEqual(['app.tamber', 'app.tamber']);
    await set('b.tamber', 'b.ink', '#030303');
    const stale = await call({ method: 'accept', client: 'ada', proposal: held.ok.proposal });
    expect(stale.status, JSON.stringify(stale)).toBe(409);
    expect(stale.error.refused[0]).toMatchObject({ file: 'b.tamber', edit: { name: 'b.ink' }, now: '#030303' });
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toContain('ink = #222222');
    // Only `ink` kept; then the file moves on the item left out: planned
    // again, it is still only `ink`.
    await set('b.tamber', 'b.ink', '#010101');
    const two = await call({ method: 'revert', client: 'ada', commit });
    const inkAt = two.ok.ops.findIndex(o => o.op.name === 'ink');
    const part = await call({ method: 'accept', client: 'ada', proposal: two.ok.proposal, items: [inkAt] });
    expect(part.ok.ops.map(o => o.op.name)).toEqual(['ink']);
    await set('app.tamber', 'edge', '#2b2b2b');
    const again = await call({ method: 'accept', client: 'ada', proposal: part.ok.proposal });
    expect(again.ok?.recomputed, JSON.stringify(again)).toBe(true);
    expect(again.ok.ops.map(o => o.op.name)).toEqual(['ink']);
    expect((await call({ method: 'accept', client: 'ada', proposal: again.ok.proposal })).status).toBe(200);
    expect(readFileSync(join(repo, 'app.tamber'), 'utf8')).toBe(A1.replace('#101010', '#2b2b2b'));
    // An axis's values are each their own item: only `dim` kept stays only `dim`.
    const AX = v => `tamber b\n\naxes\n  tone = ${v}\n\ntheme\n  b.ink = #010101\n`;
    for (const v of ['black | dim | navy', 'black']) { writeFileSync(join(repo, 'b.tamber'), AX(v)); g('add', 'b.tamber'); g('commit', '-q', '-m', `tone ${v}`); }
    // The server reads the checkout.
    for (let i = 0; i < 100 && !(await call({ method: 'hello' })).ok.files['b.tamber']?.startsWith(digest('b.tamber').slice(0, 8)); i++) await Bun.sleep(50);
    const axes = await call({ method: 'revert', client: 'ada', commit: g('rev-parse', 'HEAD').stdout.trim() });
    expect(axes.ok.ops.map(o => o.op.value)).toEqual(['dim', 'navy']);
    const dimOnly = await call({ method: 'accept', client: 'ada', proposal: axes.ok.proposal, items: [0] });
    await set('b.tamber', 'b.ink', '#040404');
    const again2 = await call({ method: 'accept', client: 'ada', proposal: dimOnly.ok.proposal });
    expect(again2.ok?.ops.map(o => o.op.value), JSON.stringify(again2)).toEqual(['dim']);
    // A client's own review cannot pose as a revert.
    expect((await call({ method: 'propose', hold: true, client: 'agent', ops: [{ op: 'set-token', name: 'ink', value: '#090909' }], review: { revert: { commit, only: {} } } })).status).toBe(400);
  } finally {
    await wire.close();
    listener.close();
    rmSync(repo, { recursive: true, force: true });
  }
}, 600_000);
