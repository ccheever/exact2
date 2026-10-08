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

test('a write started just before the tab closes is there in the next page', async () => {
  await withBrowser(async open => {
    const first = await open();
    await first.evaluate(() => fs.mkdir('app:/data/s'));
    // Started, not awaited: the tab closes while they run. The last write is the one that stands.
    await first.evaluate(() => { for (const m of [5, 6, 7]) void fs.atomicWriteFile('app:/data/s/settings.json', new TextEncoder().encode(`{"minutes":${m}}`)); });
    await first.close();
    const second = await open();
    expect(await second.evaluate(async () => text(await fs.readFile('app:/data/s/settings.json')))).toBe('{"minutes":7}');
    expect(await second.evaluate(() => localStorage.getItem('exact-storage-journal:com.example.unload'))).toBeNull();
  });
}, 60_000);

test('an append in flight at the close lands exactly once, after the writes before it', async () => {
  await withBrowser(async open => {
    const first = await open();
    await first.evaluate(() => fs.writeFile('app:/data/log.txt', new TextEncoder().encode('a')));
    await first.evaluate(() => { for (const c of 'bcd') void fs.appendFile('app:/data/log.txt', new TextEncoder().encode(c)); });
    await first.close();
    const second = await open();
    expect(await second.evaluate(async () => text(await fs.readFile('app:/data/log.txt')))).toBe('abcd');
    // The next page's own mutations queue behind the replay, never ahead of it.
    await second.evaluate(() => fs.appendFile('app:/data/log.txt', new TextEncoder().encode('e')));
    expect(await second.evaluate(async () => text(await fs.readFile('app:/data/log.txt')))).toBe('abcde');
  });
}, 60_000);

test('a journal whose entries already committed replays nothing', async () => {
  await withBrowser(async open => {
    const first = await open();
    await first.evaluate(async () => { await fs.appendFile('app:/data/once.txt', new TextEncoder().encode('x')); });
    // The tab is hidden after the append committed and journals nothing; then a stale journal
    // naming that committed append (as if it was written while the append ran) must not replay it.
    const marker = await first.evaluate(() => new Promise(resolve => {
      const request = indexedDB.open('exact-storage:com.example.unload');
      request.onsuccess = () => {
        const get = request.result.transaction('files').objectStore('files').get('\u0000exact-applied');
        get.onsuccess = () => resolve(get.result);
      };
    }));
    expect(marker?.seq).toBeGreaterThan(0);
    await first.evaluate(m => localStorage.setItem('exact-storage-journal:com.example.unload', JSON.stringify({
      entries: [{ session: m.session, seq: m.seq, method: 'appendFile', args: ['app:/data/once.txt', { bytes: btoa('x') }] }],
    })), marker);
    await first.close();
    const second = await open();
    expect(await second.evaluate(async () => text(await fs.readFile('app:/data/once.txt')))).toBe('x');
  });
}, 60_000);
