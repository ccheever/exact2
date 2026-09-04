#!/usr/bin/env node
// Build the web app: the wasm under the `web` profile (size-tuned), `wasm-opt -Oz`
// when binaryen is on PATH, then `dist/` = index.html + glue.js + app.wasm.
// Usage: node host/web/build.mjs [crate=caltrain-web]
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { gzipSync } from 'node:zlib';
import { resolveApp } from '../../scripts/app.mjs';
import { appManifestDigest, copyStaticTree, listAssets, publicFileCards, webEnvelope } from './serve.mjs';

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
const r = spawnSync('cargo', ['build', '-p', crate, '--lib', '--profile', 'web', '--target', 'wasm32-unknown-unknown'], { cwd: app.workspace, stdio: 'inherit' });
if (r.status !== 0) process.exit(r.status ?? 1);
const built = resolve(app.target, 'wasm32-unknown-unknown/web', crate.replace(/-/g, '_') + '.wasm');
// Build one app into its own staging directory. Only a complete build replaces
// dist, so a server sees the previous app or the next one, never a mixture;
// replacing the directory also drops every stale optional/private artifact.
// Stages live under ignored target/, so even a SIGKILL leaves no source dirt.
const stages = resolve(root, 'target/web-dist-stages');
mkdirSync(stages, { recursive: true });
let stage = mkdtempSync(resolve(stages, `${app.name.replace(/[^a-zA-Z0-9_-]/g, '_')}-`));
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
if (existsSync(assets)) copyStaticTree(assets, resolve(stage, 'assets'));
const deck = resolve(app.dir, 'deck');
if (existsSync(deck)) copyStaticTree(deck, resolve(stage, 'deck'));
// The GPU crate's shaders (LLP 1030 D8): `shaders/<name>.wgsl` beside the
// page, fetched and registered by the GPU glue before a surface is created
// — never a string in the wasm.
const shaders = resolve(app.dir, 'gpu', 'shaders');
if (existsSync(shaders)) copyStaticTree(shaders, resolve(stage, 'shaders'));
copyFileSync(resolve(root, 'host/web/index.html'), resolve(stage, 'index.html'));
copyFileSync(resolve(root, 'host/web/glue.js'), resolve(stage, 'glue.js'));

// The plan and its pointer card (LLP 1023 D1/D2): extract the exact bytes
// baked into the produced, optimized wasm. Compiling app.contract a second
// time here could pair app.wasm with a later source revision. A native client
// GETs the page URL, follows index.html's link to exact.json, and fetches this
// app.plan; a browser never notices. Header offsets are the generated
// encoder's fixed little-endian layout.
const planOut = resolve(stage, 'app.plan');
const wasm = readFileSync(out);
const { instance } = await WebAssembly.instantiate(wasm, {});
const exports = instance.exports;
if (typeof exports.exact_plan !== 'function' || typeof exports.exact_out !== 'function' || !(exports.memory instanceof WebAssembly.Memory)) {
  throw new Error('the web wasm does not export exact_plan, exact_out, and memory');
}
const planLen = exports.exact_plan();
const planPtr = exports.exact_out();
const planBytes = Buffer.from(new Uint8Array(exports.memory.buffer, planPtr, planLen));
if (planBytes.length < 36 || planBytes.subarray(0, 4).toString() !== 'EXPL') throw new Error('the web wasm returned an invalid baked plan');
writeFileSync(planOut, planBytes);
writeFileSync(resolve(stage, 'exact.json'), JSON.stringify(webEnvelope(app, planBytes, listAssets(stage))) + '\n');
// The web app manifest (LLP 1030 D2/D10; 1030.000 D7): the W3C keys of
// `app.json`, copied out as `manifest.json`; the page links it, takes its
// name as the title, and its first icon as the favicon. An installed PWA's
// icon and name are the browser's cached copies of these — the origin's
// carrier, at its real strength.
const webKeys = ['name', 'short_name', 'id', 'start_url', 'display', 'theme_color', 'background_color', 'icons'];
const webManifest = Object.fromEntries(webKeys.filter((k) => app.manifest[k] !== undefined).map((k) => [k, app.manifest[k]]));
webManifest.name ??= app.displayName;
webManifest.start_url ??= '/';
writeFileSync(resolve(stage, 'manifest.json'), JSON.stringify(webManifest, null, 2) + '\n');
const icon = webManifest.icons?.[0];
const escapeHtml = (t) => String(t).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
writeFileSync(resolve(stage, 'index.html'), readFileSync(resolve(stage, 'index.html'), 'utf8')
  .replace('<title>Exact</title>', `<title>${escapeHtml(webManifest.name)}</title>`)
  .replace(
    '<script type="module" src="./glue.js"></script>',
    `<link rel="alternate" type="application/vnd.exact.envelope+json" href="./exact.json">\n<link rel="manifest" href="./manifest.json">\n${icon ? `<link rel="icon" type="${escapeHtml(icon.type ?? 'image/png')}" href="./${escapeHtml(icon.src)}">\n` : ''}${webManifest.theme_color ? `<meta name="theme-color" content="${escapeHtml(webManifest.theme_color)}">\n` : ''}<script type="module" src="./glue.js"></script>`,
  ));
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
const gpuCrate = crate.replace(/-web$/, '-gpu');
let gpuNote = 'no GPU crate';
if (existsSync(resolve(app.dir, 'gpu', 'Cargo.toml'))) {
  const g = spawnSync('cargo', ['build', '-p', gpuCrate, '--lib', '--profile', 'web', '--target', 'wasm32-unknown-unknown', '--config', 'profile.web.strip=false'], { cwd: app.workspace, stdio: 'inherit' });
  if (g.status !== 0) process.exit(g.status ?? 1);
  const gpuWasm = resolve(app.target, 'wasm32-unknown-unknown/web', gpuCrate.replace(/-/g, '_') + '.wasm');
  const wb = spawnSync('wasm-bindgen', ['--target', 'web', '--no-typescript', '--out-dir', stage, '--out-name', 'gpu', gpuWasm], { stdio: 'inherit' });
  if (wb.error?.code === 'ENOENT') { gpuNote = 'wasm-bindgen not on PATH (cargo install wasm-bindgen-cli): GPU module not built'; }
  else if (wb.status !== 0) process.exit(wb.status ?? 1);
  else {
    const bg = resolve(stage, 'gpu_bg.wasm');
    const o = spawnSync('wasm-opt', ['-Oz', '--enable-bulk-memory', '--enable-nontrapping-float-to-int', '--enable-sign-ext', '--enable-mutable-globals', '--strip-debug', '--strip-producers', '-o', bg, bg], { stdio: 'inherit' });
    copyFileSync(resolve(root, 'host/web/gpu-glue.js'), resolve(stage, 'gpu-glue.js'));
    const gw = readFileSync(bg);
    gpuNote = `gpu_bg.wasm ${kib(gw.length)} (${kib(gzipSync(gw, { level: 9 }).length)} gzip${o.status === 0 ? ', wasm-opt' : ''}), gpu.js ${kib(readFileSync(resolve(stage, 'gpu.js')).length)}, on demand`;
  }
}
// Written last inside the private stage. Dev startup trusts a dist only when
// this marker and the public plan card agree, so a partial/corrupt directory
// can never be mistaken for a completed build of the requested app.
writeFileSync(resolve(stage, '.exact-build.json'), JSON.stringify({
  exactBuild: 1, app: { id: app.id, name: app.displayName }, manifestSha256: appManifestDigest(app),
  files: publicFileCards(stage),
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
console.log(`host/web/dist: app.wasm ${kib(wasm.length)} (${kib(gzipSync(wasm, { level: 9 }).length)} gzip; ${optNote}), index.html, glue.js, app.plan ${kib(planBytes.length)}, exact.json; GPU: ${gpuNote}`);
