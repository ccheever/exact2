// LLP 1069.010.000 D4–D7 on the web: a document an app granted `doc.keep`
// chose is its FileSystemHandle kept in IndexedDB, so after a reload it
// lists and reads at the same path; a permission the browser withholds
// refuses `EACCES` until `requestPermission` asks again under a press; and
// a forgotten one is gone after the next reload. In Chrome, over a file in
// the origin-private file system, whose handles are real FileSystemHandles;
// the browser's permission is stood in for, since it grants those always.
import { expect } from 'bun:test';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { cdpBrowser, chromium } from '../../scripts/agent-launch.mjs';
import { need } from '../../scripts/coverage.mjs';
const { unavailable } = chromium(), chromeTest = need('chrome', unavailable);

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
function normalized(spec) {
  const built = spawnSync('cargo', ['build', '-q', '-p', 'exact-web-js'], { cwd: ROOT, encoding: 'utf8' });
  if (built.status !== 0) throw new Error(built.stderr || 'grant normalizer build failed');
  const normalizer = resolve(process.env.CARGO_TARGET_DIR || resolve(ROOT, 'target'), `debug/exact-web-js${process.platform === 'win32' ? '.exe' : ''}`);
  const scratch = mkdtempSync(resolve(tmpdir(), 'exact-grants-')), input = resolve(scratch, 'grants.txt');
  writeFileSync(input, spec);
  const result = spawnSync(normalizer, ['normalize-grants', input], { cwd: ROOT, encoding: 'utf8' });
  rmSync(scratch, { recursive: true });
  if (result.status !== 0) throw new Error(result.stderr || 'grant normalizer failed');
  return JSON.parse(result.stdout);
}

// A page's start: the glue, keeping for one app, the browser's permission
// stood in for (`withheld` until a press asks), and a button whose press
// asks, as a `requestPermission` command does.
const start = (grants, app) => `(async () => {
  globalThis.exact = {};
  await import('/documents-glue.js');
  globalThis.grants = ${JSON.stringify(grants)};
  globalThis.withheld = false;
  const query = FileSystemHandle.prototype.queryPermission;
  FileSystemHandle.prototype.queryPermission = function (o) { return withheld ? Promise.resolve('prompt') : query ? query.call(this, o) : Promise.resolve('granted'); };
  FileSystemHandle.prototype.requestPermission = function () {
    if (!navigator.userActivation.isActive) return Promise.reject(new DOMException('needs a press', 'SecurityError'));
    withheld = false; return Promise.resolve('granted');
  };
  await exact.documents.keeping(${JSON.stringify(app)}, grants);
  globalThis.host = exact.documents.install({ generation: () => 0, dispatch: (_, kind, payload) => { globalThis.chosen = payload; }, log() {}, openFile: () => false });
  globalThis.files = exact.documents.files(grants);
  globalThis.read = (path) => files.readFile(path).then((b) => new TextDecoder().decode(b), (e) => e.code);
  globalThis.listed = () => exact.documents.recent(grants).then((l) => l.map((d) => [d.path, d.kind, d.mode].join(' ')));
  const button = document.body.appendChild(Object.assign(document.createElement('button'), { textContent: 'ask' }));
  Object.assign(button.style, { position: 'fixed', left: '0', top: '0', width: '100px', height: '100px' });
  button.onclick = () => exact.documents.permission(globalThis.asking, 'read', (v) => { globalThis.answered = v; });
  return 'ready';
})()`;

chromeTest('a kept document reopens after a reload, asks again under a press, and is forgotten', async () => {
  const grants = normalized('fs.read doc:/\nfs.write doc:/\ndoc.keep'), app = `test.kept-${Date.now()}`;
  const dir = resolve(ROOT, 'host/web');
  const page = Bun.serve({ port: 0, fetch(request) {
    const name = decodeURIComponent(new URL(request.url).pathname.slice(1)), file = resolve(dir, name);
    if (!name) return new Response('<!doctype html><title>kept</title><body></body>', { headers: { 'content-type': 'text/html' } });
    if (!file.startsWith(dir + sep) || !existsSync(file)) return new Response('not found', { status: 404 });
    return new Response(Bun.file(file), { headers: { 'content-type': 'text/javascript' } });
  } });
  let suite;
  try {
    suite = await cdpBrowser();
    const { cdp } = suite, { sessionId } = await suite.page();
    const call = (method, params = {}) => cdp.send(method, params, sessionId);
    const run = async (expression) => {
      const out = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (out.exceptionDetails) throw new Error(JSON.stringify(out.exceptionDetails));
      return out.result.value;
    };
    const launch = async () => {
      await call('Page.navigate', { url: page.url.href });
      for (let i = 0; i < 100 && await run('document.readyState') !== 'complete'; i++) await new Promise((r) => setTimeout(r, 50));
      expect(await run(start(grants, app))).toBe('ready');
    };
    const press = async () => {
      for (const type of ['mousePressed', 'mouseReleased']) await call('Input.dispatchMouseEvent', { type, x: 50, y: 50, button: 'left', clickCount: 1 });
      for (let i = 0; i < 100 && await run('globalThis.answered ?? null') === null; i++) await new Promise((r) => setTimeout(r, 20));
      return run('globalThis.answered');
    };
    await call('Page.enable');
    await launch();
    // Chosen through the open picker (the browser's, given an origin-private file).
    const doc = await run(`(async () => {
      const root = await navigator.storage.getDirectory(), file = await root.getFileHandle('notes.md', { create: true });
      const w = await file.createWritable(); await w.write('kept text'); await w.close();
      globalThis.showOpenFilePicker = async () => [file];
      await host.show({ present: true, view: 1 }, 'showOpenFilePicker');
      return chosen;
    })()`);
    expect(doc).toMatch(/^doc:\/\d+\/notes\.md$/);
    expect(await run('listed()')).toEqual([`${doc} file readwrite`]);
    // Chosen again: the same entry.
    expect(await run('host.show({ present: true, view: 1 }, "showOpenFilePicker").then(() => chosen)')).toBe(doc);

    await launch();
    expect(await run('listed()')).toEqual([`${doc} file readwrite`]);
    expect(await run(`read(${JSON.stringify(doc)})`)).toBe('kept text');
    // A handle minted now is numbered above every one an earlier launch gave.
    const fresh = await run(`navigator.storage.getDirectory().then((r) => r.getFileHandle('other.md', { create: true })).then((h) => exact.documents.mint(h))`);
    expect(Number(fresh.split('/')[1])).toBeGreaterThan(Number(doc.split('/')[1]));

    // The browser withholds the permission after a reload: EACCES, and asked
    // again without a press it stays denied; a press grants it.
    await launch();
    await run(`withheld = true; asking = ${JSON.stringify(doc)}`);
    expect(await run(`read(${JSON.stringify(doc)})`)).toBe('EACCES');
    expect(await run(`new Promise((ok) => exact.documents.permission(asking, 'read', ok))`)).toBe('denied');
    expect(await press()).toBe('granted');
    expect(await run(`read(${JSON.stringify(doc)})`)).toBe('kept text');

    // Forgotten: off the list now, still read this launch, gone at the next.
    await run(`exact.documents.forget(${JSON.stringify(doc)}, grants)`);
    expect(await run('listed()')).toEqual([]);
    expect(await run(`exact.documents.forget(${JSON.stringify(doc)}, grants).then(() => 'forgot', (e) => e.code)`)).toBe('ENOENT');
    expect(await run(`read(${JSON.stringify(doc)})`)).toBe('kept text');
    await launch();
    expect(await run('listed()')).toEqual([]);
    expect(await run(`read(${JSON.stringify(doc)})`)).toBe('ENOENT');

    // A kept folder: beneath it, a withheld permission is EACCES too.
    const folder = await run(`(async () => {
      const root = await navigator.storage.getDirectory(), dir = await root.getDirectoryHandle('notes', { create: true });
      const sub = await dir.getDirectoryHandle('sub', { create: true }), f = await sub.getFileHandle('deep.md', { create: true });
      const w = await f.createWritable(); await w.write('deep'); await w.close();
      globalThis.showDirectoryPicker = async () => dir;
      await host.show({ present: true, view: 1 }, 'showDirectoryPicker');
      return chosen;
    })()`);
    await launch();
    expect(await run(`read(${JSON.stringify(`${folder}/sub/deep.md`)})`)).toBe('deep');
    await run('withheld = true');
    expect(await run(`read(${JSON.stringify(`${folder}/sub/deep.md`)})`)).toBe('EACCES');
    await run('withheld = false');
    // A kept file found gone by a read is forgotten.
    const lost = await run(`(async () => {
      const root = await navigator.storage.getDirectory(), f = await root.getFileHandle('lost.md', { create: true });
      globalThis.showOpenFilePicker = async () => [f];
      await host.show({ present: true, view: 1 }, 'showOpenFilePicker');
      await root.removeEntry('lost.md');
      return chosen;
    })()`);
    expect(await run(`read(${JSON.stringify(lost)})`)).toBe('ENOENT');
    expect(await run('listed()')).toEqual([`${folder} directory readwrite`]);
    // Chosen together, past the bound: the newest 20 stay, and one chosen twice is one entry.
    await run(`(async () => {
      const root = await navigator.storage.getDirectory(), many = [];
      for (let i = 0; i < 24; i++) many.push(await root.getFileHandle('many-' + i + '.md', { create: true }));
      globalThis.showOpenFilePicker = async () => [...many, many[0]];
      await host.show({ present: true, view: 1 }, 'showOpenFilePicker');
    })()`);
    const kept = await run('listed()');
    expect(kept.length).toBe(20);
    expect(new Set(kept).size).toBe(20);
    // Numbers are never given past the block reserved: a run of synchronous
    // mints stops, and the next block, once reserved, gives more.
    expect(await run(`(() => { let given = 0; for (let i = 0; i < 600; i++) if (exact.documents.mint({ kind: 'file', name: 'x' + i })) given++; return given < 600; })()`)).toBe(true);
    expect(await run(`new Promise((ok) => setTimeout(ok, 100)).then(() => exact.documents.mint({ kind: 'file', name: 'later' }))`)).toMatch(/^doc:\/\d+\/later$/);
    // Without `doc.keep`, both calls are refused.
    const plain = JSON.stringify(normalized('fs.read doc:/'));
    expect(await run(`exact.documents.recent(${plain}).then(() => 'listed', (e) => e.message)`)).toMatch(/need the doc\.keep grant/);
  } finally {
    await suite?.close();
    page.stop(true);
  }
}, 120_000);
