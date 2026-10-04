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
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, normalize, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { transformSync } from 'rolldown/utils';
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
// `--dev` (host/web-js/dev.mjs): typed state checkpoint hooks that ordinary
// and production builds neither emit nor link.
const devReload = args.includes('--dev');
if (production && devReload) { console.error('--production and --dev are mutually exclusive'); process.exit(2); }
if (production && !opt('--plan')) { console.error('--production builds over a wasm bake: name its --plan <dist>/app.plan'); process.exit(2); }
// The source revision a development build's install pages name, read while
// it builds: optional, so no git, no repository, or a git held past 5 s (a
// held index lock) leaves it unknown rather than failing or hanging the build.
const git = (...a) => new Promise((done) => {
  let text = '', c;
  try { c = spawn('git', a, { cwd: appDir, stdio: ['ignore', 'pipe', 'ignore'] }); } catch { return done(null); }
  const timer = setTimeout(() => { c.kill('SIGKILL'); done(null); }, 5000);
  c.on('error', () => { clearTimeout(timer); done(null); });
  c.on('close', (code) => { clearTimeout(timer); done(code === 0 ? text.trim() : null); });
  c.stdout?.on('data', (d) => { text += d; });
});
const revision = production ? null : Promise.all([git('rev-parse', '--short=10', 'HEAD'), git('status', '--porcelain')]);
const gen = resolve(out, '.gen');
rmSync(out, { recursive: true, force: true });
mkdirSync(gen, { recursive: true });

const input = opt('--plan') ?? opt('--contract') ?? resolve(appDir, 'app.contract');
const manifest = JSON.parse(readFileSync(resolve(appDir, 'app.json'), 'utf8'));
// documents-glue reads the app's web manifest lazily. The JS path exits from
// host/web/build.mjs after this builder succeeds, so it owns the same artifact.
const webManifestKeys = ['name', 'short_name', 'id', 'start_url', 'display', 'theme_color', 'background_color', 'icons', 'lang', 'file_handlers', 'launch_handler'];
const webManifest = Object.fromEntries(webManifestKeys.filter(key => manifest[key] !== undefined).map(key => [key, manifest[key]]));
if (webManifest.file_handlers) webManifest.file_handlers = webManifest.file_handlers.filter(handler => !Object.keys(handler.accept ?? {}).includes('inode/directory'));
if (!webManifest.file_handlers?.length) delete webManifest.file_handlers;
webManifest.name ??= manifest.app?.name ?? manifest.name;
webManifest.start_url ??= '/';
webManifest.lang ??= 'en';
writeFileSync(resolve(out, 'manifest.json'), JSON.stringify(webManifest, null, 2) + '\n');
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
const appTs = resolve(appDir, 'app.ts');
const devLogic = [];
// Native modules (LLP 1024): the app's module artifact, `modules/web/` beside
// the page as `modules/`, with the web host's adapter (native.js).
const pageModules = existsSync(resolve(appDir, 'modules/web/index.js'));
// The page module's container hooks (LLP 1075.003.000 §3.7): their glue loads
// only for a page module that exports one. Its exports are read by Bun's
// parser, never run: a browser module may touch the DOM as it loads. An
// `export *` may export one.
const pageSource = pageModules ? readFileSync(resolve(appDir, 'modules/web/index.js'), 'utf8') : '';
const pageExports = pageModules ? new Bun.Transpiler({ loader: 'js' }).scan(pageSource) : { exports: [], imports: [] };
const containerHooks = pageExports.exports.some(n => ['navigation', 'route', 'routeEnded', 'tabs'].includes(n))
  || /\bexport\s*\*\s*from\b/.test(pageSource);
// The surfaces the app's GPU module draws (its crate's surface table); any
// other surface is drawn by a data source on Canvas 2D, which the backend
// refuses by name.
const gpuLib = resolve(appDir, 'gpu/src/lib.rs');
const gpuSurfaces = existsSync(gpuLib) ? [...readFileSync(gpuLib, 'utf8').matchAll(/\("([a-z][a-z0-9-]*)", \d+, [a-z_:]+\)/g)].map(m => m[1]) : [];
// The compiler, run as its built binary when nothing it was built from
// changed (module.mjs `fresh`; `cargo run`'s own check costs ~0.4 s an edit).
const compiler = resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'debug/exact-web-js');
const [cmd, pre] = fresh(compiler, `${compiler}.d`) ? [compiler, []] : ['cargo', ['run', '-q', '-p', 'exact-web-js', '--']];
const cargo = spawnSync(cmd, [...pre, 'js', input, '-o', gen, ...(production ? [] : ['--sites']), ...(devReload ? ['--dev-reload'] : [])], { cwd: root, stdio: 'inherit', env: { ...process.env, EXACT_JS_GPU_SURFACES: gpuSurfaces.join(',') } });
if (cargo.status !== 0) process.exit(cargo.status ?? 1);
for (const f of ['rt.js', 'shape.js', 'pointer.js', 'paint.js', 'document.js']) cpSync(resolve(here, f), resolve(gen, f));
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
// The whole map: its type strings hold braces, so a source sorted before it
// (`advance`, a nested shape) once hid `exactTime` and its app could not boot.
const sources = JSON.parse(readFileSync(resolve(gen, 'app.js'), 'utf8').match(/export const sources=(\{.*?\});export const wait=/)?.[1] ?? '{}');
const time = Object.hasOwn(sources, 'exactTime');
// A source a data module answers: not one the runtime reserves (`exact…`).
const asks = Object.keys(sources).some(name => !/^exact[A-Z]/.test(name));
// A file input, `saveFile` or `share` (files.js), registered before any press.
const files = existsSync(resolve(gen, 'files.flag'));
// App generation may create files imported by app.ts. Run it before reading
// the declaration, as the wasm build does.
const webScript = resolve(appDir, 'web/build.rs');
if (ts && existsSync(webScript) && !/^\s*fn main\(\)\s*\{\s*exact_js_bake::build\w*\(/m.test(readFileSync(webScript, 'utf8'))) {
  const r = spawnSync('cargo', ['check', '-q', '--manifest-path', resolve(appDir, 'web/Cargo.toml')], { cwd: root, stdio: 'inherit' });
  if (r.status !== 0) { console.error(`${app}: its web build script failed`); process.exit(1); }
}
// `app.ts` is type-checked as the native bake checks it: the same
// configuration and check (js/bake/src/typescript.mjs) over the same capture
// and entry, against this plan's declarations, so an app.ts the web builds
// is one every host builds, refused with the same diagnostics (calc F2,
// calendar F9/F11). It runs while the page bundles; the build waits for it.
const typeChecked = ts ? typecheck().then(() => null, error => error) : null;
async function typecheck() {
  const { configure, check } = await import(resolve(root, 'js/bake/src/typescript.mjs'));
  const libraries = resolve(dirname(fileURLToPath(import.meta.resolve(`@typescript/typescript-${process.platform}-${process.arch}/package.json`))), 'lib');
  const source = readFileSync(appTs, 'utf8');
  let declarations = readFileSync(resolve(gen, 'app.contract.d.ts'), 'utf8');
  if (/export\s+(?:function|const|let)\s+draw\b|export\s*\{[^}]*\bdraw\b/.test(source)) declarations += readFileSync(resolve(root, 'js/bake/src/canvas-types.d.ts'), 'utf8');
  // Beside app.ts too, for an editor, as the native development bake writes
  // it: never a captured source, written only when it changes.
  const beside = resolve(appDir, 'app.contract.d.ts');
  if (!production && (!existsSync(beside) || readFileSync(beside, 'utf8') !== declarations)) writeFileSync(beside, declarations);
  // The bake's capture (js/bake/src/lib.rs `sources`): the app's TypeScript
  // and JSON, and each `typescript.sources` mount under its name.
  const stage = resolve(gen, 'typescript');
  // This build's own output may sit inside the app (`--out <app>/web-out`):
  // it is no source, and the stage is inside it (review r4a 2).
  const output = realpathSync(out);
  const mounts = Object.entries(manifest.typescript?.sources ?? {}).map(([name, path]) => [name, realpathSync(resolve(appDir, path))]);
  const capture = (from, to, top) => {
    for (const entry of readdirSync(from, { withFileTypes: true })) {
      const name = entry.name, path = resolve(from, name);
      if (['.git', 'node_modules', 'target', 'dist'].includes(name) || name.startsWith('.exact-js-bake-') || (top && name === 'app.contract.d.ts')) continue;
      if (top && mounts.some(([mount]) => mount === name)) continue;
      if (entry.isSymbolicLink()) throw new Error(`source links are not captured: ${path}`);
      if (entry.isDirectory()) { if (realpathSync(path) !== output) capture(path, resolve(to, name), false); }
      else if (/\.(ts|json)$/.test(name)) { mkdirSync(to, { recursive: true }); cpSync(path, resolve(to, name)); }
    }
  };
  capture(appDir, stage, true);
  for (const [name, dir] of mounts) capture(dir, resolve(stage, name), false);
  writeFileSync(resolve(stage, 'app.contract.d.ts'), declarations);
  // The bake's generated entry (js/bake/src/lib.rs `bake_in`), less the
  // Canvas 2D seam, which adds no type the module must meet.
  writeFileSync(resolve(stage, '__exact_entry.ts'), "import * as app from './app';\nimport type { Answer } from './app.contract.d.ts';\nexport const appId: string = app.appId;\nexport const grants: string = app.grants;\nexport const answer: Answer = app.answer;\n");
  writeFileSync(resolve(stage, '__exact_paths.json'), JSON.stringify({ app: realpathSync(appDir), mounts }));
  const real = realpathSync(stage);
  configure(real);
  await check(real, resolve(libraries, 'tsc'), libraries);
}
const normalizeGrants = (label, spec, stem) => {
  const file = resolve(gen, `${stem}.grants`);
  writeFileSync(file, spec);
  const result = spawnSync(cmd, [...pre, 'normalize-grants', file], { cwd: root, encoding: 'utf8' });
  if (result.status !== 0) { console.error(result.stderr); process.exit(result.status ?? 1); }
  const set = JSON.parse(result.stdout);
  if (set.error) throw new Error(`grant-parse: ${label}: ${set.error}`);
  return set;
};
const grants = ts ? String((await import(resolve(appDir, 'app.ts'))).grants ?? '') : '';
const tsGrantSet = normalizeGrants(ts ? 'app.ts' : 'TypeScript', grants, 'typescript');
let moduleStorage = false, rustGrants = '', rustGrantSet = normalizeGrants('Rust module', '', 'rust');
if (rust) {
  mkdirSync(resolve(out, 'rust/wasm'), { recursive: true });
  const from = opt('--data') ?? (opt('--plan') && dirname(resolve(opt('--plan'))));
  const built = from && existsSync(resolve(from, 'rust/wasm/app.module.wasm')) ? resolve(from, 'rust/wasm/app.module.wasm') : buildModule(app, undefined, canvas2d, appDir);
  cpSync(built, resolve(out, 'rust/wasm/app.module.wasm'));
  if (devReload) devLogic.push(['rust', createHash('sha256').update(readFileSync(resolve(out, 'rust/wasm/app.module.wasm'))).digest('hex')]);
  rustGrants = await moduleGrants(built);
  rustGrantSet = normalizeGrants(ts ? 'Rust module' : 'app', rustGrants, 'rust');
  moduleStorage = /^\s*(?:fs|sqlite)\./m.test(rustGrants);
  // The module binds with the plan's declarations, not the bake's answers.
  cpSync(resolve(gen, 'app.bind.plan'), resolve(out, 'app.bind.plan'));
}
writeFileSync(resolve(gen, 'admission-data.js'), `import {createGrantSet} from './admission.js';export const tsGrantSet=createGrantSet(${JSON.stringify(tsGrantSet)}),rustGrantSet=createGrantSet(${JSON.stringify(rustGrantSet)});\n`);
writeFileSync(resolve(gen, 'main.js'), [
  "import app, { sources, wait } from './app.js';",
  ...(devReload ? ["import { prepareDev } from './checkpoint.js';", "const finishDev = prepareDev();"] : []),
  ...(files ? ["import './files.js';"] : []),
  "import { data, journal, clock, advance, commit, inflight, Views, viewId, After, resolvedLocale, Resources } from './rt.js';",
  ...(production ? [] : ["import { develop } from './perf.js';"]),
  // A data module's answers, watched from before the app asks (seam.js).
  ...(production || !asks ? [] : ["import { seam } from './perf.js';", 'seam();']),
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
  ...(devReload ? ["  finishDev();"] : []),
  "  globalThis.exact = Object.assign(globalThis.exact ?? {}, { ready: true, journal, clock, advance, commit, data, state, inflight, views: Views, viewId, After, resources: Resources });",
  // A development page counts its work and samples its frames (LLP 1079); the agent adapter, only when the agent drives it.
  // The served plan's digest, which a development page's `perf` names (LLP 1079 D2).
  ...(production ? [] : [`  globalThis.exact.plan = ${JSON.stringify(createHash('sha256').update(readFileSync(opt('--plan') ? resolve(opt('--plan')) : resolve(gen, 'app.plan'))).digest('hex'))};`, "  develop(globalThis.exact).catch(console.error);", "  if (clock.agent) globalThis.exact.ready = import('./agent.js').then(m => m.install(globalThis.exact));"]),
  "};",
  ...(rust ? [
    // Rust data: loaded after first pixel, asked synchronously once ready;
    // a plan with a resource that has no compiled value waits for it.
    // Counted in flight, so `clock settle` waits for the source to be ready.
    "const load = () => import('./rust-data.js').then(m => m.install(data, sources)).finally(() => inflight.n--);",
    "inflight.n++;",
    `if (wait || ${mixed}) load().then(start); else { start(); requestAnimationFrame(() => setTimeout(load)); }`,
  ] : ['start();']),
  ...(containerHooks ? ["requestAnimationFrame(() => requestAnimationFrame(() => import('./hooks.js').then(m => m.containers())));"] : []),
].join('\n'));
for (const f of ['agent.js', 'perf.js', 'seam.js', 'rust-data.js', 'list.js', 'facts.js', 'symbols.js', 'motion.js', 'transform.js', 'svg-transform.js', 'dataset.js', 'format.js', 'hooks.js', 'arrange.js', 'reorder.js', 'flow.js', 'native.js']) cpSync(resolve(here, f), resolve(gen, f));
// The web host's own pieces, loaded after first paint (motion.js, a pan, `select`, text flow, rt.js `pr`, native.js, rt.js `geo`).
for (const f of ['frames.js', 'motion-glue.js', 'input-glue.js', 'markup-editor.js', 'textflow-glue.js', 'timer-glue.js', 'presence-glue.js', 'native-glue.js', 'geometry-glue.js']) cpSync(resolve(root, 'host/web', f), resolve(gen, f));
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
// A source granted `auth.session` signs in through the system browser (auth.js, LLP 1069.006).
const auth = /^\s*auth\.session\s/m.test(grants);
if (ts) writeFileSync(resolve(gen, 'ts-data.js'), readFileSync(resolve(here, 'ts-data.js'), 'utf8').replace('__APP_TS__', resolve(appDir, 'app.ts'))
  .replace('__AUTH_IMPORT__', auth ? "import { install as signIn } from './auth.js';" : '')
  .replace('__AUTH_INSTALL__', auth ? `signIn(${JSON.stringify(grants)}, () => asking);` : ''));
for (const f of ['auth-glue.js', 'storage-environment.js', 'http-body.js', 'grant-admission.js']) cpSync(resolve(root, 'host/web', f), resolve(gen, f));
writeFileSync(resolve(gen, 'admission.js'), readFileSync(resolve(here, 'admission.js'), 'utf8').replaceAll("'../web/grant-admission.js'", "'./grant-admission.js'"));
cpSync(resolve(here, 'ts-fetch.js'), resolve(gen, 'ts-fetch.js'));
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
// Bun's bundler, in this process, for the server bundle. A build that
// renders nothing (the dev loop's) makes no server bundle.
const how = opt('--render') ?? 'rust';
// Inject only into the app's module graph, never the copied host runtime.
// Oxc resolves lexical bindings, so an authored local `fetch` stays local.
const scopedModule = (code, id) => {
  if (!ts || id.startsWith(gen + '/') || id.startsWith(realpathSync(gen) + '/') || !/\.[cm]?[jt]sx?$/.test(id)) return null;
  // And the clock, timers and Math.random refused by name (LLP 1027.000 D3).
  const bound = ['fetch', 'Date', 'Math', 'Intl', 'setTimeout', 'setInterval', 'requestAnimationFrame', 'requestIdleCallback',
    'clearTimeout', 'clearInterval', 'cancelAnimationFrame', 'cancelIdleCallback', 'performance'];
  const result = transformSync(id, code, { inject: { ...Object.fromEntries(bound.map(name => [name, [resolve(gen, 'ts-fetch.js'), name]])),
    ...Object.fromEntries(['globalThis', 'window', 'self'].map(name => [name, [resolve(gen, 'ts-fetch.js'), 'appGlobal']])) } });
  if (result.errors.length) throw new Error(result.errors.map(e => e.message).join('\n'));
  return result.code;
};
const bundle = async (options) => {
  const r = await Bun.build({ ...options, plugins: [{ name: 'source-grants', setup(build) {
    build.onLoad({ filter: /\.[cm]?[jt]sx?$/ }, ({ path }) => { const contents = scopedModule(readFileSync(path, 'utf8'), path); return contents == null ? undefined : { contents, loader: 'js' }; });
  } }] });
  for (const m of r.logs) console.error(String(m));
  if (!r.success) process.exit(1);
};
if (how !== 'none') await bundle({ entrypoints: [resolve(gen, 'main-server.js')], format: 'iife', outdir: gen, naming: 'server.js' });
// The page's module and its chunks: Rolldown, the repo's app bundler, whose
// minifier leaves the entry 6–10% smaller than Bun's after brotli (RealWorld
// 25.8 → 23.6 KB, the feed bench's 30.9 → 27.9, the grid's 19.3 → 18.1),
// bytes a page downloads before its runtime is up.
{
  const { rolldown } = await import('rolldown');
  const b = await rolldown({ input: resolve(gen, 'main.js'), plugins: [{ name: 'source-grants', transform: scopedModule }], logLevel: 'warn', onLog: (level, log) => console.error(log.message) });
  // In a development reload build, put the TypeScript data module and every
  // helper it imports in one chunk the page really loads. Its emitted bytes,
  // rather than watcher filenames or source mtimes, are the logic revision.
  const fromData = (id, getModuleInfo, seen = new Set()) => {
    if (id === appTs) return true;
    if (seen.has(id)) return false;
    seen.add(id);
    return (getModuleInfo(id)?.importers ?? []).some(parent => fromData(parent, getModuleInfo, seen));
  };
  const built = await b.write({ dir: out, format: 'esm', minify: true, comments: false, entryFileNames: 'app.js', chunkFileNames: '[name]-[hash].js',
    ...(devReload && ts ? { manualChunks: (id, { getModuleInfo }) => fromData(id, getModuleInfo) ? 'dev-data' : undefined } : {}) });
  if (devReload && ts) for (const chunk of built.output.filter(file => file.type === 'chunk' && Object.keys(file.modules).includes(appTs))) {
    devLogic.push(['typescript', createHash('sha256').update(readFileSync(resolve(out, chunk.fileName))).digest('hex')]);
  }
  await b.close();
}

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
const literalModuleURLs = code => [...code.matchAll(/new\s+URL\(\s*['"]([^'":/#][^'"]*)['"]\s*,\s*import\.meta\.url\s*\)/g)].map(match => match[1]);
const moduleImports = code => new Bun.Transpiler({ loader: 'js' }).scanImports(code).map(entry => entry.path).filter(path => path.startsWith('.'));
const publicDependency = (from, specifier) => normalize(resolve('/', dirname(from), specifier)).replace(/^\//, '');
async function copyLazyModules(roots) {
  const { webHostFiles } = await import('../../scripts/app.mjs');
  const inventory = webHostFiles(), queue = roots.map(name => ({ name, source: inventory[name] })), copied = new Set();
  while (queue.length) {
    const { name, source } = queue.shift();
    if (copied.has(name)) continue;
    copied.add(name);
    if (!source) {
      if (!existsSync(resolve(out, name))) throw new Error(`lazy module dependency ${name} has no build source`);
      continue;
    }
    const from = resolve(root, source), code = name.endsWith('.js') || name.endsWith('.mjs') ? readFileSync(from, 'utf8') : null;
    mkdirSync(dirname(resolve(out, name)), { recursive: true });
    cpSync(from, resolve(out, name));
    if (code != null) for (const specifier of [...moduleImports(code), ...literalModuleURLs(code)]) {
      const dependency = publicDependency(name, specifier);
      if (dependency.startsWith('..')) throw new Error(`${name}: lazy dependency leaves the build root: ${specifier}`);
      const sibling = resolve(dirname(from), specifier);
      queue.push({ name: dependency, source: inventory[dependency] ?? (existsSync(sibling) ? sibling : null) });
    }
  }
}

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
  await copyLazyModules(['gpu-glue.js']);
}
if (pageModules) cpSync(resolve(appDir, 'modules/web'), resolve(out, 'modules'), { recursive: true });
if (existsSync(resolve(appDir, 'assets'))) cpSync(resolve(appDir, 'assets'), resolve(out, 'assets'), { recursive: true });
if (existsSync(resolve(appDir, 'deck'))) cpSync(resolve(appDir, 'deck'), resolve(out, 'deck'), { recursive: true });
if (devReload) writeFileSync(resolve(out, '.exact-dev-logic.json'), JSON.stringify({ version: 1, modules: devLogic.sort(([a], [b]) => a.localeCompare(b)) }) + '\n');
// The web host's own picker and storage adapters beside the page, fetched on
// first use (files.js; a source's `storage`, ts-data.js and rust-data.js; an
// `app:/` image's file, symbols.js): what host/web/build.mjs ships.
if (files || moduleStorage || /^\s*(?:fs|sqlite)\./m.test(grants)) {
  const storageGrants = /^\s*(?:fs|sqlite)\./m.test(grants);
  const seeds = [resolve(gen, 'symbols.js'), files && resolve(gen, 'files.js'), (moduleStorage || storageGrants) && resolve(gen, 'admission.js'), storageGrants && resolve(gen, 'ts-data.js')].filter(Boolean);
  const roots = seeds.flatMap(seed => literalModuleURLs(readFileSync(seed, 'utf8'))).map(specifier => specifier.replace(/^\.\//, ''));
  await copyLazyModules(roots);
}
// The plan beside the pages: a render server (either renderer) reads it.
cpSync(opt('--plan') ? resolve(opt('--plan')) : resolve(gen, 'app.plan'), resolve(out, 'app.plan'));
// A development build's source map for that plan, for the agent driver only
// (LLP 1012.001.000 D6): never in a production build, never a stale one.
rmSync(resolve(out, 'app.plan.map.json'), { force: true });
if (!production && !opt('--plan') && existsSync(resolve(gen, 'app.plan.map.json'))) cpSync(resolve(gen, 'app.plan.map.json'), resolve(out, 'app.plan.map.json'));
// The install pages and their data (LLP 1030.003 D6a), as the wasm build
// writes them (a release carries its bake's, above). The build is named by
// the program's bytes: the plan and the page's module.
if (!production) {
  const [{ readManifest }, { writeInstallPages }] = await Promise.all([import('../../scripts/app.mjs'), import('../../scripts/install-page.mjs')]);
  // A release is `--production`'s, so this is a development build whatever
  // the shell's EXACT_UPDATE_TRUST; with no bake receipt its reach is unknown.
  const [source, changes] = await revision;
  const id = createHash('sha256').update(readFileSync(resolve(out, 'app.plan'))).update(readFileSync(resolve(out, 'app.js'))).digest('hex');
  writeInstallPages(out, readManifest(appDir, app), { id, source, dirty: changes === null ? null : !!changes, builtAt: new Date().toISOString(),
    mode: 'Development build' });
}
// The type check, which ran while the page bundled: its diagnostics are the
// native bake's (host/web/build.mjs keeps them in its summary).
const typeError = await typeChecked;
if (typeError) { console.error(`${typeError.message ?? typeError}\nerror: ${app}: app.ts failed the type check every host's build runs`); process.exit(1); }
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
