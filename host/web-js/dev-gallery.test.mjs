// The variant gallery on the JS dev loop (host/web-js/dev-gallery.mjs, the
// design tool's G3): nothing is made until a page asks, through the loop's
// gate; then `contract gallery`'s source is built into its own dist and
// served at /__dev/gallery/ with its own stream, which registers with
// `page: 'gallery'`; a kept design edit reaches the app as a batch and the
// gallery as a build, neither stream carrying the other's frames; the
// gallery stops a while after its last page goes, and with the loop.
// On a copy of apps/library-gate under the repo's target (its packages are
// the repo's), with an id of its own, so the gallery's cache is its own.
import { afterEach, expect, test } from 'bun:test';
import { spawnOwned, stopAll } from '../../scripts/test-children.mjs';
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer, request } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { galleryCache } from './dev-gallery.mjs';
import { FRAME_ANCESTORS } from '../../scripts/tamber-server.mjs';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
afterEach(() => stopAll());

/** A copy of library-gate with id `id`, under the repo's target. */
function copy(id) {
  const dir = resolve(ROOT, 'target', `dev-gallery-test-${process.pid}`, 'app');
  rmSync(dir, { recursive: true, force: true });
  for (const f of ['app.contract', 'app.tamber']) cpSync(resolve(ROOT, 'apps/library-gate', f), join(dir, f), { recursive: true });
  // An asset of the app's: the gallery's are the app's.
  mkdirSync(join(dir, 'assets'), { recursive: true });
  writeFileSync(join(dir, 'assets', 'note.txt'), 'the app\'s asset\n');
  const manifest = JSON.parse(readFileSync(resolve(ROOT, 'apps/library-gate/app.json'), 'utf8'));
  manifest.app.id = id; manifest.id = id;
  writeFileSync(join(dir, 'app.json'), JSON.stringify(manifest, null, 2));
  return dir;
}

async function devLoop(dir, env = {}) {
  const listener = createServer();
  await new Promise((ok) => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise((ok) => listener.close(ok));
  const dist = mkdtempSync(join(tmpdir(), 'exact-dev-gallery-'));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', dir, '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  dev.stderr.on('data', (d) => { out = (out + d).slice(-20000); });
  await new Promise((ok, fail) => {
    dev.stdout.on('data', (d) => { out = (out + d).slice(-20000); if (out.includes('plan ready') && out.includes(`127.0.0.1:${port}/`)) ok(); });
    dev.on('exit', (code) => fail(new Error(`the dev loop exited ${code}: ${out}`)));
  });
  return { base: `http://127.0.0.1:${port}`, port, dev, said: () => out, stop: async () => { await dev.stop(30_000); rmSync(dist, { recursive: true, force: true }); } };
}

/** An SSE stream's frames, as they come; `close` ends it. */
function frames(url) {
  const got = [];
  let buffered = '';
  const req = request(url, (res) => res.on('data', (d) => {
    buffered += d;
    const parts = buffered.split('\n\n');
    buffered = parts.pop();
    for (const p of parts) for (const line of p.split('\n')) if (line.startsWith('data: ')) got.push(JSON.parse(line.slice(6)));
  }));
  req.on('error', () => {});
  req.end();
  return { got, close: () => req.destroy() };
}

const until = async (what, ms = 300_000) => {
  const end = Date.now() + ms;
  while (!what()) { if (Date.now() > end) throw new Error('timed out'); await Bun.sleep(100); }
};

test('the gallery: made only once a page asks through the gate, its own build, stream and registration; a kept design edit is a batch to the app and a build to the gallery, never crossing; stopped when idle and with the loop', async () => {
  const id = `com.exact.dev-gallery-test-${process.pid}`;
  const dir = copy(id);
  const cache = galleryCache({ dir, id }, ROOT);
  rmSync(cache, { recursive: true, force: true });
  const loop = await devLoop(dir, { EXACT_DEV_GALLERY_IDLE_MS: '3000' });
  try {
    // A refused origin starts nothing.
    const refused = await new Promise((ok) => request({ host: '127.0.0.1', port: loop.port, path: '/__dev/gallery/', headers: { host: 'evil.example' } }, (res) => { res.resume(); ok(res.statusCode); }).end());
    expect(refused).toBe(421);
    await Bun.sleep(500);
    expect(existsSync(join(cache, 'live/app.contract'))).toBe(false);
    // The first page waits on the gallery's stream while it builds.
    expect(await (await fetch(`${loop.base}/__dev/gallery/`)).text()).toContain('Building the gallery');
    const gallery = frames(`${loop.base}/__dev/gallery/stream`);
    await until(() => gallery.got.some((f) => f.reload >= 1));
    const nonce = gallery.got[0].nonce;
    expect(typeof nonce).toBe('string');
    expect(existsSync(join(cache, 'live/app.contract'))).toBe(true);
    // Its page: its files under its path, its own dev client.
    const page = await (await fetch(`${loop.base}/__dev/gallery/`)).text();
    expect(page).toContain('<base href="/__dev/gallery/">');
    // Framed only by the design tool's origin and its own, as every page of the loop (frame-ancestors).
    expect((await fetch(`${loop.base}/__dev/gallery/`)).headers.get('content-security-policy')).toBe(FRAME_ANCESTORS);
    expect(page).toContain("new EventSource('/__dev/gallery/stream')");
    expect(page).toContain("page:'gallery'");
    expect(page).not.toContain("'navigate']");
    const script = /src="\.\/([^"]+\.js)"/.exec(page)?.[1];
    expect((await fetch(`${loop.base}/__dev/gallery/${script}`)).status).toBe(200);
    expect(await (await fetch(`${loop.base}/__dev/gallery/assets/note.txt`)).text()).toBe("the app's asset\n");
    // Its registration, listed as it said it.
    const reg = await fetch(`${loop.base}/__dev/report`, { method: 'POST', body: JSON.stringify({ nonce, kind: 'register', id: 'web-js-gallery-test', host: 'web-js', capabilities: ['pin', 'inspect', 'hold'], plan: null, page: 'gallery' }) });
    expect(reg.status).toBeLessThan(300);
    const { token } = await (await fetch(`${loop.base}/__tamber/token`)).json();
    const hello = (await (await fetch(`${loop.base}/__tamber`, { method: 'POST', headers: { 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json()).ok;
    expect(hello.builds.find((b) => b.id === 'web-js-gallery-test')?.page).toBe('gallery');
    // A kept design edit: a batch to the app, a build to the gallery; neither stream carries the other's.
    const app = frames(`${loop.base}/__dev/page`);
    await until(() => app.got.length > 0);
    const [appBefore, galleryBefore] = [app.got.length, gallery.got.length];
    const builtBefore = Math.max(...gallery.got.map((f) => f.reload ?? 0));
    const tamber = readFileSync(join(dir, 'app.tamber'), 'utf8');
    writeFileSync(join(dir, 'app.tamber'), tamber.replace('#ffffff', '#fefefe'));
    await until(() => app.got.slice(appBefore).some((f) => f.kind === 'tamber-batch'));
    await until(() => gallery.got.slice(galleryBefore).some((f) => f.reload > builtBefore));
    expect(app.got.slice(appBefore).filter((f) => f.reload !== undefined || f.error)).toEqual([]);
    expect(gallery.got.slice(galleryBefore).filter((f) => f.kind === 'tamber-batch' || f.error)).toEqual([]);
    app.close();
    // Its last page gone: its build stopped a while after (EXACT_DEV_GALLERY_IDLE_MS).
    gallery.close();
    await until(() => loop.said().includes('gallery: no page for a while'), 30_000);
  } finally {
    await loop.stop();
  }
  rmSync(resolve(dir, '..'), { recursive: true, force: true });
  rmSync(cache, { recursive: true, force: true });
}, 900_000);

/** The gallery's processes running now (its generator or its build), by pid. */
const galleryProcesses = (cache) => spawnSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' }).stdout.split('\n')
  .filter((l) => l.includes(cache) && !l.includes('ps -axo')).map((l) => Number(l.trim().split(/\s+/)[0])).filter(Boolean);
const alive = (pid) => { try { process.kill(pid, 0); return true; } catch { return false; } };

test("a gallery that can't be made says so on its stream only, and is made once it can be; the loop interrupted mid-build leaves none of its processes", async () => {
  const id = `com.exact.dev-gallery-fail-${process.pid}`;
  const dir = copy(id);
  const cache = galleryCache({ dir, id }, ROOT);
  rmSync(cache, { recursive: true, force: true });
  // Its source's directory a file: `contract gallery` refuses.
  mkdirSync(cache, { recursive: true });
  writeFileSync(join(cache, 'source'), 'not a directory\n');
  const loop = await devLoop(dir);
  try {
    const app = frames(`${loop.base}/__dev/page`);
    const gallery = frames(`${loop.base}/__dev/gallery/stream`);
    await until(() => gallery.got.some((f) => f.error));
    expect(app.got.filter((f) => f.error)).toEqual([]);
    // A page asking again tries again (and fails again, said again, still on its stream only).
    const errors = gallery.got.filter((f) => f.error).length;
    await fetch(`${loop.base}/__dev/gallery/`);
    await until(() => gallery.got.filter((f) => f.error).length > errors);
    expect(app.got.filter((f) => f.error)).toEqual([]);
    // Fixed: the next page asks again, and it is made.
    rmSync(join(cache, 'source'), { force: true });
    await fetch(`${loop.base}/__dev/gallery/`);
    await until(() => gallery.got.some((f) => f.reload >= 1));
    gallery.close(); app.close();
    // A build under way (an edit makes it again), and the loop alone interrupted: its stop takes the gallery's with it.
    const again = frames(`${loop.base}/__dev/gallery/stream`);
    writeFileSync(join(dir, 'app.contract'), readFileSync(join(dir, 'app.contract'), 'utf8') + '\n');
    let seen = [];
    await until(() => (seen = galleryProcesses(cache)).length > 0, 120_000);
    loop.dev.kill('SIGINT');
    await new Promise((ok) => (loop.dev.exitCode !== null ? ok() : loop.dev.once('exit', ok)));
    again.close();
    expect(seen.filter(alive)).toEqual([]);
  } finally {
    await loop.stop();
  }
  rmSync(resolve(dir, '..'), { recursive: true, force: true });
  rmSync(cache, { recursive: true, force: true });
}, 900_000);

test("the gallery is told of a kept design only after the app's own publication: each call deferred past the batch", () => {
  const s = readFileSync(resolve(ROOT, 'host/web-js/dev.mjs'), 'utf8');
  const calls = s.split('\n').filter((l) => l.includes('gallery.designKept('));
  expect(calls.length).toBe(3);
  for (const l of calls) expect(l).toContain('setImmediate(() => gallery.designKept(');
  // A reload's: after the push.
  expect(s).toContain('push({ reload: seq, revision: logicRevision }); gallery.appChanged();');
});

/** A stand-in root and target: a `contract` that writes the gallery's source (or waits, holding a child of its
 * own), and a builder that waits holding one; each records its pid and its child's. */
function standIns(stall) {
  const dir = mkdtempSync(join(tmpdir(), 'exact-gallery-life-'));
  const pids = join(dir, 'pids');
  writeFileSync(pids, '');
  const hold = `sleep 30 & echo $! >> ${pids}; echo $$ >> ${pids};`;
  mkdirSync(join(dir, 'target/debug'), { recursive: true });
  writeFileSync(join(dir, 'src.rs'), '');
  writeFileSync(join(dir, 'target/debug/contract'), `#!/bin/sh\n${stall === 'generate' ? `${hold} sleep 30` : 'mkdir -p "$4"; echo "// Generated" > "$4/app.contract"; echo "{}" > "$4/app.json"; echo made'}\n`, { mode: 0o755 });
  writeFileSync(join(dir, 'target/debug/contract.d'), `${join(dir, 'target/debug/contract')}: ${join(dir, 'src.rs')}\n`);
  mkdirSync(join(dir, 'root/host/web-js'), { recursive: true });
  writeFileSync(join(dir, 'root/host/web-js/build.mjs'), `const { spawn } = require('node:child_process'); const fs = require('node:fs'); const c = spawn('sleep', ['30']); fs.appendFileSync(${JSON.stringify(pids)}, c.pid + '\\n' + process.pid + '\\n'); setTimeout(() => {}, 30000);\n`);
  mkdirSync(join(dir, 'app'), { recursive: true });
  return { dir, pids: () => readFileSync(pids, 'utf8').split('\n').filter(Boolean).map(Number) };
}
const fakeRes = () => { const handlers = {}; return { writeHead() {}, write() {}, end() {}, on: (e, f) => { handlers[e] = f; }, close: () => handlers.close?.() }; };
const fakeReq = () => { const handlers = {}; return { method: 'GET', headers: {}, on: (e, f) => { handlers[e] = f; }, close: () => handlers.close?.() }; };

test('every process the gallery starts, a child of its generator or its builder included, stops with it: at stop, at either stage, and when its last page has been gone a while', async () => {
  const { galleryLoop } = await import('./dev-gallery.mjs');
  // ('build, res only': its page gone with only the response's close, as under Bun on a handed-off socket.)
  for (const [stage, by] of [['generate', null], ['build', 'req'], ['build', 'res']]) {
    const s = standIns(stage);
    const loop = galleryLoop({ app: { dir: join(s.dir, 'app'), id: `life-${stage}` }, root: join(s.dir, 'root'), appDist: join(s.dir, 'dist'), env: { ...process.env, CARGO_TARGET_DIR: join(s.dir, 'target') }, client: () => '', log: () => {}, idleMs: 200 });
    rmSync(loop.cache, { recursive: true, force: true });
    const req = fakeReq(), res = fakeRes();
    await loop.handle(req, res, new URL('http://x/__dev/gallery/stream'));
    await until(() => s.pids().length >= 2, 30_000);
    const started = s.pids();
    expect(started.every(alive)).toBe(true);
    if (stage === 'generate') await loop.stop();
    // Its last page gone: after the quiet period, its processes stopped with it.
    else { (by === 'req' ? req : res).close(); await until(() => !started.some(alive), 10_000); }
    await until(() => !started.some(alive), 10_000);
    expect(started.filter(alive)).toEqual([]);
    rmSync(loop.cache, { recursive: true, force: true });
    rmSync(s.dir, { recursive: true, force: true });
  }
}, 120_000);

test("an inspect table asked with no page of the gallery open is a process of the gallery's, stopped with it after the quiet period", async () => {
  const { galleryLoop } = await import('./dev-gallery.mjs');
  const s = standIns('generate');
  const pids = join(s.dir, 'table-pids');
  writeFileSync(pids, '');
  writeFileSync(join(s.dir, 'table'), `#!/bin/sh\nsleep 30 & echo $! >> ${pids}; echo $$ >> ${pids}; sleep 30\n`, { mode: 0o755 });
  // (A quiet period longer than a loaded machine takes from the table writing its pids to the check that they run:
  // at 200 ms the gallery could already have stopped them, 1 run in 5 on main.)
  const loop = galleryLoop({ app: { dir: join(s.dir, 'app'), id: 'life-table' }, root: join(s.dir, 'root'), appDist: join(s.dir, 'dist'), env: { ...process.env, CARGO_TARGET_DIR: join(s.dir, 'target') }, client: () => '', log: () => {}, idleMs: 3000, tableTool: () => ({ cmd: join(s.dir, 'table'), pre: [] }) });
  rmSync(loop.cache, { recursive: true, force: true });
  mkdirSync(join(loop.cache, 'dist'), { recursive: true });
  writeFileSync(join(loop.cache, 'dist', 'app.plan'), 'a plan');
  void loop.handle({ method: 'GET', headers: {}, on() {} }, fakeRes(), new URL('http://x/__dev/gallery/dev-inspect.json'));
  const started = () => readFileSync(pids, 'utf8').split('\n').filter(Boolean).map(Number);
  await until(() => started().length >= 2, 30_000);
  expect(started().every(alive)).toBe(true);
  await until(() => !started().some(alive), 10_000);
  rmSync(loop.cache, { recursive: true, force: true });
  rmSync(s.dir, { recursive: true, force: true });
}, 120_000);
