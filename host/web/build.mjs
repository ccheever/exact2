#!/usr/bin/env bun
// Build the web app: the wasm under the `web` profile (size-tuned), `wasm-opt -Oz`
// when binaryen is on PATH, then `dist/` = index.html + glue.js + app.wasm.
// Usage: bun host/web/build.mjs [crate=caltrain-web]
// Developer builds bake development trust; EXACT_UPDATE_TRUST=production
// requires signing keys, and the deploy verb always selects production.
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { gzipSync } from 'node:zlib';
import { writeInstallPages } from '../../scripts/install-page.mjs';
import { gpuModules, rustPolicy, webHostFiles } from '../../scripts/app.mjs';
import { buildRust, rustFiles, rustCards, rustPackage } from '../../scripts/rust.mjs';
import { copyShaders, bakeOutput, buildBake, readBake, verifyBakeFiles, developmentBuildEnv, resolveApp } from '../../scripts/app.mjs';
import { closeFilesystemReader } from '../../scripts/filesystem.mjs';
import { appManifestDigest, copyStaticTreeIfPresent, listAssets, publicFileCards, webEnvelope, moduleCards, MODULE_FILES } from './serve.mjs';

const app = resolveApp(process.argv[2]);
const crate = app.crate('web');
const kib = (n) => `${(n / 1024).toFixed(0)} KiB`;
const root = resolve(new URL('../..', import.meta.url).pathname);
// `EXACT_WEB_DIST` names another output directory: `exact deploy` bakes into a
// run-specific one and never publishes from the dev server's shared dist/.
const dist = process.env.EXACT_WEB_DIST ? resolve(process.env.EXACT_WEB_DIST) : resolve(root, 'host/web/dist');
const previous = `${dist}.previous`;
// A hard stop can land after dist moved aside but before the completed stage
// took its place. Restore the prior complete build before doing slow work;
// serve.mjs/dev.mjs also fall back to it during the live rename window.
if (!existsSync(dist) && existsSync(previous)) renameSync(previous, dist);
else if (existsSync(dist) && existsSync(previous)) rmSync(previous, { recursive: true, force: true });
const buildEnv = developmentBuildEnv();
buildEnv.EXACT_BAKE_OUTPUT = bakeOutput(app, buildEnv);
const buildReceipt = buildBake(app, 'web', 'wasm32-unknown-unknown', {env:buildEnv});
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

// Post-link optimization. The feature flags match what rustc's wasm32 target emits.
const opt = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', out, built], { stdio: 'inherit' });
let optNote;
if (opt.error?.code === 'ENOENT') { copyFileSync(built, out); optNote = 'wasm-opt not on PATH (brew install binaryen): shipped unoptimized'; }
else if (opt.status !== 0) process.exit(opt.status ?? 1);
else optNote = 'wasm-opt -Oz';

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
function copyHostFiles(group) {
  for (const [name, source] of Object.entries(webHostFiles(group))) copyFileSync(resolve(root, source), resolve(stage, name));
}
copyHostFiles('base');
// The Markdown editor's rules (exact-markdown-editor, LLP 1045 D5) are their
// own wasm beside markup-editor.js, fetched only when a Markdown textarea mounts.
const editor = spawnSync('cargo', ['build', '--locked', '--offline', '-q', '-p', 'exact-markdown-editor', '--lib', '--target', 'wasm32-unknown-unknown', '--profile', 'web'], { cwd: root, env: buildEnv, stdio: 'inherit' });
if (editor.status !== 0) process.exit(editor.status ?? 1);
const editorBuilt = resolve(process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(root, 'target'), 'wasm32-unknown-unknown/web/exact_markdown_editor.wasm');
const editorWasm = resolve(stage, 'markup-editor.wasm');
if (spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', editorWasm, editorBuilt], { stdio: 'inherit' }).status !== 0) copyFileSync(editorBuilt, editorWasm);

// The plan and its pointer card (LLP 1023 D1/D2): extract the exact bytes
// baked into the produced, optimized wasm. Compiling app.contract a second
// time here could pair app.wasm with a later source revision. A native client
// GETs the page URL, follows index.html's link to exact.json, and fetches this
// app.plan; a browser never notices. Header offsets are the generated
// encoder's fixed little-endian layout.
const planOut = resolve(stage, 'app.plan');
const wasm = readFileSync(out);
const unbooted = () => { throw new Error('app logic ran while extracting baked bytes'); };
const { instance } = await WebAssembly.instantiate(wasm, { exact_js: { call: unbooted }, exact_rust: { load: unbooted, call: unbooted, read: unbooted, drop: unbooted } });
const exports = instance.exports;
if (typeof exports.exact_plan !== 'function' || typeof exports.exact_out !== 'function' || !(exports.memory instanceof WebAssembly.Memory)) {
  throw new Error('the web wasm does not export exact_plan, exact_out, and memory');
}
const planLen = exports.exact_plan();
const planPtr = exports.exact_out();
const planBytes = Buffer.from(new Uint8Array(exports.memory.buffer, planPtr, planLen));
if (planBytes.length < 36 || planBytes.subarray(0, 4).toString() !== 'EXPL') throw new Error('the web wasm returned an invalid baked plan');
writeFileSync(planOut, planBytes);
let pairedModule = null;
// A module client's build script emits paired artifacts beside its receipt.
// Extract the exact embedded receipt/JS/HBC rather than rebaking moving sources.
if (typeof exports.exact_module_artifact === 'function') {
  const files = new Map([['app.plan', planBytes]]);
  for (const [index, name] of ['app.module.json', 'app.js', 'app.hbc'].entries()) {
    const len = exports.exact_module_artifact(index);
    const body = Buffer.from(new Uint8Array(exports.memory.buffer, exports.exact_out(), len));
    files.set(name, body); writeFileSync(resolve(stage, name), body);
  }
  pairedModule = Object.fromEntries(Object.entries(moduleCards(files, app.id)).map(([key, card]) => [key, { ...card, url: './' + MODULE_FILES[key] }]));
  copyHostFiles('module');
}
if (typeof exports.exact_compat !== 'function') throw new Error('the web wasm exposes no baked receipt');
const compatLen = exports.exact_compat();
const embeddedCompat = Buffer.from(new Uint8Array(exports.memory.buffer, exports.exact_out(), compatLen)).toString('utf8');
const bakedReceipt = readBake(app, 'web', 'wasm32-unknown-unknown', buildEnv.EXACT_BAKE_OUTPUT);
if (JSON.stringify(JSON.parse(embeddedCompat)) !== JSON.stringify(bakedReceipt)) throw new Error('the emitted receipt differs from the wasm receipt');
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
const webKeys = ['name', 'short_name', 'id', 'start_url', 'display', 'theme_color', 'background_color', 'icons', 'lang'];
const webManifest = Object.fromEntries(webKeys.filter((k) => app.manifest[k] !== undefined).map((k) => [k, app.manifest[k]]));
webManifest.name ??= app.displayName;
webManifest.start_url ??= '/';
// The document's language (`<html lang>`, WCAG 3.1.1). Every app here is in
// English, so an app that declares none is `en`.
webManifest.lang ??= 'en';
writeFileSync(resolve(stage, 'manifest.json'), JSON.stringify(webManifest, null, 2) + '\n');
const sourceRevision = spawnSync('git', ['rev-parse', '--short=10', 'HEAD'], {cwd:app.dir,encoding:'utf8'});
const sourceChanges = spawnSync('git', ['status', '--porcelain'], {cwd:app.dir,encoding:'utf8'});
writeInstallPages(stage, app.manifest, {id:buildReceipt.binary.sha256, source:sourceRevision.status === 0 ? sourceRevision.stdout.trim() : null, dirty:sourceChanges.status === 0 && !!sourceChanges.stdout.trim(), builtAt:new Date().toISOString(), mode:buildEnv.EXACT_UPDATE_TRUST === 'production' ? 'Release build' : 'Development build'});
const icon = webManifest.icons?.[0];
const escapeHtml = (t) => String(t).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
writeFileSync(resolve(stage, 'index.html'), readFileSync(resolve(stage, 'index.html'), 'utf8')
  .replace('<html lang="en">', `<html lang="${escapeHtml(webManifest.lang)}">`)
  .replace('<title>Exact</title>', `<title>${escapeHtml(webManifest.name)}</title>`)
  .replace(
    '<script type="module" src="./glue.js"></script>',
    `<link rel="alternate" type="application/vnd.exact.envelope+json" href="./exact.json">\n<link rel="manifest" href="./manifest.json">\n${icon ? `<link rel="icon" type="${escapeHtml(icon.type ?? 'image/png')}" href="./${escapeHtml(icon.src)}">\n` : ''}${webManifest.theme_color ? `<meta name="theme-color" content="${escapeHtml(webManifest.theme_color)}">\n` : ''}<script type="module" src="./glue.js"></script>`,
  ));
// Documents (LLP 1048.000 D3, D7, D9): every route the plan declares
// `render=build`, rendered by the app's native render entry (`<app>-render`
// beside its native data source, in its Linux crate; exact_render::main)
// from the plan the wasm carries, each a whole page composed over this
// shell (exact_render::page): the renderer's <head>, the document in
// #exact-root, its checkpoint; nothing preloads the glue or the wasm. Then
// 404.html, sitemap.xml (absolute, against the manifest's origin),
// robots.txt, and the shell itself as shell.html — what a client route is
// served, and what the render server composes documents over.
const renderBin = crate.replace(/-web$/, '-render');
const renderCrate = app.crate('linux');
// Pay for what you use: an app that renders nothing at build builds and runs
// no render entry (the wasm says which locations it renders, from its plan).
const locationsLen = typeof exports.exact_build_locations === 'function' ? exports.exact_build_locations() : null;
const buildLocations = locationsLen === null ? [] : JSON.parse(Buffer.from(new Uint8Array(exports.memory.buffer, exports.exact_out(), locationsLen)).toString('utf8'));
if (buildLocations.error) throw new Error(`the plan's render=build routes: ${buildLocations.error}`);
const renderEntry = existsSync(resolve(app.dir, 'linux/src/bin', `${renderBin}.rs`));
let documentNote = buildLocations.length ? `${buildLocations.length} declared, but no ${renderBin} entry in ${renderCrate}` : 'none declared';
if (buildLocations.length && renderEntry) {
  const renderEnv = { ...buildEnv, CARGO_TARGET_DIR: app.target };
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
if (ios.associatedDomains && ios.team) {
  mkdirSync(resolve(stage, '.well-known'), { recursive: true });
  writeFileSync(resolve(stage, '.well-known/apple-app-site-association'), JSON.stringify({ applinks: { details: [{ appIDs: [`${ios.team}.${app.id}`], components: [{ '/': '*' }] }] } }) + '\n');
}

// The app's GPU module (LLP 1009 D2): a second wasm the page fetches on
// demand, built with wasm-bindgen's glue (its exports are the module's ABI on
// the web) and wasm-opt. Only when the app has a GPU crate.
// Each declared GPU module (LLP 1009 D6) is its own wasm under gpu/, fetched
// the first time a canvas of one of its surfaces mounts.
const gpuArtifacts = [...(app.hasGpu ? [{ crate: crate.replace(/-web$/, '-gpu'), stem: 'gpu' }] : []),
  ...gpuModules(app.manifest).map(({ name }) => ({ crate: app.crate(`gpu-${name}`), stem: `gpu/${name}` }))];
let gpuNote = gpuArtifacts.length ? '' : 'no GPU crate';
for (const { crate: gpuCrate, stem } of gpuArtifacts) {
  const gpuWasm = resolve(app.target, 'wasm32-unknown-unknown/web', gpuCrate.replace(/-/g, '_') + '.wasm');
  const [dir, name] = stem.includes('/') ? [resolve(stage, 'gpu'), stem.slice(4)] : [stage, stem];
  const wb = spawnSync('wasm-bindgen', ['--target', 'web', '--no-typescript', '--out-dir', dir, '--out-name', name, gpuWasm], { stdio: 'inherit' });
  if (wb.error?.code === 'ENOENT') { gpuNote = 'wasm-bindgen not on PATH (cargo install wasm-bindgen-cli): GPU module not built'; break; }
  else if (wb.status !== 0) process.exit(wb.status ?? 1);
  const bg = resolve(stage, `${stem}_bg.wasm`);
  const o = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', bg, bg], { stdio: 'inherit' });
  const gw = readFileSync(bg);
  gpuNote += `${gpuNote ? '; ' : ''}${stem}_bg.wasm ${kib(gw.length)} (${kib(gzipSync(gw, { level: 9 }).length)} gzip${o.status === 0 ? ', wasm-opt' : ''}), ${stem}.js ${kib(readFileSync(resolve(stage, `${stem}.js`)).length)}`;
}
if (gpuArtifacts.length && !gpuNote.startsWith('wasm-bindgen')) {
  copyHostFiles('gpu');
  if (gpuModules(app.manifest).length) copyHostFiles('gpuModules');
  gpuNote += ', on demand';
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
const markdownEditor = readFileSync(resolve(dist, 'markup-editor.wasm'));
console.log(`host/web/dist: app.wasm ${kib(wasm.length)} (${kib(gzipSync(wasm, { level: 9 }).length)} gzip; ${optNote}), index.html, glue.js, app.plan ${kib(planBytes.length)}, exact.json; documents: ${documentNote}; GPU: ${gpuNote}; markup-editor.wasm ${kib(markdownEditor.length)} (${kib(gzipSync(markdownEditor, { level: 9 }).length)} gzip), on demand`);
