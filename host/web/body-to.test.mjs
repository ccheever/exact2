// `fetch(url, {exactBodyTo})` on the web (LLP 1108 D6 R2, 2026-10-26), in a
// real browser with its own IndexedDB and Web Locks: the wasm host's request
// (http-body.js) and the JS target's fetch (host/web-js/ts-fetch.js) write a
// 2xx body into the app's store (storage-fs.js `bodyTarget`), whole or not
// at all, and read any other reply as usual.
import { expect, test } from 'bun:test';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

const ROOT = resolve(import.meta.dirname, '../..');
const BIG = (17 << 20) + 5;
const byte = i => (i * 31 ^ i >> 9) & 255;

// The grant set as the build makes it (the exact-web-js normalizer).
function normalized(spec) {
  const built = spawnSync('cargo', ['build', '-q', '-p', 'exact-web-js'], { cwd: ROOT, encoding: 'utf8' });
  if (built.status !== 0) throw new Error(built.stderr || 'grant normalizer build failed');
  const scratch = mkdtempSync(resolve(tmpdir(), 'exact-grants-'));
  try {
    writeFileSync(resolve(scratch, 'grants.txt'), spec);
    const bin = resolve(process.env.CARGO_TARGET_DIR || resolve(ROOT, 'target'), 'debug/exact-web-js');
    const run = spawnSync(bin, ['normalize-grants', resolve(scratch, 'grants.txt')], { cwd: ROOT, encoding: 'utf8' });
    if (run.status !== 0) throw new Error(run.stderr || 'grant normalizer failed');
    return JSON.parse(run.stdout);
  } finally { rmSync(scratch, { recursive: true }); }
}

// Host modules under /web/ and /web-js/ (a JS-target build puts host/web's
// beside its own), `/admission-data.js` as the build writes it, and the
// downloads: `/big` (chunked), `/missing` (404), `/stall` (never ends).
function serve() {
  const seen = [];
  let grants = null;
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    seen.push(path);
    if (path === '/web-js/admission-data.js') return new Response(`export const tsGrantSet = ${JSON.stringify(grants)};`, { headers: { 'content-type': 'text/javascript' } });
    const js = path.match(/^\/(web|web-js)\/([\w.-]+\.js)$/);
    if (js) {
      const own = resolve(ROOT, 'host', js[1], js[2]), file = existsSync(own) ? own : resolve(ROOT, 'host/web', js[2]);
      return new Response(readFileSync(file), { headers: { 'content-type': 'text/javascript' } });
    }
    if (path === '/big') {
      let at = 0;
      return new Response(new ReadableStream({ pull(out) {
        const n = Math.min(64 << 10, BIG - at);
        if (n === 0) return out.close();
        const chunk = new Uint8Array(n);
        for (let i = 0; i < n; i++) chunk[i] = byte(at + i);
        at += n;
        out.enqueue(chunk);
      } }), { headers: { 'content-type': 'application/octet-stream' } });
    }
    if (path === '/stall') return new Response(new ReadableStream({ start(out) { out.enqueue(new Uint8Array(256 << 10)); } }));
    if (path === '/missing') return new Response('no such thing', { status: 404 });
    return new Response('<!doctype html>', { headers: { 'content-type': 'text/html' } });
  } });
  return { server, seen, origin: `http://127.0.0.1:${server.port}`, setGrants: g => { grants = g; } };
}

// In the page: a store file's size and whether every byte is the served one.
const CHECK = `globalThis.check = async (store, path) => {
  const bytes = new Uint8Array(await store.readFile(path));
  for (let i = 0; i < bytes.length; i++) if (bytes[i] !== ((i * 31 ^ i >> 9) & 255)) return { size: bytes.length, same: false };
  return { size: bytes.length, same: true };
};`;

test('the wasm host writes a 2xx body to the app file and reads anything else', async () => {
  const site = serve();
  const grants = normalized(`net.fetch ${site.origin}\nfs.write app:/data`);
  const browser = await suiteBrowser();
  try {
    const page = await browser.newPage();
    await page.goto(site.origin);
    await page.addScriptTag({ content: CHECK });
    const out = await page.evaluate(async ({ grants, origin }) => {
      const { request } = await import('/web/http-body.js');
      const { bodyTarget, createFileStore } = await import('/web/storage-fs.js');
      const store = createFileStore('body-to-wasm');
      await store.writeFile('app:/data/kept', new TextEncoder().encode('old'));
      const host = { grantSet: grants, controllers: new Set(), localAssetURL: u => u, bodyTarget: (path, g) => bodyTarget('body-to-wasm', g, path) };
      const get = (path, bodyTo, more = {}) => request({ method: 'GET', url: origin + path, headers: [], body: '', bodyTo, ...more }, host);
      const said = r => ({ kind: r.kind, status: r.status, headers: r.headers.split('\n')[0], body: new TextDecoder().decode(r.body) });
      const kept = async () => new TextDecoder().decode(await store.readFile('app:/data/kept'));
      const big = said(await get('/big', 'app:/data/big'));
      const written = await check(store, 'app:/data/big');
      const missing = said(await get('/missing', 'app:/data/kept'));
      const refusals = [];
      for (const [path, more] of [['app:/tmp/x', {}], ['app:/data/none/x', {}]]) refusals.push(said(await get('/never', path, more)));
      const over = said(await get('/big', 'app:/data/kept', { nativeHttp: 'independent', maxResponseBytes: 1 << 20 }));
      const stalled = said(await get('/stall', 'app:/data/kept', { timeoutMs: 300 }));
      const bare = said(await request({ method: 'GET', url: origin + '/never', headers: [], body: '', bodyTo: 'app:/data/x' }, { ...host, bodyTarget: undefined }));
      return { big, written, missing, refusals, over, stalled, bare, kept: await kept(), names: (await store.readdir('app:/data')).sort() };
    }, { grants, origin: site.origin });
    expect(out.big).toEqual({ kind: 0, status: 200, headers: `:exact-body-to: ${BIG}`, body: '' });
    expect(out.written).toEqual({ size: BIG, same: true });
    expect(out.missing).toMatchObject({ kind: 0, status: 404, body: 'no such thing' });
    expect(out.refusals[0]).toMatchObject({ kind: 2, body: expect.stringContaining('exactBodyTo app:/tmp/x: denied: fs.write app:/tmp/x') });
    expect(out.refusals[1]).toMatchObject({ kind: 2, body: expect.stringContaining('exactBodyTo app:/data/none/x: no such file or directory') });
    expect(out.over).toMatchObject({ kind: 2, body: 'HTTP response exceeds limit' });
    expect(out.stalled).toMatchObject({ kind: 10 });
    expect(out.bare).toMatchObject({ kind: 3 });
    expect(out.kept).toBe('old');
    expect(out.names).toEqual(['big', 'kept']);
    expect(site.seen).not.toContain('/never');
  } finally { await browser.close(); site.server.stop(true); }
}, 120_000);

test('the JS target writes a 2xx body to the app file, says what it wrote, and refuses as Hermes does', async () => {
  const site = serve();
  site.setGrants(normalized(`net.fetch ${site.origin}\nfs.write app:/data\nfs.read app:/data`));
  const browser = await suiteBrowser();
  try {
    const page = await browser.newPage();
    await page.goto(site.origin);
    await page.addScriptTag({ content: CHECK });
    const out = await page.evaluate(async origin => {
      const ts = await import('/web-js/ts-fetch.js');
      const { createFileStore } = await import('/web-js/storage-fs.js');
      ts.files.appId = 'body-to-js';
      const store = createFileStore('body-to-js');
      await store.writeFile('app:/data/kept', new TextEncoder().encode('old'));
      const fail = p => p.then(() => 'sent', e => `${e.name}${e.kind ? ' ' + e.kind : ''}: ${e.message}`);
      const big = await ts.fetch(origin + '/big', { exactBodyTo: 'app:/data/big' });
      const result = { status: big.status, exactBodyTo: big.exactBodyTo, text: await big.text(), written: await check(store, 'app:/data/big') };
      const missing = await ts.fetch(origin + '/missing', { exactBodyTo: 'app:/data/kept' });
      result.missing = { status: missing.status, exactBodyTo: missing.exactBodyTo ?? null, text: await missing.text() };
      result.refusals = [];
      for (const init of [
        { exactBodyTo: '/data/x' }, { exactBodyTo: 'app:/data/../x' }, { method: 'HEAD', exactBodyTo: 'app:/data/x' },
        { exactBodyTo: 'app:/data/x', exactStream: () => null }, { exactBodyTo: 'app:/data/x', exactIndependentHttp: { maxResponseBytes: 2 ** 30 + 1 } },
        { exactBodyTo: 'app:/tmp/x' },
      ]) result.refusals.push(await fail(ts.fetch(origin + '/never', init)));
      result.over = await fail(ts.fetch(origin + '/big', { exactBodyTo: 'app:/data/kept', exactIndependentHttp: { maxResponseBytes: 1 << 20 } }));
      result.stalled = await fail(ts.fetch(origin + '/stall', { exactBodyTo: 'app:/data/kept', exactTimeout: 300 }));
      result.kept = new TextDecoder().decode(await store.readFile('app:/data/kept'));
      result.names = (await store.readdir('app:/data')).sort();
      return result;
    }, site.origin);
    expect(out).toMatchObject({ status: 200, exactBodyTo: { path: 'app:/data/big', bytes: BIG }, text: '', written: { size: BIG, same: true } });
    expect(out.missing).toEqual({ status: 404, exactBodyTo: null, text: 'no such thing' });
    expect(out.refusals).toEqual([
      'TypeError: exactBodyTo must be an app:/ path',
      'TypeError: exactBodyTo: an app:/ path has no . or .. segment',
      'TypeError: exactBodyTo: a HEAD reply has no body to write',
      "TypeError: exactBodyTo: a stream's body is its messages, not a file",
      'TypeError: exactIndependentHttp.maxResponseBytes must be an integer from 1 to 1073741824',
      'FetchError Refused: exactBodyTo app:/tmp/x: denied: fs.write app:/tmp/x: no grant covers it; grant `fs.write app:/tmp/x`',
    ]);
    expect(out.over).toBe('FetchError Refused: HTTP response exceeds limit');
    expect(out.stalled).toBe('FetchError Timeout: the request timed out after 300 ms');
    expect(out.kept).toBe('old');
    expect(out.names).toEqual(['big', 'kept']);
    expect(site.seen).not.toContain('/never');
  } finally { await browser.close(); site.server.stop(true); }
}, 120_000);
