// @exact/local-auth on the web (LLP 1133.010 D15, §9 drive 9): the fixture
// app made with `exact new` outside the repo, depending on this package by
// `file:`, built for the web and driven in each engine with no agent. The
// web's cell is `later`: the status says nothing can be checked or kept
// locked, `authenticate` and `protect` refuse `NotSupportedError`, a name
// that must be locked is refused rather than kept in `localStorage`, and a
// name the person may leave plain stays plain.
//
// usage: bun packages/local-auth/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { rmSync } from 'node:fs';
import { dirname, extname, join, resolve } from 'node:path';
import { checker, makeFixture, root, run } from './fixture.mjs';

const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

const { app } = makeFixture();
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);

const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  let path = new URL(req.url).pathname;
  if (path === '/') path = '/index.html';
  const file = Bun.file(join(dist, path));
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } });
} });
const origin = `http://127.0.0.1:${server.port}`;
const { check, failed } = checker();

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const text = (page, id) => page.getByTestId(id).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(100); }
    return got;
  };
  const press = async (page, id, want) => { await page.getByTestId(id).click(); return until(page, 'wrote', want); };
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    await page.goto(origin);
    const status = await until(page, 'status', t => t.startsWith('false'));
    check(engine, 'the status says nothing is checked or kept locked here', status === 'false false unknown none', status);
    await page.getByTestId('authenticate').click();
    const checked = await until(page, 'check', t => t !== '');
    check(engine, 'authenticate is NotSupportedError', checked === 'NotSupportedError', checked);
    const key = await press(page, 'set-key', t => t !== '');
    check(engine, 'a name that must be locked is refused, never kept plain', key === 'NotSupportedError NotSupportedError', key);
    const stored = await page.evaluate(() => Object.keys(localStorage).filter(k => /key|token/.test(k)).join(' '));
    check(engine, 'nothing of it is in localStorage', !/\bkey\b|\.key$/.test(stored), stored);
    const token = await press(page, 'set-token', t => t === 'kept');
    const facts = await until(page, 'facts', t => t.startsWith('true'));
    check(engine, 'an optional name stays plain', token === 'kept' && facts === 'true false false false', `${token} / ${facts}`);
    const protect = await press(page, 'protect', t => t !== 'kept');
    check(engine, 'protect rejects NotSupportedError', protect === 'NotSupportedError NotSupportedError', protect);
    const unlock = await press(page, 'unlock', t => t.startsWith('unlocked') || t.includes('Error'));
    check(engine, 'unlock of a plain name answers at once', unlock === 'unlocked open', unlock);
    await context.close();
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dirname(app), { recursive: true, force: true });
console.log(failed() ? `${failed()} failed` : 'all passed');
process.exit(failed() ? 1 : 0);
