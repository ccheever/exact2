// LLP 1133.006 D8's seams, through this module outside the tree: an app
// `exact new` made outside the repo depends on a copy of it by `file:`, and
// imports it from data code only (its Contract names none of it).
//
// - Bytes from a view are copied when called; a path the source's `fs.read`
//   covers reaches the implementation as the file; one it does not cover is
//   refused `NotAllowedError` before the implementation sees it; more bytes
//   than a call carries reject `DataError`.
// - A put whose data resolves late, then one whose data is ready: the
//   earlier rejects `AbortError` and the later is what the implementation
//   kept (the call order, D2).
// - The implementation's `$state` shows in `state.extensions.tray` beside
//   the count of calls that reached the host (a superseded put and an
//   oversized one never leave the binding); under the agent the implementation runs as its own
//   substitute and says so.
//
// usage: bun contract/cli/tests/tray/seams.mjs [--keep] [--macos] [--ios]
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, root = resolve(here, '../../../..');
const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 << 20 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};
let failures = 0;
const check = (where, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
};

const scratch = mkdtempSync(join(tmpdir(), 'exact-tray-'));
const pkg = join(scratch, 'tray');
cpSync(here, pkg, { recursive: true, filter: (src) => src !== join(here, 'app') && !src.startsWith(join(here, 'app') + '/') && !src.endsWith('seams.mjs') });
const app = join(scratch, 'trayapp');
run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
cpSync(join(here, 'app'), app, { recursive: true });
rmSync(join(app, 'app.test.contract'), { force: true });
const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
manifest.dependencies = { ...manifest.dependencies, '@fixture/tray': `file:${pkg}` };
writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], app);

// What every host must show after each press.
const expect = {
  bytes: ['ok', [{ 'application/octet-stream': 'bytes 3', 'text/plain': 'text hi' }]],
  file: ['ok', [{ 'application/octet-stream': 'file 1000' }]],
  denied: ['NotAllowedError', null],
  race: ['AbortError ok', [{ 'text/plain': 'text B' }]],
  big: ['DataError', null],
};

// The web, bound.
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
  return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
} });
const origin = `http://127.0.0.1:${server.port}`;
const { chromium } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
const browser = await chromium.launch();
const text = (page, testId) => page.getByTestId(testId).innerText().catch(() => '');
const until = async (page, testId, ms = 5000) => {
  const end = Date.now() + ms;
  let got = '';
  while (Date.now() < end) { got = await text(page, testId); if (got && got !== 'none') return got; await page.waitForTimeout(50); }
  return got;
};
try {
  const page = await browser.newPage();
  await page.goto(origin);
  for (const [name, [said, last]] of Object.entries(expect)) {
    await page.getByTestId(name).click();
    const got = await until(page, `${name}-said`);
    check('web', `${name} answers ${said}`, got === said, got);
    if (last) {
      const state = await page.evaluate(() => globalThis.exact.direct.state().tray);
      check('web', `${name}: the implementation kept what the seam handed it`, JSON.stringify(state?.last) === JSON.stringify(last), JSON.stringify(state));
    }
  }
  const state = await page.evaluate(() => globalThis.exact.direct.state().tray);
  check('web', 'each call that reached the host is counted, the refused one too', state?.calls?.put === 4 && state?.substituting === false, JSON.stringify(state));
} finally {
  await browser.close();
  server.stop();
}

/** Each press under the agent on `host`, with its answer and `state.extensions.tray` after it. */
const agentDrive = (host, extra = []) => {
  const ops = Object.keys(expect).flatMap((name) => [`tap ${name}`, 'clock settle', 'state']);
  const out = run('bun', ['exact.mjs', 'agent', host, ...extra, '--storage', 'tray', '--json', ...ops], app).trim().split('\n').map((l) => JSON.parse(l));
  const states = out.filter((o) => o && o.slots !== undefined);
  Object.entries(expect).forEach(([name, [said, last]], i) => {
    const s = states[i], slot = s?.mutations?.[name] ?? s?.slots?.[name];
    const got = JSON.stringify(s ?? {}).match(new RegExp(`"said":"([^"]*)"`, 'g'));
    const tray = s?.extensions?.tray;
    check(host, `${name} answers ${said}`, JSON.stringify(slot ?? got ?? '').includes(said), JSON.stringify(slot ?? got));
    if (last) check(host, `${name}: the substitute kept what the seam handed it`, JSON.stringify(tray?.last) === JSON.stringify(last), JSON.stringify(tray));
  });
  const tray = states.at(-1)?.extensions?.tray;
  check(host, 'the implementation is its own substitute under the agent', tray?.substituting === true && tray?.calls?.put === 4, JSON.stringify(tray));
};
agentDrive('web');
if (process.argv.includes('--macos')) agentDrive('macos');
if (process.argv.includes('--ios')) agentDrive('ios');

if (!process.argv.includes('--keep')) rmSync(scratch, { recursive: true, force: true });
else console.log(`kept ${scratch}`);
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
