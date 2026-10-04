#!/usr/bin/env bun
// Build the web app (LLP 1071): the JS target, `host/web-js/build.mjs`, the
// plan compiled to one ES module over a small runtime, into the same dist;
// what it refuses (a capability the runtime lacks, named) fails the build.
// A game (LLP 1046), and `--wasm`, get the wasm target: the wasm under the `web` profile (size-tuned),
// `wasm-opt -Oz` when binaryen is on PATH, then `dist/` = index.html +
// glue.js + app.wasm.
// Usage: bun host/web/build.mjs [crate=caltrain-web]
// (`--js` is accepted and is the default.) `--wasm` is internal, for what the
// retiring wasm target still serves (LLP 1071 §7, "Retiring the wasm target
// on the web"): conformance's reference, delivery's bake (the streams'
// bundle, whose plan the JS web root compiles), the resident dev loop a
// native client opens, `motionparity` and the smoke's plan fixtures.
// `--render <rust|js|none>` passes to the JS target's build. A JS build's completion marker says
// `target: 'js'`, so the dev loop, the agent and the smoke know which they got.
// EXACT_WEB_NAMES=1 keeps the wasm's function names, for metrics' byte
// attribution (LLP 1047 D9); EXACT_WEB_LINK=all links every capability, as
// the dev loop does (LLP 1047 D7).
// Developer builds bake development trust; EXACT_UPDATE_TRUST=production
// requires signing keys, and the deploy verb always selects production.
import { grantOrigins } from './navigation.js';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { writeInstallPages } from '../../scripts/install-page.mjs';
import { authClientMetadata, checkModuleRoster, gpuModules, rustPolicy, webGpuArtifacts, webHostFiles } from '../../scripts/app.mjs';
import { buildRust, rustFiles, rustCards, rustPackage } from '../../scripts/rust.mjs';
import { webDist, copyShaders, bakeOutput, buildBake, contractLast, readBake, verifyBakeFiles, developmentBuildEnv, resolveApp, wasmRemapFlags, WEB_STD, WEB_TOOLCHAIN, webToolchainEnv } from '../../scripts/app.mjs';
import { closeFilesystemReader } from '../../scripts/filesystem.mjs';
import { BINARYEN_DOWNLOAD, splitStages, unsplitReason } from './stages.mjs';
import { appManifestDigest, buildFileCards, copyStaticTreeIfPresent, listAssets, publicFileCards, webEnvelope, moduleCards, MODULE_FILES } from './serve.mjs';

const target = ['--js', '--wasm'].find((flag) => process.argv.includes(flag));
// `--bake` (internal, delivery's): the web crate's bake without its wasm —
// `cargo check` runs the build script (the baked plan, its receipt, a
// TypeScript module's artifacts), and the origin files are written as the
// wasm build writes them, with no `app.wasm`, glue or leaf wasm (LLP 1071
// §7, "Retiring the wasm target on the web", step c).
const bakeOnly = process.argv.includes('--bake');
process.argv = process.argv.filter((a) => a !== '--js' && a !== '--wasm' && a !== '--bake');
// `--render <rust|js|none>`: the JS target's pages at build (host/web-js/build.mjs).
const renderFlag = process.argv.indexOf('--render');
const render = renderFlag < 0 ? [] : process.argv.splice(renderFlag, 2);
const app = resolveApp(process.argv[2]);
// After resolveApp, which names `bun install` when the packages are missing.
const [{ rolldown }, { minifySync }] = await Promise.all([import('rolldown'), import('rolldown/experimental')]);
// A game's web build is the wasm target (Charlie, 2026-09-29: "Game runtime
// is fine to be on wasm"; LLP 1071 §8). For an app the JS target is the web
// build, and what it refuses is an error; `--wasm` is internal (below).
const game = app.manifest.game !== undefined;
if (target !== '--wasm' && !game && !bakeOnly) {
  // An app outside apps/ reaches it through EXACT_APP_DIR, as here.
  const js = spawnSync(process.execPath, [fileURLToPath(new URL('../web-js/build.mjs', import.meta.url)), app.name, '--out', webDist(), ...render], { stdio: ['ignore', 'inherit', 'pipe'], encoding: 'utf8', env: { ...process.env, EXACT_APP_DIR: app.dir } });
  if (js.status === 0) {
    // What the JS target warned of, in its own words: the count it prints names nothing (workout F3, kanban F30).
    // The bundler's notes on the generated glue are not the app's to act on.
    for (const line of (js.stderr ?? '').split('\n')) if (/^warning: /.test(line)) console.error(line);
    writeFileSync(resolve(webDist(), '.exact-build.json'), JSON.stringify({ exactBuild: 1, target: 'js', app: { id: app.id, name: app.displayName },
      manifestSha256: appManifestDigest(app), files: buildFileCards(webDist()) }) + '\n');
    process.exit(0);
  }
  // The child's own message, not the tail of Bun's trace (a frame and its version line).
  const all = (js.stderr ?? '').trim().split('\n').filter((l) => !/^\s*(Compiling|Finished|Running)/.test(l));
  const warnings = all.filter((l) => /^warning: /.test(l)), lines = all.filter((l) => !/^\s*warning/.test(l));
  // A type check's diagnostics are TypeScript's own lines (`app.ts(2,8): error TS…`).
  const message = lines.filter((l) => /^(error|[A-Z]\w*Error|E[A-Z]+)\b:?/.test(l.trim()) || /\): error TS\d+:|^(?:tsconfig: |module outside captured app: |source links are not captured: )/.test(l) || /\bunoptimized$|\bnot on PATH\b/.test(l));
  const reason = (message.length ? message : lines.slice(-3)).join('\n');
  console.error(`${[...warnings, reason].join('\n')}\n${app.name}: the web build (the JS target) failed; the wasm target is internal (--wasm)`);
  process.exit(1);
}
const crate = app.crate('web');
const kib = (n) => `${(n / 1024).toFixed(0)} KiB`;
const root = resolve(fileURLToPath(new URL('../..', import.meta.url)));
// `EXACT_WEB_DIST` names another output directory: `exact deploy` bakes into a
// run-specific one and never publishes from the dev server's shared dist/.
const dist = webDist();
const previous = `${dist}.previous`;
// A hard stop can land after dist moved aside but before the completed stage
// took its place. Restore the prior complete build before doing slow work;
// serve.mjs/dev.mjs also fall back to it during the live rename window.
if (!existsSync(dist) && existsSync(previous)) renameSync(previous, dist);
else if (existsSync(dist) && existsSync(previous)) rmSync(previous, { recursive: true, force: true });
const buildEnv = developmentBuildEnv();
buildEnv.EXACT_BAKE_OUTPUT = bakeOutput(app, buildEnv);
// `EXACT_WEB_NAMES=1` keeps the wasm's name section for attribution (LLP 1047
// D9, metrics.mjs --long): rustc strips only DWARF, which would stop binaryen's
// duplicate-function elimination, and wasm-opt keeps names. The code is the
// shipped code's; the file is not for shipping.
const keepNames = process.env.EXACT_WEB_NAMES === '1';
// A production artifact is split into a core and its staged capabilities
// (LLP 1047.000 §9), which needs the names too; they are stripped after the
// split. The dev loop's `EXACT_WEB_LINK=all` build and the names build stay
// whole, as does a machine whose binaryen isn't the pinned one.
const unsplit = keepNames || process.env.EXACT_WEB_LINK === 'all' ? 'a development or names build' : unsplitReason();
if (keepNames || !unsplit) buildEnv.CARGO_PROFILE_WEB_STRIP = 'debuginfo';
const buildReceipt = contractLast(() => buildBake(app, 'web', 'wasm32-unknown-unknown', {env:buildEnv, check:bakeOnly}));
const built = resolve(app.target, 'wasm32-unknown-unknown/web', crate.replace(/-/g, '_') + '.wasm');
// Build one app into its own staging directory. Only a complete build replaces
// dist, so a server sees the previous app or the next one, never a mixture;
// replacing the directory also drops every stale optional/private artifact.
// Stages live under ignored target/, so even a SIGKILL leaves no source dirt.
const stages = resolve(app.target, 'web-dist-stages');
mkdirSync(stages, { recursive: true });
// A worktree may share target/ through a symlink. This directory is ours;
// retain its physical name before the strict filesystem reader inventories it.
let stage = realpathSync(mkdtempSync(resolve(stages, `${app.name.replace(/[^a-zA-Z0-9_-]/g, '_')}-`)));
process.on('exit', () => { if (stage) rmSync(stage, { recursive: true, force: true }); });
const out = resolve(stage, 'app.wasm');

// Keep small single-caller functions inline, but bound large expansions: -Oz's
// unlimited default shrinks raw bytes while increasing both Brotli and gzip.
// Bound 20 and passes to convergence ship the fewest compressed bytes
// (2026-09-24: 3 KB less Brotli per app than 50 alone, for ~0.1% more raw).
// Functions of up to 6 instructions are inlined everywhere: V8 compiles a wasm
// function lazily, on the main thread, at its first call, and a tiny one costs
// about what a large one does to set up. RealWorld's boot compiles 15% fewer
// (1,314 -> 1,123), for 511 B less Brotli (+7 KB raw); 10 or 16 add Brotli.
// The feature flags match what rustc's wasm32 target emits.
// `--low-memory-unused` lets binaryen fold a constant under 1024 added to a
// pointer into the access's offset, which differs only when the addition
// wraps below zero. Rust's pointers never wrap, and nothing lives there:
// the stack is first in memory and grows down from 1 MiB, so it reaches
// below 1024 only in its last KiB (2026-09-28: 0.7–1.4 KiB brotli).
let optNote = 'none (--bake)';
if (!bakeOnly) {
const named = resolve(stage, 'app.named.wasm');
const opt = spawnSync('wasm-opt', ['-Oz', '--one-caller-inline-max-function-size', '20', '--always-inline-max-function-size', '6', '--converge', '--low-memory-unused', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', keepNames || !unsplit ? '-g' : '--strip-debug', '--strip-producers', '-o', unsplit ? out : named, built], { stdio: 'inherit' });
if (opt.error?.code === 'ENOENT') { copyFileSync(built, out); optNote = `wasm-opt not on PATH (binaryen: brew install binaryen, or ${BINARYEN_DOWNLOAD}): shipped unoptimized`; }
else if (opt.status !== 0) process.exit(opt.status ?? 1);
else if (unsplit) optNote = `wasm-opt -Oz; unsplit: ${unsplit}`;
else {
  const split = splitStages(named);
  writeFileSync(out, split.core);
  for (const [path, bytes] of split.files) { mkdirSync(resolve(stage, path, '..'), { recursive: true }); writeFileSync(resolve(stage, path), bytes); }
  rmSync(named);
  optNote = `wasm-opt -Oz; the core, with stages: ${split.report.join('; ')}`;
}
}

// The app's static files ride beside the page: `assets/…` images and an
// optional `deck/` iframe guest (@ref LLP 1020 M1). Replaced whole, so a
// deleted file does not linger in dist.
const assets = resolve(app.dir, 'assets');
copyStaticTreeIfPresent(assets, resolve(stage, 'assets'));
const deck = resolve(app.dir, 'deck');
copyStaticTreeIfPresent(deck, resolve(stage, 'deck'));
// The GPU crate's shaders (LLP 1030 D8): `shaders/<name>.wgsl` beside the
// page, fetched and registered by the GPU glue before a surface is created
// — never a string in the wasm.
copyShaders(app, resolve(stage, 'shaders'));
copyFileSync(resolve(root, 'host/web/index.html'), resolve(stage, 'index.html'));
// A production bake never enters agent mode (LLP 1069.007 D2, ruled): every
// host file that reads `?agent` declares `AGENT_ADMITTED`, and this build
// ships it false, so the minifier drops the agent's paths from a release.
const AGENT_ADMITTED = 'const AGENT_ADMITTED = true;';
const production = buildEnv.EXACT_UPDATE_TRUST === 'production';
const gateAgent = (code, name) => {
  if (!production) return code;
  if (/searchParams\.has\(["']agent["']\)|params\.has\(["']agent["']\)/.test(code) && !code.includes(AGENT_ADMITTED)) throw new Error(`${name} reads ?agent without AGENT_ADMITTED; a production build must not admit agent mode (LLP 1069.007 D2)`);
  return code.replaceAll(AGENT_ADMITTED, 'const AGENT_ADMITTED = false;');
};
function copyHostFiles(group) {
  for (const [name, source] of Object.entries(webHostFiles(group))) {
    if (!name.endsWith('.js')) { copyFileSync(resolve(root, source), resolve(stage, name)); continue; }
    // Production ships executable code, without source comments and long
    // local names. Keep exports and property names intact across modules.
    const result = minifySync(name, gateAgent(readFileSync(resolve(root, source), 'utf8'), name), { module: name !== 'module-prelude.js' });
    if (result.errors.length) throw new Error(`${name}: ${JSON.stringify(result.errors)}`);
    writeFileSync(resolve(stage, name), result.code);
  }
}
if (!bakeOnly) copyHostFiles('base');
if (!bakeOnly && !production) copyHostFiles('development');
// The Markdown editor's rules (exact-markdown-editor, LLP 1045 D5) are their
// own wasm beside markup-editor.js, fetched only when a Markdown textarea mounts.
const webEnv = webToolchainEnv(buildEnv);
if (!bakeOnly) {
const editor = spawnSync('cargo', ['build', '--locked', '--offline', '-q', ...WEB_STD, ...wasmRemapFlags(app, WEB_TOOLCHAIN), '-p', 'exact-markdown-editor', '--lib', '--target', 'wasm32-unknown-unknown', '--profile', 'web'], { cwd: root, env: webEnv, stdio: 'inherit' });
if (editor.status !== 0) process.exit(editor.status ?? 1);
const editorBuilt = resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'wasm32-unknown-unknown/web/exact_markdown_editor.wasm');
const editorWasm = resolve(stage, 'markup-editor.wasm');
if (spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', editorWasm, editorBuilt], { stdio: 'inherit' }).status !== 0) copyFileSync(editorBuilt, editorWasm);

// The exclusions walker is its own leaf artifact; ordinary apps fetch none of it.
const flow = spawnSync('cargo', ['build', '--locked', '--offline', '-q', ...WEB_STD, ...wasmRemapFlags(app, WEB_TOOLCHAIN), '-p', 'exact-textflow', '--bin', 'textflow-web', '--target', 'wasm32-unknown-unknown', '--profile', 'web'], { cwd: root, env: webEnv, stdio: 'inherit' });
if (flow.status !== 0) process.exit(flow.status ?? 1);
const flowBuilt = resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'wasm32-unknown-unknown/web/textflow-web.wasm');
const flowWasm = resolve(stage, 'textflow.wasm');
if (spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', flowWasm, flowBuilt], { stdio: 'inherit' }).status !== 0) copyFileSync(flowBuilt, flowWasm);
}

// The plan and its pointer card (LLP 1023 D1/D2): extract the exact bytes
// baked into the produced, optimized wasm. Compiling app.contract a second
// time here could pair app.wasm with a later source revision. A native client
// GETs the page URL, follows index.html's link to exact.json, and fetches this
// app.plan; a browser never notices. Header offsets are the generated
// encoder's fixed little-endian layout.
// `--bake` reads the same bytes from the bake's own output.
const planOut = resolve(stage, 'app.plan');
let wasm = null, exports = {}, planBytes;
if (bakeOnly) planBytes = readFileSync(resolve(buildEnv.EXACT_BAKE_OUTPUT, 'web-wasm32-unknown-unknown.plan'));
else {
wasm = readFileSync(out);
const unbooted = () => { throw new Error('app logic ran while extracting baked bytes'); };
const { instance } = await WebAssembly.instantiate(wasm, { exact_grants: grantOrigins(() => instance.exports.memory), exact_js: { call: unbooted }, exact_rust: { load: unbooted, call: unbooted, read: unbooted, drop: unbooted }, exact_data: { random: unbooted, agent_seed: unbooted }, exact_geometry: { read: unbooted } });
exports = instance.exports;
if (typeof exports.exact_plan !== 'function' || typeof exports.exact_out !== 'function' || !(exports.memory instanceof WebAssembly.Memory)) {
  throw new Error('the web wasm does not export exact_plan, exact_out, and memory');
}
const planLen = exports.exact_plan();
const planPtr = exports.exact_out();
planBytes = Buffer.from(new Uint8Array(exports.memory.buffer, planPtr, planLen));
}
if (planBytes.length < 36 || planBytes.subarray(0, 4).toString() !== 'EXPL') throw new Error('the web wasm returned an invalid baked plan');
writeFileSync(planOut, planBytes);
let pairedModule = null;
// A module client's build script emits paired artifacts beside its receipt.
// Extract the exact embedded receipt/JS rather than rebaking moving sources.
// Native bytecode is a separate download: it never executes in the browser.
// `--bake` reads them from the build script's output, which the wasm embeds.
const moduleInput = buildReceipt.binary.inputs.find(input => input.name === `generated:${crate}:wasm32-unknown-unknown/app.module.json`);
if (bakeOnly ? !!moduleInput : typeof exports.exact_module_artifact === 'function') {
  const files = new Map([['app.plan', planBytes]]);
  for (const [index, name] of ['app.module.json', 'app.js'].entries()) {
    // The length first: the call can grow memory, detaching an earlier view of it.
    const len = bakeOnly ? 0 : exports.exact_module_artifact(index);
    const body = bakeOnly ? readFileSync(resolve(moduleInput.path, '..', name)) : Buffer.from(new Uint8Array(exports.memory.buffer, exports.exact_out(), len));
    files.set(name, body); writeFileSync(resolve(stage, name), body);
  }
  if (!moduleInput) throw new Error('the build receipt does not name the paired module output');
  const bytecode = readFileSync(resolve(moduleInput.path, '..', 'app.hbc'));
  files.set('app.hbc', bytecode);
  // moduleCards checks every byte against the receipt embedded in this wasm,
  // including the native digest/version. A concurrent rebake cannot mix pairs.
  pairedModule = Object.fromEntries(Object.entries(moduleCards(files, app.id)).map(([key, card]) => [key, { ...card, url: './' + MODULE_FILES[key] }]));
  writeFileSync(resolve(stage, 'app.hbc'), bytecode);
  copyHostFiles('module');
  // Remove the module-glue → storage → fs/sqlite request chain's middle
  // step. Keep the stateful adapters as shared modules: Rust requests also
  // import them, and must share the same filesystem mutation queues.
  const bundle = await rolldown({ input: resolve(root, 'host/web/module-glue.js'), platform: 'browser',
    external: ['./storage-fs.js', './storage-sqlite.js'], plugins: [{ name: 'agent-gate', transform: (code, id) => ({ code: gateAgent(code, id) }) }] });
  try { await bundle.write({ file: resolve(stage, 'module-glue.js'), format: 'es', minify: true }); }
  finally { await bundle.close(); }
}
const bakedReceipt = readBake(app, 'web', 'wasm32-unknown-unknown', buildEnv.EXACT_BAKE_OUTPUT);
if (!bakeOnly) {
  if (typeof exports.exact_compat !== 'function') throw new Error('the web wasm exposes no baked receipt');
  const compatLen = exports.exact_compat();
  const embeddedCompat = Buffer.from(new Uint8Array(exports.memory.buffer, exports.exact_out(), compatLen)).toString('utf8');
  if (JSON.stringify(JSON.parse(embeddedCompat)) !== JSON.stringify(bakedReceipt)) throw new Error('the emitted receipt differs from the wasm receipt');
}
// Storage is an app capability, including apps whose only logic is Rust.
if (pairedModule || /^\s*(?:fs\.|sqlite\.)/m.test(bakedReceipt.inputs.grantCeiling ?? '')) {
  copyHostFiles('storage');
}
const copiedAssets = listAssets(stage);
verifyBakeFiles(bakedReceipt, planBytes, copiedAssets);
let pairedRust = null;
if (rustPackage(app) && rustPolicy(app.manifest, 'web', buildEnv.EXACT_UPDATE_TRUST === 'production' ? 'prod' : 'dev') !== 'off') {
  const built = await buildRust(app, { compat: bakedReceipt, env: buildEnv, plan: planBytes, profile: buildEnv.EXACT_UPDATE_TRUST === 'production' ? 'release' : 'logic-dev' });
  const files = rustFiles(built);
  for (const [name, bytes] of files) { mkdirSync(resolve(stage, name, '..'), {recursive:true}); writeFileSync(resolve(stage, name), bytes); }
  pairedRust = rustCards(files);
  copyHostFiles('rust');
}
writeFileSync(resolve(stage, 'bake.json'), JSON.stringify(buildReceipt) + '\n');
writeFileSync(resolve(stage, 'exact.json'), JSON.stringify({ ...webEnvelope(app, planBytes, copiedAssets), ...(pairedModule ? { module: pairedModule } : {}), ...(pairedRust ? { rust: pairedRust } : {}) }) + '\n');
// The web app manifest (LLP 1030 D2/D10; 1030.000 D7): the W3C keys of
// `app.json`, copied out as `manifest.json`; the page links it, takes its
// name as the title, and its first icon as the favicon. An installed PWA's
// icon and name are the browser's cached copies of these — the origin's
// carrier, at its real strength.
const webKeys = ['name', 'short_name', 'id', 'start_url', 'display', 'theme_color', 'background_color', 'icons', 'lang', 'file_handlers', 'launch_handler'];
const webManifest = Object.fromEntries(webKeys.filter((k) => app.manifest[k] !== undefined).map((k) => [k, app.manifest[k]]));
// `inode/directory` is the Apple bake's word for a folder; a browser's
// file handler opens files only (LLP 1069.010 slice 4).
if (webManifest.file_handlers) webManifest.file_handlers = webManifest.file_handlers.filter((h) => !Object.keys(h.accept ?? {}).includes('inode/directory'));
if (!webManifest.file_handlers?.length) delete webManifest.file_handlers;
webManifest.name ??= app.displayName;
webManifest.start_url ??= '/';
// The document's language (`<html lang>`, WCAG 3.1.1). Every app here is in
// English, so an app that declares none is `en`.
webManifest.lang ??= 'en';
writeFileSync(resolve(stage, 'manifest.json'), JSON.stringify(webManifest, null, 2) + '\n');
const sourceRevision = spawnSync('git', ['rev-parse', '--short=10', 'HEAD'], {cwd:app.dir,encoding:'utf8'});
const sourceChanges = spawnSync('git', ['status', '--porcelain'], {cwd:app.dir,encoding:'utf8'});
writeInstallPages(stage, app.manifest, {id:buildReceipt.binary.sha256, source:sourceRevision.status === 0 ? sourceRevision.stdout.trim() : null, dirty:sourceChanges.status === 0 && !!sourceChanges.stdout.trim(), builtAt:new Date().toISOString(), mode:buildEnv.EXACT_UPDATE_TRUST === 'production' ? 'Release build' : 'Development build', reach:bakedReceipt.reach?.rows ?? []});
const icon = webManifest.icons?.[0];
const escapeHtml = (t) => String(t).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
// The shell's <style> ships without its comments and indentation: they are
// for readers of host/web/index.html, and every served document's first bytes
// carry the shell's head (LLP 1048.000 D3). Strings stay whole; whitespace
// goes only where CSS never reads it (never before a `:`, which in a selector
// is a descendant's pseudo-class).
function minifyCss(css) {
  let out = '', space = false;
  const put = (text) => {
    if (space && out && !'{};,:'.includes(out.at(-1)) && !'{};,!'.includes(text[0])) out += ' ';
    space = false;
    if (text === '}' && out.endsWith(';')) out = out.slice(0, -1);
    out += text;
  };
  for (let i = 0; i < css.length;) {
    if (css.startsWith('/*', i)) { const end = css.indexOf('*/', i + 2); i = end < 0 ? css.length : end + 2; space = true; continue; }
    const c = css[i];
    if (/\s/.test(c)) { space = true; i++; continue; }
    if (c === '"' || c === "'") { let j = i + 1; while (j < css.length && css[j] !== c) j += css[j] === '\\' ? 2 : 1; put(css.slice(i, j + 1)); i = j + 1; continue; }
    put(c); i++;
  }
  return out;
}
// The app's module artifact answers `native.later` on the page too (LLP
// 1067 D5): the glue finds it by the meta tag below.
const pageNative = app.modules.web;
// The page in the app's first-frame background from its first paint (the
// manifest's background colours, as the iOS launch screen), so nothing lighter or
// darker shows before the first frame.
const launchLight = app.manifest.background_color, launchDark = app.manifest.background_color_dark;
const hex = (value) => /^#(?:[0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(value ?? '') ? value : null;
const launchCss = hex(launchLight) ? `html{background-color:${launchLight}}${hex(launchDark) ? `@media (prefers-color-scheme:dark){html{background-color:${launchDark}}}` : ''}` : '';
writeFileSync(resolve(stage, 'index.html'), readFileSync(resolve(stage, 'index.html'), 'utf8')
  .replace(/<style>([\s\S]*?)<\/style>/, (_, css) => `<style>${minifyCss(css)}${launchCss}</style>`)
  .replace('<html lang="en">', `<html lang="${escapeHtml(webManifest.lang)}">`)
  .replace('<title>Exact</title>', `<title>${escapeHtml(webManifest.name)}</title>`)
  .replace(
    '<script type="module" src="./glue.js"></script>',
    `<link rel="alternate" type="application/vnd.exact.envelope+json" href="./exact.json">\n<link rel="manifest" href="./manifest.json">\n${icon ? `<link rel="icon" type="${escapeHtml(icon.type ?? 'image/png')}" href="./${escapeHtml(icon.src)}">\n` : ''}${webManifest.theme_color ? `<meta name="theme-color" content="${escapeHtml(webManifest.theme_color)}">\n` : ''}${pageNative ? '<meta name="exact-native" content="./modules/index.js">\n' : ''}<script type="module" src="./glue.js"></script>`,
  ));
// app.wasm's URL names its build (LLP 1047.000 §9): `./app.wasm?v=` and
// the first 16 hex digits of its SHA-256, in the shell's preload, which the
// capture script and the glue fetch (and a document's checkpoint, when it
// drops the preload). A URL that names its content caches for good, so a
// browser keeps the build as the dictionary the render server sends the next
// one against (`--generations`). The file keeps its name, and the glue stays
// the same across builds: a deploy that changes only the wasm leaves it cached.
if (!bakeOnly) {
  const wasmUrl = `./app.wasm?v=${createHash('sha256').update(readFileSync(out)).digest('hex').slice(0, 16)}`;
  const shellText = readFileSync(resolve(stage, 'index.html'), 'utf8');
  if (!shellText.includes('href="./app.wasm"')) throw new Error('index.html no longer preloads ./app.wasm');
  writeFileSync(resolve(stage, 'index.html'), shellText.replace('href="./app.wasm"', `href="${wasmUrl}"`));
}
// Documents (LLP 1048.000 D3, D7, D9): every route the plan declares
// `render=build`, rendered by the app's native render entry (`<app>-render`,
// exact_render::main; looked up in its Linux crate beside its native data
// source, then in its web crate)
// from the plan the wasm carries, each a whole page composed over this
// shell (exact_render::page): the renderer's <head>, the document in
// #exact-root, its checkpoint; an idle page preloads its wasm and
// navigation.js with the document, an interaction page nothing. Then
// 404.html, sitemap.xml (absolute, against the manifest's origin),
// robots.txt, and the shell itself as shell.html — what a client route is
// served, and what the render server composes documents over.
const renderBin = crate.replace(/-web$/, '-render');
const renderAt = [['linux', app.crate('linux')], ['web', crate]]
  .find(([dir]) => existsSync(resolve(app.dir, dir, 'src/bin', `${renderBin}.rs`)));
const renderCrate = renderAt?.[1];
// Pay for what you use: an app that renders nothing at build builds and runs
// no render entry (the wasm says which locations it renders, from its plan).
// `--bake` has no wasm to ask: the entry, where there is one, renders what the plan declares.
const locationsLen = typeof exports.exact_build_locations === 'function' ? exports.exact_build_locations() : null;
const buildLocations = bakeOnly ? (renderAt ? ['the plan\'s'] : []) : locationsLen === null ? [] : JSON.parse(Buffer.from(new Uint8Array(exports.memory.buffer, exports.exact_out(), locationsLen)).toString('utf8'));
if (buildLocations.error) throw new Error(`the plan's render=build routes: ${buildLocations.error}`);
let documentNote = buildLocations.length ? `${buildLocations.length} declared, but no ${renderBin} entry in ${app.crate('linux')} or ${crate}` : 'none declared';
if (buildLocations.length && renderAt) {
  // A build-time tool, never shipped: it renders the plan it is handed, so
  // its own Linux bake takes development trust (a production bake would
  // demand a publisher receipt for an updater nothing publishes).
  const renderEnv = { ...buildEnv, CARGO_TARGET_DIR: app.target, EXACT_UPDATE_TRUST: 'development' };
  delete renderEnv.EXACT_BAKE_OUTPUT;
  const rendered = spawnSync('cargo', ['run', '-q', '-p', renderCrate, '--bin', renderBin, '--', '--plan', planOut,
    '--name', webManifest.name, ...(app.origin ? ['--origin', app.origin] : []), '--shell', resolve(stage, 'index.html'), '--build'],
  { cwd: app.dir, env: renderEnv, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (rendered.status !== 0) throw new Error(`${renderBin}: ${rendered.stderr}${rendered.stdout}`);
  const shell = readFileSync(resolve(stage, 'index.html'), 'utf8');
  const pages = rendered.stdout.split('\n').filter(Boolean).map((line) => JSON.parse(line));
  if (pages.length) writeFileSync(resolve(stage, 'shell.html'), shell);
  const listed = [];
  for (const doc of pages) {
    if (doc.error) throw new Error(`${renderBin} ${doc.location}: ${doc.error}`);
    const html = doc.page;
    const file = doc.notfound ? '404.html' : `${decodeURIComponent(doc.location).replace(/^\/|\/$/g, '')}/index.html`.replace(/^\//, '');
    if (!resolve(stage, file).startsWith(stage + '/')) throw new Error(`${renderBin}: location ${doc.location} leaves dist`);
    mkdirSync(resolve(stage, file, '..'), { recursive: true });
    writeFileSync(resolve(stage, file), html);
    if (!doc.notfound && !/noindex/i.test(doc.robots ?? '')) listed.push(doc.location);
  }
  if (pages.length) {
    const xml = (t) => String(t).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    const origin = app.origin?.replace(/\/+$/, '');
    if (origin) writeFileSync(resolve(stage, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${listed.map((location) => `  <url><loc>${xml(origin + location)}</loc></url>\n`).join('')}</urlset>\n`);
    writeFileSync(resolve(stage, 'robots.txt'), `User-agent: *\nAllow: /\n${origin ? `Sitemap: ${origin}/sitemap.xml\n` : ''}`);
  }
  // A render that reached its deadline still ships: its placeholders show
  // until the runtime asks what was pending (LLP 1048.000 D9).
  const late = pages.filter((doc) => doc.settled === false).map((doc) => doc.location);
  documentNote = `${pages.length} document${pages.length === 1 ? '' : 's'}${pages.length && !app.origin ? ' (no origin: no sitemap)' : ''}${late.length ? `; at the deadline, with placeholders: ${late.join(', ')}` : ''}`;
}
// The deep-link association file (LLP 1030 D1; 1030.000 D2): generated from
// the manifest when the iOS host claims the domain and names its team; a
// static origin file Apple's CDN fetches, never a dev-server claim.
const ios = app.manifest.host?.ios ?? {};
// @ref LLP 1069.006 D2 — a claimed https auth callback on Apple needs this
// origin's `webcredentials` entry too (not `applinks:`, which stays routing).
const authReach = bakedReceipt.reach?.auth ?? {};
const webcredentials = (authReach.callbacks ?? []).some((c) => app.origin && c.startsWith(new URL(app.origin).origin + '/'));
if ((ios.associatedDomains || webcredentials) && ios.team) {
  mkdirSync(resolve(stage, '.well-known'), { recursive: true });
  writeFileSync(resolve(stage, '.well-known/apple-app-site-association'), JSON.stringify({
    ...(ios.associatedDomains ? { applinks: { details: [{ appIDs: [`${ios.team}.${app.id}`], components: [{ '/': '*' }] }] } } : {}),
    ...(webcredentials ? { webcredentials: { apps: [`${ios.team}.${app.id}`] } } : {}),
  }) + '\n');
}
// The web's auth callback page and the two client-metadata documents
// (LLP 1069.006 D4; after review, item 3: one client id per
// `application_type`, since the AT Protocol's metadata names one type and
// a `web` client's redirect URIs are https only).
let authNote = '';
if (authReach.sessions) {
  mkdirSync(resolve(stage, '.exact/auth'), { recursive: true });
  copyFileSync(resolve(root, 'host/web/auth-callback.html'), resolve(stage, '.exact/auth/callback'));
  copyFileSync(resolve(root, 'host/web/auth-callback.js'), resolve(stage, '.exact/auth/callback.js'));
  const docs = authClientMetadata(app, authReach.callbacks ?? []);
  for (const [name, doc] of Object.entries(docs)) writeFileSync(resolve(stage, `.exact/auth/${name}.json`), JSON.stringify(doc, null, 2) + '\n');
  authNote = `; auth: callback page${Object.keys(docs).length ? `, client metadata (${Object.keys(docs).join(', ')})` : ' (no origin: no client metadata)'}`;
}

// The app's GPU module (LLP 1009 D2): a second wasm the page fetches on
// demand, built with wasm-bindgen's glue (its exports are the module's ABI on
// the web) and wasm-opt. Only when the app has a GPU crate.
// Each declared GPU module (LLP 1009 D6) is its own wasm under gpu/, fetched
// the first time a canvas of one of its surfaces mounts.
const gpu = bakeOnly ? { note: 'none (--bake)', built: false } : webGpuArtifacts(app, stage);
let gpuNote = gpu.note;
if (gpu.built) {
  copyHostFiles('gpu');
  if (gpuModules(app.manifest).length) copyHostFiles('gpuModules');
  gpuNote += ', on demand';
}
// @ref LLP 1024 D3/D5 — the app's module table, beside the page only when the
// app has modules (the GPU gate): its web executor under `modules/`, fetched
// after first paint by the host's adapter.
let moduleNote = 'no native modules';
if (app.modules.tags.length || app.modules.web) {
  const release = buildEnv.EXACT_UPDATE_TRUST === 'production';
  let provided = [];
  if (app.modules.web) {
    copyStaticTreeIfPresent(dirname(app.modules.web), resolve(stage, 'modules'));
    provided = Object.keys((await import(app.modules.web)).roster ?? {});
  }
  checkModuleRoster(app, provided, 'web', release);
  copyHostFiles('native');
  moduleNote = `modules/ (${provided.join(', ') || 'none'}), on demand`;
}
// Written last inside the private stage. Dev startup trusts a dist only when
// this marker and the public plan card agree, so a partial/corrupt directory
// can never be mistaken for a completed build of the requested app.
writeFileSync(resolve(stage, '.exact-build.json'), JSON.stringify({
  exactBuild: 1, app: { id: app.id, name: app.displayName }, manifestSha256: appManifestDigest(app),
  files: await publicFileCards(stage).finally(closeFilesystemReader),
}) + '\n');
rmSync(previous, { recursive: true, force: true });
if (existsSync(dist)) renameSync(dist, previous);
try {
  renameSync(stage, dist);
  stage = null;
} catch (error) {
  if (existsSync(previous)) renameSync(previous, dist);
  throw error;
}
rmSync(previous, { recursive: true, force: true });
if (bakeOnly) { console.log(`${relative(process.cwd(), dist) || "."}: the bake (no wasm): app.plan ${kib(planBytes.length)}, exact.json, index.html; documents: ${documentNote}${authNote}; modules: ${moduleNote}`); process.exit(0); }
const textFlowWasm = readFileSync(resolve(dist, 'textflow.wasm'));
const markdownEditor = readFileSync(resolve(dist, 'markup-editor.wasm'));
console.log(`${relative(process.cwd(), dist) || "."}: app.wasm ${kib(wasm.length)} (${kib(gzipSync(wasm, { level: 9 }).length)} gzip; ${optNote}), index.html, glue.js, app.plan ${kib(planBytes.length)}, exact.json; documents: ${documentNote}${authNote}; GPU: ${gpuNote}; modules: ${moduleNote}; markup-editor.wasm ${kib(markdownEditor.length)} (${kib(gzipSync(markdownEditor, { level: 9 }).length)} gzip), on demand; textflow.wasm ${kib(textFlowWasm.length)} (${kib(gzipSync(textFlowWasm, { level: 9 }).length)} gzip), on demand`);
