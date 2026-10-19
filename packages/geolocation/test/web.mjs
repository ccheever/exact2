// @exact/geolocation's conformance suite on the web (LLP 1133.001 D7, LLP
// 1133.002 stage 1): the fixture app (`test/fixture`) made with `exact new`
// outside the repo, depending on this package by `file:`, built for the
// web, and driven in each engine with the web implementation bound. No
// agent: a substitute would prove nothing about the implementation. The
// browser's own means answer the platform's side: Playwright grants the
// permission and sets the position.
//
// usage: bun packages/geolocation/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.63.0 install firefox webkit` first)
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

const run = (cmd, args, cwd) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
};

// The fixture: an app `exact new` made, its files replaced by the fixture's.
const dir = mkdtempSync(join(tmpdir(), 'exact-geolocation-')), app = join(dir, 'geofixture');
run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
rmSync(join(app, 'app.test.contract'), { force: true });
const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
manifest.dependencies = { ...manifest.dependencies, '@exact/geolocation': `file:${pkg}` };
writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], app);
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

// Counts the page's watch registrations and clears, for the release check (D5).
const count = () => {
  const g = navigator.geolocation, w = g.watchPosition.bind(g), c = g.clearWatch.bind(g);
  globalThis.__geo = { watches: 0, clears: 0 };
  g.watchPosition = (...a) => { globalThis.__geo.watches++; return w(...a); };
  g.clearWatch = (...a) => { globalThis.__geo.clears++; return c(...a); };
};

let failed = 0;
const check = (engine, name, ok, detail) => {
  if (!ok) failed++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${engine} ${name}${ok ? '' : `: ${detail}`}`);
};

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const text = (page, id) => page.getByTestId(id).innerText({ timeout: 10000 }).catch(e => `(${e.message.split('\n')[0]})`);
  const until = async (page, id, want) => {
    const end = Date.now() + 10000;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(100); }
    return got;
  };
  try {
    // Not yet granted: the fact reads `prompt`; the web says nothing of approximate.
    const asking = await browser.newContext();
    const fresh = await asking.newPage();
    await fresh.goto(origin);
    const prompt = await until(fresh, 'status', t => t === 'true prompt unknown');
    check(engine, 'state prompt before a grant', prompt === 'true prompt unknown', prompt);
    // The fact is live: a grant while the app runs reaches it (`PermissionStatus`'s change).
    await asking.grantPermissions(['geolocation'], { origin });
    await fresh.evaluate(() => dispatchEvent(new Event('focus'))); // as returning from the browser's settings
    const live = await until(fresh, 'status', t => t === 'true granted unknown');
    check(engine, 'a grant while running reaches the state', live === 'true granted unknown', live);
    await asking.close();

    const context = await browser.newContext();
    await context.grantPermissions(['geolocation'], { origin });
    await context.setGeolocation({ latitude: 48.8566, longitude: 2.3522, accuracy: 50 });
    await context.addInitScript(count);
    const page = await context.newPage();
    await page.goto(origin);
    const granted = await until(page, 'status', t => t === 'true granted unknown');
    check(engine, 'state granted', granted === 'true granted unknown', granted);

    await page.getByTestId('locate').click();
    const once = await until(page, 'once', t => t.startsWith('48.8566'));
    check(engine, 'getCurrentPosition answers the position', once === '48.8566,2.3522,50 none', once);

    await page.getByTestId('watch').click();
    const spot = await until(page, 'spot', t => t === '48.8566,2.3522');
    check(engine, 'watchPosition in data code', spot === '48.8566,2.3522', spot);
    await context.setGeolocation({ latitude: 51.5074, longitude: -0.1278, accuracy: 50 });
    const moved = await until(page, 'spot', t => t === '51.5074,-0.1278');
    check(engine, 'a move asks the reader again', moved === '51.5074,-0.1278', moved);

    await page.getByTestId('watch').click();
    const off = await until(page, 'spot', t => t === 'off');
    await page.waitForTimeout(200);
    const geo = await page.evaluate(() => globalThis.__geo);
    check(engine, 'the last reader gone, the watch is cleared', off === 'off' && geo.watches === 1 && geo.clears === 1, `${off} ${JSON.stringify(geo)}`);

    await page.getByTestId('raise').click();
    const precise = await until(page, 'precise-error', t => t !== '');
    check(engine, 'requestPreciseLocation is NotSupportedError on the web', precise === 'NotSupportedError', precise);
    await context.close();
  } finally {
    await browser.close();
  }
}
server.stop();
rmSync(dir, { recursive: true, force: true });
console.log(failed ? `${failed} failed` : 'all passed');
process.exit(failed ? 1 : 0);
