// @exact/background's conformance suite on the web (LLP 1133.017 stage 1):
// the fixture app (`test/fixture`) made with `exact new` outside the repo,
// depending on this package by `file:`, built for the web, and driven in
// each engine with the web implementation bound. No agent: a substitute
// would prove nothing about the implementation. Playwright's offline switch
// is the network going and coming back (`navigator.onLine` and the page's
// `online` event).
//
// - The status: available, `sync` true, `periodic` false (D17's `later`), the
//   state the Permissions API's word where the engine has `background-sync`
//   (Chromium), else `unknown`.
// - A tag registered offline is listed and fires at the page's `online`
//   event; one registered online fires at once; each is retired when its
//   invocation completes.
// - A handler whose request is refused registers its tag again: it stays
//   listed, past its backoff, and outlives a reload (`localStorage`).
// - `registerPeriodicSync` rejects `NotSupportedError` in data code.
//
// usage: bun packages/background/test/web.mjs [chromium] [firefox] [webkit]
//   (every engine by default; `bunx playwright@1.64.1 install firefox webkit` first)
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');
const engines = process.argv.slice(2).length ? process.argv.slice(2) : ['chromium', 'firefox', 'webkit'];
const run = (cmd, args, cwd) => {
  const r = spawnSync(cmd, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout}\n${r.stderr}`);
  return r.stdout;
};
let failures = 0;
const check = (where, name, ok, detail) => {
  if (!ok) failures++;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${where} ${name}${ok ? '' : `: ${detail}`}`);
};

// The handler's request: answered, or refused while `refuse` is set.
let refuse = false, pulls = 0;
const puller = Bun.serve({ port: 0, fetch(req) {
  const cors = { 'access-control-allow-origin': '*' };
  if (req.method === 'OPTIONS') return new Response(null, { headers: { ...cors, 'access-control-allow-headers': '*' } });
  pulls++;
  return new Response(refuse ? 'no' : 'ok', { status: refuse ? 503 : 200, headers: cors });
} });

const dir = mkdtempSync(join(tmpdir(), 'exact-background-')), app = join(dir, 'bgfixture');
run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
for (const f of ['app.contract', 'app.ts']) cpSync(join(here, 'fixture', f), join(app, f));
rmSync(join(app, 'app.test.contract'), { force: true });
const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id).replace('__PULL_ORIGIN__', `http://127.0.0.1:${puller.port}`));
const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
manifest.dependencies = { ...manifest.dependencies, '@exact/background': `file:${pkg}` };
writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
run('bun', ['install'], app);
run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);

const dist = join(app, 'target/web-dist');
const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
const server = Bun.serve({ port: 0, async fetch(req) {
  const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
  return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
} });
const origin = `http://127.0.0.1:${server.port}`;
const { chromium, firefox, webkit } = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));

for (const engine of engines) {
  const browser = await { chromium, firefox, webkit }[engine].launch();
  const text = (page, id) => page.getByTestId(id).innerText({ timeout: 10000 }).catch((e) => `(${e.message.split('\n')[0]})`);
  const until = async (page, id, want, ms = 10000) => {
    const end = Date.now() + ms;
    let got;
    while (Date.now() < end) { if (want(got = await text(page, id))) return got; await page.waitForTimeout(100); }
    return got;
  };
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    await page.goto(origin);
    const status = await until(page, 'status', (t) => t.startsWith('true '));
    const want = engine === 'chromium' ? 'true granted false true' : 'true unknown false true';
    check(engine, `the status reads ${want}`, status === want, status);

    // Registered offline: listed, and fired at the page's `online` event.
    refuse = false;
    pulls = 0;
    await context.setOffline(true);
    await page.getByTestId('queue-t1').click();
    const listed = await until(page, 'tags', (t) => t === 't1');
    await page.waitForTimeout(300);
    check(engine, 'a tag registered offline is listed and does not fire', listed === 't1' && pulls === 0, `${listed} pulls=${pulls}`);
    await context.setOffline(false);
    const pulled = await until(page, 'pulled', (t) => t === 't1 true false');
    check(engine, 'online, the wake runs the handler, whose request lands', pulled === 't1 true false' && pulls === 1, `${pulled} pulls=${pulls}`);
    const retired = await until(page, 'tags', (t) => t === '');
    check(engine, 'its invocation completed, the tag is retired', retired === '', retired);

    // Refused: the handler registers the tag again, which stays through its backoff and a reload.
    refuse = true;
    await page.getByTestId('queue-t2').click();
    const failed = await until(page, 'pulled', (t) => t === 't2 false false');
    await page.waitForTimeout(500);
    check(engine, 'registered online, it fires at once; refused, its handler registers it again', failed === 't2 false false' && pulls === 2, `${failed} pulls=${pulls}`);
    check(engine, 'registered again, it stays listed through its backoff, not fired again', (await text(page, 'tags')) === 't2' && pulls === 2, `${await text(page, 'tags')} pulls=${pulls}`);
    await page.reload();
    const kept = await until(page, 'tags', (t) => t === 't2');
    await page.waitForTimeout(500);
    check(engine, 'a reload keeps the tag and its backoff', kept === 't2' && pulls === 2, `${kept} pulls=${pulls}`);

    await page.getByTestId('try-periodic').click();
    const periodic = await until(page, 'periodic', (t) => t !== 'none');
    check(engine, 'registerPeriodicSync rejects NotSupportedError', periodic === 'NotSupportedError', periodic);
    await context.close();
  } finally {
    await browser.close();
  }
}

server.stop();
puller.stop();
rmSync(dir, { recursive: true, force: true });
console.log(failures ? `${failures} failed` : 'all passed');
process.exit(failures ? 1 : 0);
