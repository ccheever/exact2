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
if (!app) { console.error('usage: bun host/web3/build.mjs <app> [--plan <app.plan>] [--out <dir>]'); process.exit(2); }
const appDir = resolve(root, 'apps', app);
const out = resolve(opt('--out') ?? `/tmp/exact3-dist/${app}`);
const gen = resolve(out, '.gen');
rmSync(out, { recursive: true, force: true });
mkdirSync(gen, { recursive: true });

const input = opt('--plan') ?? resolve(appDir, 'app.contract');
const cargo = spawnSync('cargo', ['run', '-q', '-p', 'exact-web3', '--', 'js', input, '-o', gen], { cwd: root, stdio: 'inherit' });
if (cargo.status !== 0) process.exit(cargo.status ?? 1);
cpSync(resolve(here, 'rt.js'), resolve(gen, 'rt.js'));
const manifest = JSON.parse(readFileSync(resolve(appDir, 'app.json'), 'utf8'));
const rust = !!manifest.rust?.module;
writeFileSync(resolve(gen, 'main.js'), [
  "import app from './app.js';",
  "import { data, journal, clock, advance, commit } from './rt.js';",
  'app();',
  // The agent adapter, only when the agent drives the page.
  "globalThis.exact = { ready: true, journal, clock, advance, commit, data };",
  "if (clock.agent) globalThis.exact.ready = import('./agent.js').then(m => m.install(globalThis.exact));",
  ...(rust ? [
    // Rust data: loaded after first pixel, asked synchronously once ready.
    "requestAnimationFrame(() => setTimeout(() => import('./rust-data.js').then(m => m.install(data, app.sources))));",
  ] : []),
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
<script type="module" src="./app.js"></script>
`);
if (existsSync(resolve(appDir, 'assets'))) cpSync(resolve(appDir, 'assets'), resolve(out, 'assets'), { recursive: true });
if (existsSync(resolve(appDir, 'deck'))) cpSync(resolve(appDir, 'deck'), resolve(out, 'deck'), { recursive: true });
console.log(`${out}: built`);
