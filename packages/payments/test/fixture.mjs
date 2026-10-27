// The conformance fixture (LLP 1133.001 D7), as the web suites make it: an app
// `exact new` made outside the repo, its files replaced by `fixture/`'s,
// depending on this package by `file:` and installed; the print lab it pays;
// and a built fixture's page in a Playwright browser.
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
export { checker } from '../../camera/test/fixture.mjs';

export const here = import.meta.dir, pkg = resolve(here, '..'), root = resolve(pkg, '../..');

export const run = (cmd, args, cwd, env = process.env) => {
  const r = spawnSync(cmd, args, { cwd, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')}: ${r.stdout.slice(-4000)}\n${r.stderr.slice(-4000)}`);
  return r.stdout;
};

/** The fixture app's directory and its bundle id; `payments` replaces this package (a tarball's path). */
export function makeFixture({ payments = `file:${pkg}`, dependencies = {} } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'exact-payments-')), app = join(dir, 'paymentsfixture');
  run('bun', [resolve(root, 'scripts/exact.mjs'), 'new', app], root);
  for (const f of ['app.contract', 'app.ts', 'strings']) cpSync(join(here, 'fixture', f), join(app, f), { recursive: true });
  rmSync(join(app, 'app.test.contract'), { force: true });
  const id = JSON.parse(readFileSync(join(app, 'app.json'), 'utf8')).app.id;
  writeFileSync(join(app, 'app.ts'), readFileSync(join(app, 'app.ts'), 'utf8').replace('__APP_ID__', id));
  const manifest = JSON.parse(readFileSync(join(app, 'package.json'), 'utf8'));
  manifest.dependencies = { ...manifest.dependencies, '@exact/payments': payments, ...dependencies };
  writeFileSync(join(app, 'package.json'), JSON.stringify(manifest, null, 2));
  run('bun', ['install'], app);
  return { dir, app, id };
}

/** The print lab on 4334 with `--agent` (synthetic payments accepted); `stop()` ends it. */
export async function lab(...switches) {
  const child = spawn('bun', [resolve(root, 'apps/photo-editor/fixture/prints.ts'), '--agent', ...switches], { stdio: ['ignore', 'pipe', 'inherit'] });
  await new Promise((ok, fail) => {
    child.stdout.on('data', (b) => { if (String(b).includes('prints: http://')) ok(); });
    child.on('exit', (code) => fail(new Error(`the print lab exited (${code}): is 4334 taken?`)));
  });
  return { log: () => fetch('http://127.0.0.1:4334/fixture/log').then((r) => r.json()), stop: () => child.kill() };
}

/** A built app's page in `engine` (`chromium`, `firefox` or `webkit`): `f(page, until, tap)`'s result. */
export async function inPage(app, engine, f) {
  const dist = join(app, 'target/web-dist');
  const types = { '.js': 'text/javascript', '.html': 'text/html', '.wasm': 'application/wasm', '.json': 'application/json', '.css': 'text/css' };
  const server = Bun.serve({ port: 0, async fetch(req) {
    const asked = new URL(req.url).pathname, path = asked === '/' ? '/index.html' : asked, file = Bun.file(join(dist, path));
    return (await file.exists()) ? new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream' } }) : new Response('not found', { status: 404 });
  } });
  const playwright = await import(resolve(root, 'node_modules/playwright-core/index.mjs'));
  const browser = await playwright[engine].launch();
  try {
    const page = await (await browser.newContext()).newPage();
    await page.goto(`http://127.0.0.1:${server.port}`);
    const text = (id) => page.getByTestId(id).innerText({ timeout: 2000 }).catch(() => '');
    const until = async (id, want, ms = 10000) => {
      const end = Date.now() + ms;
      let got;
      while (Date.now() < end) { if (want(got = await text(id))) return got; await page.waitForTimeout(100); }
      return got;
    };
    return await f(page, until, (id) => page.getByTestId(id).click({ timeout: 5000 }));
  } finally {
    await browser.close();
    server.stop();
  }
}

/** Builds the app for the web, as `exact.mjs web-build` does. */
export const webBuild = (app) => run('bun', ['exact.mjs', 'web-build', '--render', 'none'], app);
