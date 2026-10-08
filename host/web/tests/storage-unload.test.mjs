// LLP 1097 D10: a write in flight when the tab closes is journaled on pagehide and replayed,
// exactly once, by the next page's filesystem before anything else. Real Chrome: the loss is
// IndexedDB dropping a transaction still running when the page goes away.
import { test, expect } from 'bun:test';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dir, '..');
const repo = resolve(root, '../..');
// The grant set the app's bake would carry, from the Rust parser (as request-refusal.test.mjs does).
function grantSet(spec) {
  const scratch = mkdtempSync(join(tmpdir(), 'exact-grants-'));
  const input = join(scratch, 'grants.txt');
  writeFileSync(input, spec);
  const binary = resolve(process.env.CARGO_TARGET_DIR || resolve(repo, 'target'), 'debug/exact-web-js');
  const result = existsSync(binary)
    ? spawnSync(binary, ['normalize-grants', input], { cwd: repo, encoding: 'utf8' })
    : spawnSync('cargo', ['run', '-q', '-p', 'exact-web-js', '--', 'normalize-grants', input], { cwd: repo, encoding: 'utf8' });
  rmSync(scratch, { recursive: true });
  if (result.status !== 0) throw new Error(result.stderr || 'grant normalizer failed');
  return result.stdout.trim();
}
const page = () => `<!doctype html><script type="module">
  import { createFileSystem } from '/storage-fs.js';
  import { createGrantSet } from '/grant-admission.js';
  const grants = createGrantSet(${grantSet('fs.read app:/data\nfs.write app:/data')});
  globalThis.fs = createFileSystem('com.example.unload', grants);
  globalThis.text = buffer => new TextDecoder().decode(buffer);
  globalThis.ready = true;
</script>`;

async function withBrowser(run) {
  const { chromium: browserPath } = await import('../../../scripts/agent-launch.mjs');
  const { chromium } = await import('playwright-core');
  const html = page();
  const server = Bun.serve({
    port: 0, hostname: '127.0.0.1',
    fetch(req) {
      const path = new URL(req.url).pathname;
      if (path === '/') return new Response(html, { headers: { 'content-type': 'text/html' } });
      const file = Bun.file(join(root, path.slice(1)));
      return file.size ? new Response(file, { headers: { 'content-type': 'text/javascript' } }) : new Response('', { status: 404 });
    },
  });
  const profile = mkdtempSync(join(tmpdir(), 'exact-unload-'));
  const context = await chromium.launchPersistentContext(profile, { executablePath: browserPath().executable, headless: true });
  const open = async () => {
    const p = await context.newPage();
    await p.goto(`http://127.0.0.1:${server.port}/`);
    await p.waitForFunction(() => globalThis.ready);
    return p;
  };
  try { await run(open); }
  finally { await context.close(); server.stop(true); rmSync(profile, { recursive: true, force: true }); }
}

const APP = 'com.example.unload';
const sessionOf = p => p.evaluate(async () => (await navigator.locks.query()).held.map(l => l.name).find(n => n.startsWith('exact-storage-session:'))?.split(':').pop());
const plant = (p, session, entries, marker) => p.evaluate(async ([app, session, entries, marker]) => {
  localStorage.setItem(`exact-storage-journal:${app}:${session}`, JSON.stringify({ entries }));
  if (marker === null) return;
  await new Promise(resolve => {
    const request = indexedDB.open(`exact-storage:${app}`);
    request.onsuccess = () => {
      const t = request.result.transaction('files', 'readwrite');
      t.objectStore('files').put({ path: `\u0000exact-applied:${session}`, kind: 'marker', session, seq: marker });
      t.oncomplete = resolve;
    };
  });
}, [APP, session, entries, marker]);
const append = (seq, c) => ({ seq, at: seq, method: 'appendFile', args: ['app:/data/log.txt', { bytes: btoa(c) }] });

test('a write started just before the tab closes is there in the next page', async () => {
  await withBrowser(async open => {
    const first = await open();
    await first.evaluate(() => fs.mkdir('app:/data/s'));
    // Started, not awaited: the tab closes while they run. The last write is the one that stands.
    await first.evaluate(() => { for (const m of [5, 6, 7]) void fs.atomicWriteFile('app:/data/s/settings.json', new TextEncoder().encode(`{"minutes":${m}}`)); });
    await first.close();
    const second = await open();
    expect(await second.evaluate(async () => text(await fs.readFile('app:/data/s/settings.json')))).toBe('{"minutes":7}');
    expect(await second.evaluate(app => Object.keys(localStorage).filter(k => k.startsWith(`exact-storage-journal:${app}:`)).length, APP)).toBe(0);
  });
}, 60_000);

test('an append in flight at the close lands exactly once, and the next page queues behind the replay', async () => {
  await withBrowser(async open => {
    const first = await open();
    await first.evaluate(() => fs.writeFile('app:/data/log.txt', new TextEncoder().encode('a')));
    await first.evaluate(() => { for (const c of 'bcd') void fs.appendFile('app:/data/log.txt', new TextEncoder().encode(c)); });
    await first.close();
    const second = await open();
    // Issued at once, before anything awaits the replay: it lands after it.
    await second.evaluate(() => fs.appendFile('app:/data/log.txt', new TextEncoder().encode('e')));
    expect(await second.evaluate(async () => text(await fs.readFile('app:/data/log.txt')))).toBe('abcde');
  });
}, 60_000);

test('writes a queue above accepted (admit) survive though only the first had started', async () => {
  await withBrowser(async open => {
    const first = await open();
    await first.evaluate(() => {
      const seqs = [5, 6, 7].map(m => fs.admit('atomicWriteFile', ['app:/data/q.json', new TextEncoder().encode(String(m))]));
      // The outer queue runs one at a time; the tab closes while the first runs.
      void fs.withSeq(seqs[0], () => fs.atomicWriteFile('app:/data/q.json', new TextEncoder().encode('5')));
    });
    await first.close();
    const second = await open();
    expect(await second.evaluate(async () => text(await fs.readFile('app:/data/q.json')))).toBe('7');
  });
}, 60_000);

test("a live tab's journal is never replayed by another tab, and is replayed once it is gone", async () => {
  await withBrowser(async open => {
    const a = await open();
    await a.evaluate(() => fs.writeFile('app:/data/log.txt', new TextEncoder().encode('a')));
    const session = await sessionOf(a);
    expect(session).toBeTruthy();
    await plant(a, session, [append(99, 'b')], null); // after A's own committed writeFile (seq 1)
    const b = await open();
    expect(await b.evaluate(async () => text(await fs.readFile('app:/data/log.txt')))).toBe('a');
    await a.close();
    // The closed page's lock goes when its renderer does.
    await b.waitForFunction(async name => !(await navigator.locks.query()).held.some(l => l.name.endsWith(`:${name}`)), session);
    const c = await open();
    expect(await c.evaluate(async () => text(await fs.readFile('app:/data/log.txt')))).toBe('ab');
    await c.close();
    const d = await open();
    expect(await d.evaluate(async () => text(await fs.readFile('app:/data/log.txt')))).toBe('ab');
  });
}, 60_000);

test("a gone session's marker decides by sequence: committed entries never replay, later ones do", async () => {
  await withBrowser(async open => {
    const a = await open();
    await a.evaluate(() => fs.writeFile('app:/data/log.txt', new TextEncoder().encode('a')));
    // A gone session whose seq 1 and 2 committed (marker 2) though its journal still names 1:
    // a stale journal. And another whose marker is 1, with 2 and 3 uncommitted.
    await plant(a, 'gone1', [append(1, 'x')], 2);
    await plant(a, 'gone2', [append(1, 'y'), append(2, 'b'), append(3, 'c')], 1);
    await a.close();
    const b = await open();
    expect(await b.evaluate(async () => text(await fs.readFile('app:/data/log.txt')))).toBe('abc');
    // Markers of replayed sessions are dropped; the marker key is no file.
    expect(await b.evaluate(async () => (await fs.readdir('app:/data')).sort())).toEqual(['log.txt']);
  });
}, 60_000);
