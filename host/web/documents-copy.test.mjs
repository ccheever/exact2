// LLP 1069.010 D1 (ruled 2026-10-26): `copyFile` takes a document the person
// chose into an app file on the web, as ibex2's `copy_document` does on
// Hermes: `fs.read` on the document, `fs.write` on the app file, admitted
// before the document is opened, and the document's File, never its bytes,
// handed to the app's store.
import { expect, test } from 'bun:test';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { cdpBrowser, chromium } from '../../scripts/agent-launch.mjs';
import { need } from '../../scripts/coverage.mjs';
const { unavailable } = chromium(), chromeTest = need('chrome', unavailable);

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const built = spawnSync('cargo', ['build', '-q', '-p', 'exact-web-js'], { cwd: ROOT, encoding: 'utf8' });
if (built.status !== 0) throw new Error(built.stderr || 'grant normalizer build failed');
const normalizer = resolve(process.env.CARGO_TARGET_DIR || resolve(ROOT, 'target'), `debug/exact-web-js${process.platform === 'win32' ? '.exe' : ''}`);
function normalized(spec) {
  const scratch = mkdtempSync(resolve(tmpdir(), 'exact-grants-')), input = resolve(scratch, 'grants.txt');
  writeFileSync(input, spec);
  const result = spawnSync(normalizer, ['normalize-grants', input], { cwd: ROOT, encoding: 'utf8' });
  rmSync(scratch, { recursive: true });
  if (result.status !== 0) throw new Error(result.stderr || 'grant normalizer failed');
  return JSON.parse(result.stdout);
}
const FILES = [{ path: 'a.png', bytes: btoa('PNG!') }, { path: 'sub/b.txt', bytes: btoa('B') }];

test('a chosen file is handed to the app store as its File, under both grants', async () => {
  globalThis.exact = {};
  try {
    await import(`./documents-glue.js?copy=${Date.now()}`);
    let chosen;
    const host = globalThis.exact.documents.install({ generation: () => 0, dispatch: (_, kind, payload) => { chosen = payload; }, log() {}, openFile: () => false });
    host.answer({ node: 1 }, 'showDirectoryPicker', [{ name: 'chosen', files: FILES }]);
    const doc = chosen, { copyFile, files } = globalThis.exact.documents;
    const both = normalized('fs.read doc:/\nfs.write doc:/\nfs.write app:/data');
    // A stand-in for the app's file system: what it is handed, and whether it opened the document.
    let opened = 0, stored = null;
    const app = { async copyIn(to, open) { opened++; stored = [to, await open()]; } };
    await copyFile(`${doc}/a.png`, 'app:/data/a.png', both, app);
    expect(stored[0]).toBe('app:/data/a.png');
    expect([stored[1] instanceof File, stored[1].name, await stored[1].text()]).toEqual([true, 'a.png', 'PNG!']);
    const code = (p) => p.then(() => 'ok', (e) => `${e.kind} ${e.code}`);
    // Without `fs.read` on the document nothing is asked of the app's store.
    expect(await code(copyFile(`${doc}/a.png`, 'app:/data/a.png', normalized('fs.write app:/data'), app))).toBe('Unavailable denied');
    expect(opened).toBe(1);
    // Without `fs.write` on the app file the real store refuses before the document is opened.
    const { createFileSystem } = await import('./storage-fs.js');
    const refusing = createFileSystem('test.copy', normalized('fs.read doc:/\nfs.write app:/cache'));
    let touched = false;
    await expect(refusing.copyIn('app:/data/a.png', async () => { touched = true; })).rejects.toThrow(/^denied: fs.write app:\/data\/a.png/);
    expect(touched).toBe(false);
    // The refusals `readFile` gives the same source.
    expect(await Promise.all([
      code(copyFile(`${doc}/absent`, 'app:/data/x', both, app)), code(copyFile(`${doc}/sub`, 'app:/data/x', both, app)),
      code(copyFile(doc.replace(/\/chosen$/, ''), 'app:/data/x', both, app)), code(copyFile('doc:/999999/a.png', 'app:/data/x', both, app)),
    ])).toEqual(['Unavailable ENOENT', 'Unavailable EISDIR', 'Unavailable failed', 'Unavailable failed']);
    // Between documents, either way, and `rename`, still refused.
    const fs = files(both);
    expect(await Promise.all([code(fs.copyFile(`${doc}/a.png`, `${doc}/copy.png`)), code(fs.copyFile('app:/data/a.png', `${doc}/copy.png`)), code(fs.rename(`${doc}/a.png`, 'app:/data/a.png'))]))
      .toEqual(['Unavailable failed', 'Unavailable failed', 'Unavailable failed']);
    expect(await code(fs.readFile(`${doc}/copy.png`))).toBe('Unavailable ENOENT');
  } finally { delete globalThis.exact; }
});

// A Rust source's storage request takes the same path, and the same refusals.
test('a Rust request copies a document only into an app file it may write', async () => {
  globalThis.exact = {};
  const location = Object.getOwnPropertyDescriptor(globalThis, 'location');
  Object.defineProperty(globalThis, 'location', { configurable: true, value: { href: 'http://127.0.0.1/' } });
  try {
    const { createStorageRequests } = await import('./storage-request.js');
    let doc;
    globalThis.exact.documents.install({ generation: () => 0, dispatch: (_, kind, payload) => { doc = payload; }, log() {}, openFile: () => false })
      .answer({ node: 1 }, 'showDirectoryPicker', [{ name: 'chosen', files: FILES }]);
    const requests = createStorageRequests('test.copy', normalized('fs.read doc:/\nfs.write doc:/\nfs.write app:/cache'));
    const copy = async (path, destination) => JSON.parse(new TextDecoder().decode(await requests.run(JSON.stringify({ version: 1, op: 'fs.copyFile', args: { path, destination } })))).error;
    expect(await copy(`${doc}/a.png`, 'app:/data/a.png')).toStartWith('denied: fs.write app:/data/a.png');
    const unread = createStorageRequests('test.copy', normalized('fs.write app:/data'));
    expect(JSON.parse(new TextDecoder().decode(await unread.run(JSON.stringify({ version: 1, op: 'fs.copyFile', args: { path: `${doc}/a.png`, destination: 'app:/data/a.png' } })))).error)
      .toStartWith('denied: fs.copyFile');
    expect(await copy(`${doc}/a.png`, `${doc}/copy.png`)).toBe('fs.copyFile copies a document only into an app:/ path');
  } finally {
    delete globalThis.exact;
    if (location) Object.defineProperty(globalThis, 'location', location); else delete globalThis.location;
  }
});

// The real store in a browser: the File kept in IndexedDB reads back as the document's bytes.
chromeTest('a chosen file copied into app:/data reads back from the page store', async () => {
  const grants = normalized('fs.read doc:/\nfs.read app:/data\nfs.write app:/data');
  const dir = resolve(ROOT, 'host/web');
  const page = Bun.serve({ port: 0, fetch(request) {
    const name = new URL(request.url).pathname === '/' ? '' : decodeURIComponent(new URL(request.url).pathname.slice(1));
    const file = resolve(dir, name);
    if (!name) return new Response('<!doctype html><title>copy</title>', { headers: { 'content-type': 'text/html' } });
    if (!file.startsWith(dir + sep) || !existsSync(file)) return new Response('not found', { status: 404 });
    return new Response(Bun.file(file), { headers: { 'content-type': 'text/javascript' } });
  } });
  let suite;
  try {
    suite = await cdpBrowser();
    const { cdp } = suite, { sessionId } = await suite.page();
    const call = (method, params = {}) => cdp.send(method, params, sessionId);
    await call('Page.enable');
    await call('Page.navigate', { url: page.url.href });
    await new Promise((r) => setTimeout(r, 200));
    const expression = `(async () => {
      globalThis.exact = {};
      await import('/documents-glue.js');
      const { createFileSystem } = await import('/storage-fs.js');
      let doc;
      const host = exact.documents.install({ generation: () => 0, dispatch: (_, kind, payload) => { doc = payload; }, log() {}, openFile: () => false });
      host.answer({ node: 1 }, 'showDirectoryPicker', [{ name: 'chosen', files: ${JSON.stringify(FILES)} }]);
      const grants = ${JSON.stringify(grants)}, fs = createFileSystem('test.copy-' + Date.now(), grants);
      await exact.documents.copyFile(doc + '/a.png', 'app:/data/a.png', grants, fs);
      return [new TextDecoder().decode(await fs.readFile('app:/data/a.png')), (await fs.stat('app:/data/a.png')).size].join(' ');
    })()`;
    const out = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    expect(out.result.value, JSON.stringify(out.exceptionDetails)).toBe('PNG! 4');
  } finally {
    await suite?.close();
    page.stop(true);
  }
}, 60_000);
