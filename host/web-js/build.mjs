// The web build's JS target: `bun host/web-js/build.mjs <app> [--plan <baked app.plan>] [--out <dir>]`.
//
// 1. `exact-web-js js` compiles the plan (the app's Contract, or a baked
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
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildEditor, buildFlow, buildMarkdown, buildModule, buildMotion, fresh, moduleGrants } from './module.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '../..');
const args = process.argv.slice(2);
const app = args[0];
const opt = (name) => { const i = args.indexOf(name); return i < 0 ? null : args[i + 1]; };
if (!app) { console.error('usage: bun host/web-js/build.mjs <app> [--plan <app.plan> | --contract <file>] [--out <dir>] [--inline] [--render rust|js|none]'); process.exit(2); }
// An app outside this repo is where `EXACT_APP_DIR` says (scripts/app.mjs).
const appDir = process.env.EXACT_APP_DIR ? resolve(process.env.EXACT_APP_DIR) : resolve(root, 'apps', app);
const out = resolve(opt('--out') ?? `/tmp/exact-web-js-dist/${app}`);
// `--production` (delivery, scripts/deploy.mjs): a release over a wasm bake's
// baked plan (`--plan <dist>/app.plan`) — agent mode refused, and the bake's
// origin files (the envelope, the web manifest, install and auth pages,
// sitemap) and head links carried, so the web root it publishes is the
// wasm root's in everything but the program (LLP 1071 §7, delivery).
const production = args.includes('--production');
if (production && !opt('--plan')) { console.error('--production builds over a wasm bake: name its --plan <dist>/app.plan'); process.exit(2); }
const gen = resolve(out, '.gen');
rmSync(out, { recursive: true, force: true });
mkdirSync(gen, { recursive: true });

const input = opt('--plan') ?? opt('--contract') ?? resolve(appDir, 'app.contract');
const manifest = JSON.parse(readFileSync(resolve(appDir, 'app.json'), 'utf8'));
// Rust data: the app's own module (`rust.module`), or a module generated
// from the DataSource its web build bakes with (host/web-js/module.mjs).
const bakes = existsSync(resolve(appDir, 'web/build.rs')) && /contract::bake\(\s*plan,/.test(readFileSync(resolve(appDir, 'web/build.rs'), 'utf8'));
const rust = !!manifest.rust?.module || bakes;
// A TypeScript source (`app.ts`) runs in the page, bundled with it; beside a
// Rust one (LLP 1027.002), the Rust module answers what it owns and the
// TypeScript module the rest (ts-data.js, rust-data.js). A synthetic plan
// over another app's sources (`--data`) asks the Rust module only.
const ts = existsSync(resolve(appDir, 'app.ts')) && !(rust && opt('--data'));
const mixed = rust && ts;
// Native modules (LLP 1024): the app's module artifact, `modules/web/` beside
// the page as `modules/`, with the web host's adapter (native.js).
const pageModules = existsSync(resolve(appDir, 'modules/web/index.js'));
// The surfaces the app's GPU module draws (its crate's surface table); any
// other surface is drawn by a data source on Canvas 2D, which the backend
// refuses by name.
const gpuLib = resolve(appDir, 'gpu/src/lib.rs');
const gpuSurfaces = existsSync(gpuLib) ? [...readFileSync(gpuLib, 'utf8').matchAll(/\("([a-z][a-z0-9-]*)", \d+, [a-z_:]+\)/g)].map(m => m[1]) : [];
// The compiler, run as its built binary when nothing it was built from
// changed (module.mjs `fresh`; `cargo run`'s own check costs ~0.4 s an edit).
const compiler = resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'debug/exact-web-js');
const [cmd, pre] = fresh(compiler, `${compiler}.d`) ? [compiler, []] : ['cargo', ['run', '-q', '-p', 'exact-web-js', '--']];
const cargo = spawnSync(cmd, [...pre, 'js', input, '-o', gen], { cwd: root, stdio: 'inherit', env: { ...process.env, EXACT_JS_GPU_SURFACES: gpuSurfaces.join(',') } });
if (cargo.status !== 0) process.exit(cargo.status ?? 1);
for (const f of ['rt.js', 'shape.js']) cpSync(resolve(here, f), resolve(gen, f));
// Canvas 2D surfaces (a loaded chunk: this runtime's engine over the web
// host's own replayer) are drawn by the Rust data module, or by a
// TypeScript source's `draw` in the page (ts-draw.js, in the same chunk).
const canvas2d = existsSync(resolve(gen, 'canvas2d.flag'));
if (canvas2d && !rust && !ts) { console.error(`${app}: a Canvas 2D surface with no data source to draw it`); process.exit(1); }
cpSync(resolve(here, 'canvas2d.js'), resolve(gen, 'canvas2d.js'));
const tsDraws = canvas2d && ts && /export\s+(?:function|const|let)\s+draw\b|export\s*\{[^}]*\bdraw\b/.test(readFileSync(resolve(appDir, 'app.ts'), 'utf8'));
if (tsDraws) writeFileSync(resolve(gen, 'ts-draw.js'), readFileSync(resolve(here, 'ts-draw.js'), 'utf8').replace('__APP_TS__', resolve(appDir, 'app.ts')).replace('__RECORDER__', resolve(root, 'canvas/recorder.js')));
if (tsDraws) writeFileSync(resolve(gen, 'path2d.js'), 'export const browserPath2D = globalThis.Path2D;\n');
writeFileSync(resolve(gen, 'draw.js'), tsDraws ? "export { drawer } from './ts-draw.js';\n" : 'export const drawer = null;\n');
cpSync(resolve(root, 'host/web/canvas2d-glue.js'), resolve(gen, 'canvas2d-glue.js'));
// `exactTime`: the runner's reserved source (runner/src/time.rs), answered
// before the app's, as the web host tells the wasm runner (navigation.js).
const time = /"exactTime":/.test(readFileSync(resolve(gen, 'app.js'), 'utf8').match(/export const sources=\{[^}]*\}/)?.[0] ?? '');
// A file input, `saveFile` or `share` (files.js), registered before any press.
const files = existsSync(resolve(gen, 'files.flag'));
writeFileSync(resolve(gen, 'main.js'), [
  "import app, { sources, wait } from './app.js';",
  ...(files ? ["import './files.js';"] : []),
  "import { data, journal, clock, advance, commit, inflight, Views, viewId, After, resolvedLocale, Resources } from './rt.js';",
  ...(time ? [
    "import { sourceTypes } from './names.js';",
    "import { reportTime, reportPlace } from './navigation.js';",
    "data.reserved = { exactTime: () => {",
    "  const [epochAtZero, utcOffset] = reportTime(clock.now), [locale, timeZone, seed] = reportPlace().split('\\0');",
    "  const f = { epochAtZero, utcOffset, locale, timeZone, seed: Number(seed), resolvedLocale: resolvedLocale() };",
    "  return Object.keys(sourceTypes.exactTime[1]).map(k => f[k]);",
    "} };",
  ] : []),
  ...(ts ? ["import { install as ts } from './ts-data.js';", `ts(data, ${mixed}${pageModules ? ", () => import('./native.js')" : ''});`] : []),
  "const start = () => {",
  "  const state = app();",
  "  globalThis.exact = Object.assign(globalThis.exact ?? {}, { ready: true, journal, clock, advance, commit, data, state, inflight, views: Views, viewId, After, resources: Resources });",
  // The agent adapter, only when the agent drives the page.
  ...(production ? [] : ["  if (clock.agent) globalThis.exact.ready = import('./agent.js').then(m => m.install(globalThis.exact));"]),
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
for (const f of ['agent.js', 'rust-data.js', 'list.js', 'facts.js', 'symbols.js', 'motion.js', 'transform.js', 'svg-transform.js', 'arrange.js', 'reorder.js', 'flow.js', 'native.js']) cpSync(resolve(here, f), resolve(gen, f));
// The web host's own pieces, loaded after first paint (motion.js, a pan, `select`, text flow, rt.js `pr`, native.js, rt.js `geo`).
for (const f of ['motion-glue.js', 'input-glue.js', 'markup-editor.js', 'textflow-glue.js', 'timer-glue.js', 'presence-glue.js', 'native-glue.js', 'geometry-glue.js']) cpSync(resolve(root, 'host/web', f), resolve(gen, f));
// Virtualized lists' browser half, the web host's own, loaded after first paint.
cpSync(resolve(root, 'host/web/collection-glue.js'), resolve(gen, 'collection-glue.js'));
// Animated images on the agent's clock, the web host's own (agent.js only).
cpSync(resolve(root, 'host/web/image-glue.js'), resolve(gen, 'image-glue.js'));
cpSync(resolve(root, 'host/web/navigation.js'), resolve(gen, 'navigation.js'));
// The agent adapter reads its own copies of the modules it shares with the
// entry: a module lives in one chunk, so what only the agent reads from
// navigation.js (the guest outline and taps, the environment) or names.js
// (every slot's type) would otherwise ride in every page's entry module.
for (const f of ['navigation.js', 'names.js']) cpSync(resolve(gen, f), resolve(gen, 'agent-' + f));
writeFileSync(resolve(gen, 'agent.js'), readFileSync(resolve(gen, 'agent.js'), 'utf8').replace("from './names.js'", "from './agent-names.js'").replace("from './navigation.js'", "from './agent-navigation.js'"));
// A TypeScript app whose web build script does more than bake it (Messages
// compiles its schema and copies its device into files its TypeScript
// imports, gitignored) has that script run first, as the wasm build does:
// `cargo check` runs the build script, and Cargo reruns it only when its
// inputs changed.
const webScript = resolve(appDir, 'web/build.rs');
if (ts && existsSync(webScript) && !/^\s*fn main\(\)\s*\{\s*exact_js_bake::build\w*\(/m.test(readFileSync(webScript, 'utf8'))) {
  const r = spawnSync('cargo', ['check', '-q', '--manifest-path', resolve(appDir, 'web/Cargo.toml')], { cwd: root, stdio: 'inherit' });
  if (r.status !== 0) { console.error(`${app}: its web build script failed`); process.exit(1); }
}
// A source granted `auth.session` signs in through the system browser (auth.js, LLP 1069.006).
const grants = ts ? String((await import(resolve(appDir, 'app.ts'))).grants ?? '') : '';
const auth = /^\s*auth\.session\s/m.test(grants);
if (ts) writeFileSync(resolve(gen, 'ts-data.js'), readFileSync(resolve(here, 'ts-data.js'), 'utf8').replace('__APP_TS__', resolve(appDir, 'app.ts'))
  .replace('__AUTH_IMPORT__', auth ? "import { install as signIn } from './auth.js';" : '')
  .replace('__AUTH_INSTALL__', auth ? `signIn(${JSON.stringify(grants)}, () => asking);` : ''));
for (const f of ['auth-glue.js', 'storage-environment.js']) cpSync(resolve(root, 'host/web', f), resolve(gen, f));
cpSync(resolve(here, 'auth.js'), resolve(gen, 'auth.js'));
cpSync(resolve(here, 'files.js'), resolve(gen, 'files.js'));
// The server bundle a JavaScript render runs (render.mjs), one script per VM context.
writeFileSync(resolve(gen, 'main-server.js'), [
  `import app${rust ? ', { sources }' : ''} from './app.js';`,
  "import { data, clock, inflight, Resources, routeAt, Head } from './rt.js';",
  "import { types, sourceTypes, pages } from './names.js';",
  "import { answers } from './checkpoint.js';",
  ...(ts ? ["import { install as ts } from './ts-data.js';"] : []),
  ...(rust ? ["import { install } from './rust-data.js';"] : []),
  // Two steps, so a server can send the page's head between them
  // (render.mjs): the app starts and names its route, then settles.
  'let t0, route;',
  'globalThis.__start = async () => {',
  '  t0 = performance.now();',
  ...(ts ? [`  ts(data, ${mixed});`] : []),
  ...(rust ? ['  await install(data, sources, async p => __files(p));'] : []),
  '  app();',
  '  route = routeAt(location.pathname + location.search);',
  '  const [render, activate] = pages[route] ?? ["build", "inferred"];',
  '  return { activate: ["idle", "interaction", "never"].includes(activate) ? activate : "eager", policy: render, notfound: !!pages[route]?.[2] };',
  '};',
  'globalThis.__render = async deadline => {',
  '  const end = t0 + deadline;',
  '  do await new Promise(r => setTimeout(r, 1)); while (inflight.n && performance.now() < end);',
  '  return { root: document.rootHTML(), title: Head.headTitle, description: Head.headDescription, time: clock.now, answers: answers(Resources, types[2], sourceTypes),',
  '    pending: Resources.filter(r => r.ticket).map(r => r.name), render: performance.now() - t0 };',
  '};',
].join('\n'));
cpSync(resolve(here, 'checkpoint.js'), resolve(gen, 'checkpoint.js'));
// A release admits no agent mode (LLP 1069.007 D2): every file that reads
// `?agent` declares AGENT_ADMITTED, written false here, as host/web/build.mjs
// gates the wasm host's.
if (production) for (const f of readdirSync(gen).filter(f => f.endsWith('.js'))) {
  const code = readFileSync(resolve(gen, f), 'utf8');
  if (/searchParams\.has\(["']agent["']\)|params\.has\(["']agent["']\)|URLSearchParams\([^)]*\)\.has\(["']agent["']\)/.test(code) && !code.includes('const AGENT_ADMITTED = true;')) { console.error(`${f} reads ?agent without AGENT_ADMITTED; a production build must not admit agent mode`); process.exit(1); }
  writeFileSync(resolve(gen, f), code.replaceAll('const AGENT_ADMITTED = true;', 'const AGENT_ADMITTED = false;'));
}
// Bun's bundler, in this process. A build that renders nothing (the dev
// loop's) makes no server bundle.
const how = opt('--render') ?? 'rust';
const bundle = async (options) => {
  const r = await Bun.build(options);
  for (const m of r.logs) console.error(String(m));
  if (!r.success) process.exit(1);
};
if (how !== 'none') await bundle({ entrypoints: [resolve(gen, 'main-server.js')], format: 'iife', outdir: gen, naming: 'server.js' });
await bundle({ entrypoints: [resolve(gen, 'main.js')], minify: true, format: 'esm', splitting: true, outdir: out, naming: { entry: 'app.js', chunk: '[name]-[hash].js' } });

// The web host's base stylesheet, as its build writes it (comments out).
const base = readFileSync(resolve(root, 'host/web/index.html'), 'utf8').match(/<style>([\s\S]*?)<\/style>/)[1]
  .replace(/\/\*[\s\S]*?\*\//g, '').replace(/\s*\n\s*/g, '').replace(/\s*([{};:,>])\s*/g, '$1').replace(/;}/g, '}');
const css = readFileSync(resolve(gen, 'app.css'), 'utf8');
const viewport = existsSync(resolve(gen, 'viewport.txt')) ? readFileSync(resolve(gen, 'viewport.txt'), 'utf8') : 'width=device-width, initial-scale=1';
// The entry and the chunks it imports statically (none, unless a split
// shares one with a loaded piece): what a page preloads from its head.
const statics = ['app.js', ...new Set([...readFileSync(resolve(out, 'app.js'), 'utf8').matchAll(/(?:^|[;}\s])import(?:[^"'();]*?from)?\s*["']\.\/([^"']+\.js)["']/g)].map(m => m[1]))];
// Declared fonts: a preload each, from the head (LLP 1019; the faces are in app.css).
const fonts = existsSync(resolve(gen, 'preloads.html')) ? readFileSync(resolve(gen, 'preloads.html'), 'utf8') : '';
const preloads = fonts + (args.includes('--inline') ? '' : statics.map(f => `<link rel="modulepreload" href="./${f}">\n`).join(''));
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
if (production) {
  const bake = dirname(resolve(opt('--plan')));
  const links = readFileSync(resolve(bake, 'index.html'), 'utf8').match(/^<(?:link rel="(?:alternate|manifest|icon)"|meta name="theme-color")[^>]*>$/gm) ?? [];
  writeFileSync(resolve(out, 'index.html'), readFileSync(resolve(out, 'index.html'), 'utf8').replace(/(<meta name="viewport"[^>]*>\n)/, `$1${links.map(l => l + '\n').join('')}`)
    .replace('<html lang="en">', readFileSync(resolve(bake, 'index.html'), 'utf8').match(/<html lang="[^"]*">/)?.[0] ?? '<html lang="en">'));
  for (const f of ['exact.json', 'manifest.json', 'robots.txt', 'sitemap.xml', '.well-known', '.exact', 'rust']) if (existsSync(resolve(bake, f))) cpSync(resolve(bake, f), resolve(out, f), { recursive: true });
  // What the envelope names beside the plan and assets, for a native client
  // following the page's link (LLP 1023 D1): the Rust module's files (above,
  // the bytes this page loads) and a TypeScript module's, under `module/`,
  // since this root's `app.js` is its runtime.
  const envelope = existsSync(resolve(bake, 'exact.json')) && JSON.parse(readFileSync(resolve(bake, 'exact.json'), 'utf8'));
  if (envelope?.module) {
    mkdirSync(resolve(out, 'module'), { recursive: true });
    for (const card of Object.values(envelope.module)) {
      const name = card.url.replace(/^\.\//, '');
      cpSync(resolve(bake, name), resolve(out, 'module', name));
      card.url = `./module/${name}`;
    }
    writeFileSync(resolve(out, 'exact.json'), JSON.stringify(envelope) + '\n');
  }
}
// The web's auth callback page (host/web/build.mjs does the same for the wasm target).
if (auth) {
  mkdirSync(resolve(out, '.exact/auth'), { recursive: true });
  cpSync(resolve(root, 'host/web/auth-callback.html'), resolve(out, '.exact/auth/callback'));
  cpSync(resolve(root, 'host/web/auth-callback.js'), resolve(out, '.exact/auth/callback.js'));
  const { authClientMetadata } = await import('../../scripts/app.mjs');
  const callbacks = grants.split('\n').map(l => l.trim()).filter(l => l.startsWith('auth.callback ')).map(l => l.slice(14).trim());
  const docs = authClientMetadata({ origin: manifest.app?.origin ?? null, displayName: manifest.app?.name ?? manifest.name, manifest }, callbacks);
  for (const [name, doc] of Object.entries(docs)) writeFileSync(resolve(out, `.exact/auth/${name}.json`), JSON.stringify(doc, null, 2) + '\n');
}
if (existsSync(resolve(gen, 'markdown.flag'))) cpSync(buildMarkdown(), resolve(out, 'markdown.wasm'));
// The motion engine (host/web-js/motion), only for a plan that uses motion.
if (existsSync(resolve(gen, 'motion.flag'))) cpSync(buildMotion(), resolve(out, 'motion.wasm'));
// The Markdown editor's rules (exact-markdown-editor), beside its chunk.
if (existsSync(resolve(gen, 'editor.flag'))) cpSync(buildEditor(), resolve(out, 'markup-editor.wasm'));
// The exclusions walker (exact-textflow's `textflow-web`), beside its chunk.
if (existsSync(resolve(gen, 'flow.flag'))) cpSync(buildFlow(), resolve(out, 'textflow.wasm'));
// The app's GPU module (LLP 1009 D2), built here as the wasm target's build
// makes it, with the web host's glue: a loaded capability.
// Built again only when something a module was built from changed (Cargo's
// dep-info, module.mjs `fresh`): the bindings and wasm-opt with it.
if (existsSync(gpuLib)) {
  const { gpuModules, resolveApp, webGpuArtifacts, copyShaders, shaderWatchRoots } = await import('../../scripts/app.mjs');
  const target = resolveApp(app), cache = resolve(target.target, 'web-js-gpu', target.name);
  const crates = [['gpu', target.crate('gpu')], ...gpuModules(target.manifest).map(({ name }) => [`gpu/${name}`, target.crate(`gpu-${name}`)])];
  const shaders = shaderWatchRoots(target).filter(existsSync);
  if (!crates.every(([stem, crate]) => fresh(resolve(cache, `${stem}_bg.wasm`), resolve(target.target, 'wasm32-unknown-unknown/web', `${crate.replace(/-/g, '_')}.d`), shaders))) {
    rmSync(cache, { recursive: true, force: true });
    const gpu = webGpuArtifacts(target, cache, { cargo: true });
    if (!gpu.built) { rmSync(cache, { recursive: true, force: true }); console.error(`${app}: ${gpu.note}`); process.exit(1); }
    copyShaders(target, resolve(cache, 'shaders'));
  }
  cpSync(cache, out, { recursive: true });
  for (const f of ['gpu-glue.js', 'gpu-assets.js', 'pace.js']) cpSync(resolve(root, 'host/web', f), resolve(out, f));
}
if (pageModules) cpSync(resolve(appDir, 'modules/web'), resolve(out, 'modules'), { recursive: true });
if (existsSync(resolve(appDir, 'assets'))) cpSync(resolve(appDir, 'assets'), resolve(out, 'assets'), { recursive: true });
if (existsSync(resolve(appDir, 'deck'))) cpSync(resolve(appDir, 'deck'), resolve(out, 'deck'), { recursive: true });
// The Rust data module and the plan it binds, from the wasm build the baked plan came from.
// `--data <dist>` names another wasm build's module (a synthetic plan over an app's sources).
let moduleStorage = false;
if (rust) {
  mkdirSync(resolve(out, 'rust/wasm'), { recursive: true });
  const from = opt('--data') ?? (opt('--plan') && dirname(resolve(opt('--plan'))));
  const built = from && existsSync(resolve(from, 'rust/wasm/app.module.wasm')) ? resolve(from, 'rust/wasm/app.module.wasm') : buildModule(app, undefined, canvas2d, appDir);
  cpSync(built, resolve(out, 'rust/wasm/app.module.wasm'));
  moduleStorage = /^\s*(?:fs|sqlite)\./m.test(await moduleGrants(built));
  if (opt('--plan')) cpSync(resolve(opt('--plan')), resolve(out, 'app.plan'));
  else cpSync(resolve(gen, 'app.plan'), resolve(out, 'app.plan'));
}
// The web host's own picker and storage adapters beside the page, fetched on
// first use (files.js; a source's `storage`, ts-data.js and rust-data.js): what host/web/build.mjs ships.
if (files || moduleStorage || /^\s*(?:fs|sqlite)\./m.test(grants)) {
  const { webHostFiles } = await import('../../scripts/app.mjs');
  for (const [name, source] of Object.entries(webHostFiles('storage'))) cpSync(resolve(root, source), resolve(out, name));
  for (const f of ['picker-glue.js', 'documents-glue.js']) cpSync(resolve(root, 'host/web', f), resolve(out, f));
}
// The plan beside the pages: a render server (either renderer) reads it.
if (opt('--plan') && !existsSync(resolve(out, 'app.plan'))) cpSync(resolve(opt('--plan')), resolve(out, 'app.plan'));
else if (!existsSync(resolve(out, 'app.plan'))) cpSync(resolve(gen, 'app.plan'), resolve(out, 'app.plan'));
// Pages at build (LLP 1048.000): `--render rust` (the default) runs the app's
// native render entry (`<app>-render`, exact_render) over this shell;
// `--render js` runs this runtime under Bun (render.mjs). Either page adopts.
// `--render none` renders nothing: the dev loop's, where every route is the
// shell and the page builds itself (a render entry rebuilds on every
// Contract edit, since its crate bakes the plan).
const pages = JSON.parse(readFileSync(resolve(gen, 'pages.json'), 'utf8'));
// The shell a page is composed over stays as shell.html (a render server's, too).
if (pages.length) cpSync(resolve(out, 'index.html'), resolve(out, 'shell.html'));
if (pages.length && how === 'rust') {
  const bin = `${app}-render`;
  const at = [['linux', `${app}-linux`], ['web', `${app}-web`]].find(([dir]) => existsSync(resolve(appDir, dir, 'src/bin', `${bin}.rs`)));
  if (!at) { console.error(`--render rust: ${app} has no ${bin} entry; use --render js`); process.exit(1); }
  const plan = opt('--plan') ? resolve(opt('--plan')) : resolve(out, 'app.plan');
  const r = spawnSync('cargo', ['run', '--release', '-q', '-p', at[1], '--bin', bin, '--', '--plan', plan, '--name', manifest.name, ...(manifest.app?.origin ? ['--origin', manifest.app.origin] : []), '--shell', resolve(out, 'index.html'), '--build'],
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
