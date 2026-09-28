// The exact3 web spike's build: `bun host/web3/build.mjs <app> [--plan <baked app.plan>] [--out <dir>]`.
//
// 1. `exact-web3 js` compiles the plan (the app's Contract, or a baked
//    `app.plan` from `host/web/build.mjs`, whose resources carry their
//    build-time answers) to `app.js` + `app.css`.
// 2. Bun's bundler joins it with `rt.js`, tree-shaken and minified: one
//    module, everything needed to be interactive.
// 3. `index.html` carries the web host's own base stylesheet (from
//    `host/web/index.html`), the app's static classes, and the module.
// A data module loaded after first pixel (`rust-data.js`) and the agent
// adapter (`agent.js`, only under `?agent`) are separate files.
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const args = process.argv.slice(2);
const app = args[0];
const opt = (name) => { const i = args.indexOf(name); return i < 0 ? null : args[i + 1]; };
if (!app) { console.error('usage: bun host/web3/build.mjs <app> [--plan <app.plan> | --contract <file>] [--out <dir>] [--inline]'); process.exit(2); }
const appDir = resolve(root, 'apps', app);
const out = resolve(opt('--out') ?? `/tmp/exact3-dist/${app}`);
const gen = resolve(out, '.gen');
rmSync(out, { recursive: true, force: true });
mkdirSync(gen, { recursive: true });

const input = opt('--plan') ?? opt('--contract') ?? resolve(appDir, 'app.contract');
const cargo = spawnSync('cargo', ['run', '-q', '-p', 'exact-web3', '--', 'js', input, '-o', gen], { cwd: root, stdio: 'inherit' });
if (cargo.status !== 0) process.exit(cargo.status ?? 1);
cpSync(resolve(here, 'rt.js'), resolve(gen, 'rt.js'));
const manifest = JSON.parse(readFileSync(resolve(appDir, 'app.json'), 'utf8'));
// Rust data: the app's own module (`rust.module`), or a module generated
// from the DataSource its web build bakes with (host/web3/module.mjs).
const bakes = existsSync(resolve(appDir, 'web/build.rs')) && /contract::bake\(\s*plan,/.test(readFileSync(resolve(appDir, 'web/build.rs'), 'utf8'));
const rust = !!manifest.rust?.module || bakes;
writeFileSync(resolve(gen, 'main.js'), [
  "import app, { sources, wait } from './app.js';",
  "import { data, journal, clock, advance, commit } from './rt.js';",
  "const start = () => {",
  "  const state = app();",
  "  globalThis.exact = { ready: true, journal, clock, advance, commit, data, state };",
  // The agent adapter, only when the agent drives the page.
  "  if (clock.agent) globalThis.exact.ready = import('./agent.js').then(m => m.install(globalThis.exact));",
  "};",
  ...(rust ? [
    // Rust data: loaded after first pixel, asked synchronously once ready;
    // a plan with a resource that has no compiled value waits for it.
    "const load = () => import('./rust-data.js').then(m => m.install(data, sources));",
    "if (wait) load().then(start); else { start(); requestAnimationFrame(() => setTimeout(load)); }",
  ] : ['start();']),
].join('\n'));
for (const f of ['agent.js', 'rust-data.js']) cpSync(resolve(here, f), resolve(gen, f));
const bundled = spawnSync('bun', ['build', resolve(gen, 'main.js'), '--minify', '--format=esm', '--splitting', '--outdir', out, '--entry-naming', 'app.js', '--chunk-naming', '[name]-[hash].js'], { cwd: root, stdio: 'inherit' });
if (bundled.status !== 0) process.exit(bundled.status ?? 1);

// The web host's base stylesheet, as its build writes it (comments out).
const base = readFileSync(resolve(root, 'host/web/index.html'), 'utf8').match(/<style>([\s\S]*?)<\/style>/)[1]
  .replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s*\n\s*/g, '').replace(/\s*([{};:,>])\s*/g, '$1').replace(/;}/g, '}');
const css = readFileSync(resolve(gen, 'app.css'), 'utf8');
const viewport = existsSync(resolve(gen, 'viewport.txt')) ? readFileSync(resolve(gen, 'viewport.txt'), 'utf8') : 'width=device-width, initial-scale=1';
writeFileSync(resolve(out, 'index.html'), `<!doctype html>
<html lang="en">
<meta charset="utf-8">
<base href="/">
<title>${manifest.name}</title>
<meta name="viewport" content="${viewport}">
<style>${base}${css}</style>
<div id="exact-root"></div>
${args.includes('--inline') ? `<script type="module">${readFileSync(resolve(out, 'app.js'), 'utf8').replaceAll('</script', '<\\/script')}</script>` : '<script type="module" src="./app.js"></script>'}
`);
if (existsSync(resolve(appDir, 'assets'))) cpSync(resolve(appDir, 'assets'), resolve(out, 'assets'), { recursive: true });
if (existsSync(resolve(appDir, 'deck'))) cpSync(resolve(appDir, 'deck'), resolve(out, 'deck'), { recursive: true });
// The Rust data module and the plan it binds, from the wasm build the baked plan came from.
// `--data <dist>` names another wasm build's module (a synthetic plan over an app's sources).
if (rust) {
  mkdirSync(resolve(out, 'rust/wasm'), { recursive: true });
  const from = opt('--data') ?? (opt('--plan') && dirname(resolve(opt('--plan'))));
  const built = from && existsSync(resolve(from, 'rust/wasm/app.module.wasm')) ? resolve(from, 'rust/wasm/app.module.wasm') : (await import('./module.mjs')).buildModule(app);
  cpSync(built, resolve(out, 'rust/wasm/app.module.wasm'));
  if (opt('--plan')) cpSync(resolve(opt('--plan')), resolve(out, 'app.plan'));
  else if (spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', input, '-o', resolve(out, 'app.plan')], { cwd: root, stdio: 'inherit' }).status !== 0) process.exit(1);
}
console.log(`${out}: built`);
