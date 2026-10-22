// A page's stream on the JS dev loop (host/web-js/dev.mjs /__dev/page) ends when the page goes, whichever loopback
// it came by: the design tool frames its canvas at localhost ([::1], host/web/serve.mjs alsoOnIPv6Loopback), where
// a page that closed stayed a connected build for as long as the loop ran (the tool listed ten on the clone, and
// pinned and inspected them all).
import { afterEach, expect, test } from 'bun:test';
import { spawnOwned, stopAll } from '../../scripts/test-children.mjs';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { createServer, request } from 'node:http';
import { connect, createServer as createSocketServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

const ROOT = resolve(new URL('../..', import.meta.url).pathname);
afterEach(() => stopAll());

test('a page that closes is no build of the loop any more, by 127.0.0.1 or by localhost', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-page-streams-')), dist = mkdtempSync(join(tmpdir(), 'exact-page-streams-dist-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Streams', app: { id: 'com.exact.streams', name: 'Streams' } }));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "streams"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber streams\n\ntheme\n  ink = #000000\n');
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', dir, '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '', browser;
  dev.stdout.on('data', d => { out += d; });
  dev.stderr.on('data', d => { out = (out + d).slice(-8000); });
  const until = async (pass, ms, what) => { const end = Date.now() + ms; while (!(await pass())) { if (Date.now() > end) throw new Error(`${what}: not seen in ${out.slice(-2000)}`); await new Promise(ok => setTimeout(ok, 100)); } };
  const base = `http://127.0.0.1:${port}`;
  const builds = async () => {
    const { token } = await (await fetch(`${base}/__tamber/token`)).json();
    const hello = await (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    return hello.ok.builds.length;
  };
  try {
    // Listening (its banner comes after both listeners), and its first build done.
    await until(() => out.includes(`127.0.0.1:${port}/`) && out.includes('plan ready'), 900_000, 'the first build');
    // localhost resolved to [::1] (as the design tool's canvas is reached first), not left to Chrome to choose.
    browser = await suiteBrowser('chromium', { args: ['--host-resolver-rules=MAP localhost [::1]'] });
    // IPv6 here at all (asked of a socket of its own, not of the loop): then the loop must answer on [::1].
    const six = await new Promise(ok => { const t = createSocketServer(); t.once('error', () => ok(false)); t.listen(0, '::1', () => t.close(() => ok(true))); });
    if (six) await new Promise((ok, fail) => { const c = connect(port, '::1'); c.once('connect', () => { c.destroy(); ok(); }); c.once('error', e => fail(new Error(`the loop does not answer on [::1]: ${e.code}`))); });
    else console.warn('no IPv6 loopback here: only 127.0.0.1 is checked');
    // The development gate through the [::1] server (reload1's condition): a localhost request is local, so it is given
    // the token; another name at this port is refused.
    if (six) {
      const token = (host) => new Promise((ok, fail) => request({ host: '::1', port, path: '/__tamber/token', headers: { host } }, (res) => { let body = ''; res.on('data', d => { body += d; }); res.on('end', () => ok({ status: res.statusCode, token: res.statusCode === 200 ? typeof JSON.parse(body).token : null })); }).on('error', fail).end());
      expect(await token(`localhost:${port}`)).toEqual({ status: 200, token: 'string' });
      expect((await token(`evil.example:${port}`)).status).toBe(421);
    }
    for (const host of six ? ['127.0.0.1', 'localhost'] : ['127.0.0.1']) {
      const context = await browser.newContext(), page = await context.newPage();
      await page.goto(`http://${host}:${port}/`);
      await until(async () => (await builds()) === 1, 60_000, `${host}: the page's build`);
      await context.close();
      // Gone with its page: within a few seconds, not for as long as the loop runs.
      await until(async () => (await builds()) === 0, 10_000, `${host}: the closed page's build gone`);
    }
    expect(await builds()).toBe(0);
  } finally {
    await browser?.close();
    await dev.stop(30_000);
    rmSync(dir, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}, 1_200_000);

test('a page stream on a socket handed to the loop\'s server ends with its client, by the res close backstop alone', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-page-hand-off-')), dist = mkdtempSync(join(tmpdir(), 'exact-page-hand-off-dist-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Streams', app: { id: 'com.exact.streams', name: 'Streams' } }));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    text "streams"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber streams\n\ntheme\n  ink = #000000\n');
  const free = async () => { const l = createServer(); await new Promise(ok => l.listen(0, '127.0.0.1', ok)); const p = l.address().port; await new Promise(ok => l.close(ok)); return p; };
  const port = await free(), handOff = await free();
  const dev = spawnOwned(process.execPath, ['--preload', resolve(ROOT, 'host/web-js/hand-off.preload.mjs'), resolve(ROOT, 'host/web/dev.mjs'), '--app', dir, '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist, EXACT_TEST_HAND_OFF: `${port}:${handOff}` }, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '', socket;
  dev.stdout.on('data', d => { out += d; });
  dev.stderr.on('data', d => { out = (out + d).slice(-8000); });
  const until = async (pass, ms, what) => { const end = Date.now() + ms; while (!(await pass())) { if (Date.now() > end) throw new Error(`${what}: not seen in ${out.slice(-2000)}`); await new Promise(ok => setTimeout(ok, 100)); } };
  const base = `http://127.0.0.1:${port}`;
  const builds = async () => {
    const { token } = await (await fetch(`${base}/__tamber/token`)).json();
    const hello = await (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: '{"method":"hello"}' })).json();
    return hello.ok.builds.length;
  };
  try {
    await until(() => out.includes(`127.0.0.1:${port}/`) && out.includes('plan ready'), 900_000, 'the first build');
    // The page's stream, by the hand-off: its first frame gives the nonce a build registers with.
    let got = '';
    socket = connect(handOff, '127.0.0.1', () => socket.write(`GET /__dev/page HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nAccept: text/event-stream\r\n\r\n`));
    socket.on('data', d => { got += d; });
    socket.on('error', () => {});
    await until(() => /"nonce":"[0-9a-f]+"/.test(got), 30_000, 'the stream\'s first frame');
    const nonce = /"nonce":"([0-9a-f]+)"/.exec(got)[1];
    const registered = await fetch(`${base}/__dev/report`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ nonce, kind: 'register', id: 'handed-1', host: 'web-js', capabilities: [] }) });
    expect(registered.status).toBeLessThan(300);
    await until(async () => (await builds()) === 1, 10_000, 'the handed-off page\'s build');
    socket.destroy();
    // Gone with its client: req 'close' never comes on this socket under Bun, so res 'close' is what ends it.
    await until(async () => (await builds()) === 0, 10_000, 'the handed-off page\'s build gone');
  } finally {
    socket?.destroy();
    await dev.stop(30_000);
    rmSync(dir, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}, 1_200_000);

test('a page whose app replaces fetch still registers and inspects: the dev client keeps the page\'s own', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-page-fetch-')), dist = mkdtempSync(join(tmpdir(), 'exact-page-fetch-dist-'));
  writeFileSync(join(dir, 'app.json'), JSON.stringify({ name: 'Fetched', app: { id: 'com.exact.fetched', name: 'Fetched' } }));
  writeFileSync(join(dir, 'app.contract'), 'component App\n  view\n    column gap=8\n      text "above"\n      text "fetched"\n');
  writeFileSync(join(dir, 'app.tamber'), 'tamber fetched\n\ntheme\n  ink = #000000\n');
  // As a fixture build does (the clone's copy, data/fixture.ts): every fetch after the app loads is the app's, which
  // takes no relative URL.
  writeFileSync(join(dir, 'app.ts'), [
    "import type { Answer, Sources } from './app.contract.d.ts';",
    "(globalThis as { fetch: unknown }).fetch = (u: unknown) => { if (!/^[a-z][a-z0-9+.-]*:/i.test(String(u))) throw new TypeError(\"Failed to construct 'URL': Invalid URL\"); return Promise.reject(new Error('the app answers no fetch')); };",
    "export const appId = 'com.exact.fetched';",
    "export const grants = '';",
    "const sources: Sources = {};",
    "export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);",
    '',
  ].join('\n'));
  const listener = createServer();
  await new Promise(ok => listener.listen(0, '127.0.0.1', ok));
  const port = listener.address().port;
  await new Promise(ok => listener.close(ok));
  const dev = spawnOwned(process.execPath, [resolve(ROOT, 'host/web/dev.mjs'), '--app', dir, '--port', String(port)], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: dist }, stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '', browser, events = '';
  dev.stdout.on('data', d => { out += d; });
  dev.stderr.on('data', d => { out = (out + d).slice(-8000); });
  const until = async (pass, ms, what) => { const end = Date.now() + ms; while (!(await pass())) { if (Date.now() > end) throw new Error(`${what}: not seen in ${out.slice(-2000)}`); await new Promise(ok => setTimeout(ok, 100)); } };
  const base = `http://127.0.0.1:${port}`;
  const { token } = await until(() => out.includes(`127.0.0.1:${port}/`) && out.includes('plan ready'), 900_000, 'the first build').then(() => fetch(`${base}/__tamber/token`)).then(r => r.json());
  const call = async (body) => (await fetch(`${base}/__tamber`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: JSON.stringify(body) })).json();
  const stream = new AbortController();
  try {
    // A present client, to turn inspect on, hearing what the page refuses.
    const sub = await fetch(`${base}/__tamber/subscribe`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-exact-tamber-token': token }, body: '{"client":"probe"}', signal: stream.signal });
    (async () => { const reader = sub.body.getReader(); for (;;) { const { value, done } = await reader.read(); if (done) return; events += new TextDecoder().decode(value); } })().catch(() => {});
    browser = await suiteBrowser('chromium');
    const page = await (await browser.newContext()).newPage();
    const said = [];
    page.on('console', m => said.push(m.text()));
    await page.goto(`${base}/`);
    await until(async () => (await call({ method: 'hello' })).ok.builds.length === 1, 60_000, `the page's build (the page said: ${said.join(' / ')})`);
    // Its app's fetch is in place (the stand-in's refusal), and the dev client's register went past it.
    expect(await page.evaluate(() => { try { fetch('/x'); return 'took a relative URL'; } catch (e) { return e.message; } })).toContain('Invalid URL');
    const [build] = (await call({ method: 'hello' })).ok.builds;
    expect((await call({ method: 'inspect', client: 'probe', build: build.id, on: true })).ok).toBeTruthy();
    expect((await call({ method: 'hello' })).ok.builds[0].inspector).toBe('probe');
    // A click selects: the page reads its inspect table with its own fetch, so a selection comes, nothing refused.
    await until(async () => (await page.getByText('fetched', { exact: true }).count()) > 0, 10_000, 'the text');
    // Inspect mode on in the page itself (its banner shown), not only on the server.
    await page.locator('#exact-inspect').waitFor({ state: 'visible', timeout: 15_000 });
    const box = await page.evaluate(() => { const r = [...document.querySelectorAll('[data-exact-text]')].find(e => e.textContent === 'fetched')?.getBoundingClientRect(); return r ? { x: r.x, y: r.y, width: r.width, height: r.height } : null; });
    // (At its left end: inspect mode's banner sits over the middle of the top.)
    await page.mouse.click(box.x + 12, box.y + box.height / 2);
    await until(() => events.includes('"event":"selection"') || events.includes('"event":"unsupported"'), 15_000, 'a selection');
    expect(events).not.toContain('"event":"unsupported"');
  } finally {
    stream.abort();
    await browser?.close();
    await dev.stop(30_000);
    rmSync(dir, { recursive: true, force: true });
    rmSync(dist, { recursive: true, force: true });
  }
}, 1_200_000);
