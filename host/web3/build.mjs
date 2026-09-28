// The exact3 web spike's build: `bun host/web3/build.mjs <app> [--plan <baked app.plan>] [--out <dir>]`.
//
// 1. `exact-web3 js` compiles the plan (the app's Contract, or a baked
//    `app.plan` from `host/web/build.mjs`, whose resources carry their
//    build-time answers) to `app.js` + `app.css`.
// 2. Bun's bundler joins it with `rt.js`, tree-shaken and minified: one
//    module, everything needed to be interactive.
// 3. `index.html` carries the web host's own base stylesheet (from
//    `host/web/index.html`), the app's static classes, and the module, with a
//    `modulepreload` in the head for it and its static imports, so a served
//    page (host/render/src/page.rs `page_js`) fetches its runtime while the
//    document streams.
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
if (!app) { console.error('usage: bun host/web3/build.mjs <app> [--plan <app.plan> | --contract <file>] [--out <dir>] [--inline] [--render rust|js]'); process.exit(2); }
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
// A TypeScript source (`app.ts`) runs in the page, bundled with it.
const ts = !rust && existsSync(resolve(appDir, 'app.ts'));
writeFileSync(resolve(gen, 'main.js'), [
  "import app, { sources, wait } from './app.js';",
  "import { data, journal, clock, advance, commit, inflight, Views, viewId } from './rt.js';",
  ...(ts ? ["import { install as ts } from './ts-data.js';", 'ts(data);'] : []),
  "const start = () => {",
  "  const state = app();",
  "  globalThis.exact = Object.assign(globalThis.exact ?? {}, { ready: true, journal, clock, advance, commit, data, state, inflight, views: Views, viewId });",
  // The agent adapter, only when the agent drives the page.
  "  if (clock.agent) globalThis.exact.ready = import('./agent.js').then(m => m.install(globalThis.exact));",
  "};",
  ...(rust ? [
    // Rust data: loaded after first pixel, asked synchronously once ready;
    // a plan with a resource that has no compiled value waits for it.
    // Counted in flight, so `clock settle` waits for the source to be ready.
    "const load = () => import('./rust-data.js').then(m => m.install(data, sources)).finally(() => inflight.n--);",
    "inflight.n++;",
    "if (wait) load().then(start); else { start(); requestAnimationFrame(() => setTimeout(load)); }",
  ] : ['start();']),
].join('\n'));
for (const f of ['agent.js', 'rust-data.js']) cpSync(resolve(here, f), resolve(gen, f));
cpSync(resolve(root, 'host/web/navigation.js'), resolve(gen, 'navigation.js'));
if (ts) writeFileSync(resolve(gen, 'ts-data.js'), readFileSync(resolve(here, 'ts-data.js'), 'utf8').replace('__APP_TS__', resolve(appDir, 'app.ts')));
// The server bundle a JavaScript render runs (render.mjs), one script per VM context.
writeFileSync(resolve(gen, 'main-server.js'), [
  `import app${rust ? ', { sources }' : ''} from './app.js';`,
  "import { data, clock, inflight, Resources, routeAt, Head } from './rt.js';",
  "import { types, sourceTypes, pages } from './names.js';",
  "import { answers } from './checkpoint.js';",
  ...(ts ? ["import { install } from './ts-data.js';"] : rust ? ["import { install } from './rust-data.js';"] : []),
  'globalThis.__render = async deadline => {',
  '  const t0 = performance.now();',
  ...(ts ? ['  install(data);'] : rust ? ['  await install(data, sources, async p => __files(p));'] : []),
  '  app();',
  '  const end = t0 + deadline;',
  '  do await new Promise(r => setTimeout(r, 1)); while (inflight.n && performance.now() < end);',
  '  const route = routeAt(location.pathname + location.search), [render, activate] = pages[route] ?? ["build", "inferred"];',
  '  return { root: document.rootHTML(), title: Head.headTitle, description: Head.headDescription, time: clock.now, answers: answers(Resources, types[2], sourceTypes),',
  '    pending: Resources.filter(r => r.ticket).map(r => r.name), activate: ["idle", "interaction", "never"].includes(activate) ? activate : "eager", policy: render, notfound: !!pages[route]?.[2], render: performance.now() - t0 };',
  '};',
].join('\n'));
cpSync(resolve(here, 'checkpoint.js'), resolve(gen, 'checkpoint.js'));
if (spawnSync('bun', ['build', resolve(gen, 'main-server.js'), '--format=iife', '--outfile', resolve(gen, 'server.js')], { cwd: root, stdio: ['ignore', 'ignore', 'inherit'] }).status !== 0) process.exit(1);
const bundled = spawnSync('bun', ['build', resolve(gen, 'main.js'), '--minify', '--format=esm', '--splitting', '--outdir', out, '--entry-naming', 'app.js', '--chunk-naming', '[name]-[hash].js'], { cwd: root, stdio: 'inherit' });
if (bundled.status !== 0) process.exit(bundled.status ?? 1);

// The web host's base stylesheet, as its build writes it (comments out).
const base = readFileSync(resolve(root, 'host/web/index.html'), 'utf8').match(/<style>([\s\S]*?)<\/style>/)[1]
  .replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s*\n\s*/g, '').replace(/\s*([{};:,>])\s*/g, '$1').replace(/;}/g, '}');
const css = readFileSync(resolve(gen, 'app.css'), 'utf8');
const viewport = existsSync(resolve(gen, 'viewport.txt')) ? readFileSync(resolve(gen, 'viewport.txt'), 'utf8') : 'width=device-width, initial-scale=1';
// The entry and the chunks it imports statically (none, unless a split
// shares one with a loaded piece): what a page preloads from its head.
const statics = ['app.js', ...new Set([...readFileSync(resolve(out, 'app.js'), 'utf8').matchAll(/(?:^|[;}\s])import(?:[^"'();]*?from)?\s*["']\.\/([^"']+\.js)["']/g)].map(m => m[1]))];
const preloads = args.includes('--inline') ? '' : statics.map(f => `<link rel="modulepreload" href="./${f}">\n`).join('');
writeFileSync(resolve(out, 'index.html'), `<!doctype html>
<html lang="en">
<meta charset="utf-8">
<base href="/">
<title>${manifest.name}</title>
<meta name="viewport" content="${viewport}">
${preloads}<style>${base}${css}</style>
<div id="exact-root"></div>
${args.includes('--inline') ? `<script type="module">${readFileSync(resolve(out, 'app.js'), 'utf8').replaceAll('</script', '<\\/script')}</script>` : '<script type="module" src="./app.js"></script>'}
`);
if (existsSync(resolve(gen, 'markdown.flag'))) cpSync((await import('./module.mjs')).buildMarkdown(), resolve(out, 'markdown.wasm'));
// The app's GPU module, as its wasm build made it, with the web host's glue (a loaded capability).
const gpuFrom = opt('--data') ?? (opt('--plan') && dirname(resolve(opt('--plan'))));
if (gpuFrom && existsSync(resolve(gpuFrom, 'gpu.js'))) {
  for (const f of ['gpu.js', 'gpu_bg.wasm']) cpSync(resolve(gpuFrom, f), resolve(out, f));
  for (const f of ['gpu-glue.js', 'gpu-assets.js', 'pace.js']) cpSync(resolve(root, 'host/web', f), resolve(out, f));
  if (existsSync(resolve(gpuFrom, 'shaders'))) cpSync(resolve(gpuFrom, 'shaders'), resolve(out, 'shaders'), { recursive: true });
}
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
// The plan beside the pages: a render server (either renderer) reads it.
if (opt('--plan') && !existsSync(resolve(out, 'app.plan'))) cpSync(resolve(opt('--plan')), resolve(out, 'app.plan'));
// Pages at build (LLP 1048.000): `--render rust` (the default) runs the app's
// native render entry (`<app>-render`, exact_render) over this shell;
// `--render js` runs this runtime under Bun (render.mjs). Either page adopts.
const pages = JSON.parse(readFileSync(resolve(gen, 'pages.json'), 'utf8'));
const how = opt('--render') ?? 'rust';
// The shell a page is composed over stays as shell.html (a render server's, too).
if (pages.length) cpSync(resolve(out, 'index.html'), resolve(out, 'shell.html'));
if (pages.length && how === 'rust') {
  const bin = `${app}-render`;
  const at = [['linux', `${app}-linux`], ['web', `${app}-web`]].find(([dir]) => existsSync(resolve(appDir, dir, 'src/bin', `${bin}.rs`)));
  if (!at) { console.error(`--render rust: ${app} has no ${bin} entry; use --render js`); process.exit(1); }
  const plan = opt('--plan') ? resolve(opt('--plan')) : resolve(out, 'app.plan');
  const r = spawnSync('cargo', ['run', '--release', '-q', '-p', at[1], '--bin', bin, '--', '--plan', plan, '--name', manifest.name, '--shell', resolve(out, 'index.html'), '--build'],
    { cwd: root, encoding: 'utf8', maxBuffer: 256 << 20, env: { ...process.env, EXACT_UPDATE_TRUST: 'development' } });
  if (r.status !== 0) { console.error(r.stderr); process.exit(1); }
  for (const doc of r.stdout.split('\n').filter(Boolean).map(l => JSON.parse(l))) {
    if (doc.error) { console.error(`${bin} ${doc.location}: ${doc.error}`); process.exit(1); }
    const file = doc.notfound ? '404.html' : `${decodeURIComponent(doc.location).replace(/^\/|\/$/g, '')}/index.html`.replace(/^\//, '');
    mkdirSync(dirname(resolve(out, file)), { recursive: true });
    writeFileSync(resolve(out, file), doc.page);
  }
  console.log(`${out}: ${pages.length} pages rendered by ${bin}`);
} else if (pages.length && how === 'js') {
  const r = spawnSync('bun', [resolve(here, 'render.mjs'), out, '--build'], { cwd: root, stdio: 'inherit' });
  if (r.status !== 0) process.exit(1);
}
console.log(`${out}: built`);
