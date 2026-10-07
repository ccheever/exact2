#!/usr/bin/env bun
// Serve the current web build for a browser — and, through exact.json, for
// a native client (LLP 1023 D1). LAN by default (D8); --loopback (or
// EXACT_LOOPBACK=1) binds 127.0.0.1 only.
// Usage: bun host/web/serve.mjs [port=8765] [--loopback]
import { createServer } from 'node:http';
import { brotliCompress, constants as zlib, gzip } from 'node:zlib';
import { createHash, randomBytes } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { arch, cpus, networkInterfaces, platform } from 'node:os';
import { basename, dirname, extname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { filesystem, filesystemRead } from '../../scripts/filesystem.mjs';
import { copyShaders, developmentURLScheme, webDist, webHostFiles } from '../../scripts/app.mjs';
import { appDocumentPath, webRequestURL, parseWebRoot, sha256, webReleasePath, webRootPath } from '../../scripts/origin.mjs';

import { INSTALL_FILES, INSTALL_PUBLIC, INSTALL_ROOT, installRoute, installNetworkPage } from '../../scripts/install-page.mjs';

const PUBLIC_FILES = new Set([
  ...INSTALL_PUBLIC,
  ...Object.keys(webHostFiles()).map(name => '/' + name),
  '/app.js', '/app.hbc', '/app.module.json', '/app.plan', '/app.wasm', '/exact.json',
  '/gpu.js', '/gpu_bg.wasm', '/markup-editor.wasm', '/textflow.wasm', '/index.html', '/manifest.json',
  // The one dot path a static origin serves: the deep-link association
  // file bake generates (LLP 1030 D1), read by Apple's CDN over HTTPS.
  '/.well-known/apple-app-site-association',
]);
// `/gpu/`: each declared GPU module's wasm and its glue (LLP 1009 D6).
// `/stages/`: the core's staged capabilities, named by digest (LLP 1047.000).
// `/modules/`: the app's native-module web executor (LLP 1024 D3), page code.
const PUBLIC_TREES = ['/assets/', '/deck/', '/shaders/', '/rust/', '/gpu/', '/stages/', '/modules/'];
const REQUIRED_BUILD_FILES = ['app.plan', 'app.wasm', 'exact.json', 'glue.js', 'navigation.js', 'index.html', 'manifest.json'];
// An origin's update streams (LLP 1030.000 D7; `scripts/origin.mjs`):
// `.exact/blobs/<sha256>` and `.exact/<channel>/<compatibility id>/…` — the
// one dot path a client fetches. Inside it every other dot name (the
// stream's `.lock`) stays private.
const UPDATE_TREE = '/.exact/';
// The web's auth callback page and its script (LLP 1069.006 D4), under the
// update tree: never stored, and sent with no referrer.
export const AUTH_CALLBACK = '/.exact/auth/callback';

/** Dev-only opening instructions. Never infers installation or publishes a
 * build. `links` are the token-bearing opening links this Mac's development
 * clients admit for the page's origin (`developmentLinks`, LLP 1030.000 §7). */
export function developmentOpenPage(app, links = [], page = '/') {
  const escape = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const name = escape(app.displayName), crate = escape(app.crate('apple')), url = escape(page);
  const scheme = developmentURLScheme(app.id);
  const labels = { macos: 'Open in the Mac client', 'ios-simulator': 'Open in the Simulator client', ios: 'Open in the iPhone or iPad client' };
  const native = links.map(link => `<a class="native" href="${escape(link.href)}">${escape(labels[link.destination] ?? 'Open in native client')}</a>`).join('');
  return `<!doctype html><html lang="${escape(app.manifest.lang ?? 'en')}"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="referrer" content="no-referrer"><title>Open ${name}</title>
<style>body{font:17px/1.5 system-ui;margin:0;background:#f4f5f8;color:#172033}main{max-width:620px;margin:6vh auto;padding:28px}h1{line-height:1.15}a{color:#164bc4}nav{display:flex;flex-wrap:wrap;gap:14px;margin:28px 0}nav a{padding:12px 18px;border:1px solid #164bc4;border-radius:10px;text-decoration:none}nav a.native{background:#164bc4;color:white}input{box-sizing:border-box;width:100%;padding:12px;font:14px ui-monospace,monospace}pre{overflow:auto;background:#e7eaf0;padding:12px;border-radius:8px}small{color:#4b566b}</style>
<main><small>Exact · development</small><h1>Open ${name}</h1>
<p>One app URL. Use the browser here, or hand it to this app’s installed native development client.</p>
<nav>${native}<a id="browser" href="${url}">Continue in browser</a></nav>
${native ? '' : '<p id="status" role="status">No development client built on this Mac admits this server yet; build one with <code>--url</code> as below.</p>'}
<label for="url">App URL — also works in the native menu’s Open Project</label><input id="url" readonly value="${url}">
<details${native ? '' : ' open'}><summary>Need a native client?</summary><p>Build this app’s client from its source workspace against this server. A client built with <code>--url</code> opens <code>${scheme}://open?url=…&amp;token=…</code> links for that server only, with a token made for that build.</p>
<h2>macOS</h2><pre>bun host/apple/build.mjs ${crate} --bundle --url ${url}</pre><p>Open the printed <code>.app</code> once, then return here. This is a local development bundle, not a notarized download.</p>
<h2>iPhone / iPad</h2><pre>bun host/apple/build.mjs --device ${crate} --url ${url}</pre><p>Connect an unlocked, paired device to the signing Mac. Developer Mode and a matching provisioning profile are required. Allow Local Network access and keep the app visible; the dev server needs <code>--lan</code>.</p>
<p>For an external app, use its existing <code>EXACT_APP_DIR</code> setup. A cloud workspace needs a device-reachable URL and a Mac builder; automatic cloud builds, downloads and Exact2 Go are not available in this flow yet.</p></details>
<p><small>Open only a development server you trust. This page cannot detect whether a client is installed; it lists the clients built on this Mac. The native loader still checks compatibility and may refuse the app. LAN addresses require the same network.</small></p></main></html>`;
}

function staticRelative(name) {
  if (typeof name !== 'string' || !name || name.startsWith('/') || name.includes('\\') || name.includes('\0')) throw new Error(`not a relative static-file path: ${JSON.stringify(name)}`);
  if (name.split('/').some((part) => !part || part === '.' || part === '..')) throw new Error(`not a relative static-file path: ${JSON.stringify(name)}`);
  return name;
}

// A live iframe can navigate long after its generation left the dev head.
// Keep those ordinary files outside rebuilt dist, without guessing client
// liveness. Quota exhaustion refuses publication instead of deleting readers.
export const DEV_GENERATION_CACHE_BYTES = 4 * 1024 * 1024 * 1024;
export const MODULE_FILES = { native: 'app.hbc', receipt: 'app.module.json', web: 'app.js' };
/** Verify one producer result before it enters a served generation. The
 * receipt is pairing metadata, never publisher authentication. */
export function moduleCards(files, appId) {
  const receipt = files.get(MODULE_FILES.receipt);
  if (!receipt || receipt.length > 1024 * 1024) throw new Error('missing or oversized module receipt');
  const meta = JSON.parse(receipt.toString('utf8'));
  if (meta.version !== 1 || (meta.abi !== 1 && meta.abi !== 2) || meta.appId !== appId || typeof meta.grants !== 'string'
      || !Number.isSafeInteger(meta.bytecodeVersion) || meta.bytecodeVersion <= 0) throw new Error('incompatible module receipt');
  for (const [key, name] of [['plan', 'app.plan'], ['module', 'app.hbc'], ['web', 'app.js']]) {
    const bytes = files.get(name), card = meta[key];
    if (!bytes || bytes.length > 32 * 1024 * 1024 || card?.file !== name || card.bytes !== bytes.length || card.sha256 !== sha256(bytes)) throw new Error(`module receipt does not pair ${name}`);
  }
  const hbc = files.get('app.hbc');
  if (hbc.length < 12 || hbc.readUInt32LE(8) !== meta.bytecodeVersion) throw new Error('incompatible module bytecode header');
  return Object.fromEntries(Object.entries(MODULE_FILES).map(([key, name]) => {
    const bytes = files.get(name);
    return [key, { bytes: bytes.length, sha256: sha256(bytes) }];
  }));
}
export function retainDevGeneration(cache, epoch, seq, files, quota = DEV_GENERATION_CACHE_BYTES, previousToken = null) {
  if (!/^[0-9a-f]{32}$/.test(epoch) || !Number.isSafeInteger(seq) || seq < 0) throw new Error('invalid dev generation identity');
  try {
    return filesystem({ op: 'retain', root: resolve(cache), path: `${epoch}/${seq}`, quota, previousToken,
      files: Object.fromEntries([...files].map(([name, bytes]) => [staticRelative(name), Buffer.from(bytes).toString('base64')])) });
  } catch (error) {
    if (error.message.includes('cache quota exceeded')) throw new Error(`dev generation cache is full: ${resolve(cache)}; close all pages and native dev connections before manually removing this cache, then restart the dev server`);
    throw error;
  }
}

/** The envelope is written last. An interrupted candidate has no public
 * namespace, and undeclared files never become routes through this cache. */
function* devGenerationRead(cache, pathname) {
  try {
    const match = /^\/__dev\/generation\/([0-9a-f]{32})\/([0-9]+)\/(.+)$/.exec(pathname);
    if (!match) return null;
    const name = staticRelative(decodeURIComponent(match[3]));
    if (!['app.plan', 'app.plan.map.json', 'exact.json', ...Object.values(MODULE_FILES)].includes(name) && !PUBLIC_TREES.some((tree) => ('/' + name).startsWith(tree))) return null;
    const prefix = `${match[1]}/${match[2]}`;
    const raw = (yield { op: 'get', root: resolve(cache), path: `${prefix}/exact.json` });
    if (raw === null) return null;
    const envelopeBytes = Buffer.from(raw, 'base64');
    const envelope = JSON.parse(envelopeBytes.toString('utf8'));
    if (envelope.dev?.epoch !== match[1] || String(envelope.dev?.seq) !== match[2]) return null;
    if (name === 'exact.json') return { name, body: envelopeBytes };
    const moduleKey = Object.keys(MODULE_FILES).find(key => MODULE_FILES[key] === name);
    const rustCard = Object.values(envelope.rust ?? {}).flatMap(v => [v.receipt, v.module]).find(card => card.url?.endsWith('/' + name) || card.url === name);
    const card = name === 'app.plan.map.json' ? envelope.dev.sourceMap : name === 'app.plan' ? envelope.plan : moduleKey ? envelope.module?.[moduleKey] : rustCard ?? envelope.assets?.find((asset) => asset.name === name);
    if (!card) return null;
    const value = (yield { op: 'get', root: resolve(cache), path: `${prefix}/${name}` });
    if (value === null) return null;
    const body = Buffer.from(value, 'base64');
    if (body.length !== card.bytes || sha256(body) !== card.sha256) return null;
    if (name === 'app.plan.map.json' && JSON.parse(body.toString('utf8')).digest !== envelope.plan?.sha256) return null;
    return { name, body };
  } catch { return null; }
}

export function readDevGeneration(cache, pathname) { return runReads(devGenerationRead(cache, pathname)); }
export function readDevGenerationAsync(cache, pathname) { return runReadsAsync(devGenerationRead(cache, pathname)); }

/** Every regular file under a static source tree, sorted and refused when
 * the root or any entry is a symlink or another special filesystem object. */
export function listStaticFiles(source) {
  return Object.keys(filesystem({ op: 'names', root: resolve(source) }));
}

/** Open every component from owned directory handles; there is no path
 * validation/open gap for an intermediate-directory replacement to exploit. */
export function readStaticCandidate(source, name) {
  const bytes = filesystem({ op: 'get', root: resolve(source), path: staticRelative(name) });
  if (bytes === null) { const error = new Error(`static app file disappeared: ${name}`); error.code = 'ENOENT'; throw error; }
  return Buffer.from(bytes, 'base64');
}

/** Read a source candidate once, write it privately beside `target`, run an
 * optional validator over those exact bytes, then atomically replace the
 * served file. Any refusal leaves the last-good target untouched. */
export function installStaticCandidate(source, name, target, validate = null) {
  const bytes = readStaticCandidate(source, name);
  mkdirSync(dirname(target), { recursive: true });
  const candidate = resolve(dirname(target), `.candidate-${process.pid}-${randomBytes(4).toString('hex')}`);
  try {
    writeFileSync(candidate, bytes, { flag: 'wx' });
    if (validate) validate(candidate, bytes);
    renameSync(candidate, target);
    return bytes;
  } catch (error) {
    rmSync(candidate, { force: true });
    throw error;
  }
}

/** Merge every declared shader root before committing a dev candidate. A
 * deletion in one pack must not erase the other packs; duplicates refuse it. */
export function applyShaderTreeChange(app, target, validate = null) {
  const source = resolve(dirname(target), `.shader-packs-${process.pid}-${randomBytes(4).toString('hex')}`);
  mkdirSync(source, {recursive:true});
  try {
    copyShaders(app, source);
    return applyStaticTreeChange(source, target, validate);
  } finally { rmSync(source, {recursive:true,force:true}); }
}

/** Copy one complete static source tree under the same no-symlink policy
 * used by the live candidate path. Intended for private build stages. */
export function copyStaticTree(source, target) {
  // Capture the whole source through one root handle before exposing bytes
  // to the private candidate. A link/race fails before the caller commits it.
  filesystem({ op: 'copy', root: resolve(source), target: resolve(target) });
}

function optionalInfo(path) {
  try { return lstatSync(path); }
  catch (error) { if (error.code === 'ENOENT') return null; throw error; }
}

/** Copy a tree when it is genuinely absent, while still sending a dangling
 * root link through the static-tree refusal. `existsSync` cannot make that
 * distinction and must not guard an app-visible copy. */
export function copyStaticTreeIfPresent(source, target) {
  return filesystem({ op: 'copy', root: resolve(source), target: resolve(target), optionalRoot: true }) === true;
}

/** Mirror one complete source tree into a live dist at startup. A missing
 * source removes the formerly served tree. A present tree is first copied
 * into a private sibling and only then replaces the target, so a refused
 * link or read race preserves the last-good tree. */
export function syncStaticTree(source, target, validate = null) {
  const sourcePath = resolve(source);
  const targetPath = resolve(target);
  const parent = dirname(targetPath);
  mkdirSync(parent, { recursive: true });
  const candidate = resolve(parent, `.candidate-tree-${process.pid}-${randomBytes(4).toString('hex')}`);
  const previous = resolve(parent, `.previous-tree-${process.pid}-${randomBytes(4).toString('hex')}`);
  try {
    if (!copyStaticTreeIfPresent(sourcePath, candidate)) {
      rmSync(targetPath, { recursive: true, force: true });
      return false;
    }
    if (validate) validate(candidate);
    if (optionalInfo(targetPath)) renameSync(targetPath, previous);
    try { renameSync(candidate, targetPath); }
    catch (error) {
      if (optionalInfo(previous)) renameSync(previous, targetPath);
      throw error;
    }
    rmSync(previous, { recursive: true, force: true });
    return true;
  } catch (error) {
    rmSync(candidate, { recursive: true, force: true });
    throw error;
  }
}

function staticTreeSnapshot(root) {
  return new Map(Object.entries(filesystem({ op: 'tree', root: resolve(root), optionalRoot: true }) ?? {}).map(([name, bytes]) => [name, Buffer.from(bytes, 'base64')]));
}

/** Atomically reconcile a watched whole-tree creation/deletion and return
 * only changed leaf rows. Root events are common when Darwin removes a
 * watched directory, and are also how a previously absent tree first appears. */
export function applyStaticTreeChange(source, target, validate = null) {
  const before = staticTreeSnapshot(target);
  const present = syncStaticTree(source, target, validate);
  const after = staticTreeSnapshot(target);
  const names = [...new Set([...before.keys(), ...after.keys()])].sort();
  const files = [];
  for (const name of names) {
    const had = before.get(name), has = after.get(name);
    if (!has) files.push({ name, bytes: null, removed: true });
    else if (!had?.equals(has)) files.push({ name, bytes: has, removed: false });
  }
  return { present, files };
}

/** Reflect exact shader files through the dev loop's executable. */
export function reflectShaderFiles(files, reflectBin) {
  if (!files.length) return new Map();
  const result = spawnSync(reflectBin, ['digest', ...files], { encoding: 'utf8' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error((result.stderr || result.stdout || `shader reflection exited ${result.status}`).trim());
  const out = new Map();
  for (const line of (result.stdout ?? '').trim().split('\n')) {
    const [name, digest, ...rest] = line.split(' ');
    if (!name) continue;
    if (digest === 'error') throw new Error(rest.join(' ') || `${name}: shader reflection failed`);
    out.set(name, digest);
  }
  if (out.size !== files.length) throw new Error(`shader reflection returned ${out.size} result${out.size === 1 ? '' : 's'} for ${files.length} files`);
  return out;
}

/** Reflect a complete shader tree. Any invalid or missing result rejects the
 * candidate before its tree swap. */
export function shaderInterfaceDigests(source, reflectBin) {
  const names = listStaticFiles(source).filter((name) => name.endsWith('.wgsl'));
  const reflected = reflectShaderFiles(names.map((name) => resolve(source, name)), reflectBin);
  const out = new Map();
  for (const name of names) {
    const stem = name.slice(0, -'.wgsl'.length);
    const key = basename(stem);
    const digest = reflected.get(key);
    if (!digest || out.has(stem)) throw new Error(`shader reflection did not identify ${name} uniquely`);
    out.set(stem, digest);
  }
  return out;
}

/** Map one recursive watch event, relative to a stable app root, onto the
 * configured static tree. A root or ancestor event has an empty `relative` path; a null
 * filename conservatively reconciles every tree. */
export function staticWatchChanges(watchRoot, trees, filename) {
  if (filename === null || filename === undefined || filename === '') return trees.map(([root, targetRoot]) => ({ root: resolve(root), targetRoot, relative: '', name: targetRoot, tree: true }));
  const path = resolve(watchRoot, String(filename));
  const watched = relative(resolve(watchRoot), path);
  if (watched === '..' || watched.startsWith(`..${sep}`) || isAbsolute(watched)) return [];
  const changes = [];
  for (const [tree, targetRoot] of trees) {
    const root = resolve(tree);
    const rel = relative(root, path);
    const descendant = relative(path, root);
    if (descendant !== '..' && !descendant.startsWith(`..${sep}`) && !isAbsolute(descendant)) {
      changes.push({ root, targetRoot, relative: '', name: targetRoot, tree: true });
    }
    else if (rel !== '..' && !rel.startsWith(`..${sep}`) && !isAbsolute(rel)) {
      const portable = rel.split(sep).join('/');
      changes.push({ root, targetRoot, relative: portable, name: `${targetRoot}/${portable}`, tree: false });
    }
  }
  return changes;
}

/** Poll only static paths: recursive filesystem notifications can arrive late.
 * Keep absent roots watched; directory changes discover new files. */
export function watchStaticTrees(watchRoot, trees, onChange) {
  const watched = new Map();
  const info = path => { try { return lstatSync(path, { bigint: true }); } catch { return null; } };
  function discover(path, paths) {
    paths.add(path);
    const current = info(path);
    if (!watched.has(path)) watched.set(path, current);
    try {
      if (current?.isDirectory()) {
        for (const name of readdirSync(path)) discover(resolve(path, name), paths);
      }
    } catch { /* The validated copy reports refusals; never follow a root link. */ }
  }
  function scan() {
    const paths = new Set();
    for (const [root] of trees) discover(resolve(root), paths);
    for (const path of watched.keys()) if (!paths.has(path)) watched.delete(path);
  }
  scan();
  const timer = setInterval(() => {
    const changes = [];
    let directories = false;
    for (const [path, previous] of watched) {
      const now = info(path);
      if (!['dev', 'ino', 'size', 'mtimeNs', 'ctimeNs'].some(key => now?.[key] !== previous?.[key])) continue;
      watched.set(path, now);
      const directory = now?.isDirectory() || previous?.isDirectory();
      directories ||= directory;
      for (const change of staticWatchChanges(watchRoot, trees, path)) {
        changes.push(directory ? { ...change, relative: '', name: change.targetRoot, tree: true } : change);
      }
    }
    if (directories) scan();
    for (const change of changes) onChange(change, 'change');
  }, 100);
  return { close() { clearInterval(timer); watched.clear(); } };
}

/** Apply one recursive-watch candidate. A missing leaf or directory removes
 * its complete served counterpart; every other refusal leaves it intact. */
export function applyStaticChange(source, name, target, validate = null) {
  try { return { bytes: installStaticCandidate(source, name, target, validate), removed: false }; }
  catch (error) {
    if (error.code !== 'ENOENT' && error.code !== 'ENOTDIR') throw error;
    let removedFiles = [''];
    const targetInfo = optionalInfo(resolve(target));
    if (targetInfo?.isDirectory()) {
      const files = listStaticFiles(target);
      removedFiles = files.length ? files : [''];
    }
    rmSync(target, { recursive: true, force: true });
    return { bytes: null, removed: true, removedFiles };
  }
}

/** Resolve one URL path to the current build, or to the stable previous tree
 * while build.mjs has renamed the current one aside. Generated top-level
 * files are explicit; app assets live only under the replaced trees, and an
 * origin's update streams under `.exact/`. Every other dot path and every
 * symlink are private, even when their target is inside a build. Returns
 * null for anything that must not be served. */
export function staticFile(dist, pathname) {
  let route;
  try { route = installRoute(decodeURIComponent(pathname === '/' ? '/index.html' : pathname)); }
  catch { return null; }
  if (!route.startsWith('/') || route.includes('\\') || route.includes('\0')) return null;
  const parts = route.split('/').filter(Boolean);
  const update = route.startsWith(UPDATE_TREE);
  if (parts.some((part, i) => part.startsWith('.') && !(update && i === 0)) && !PUBLIC_FILES.has(route)) return null;
  if (!PUBLIC_FILES.has(route) && !update && !PUBLIC_TREES.some((tree) => route.startsWith(tree))) return null;
  // A complete current build is authoritative even when it lacks an optional
  // route (notably GPU files). Consult previous only while the current root
  // itself is absent; otherwise two apps' artifacts could be mixed.
  let root;
  try { root = realpathSync(dist); }
  catch {
    try { root = realpathSync(`${dist}.previous`); }
    catch { return null; }
  }
  try {
    const inside = path => {
      const child = relative(root, path);
      return child !== '' && child !== '..' && !child.startsWith(`..${sep}`) && !isAbsolute(child);
    };
    const path = resolve(root, '.' + route);
    if (!inside(path)) return null;
    const real = realpathSync(path);
    // Reject both a symlink file and a file reached through a symlink dir.
    if (real !== path || !inside(real) || !statSync(real).isFile()) return null;
    return { path: real, route };
  } catch { return null; }
}

/** Resolve and read together, retrying when a build rename moved the path
 * between those operations. The opened response is wholly old or wholly
 * new; a request never observes the rename window as a synthetic 404. */
function* publishedFile(dist, pathname) {
  let route;
  try { route = installRoute(decodeURIComponent(pathname === '/' ? '/index.html' : pathname)); }
  catch { return null; }
  // Immutable generation URLs never consult the current pointer: readers
  // that already opened an older index keep all of that generation's files.
  // A release holds only what its publisher wrote (deploy.mjs `webRootFiles`:
  // a JS-target root adds its content-named chunks and pages), so any name
  // in it is public but a dot path the allowlist does not name.
  const releaseFile = (name) => PUBLIC_FILES.has('/' + name) || !name.split('/').some((part) => part.startsWith('.'));
  const release = /^\/\.exact\/root\/web\/releases\/([0-9a-f]{64})\/(.*)$/.exec(route);
  if (release) {
    const name = release[2] || 'index.html';
    if (!releaseFile(name)) return null;
    staticRelative(name);
    const rel = `${webReleasePath(release[1])}/${name}`;
    const body = (yield { op: 'get', root: resolve(dist), path: rel });
    return body === null ? null : { path: resolve(dist, rel), route: '/' + name, body: Buffer.from(body, 'base64'), immutable: true, published: true };
  }
  if (route.startsWith('/.exact/') && !INSTALL_PUBLIC.includes(route)) return undefined; // native heads/blobs and the web pointer
  const raw = (yield { op: 'get', root: resolve(dist), path: webRootPath });
  if (raw === null) return undefined; // a local build, not a deployed root
  const root = parseWebRoot(Buffer.from(raw, 'base64'));
  const card = root.files.find((file) => '/' + file.name === route);
  if (!card || !releaseFile(card.name)) return null;
  const rel = `${webReleasePath(root.id)}/${card.name}`;
  const value = (yield { op: 'get', root: resolve(dist), path: rel });
  if (value === null) return null;
  const body = Buffer.from(value, 'base64');
  if (body.length !== card.bytes || sha256(body) !== card.sha256) return null;
  return { path: resolve(dist, rel), route, body, immutable: false, published: true };
}

function* staticRead(dist, pathname) {
  try {
    const published = yield* publishedFile(dist, pathname);
    if (published !== undefined) return published;
  } catch { return null; } // a malformed pointer or unsafe path never falls back to stale files
  for (let attempt = 0; attempt < 4; attempt++) {
    const found = staticFile(dist, pathname);
    if (!found) continue;
    try {
      const bytes = (yield { op: 'get', root: dirname(found.path), path: basename(found.path) });
      if (bytes !== null) return { ...found, body: Buffer.from(bytes, 'base64') };
    }
    catch { /* retry against dist or dist.previous */ }
  }
  return null;
}

// One route/integrity policy, driven synchronously by build tooling and through
// the resident asynchronous reader by HTTP. Read failures reenter the same guards.
export function readStaticFile(dist, pathname) { return runReads(staticRead(dist, pathname)); }
export function readStaticFileAsync(dist, pathname) { return runReadsAsync(staticRead(dist, pathname)); }
function runReads(reads) {
  let step = reads.next();
  while (!step.done) {
    try { step = reads.next(filesystem(step.value)); }
    catch (error) { step = reads.throw(error); }
  }
  return step.value;
}
async function runReadsAsync(reads) {
  let step = reads.next();
  while (!step.done) {
    try { step = reads.next(await filesystemRead(step.value)); }
    catch (error) { step = reads.throw(error); }
  }
  return step.value;
}

/** Every file a static web build is allowed to expose, relative to its root.
 * Private completion metadata and any unexpected top-level file are omitted.
 * Deploy imports this inventory, so serving and origin publication cannot
 * disagree about whether a build artifact is public. */
export function listPublicFiles(dist) {
  const root = resolve(dist);
  const out = [];
  for (const route of PUBLIC_FILES) if (staticFile(root, route)) out.push(route.slice(1));
  for (const asset of listAssets(root)) {
    if (!staticFile(root, `/${asset.name}`)) {
      throw new Error(`public web file is not safely readable: ${resolve(root, asset.name)}`);
    }
    out.push(asset.name);
  }
  for (const tree of ['rust', 'gpu', 'stages']) if (existsSync(resolve(root, tree))) for (const name of listStaticFiles(resolve(root, tree))) out.push(`${tree}/${name}`);
  return out.sort();
}

/** Digest cards for the complete public web build. The private completion
 * marker records these after every generated/optional artifact exists. */
export async function publicFileCards(dist) {
  return Promise.all(listPublicFiles(dist).map(async (name) => {
    const found = await readStaticFileAsync(dist, `/${name}`);
    if (!found) throw new Error(`public web file changed while inventorying: ${resolve(dist, name)}`);
    return { name, sha256: createHash('sha256').update(found.body).digest('hex'), bytes: found.body.length };
  }));
}

/** The manifest input identity a completed build records. Binding the whole
 * object means a name, icon, host card, or deploy-policy edit cannot reuse a
 * dist assembled from the prior app.json. */
export function appManifestDigest(app) {
  return createHash('sha256').update(JSON.stringify(app.manifest)).digest('hex');
}

function planAppId(bytes) {
  if (bytes.length < 36 || bytes.subarray(0, 4).toString() !== 'EXPL') return null;
  const idLen = bytes.readUInt32LE(32);
  if (idLen === 0 || 36 + idLen > bytes.length) return null;
  return bytes.subarray(36, 36 + idLen).toString('utf8');
}

/** Whether the complete build at `dist` belongs to `app`. The completion
 * marker, public envelope, and named plan must all agree with the requested
 * manifest identity before dev starts that app's resident compiler. */
export async function builtAppMatches(dist, app) {
  try {
    if (!app?.id || !app?.displayName) return false;
    const root = realpathSync(dist);
    const markerPath = resolve(root, '.exact-build.json');
    if (realpathSync(markerPath) !== markerPath || !statSync(markerPath).isFile()) return false;
    const marker = JSON.parse(readFileSync(markerPath, 'utf8'));
    const js = marker.target === 'js';
    const files = js ? buildFileCards(root) : await publicFileCards(root);
    const names = new Set(files.map((file) => file.name));
    if ((js ? JS_BUILD_FILES : REQUIRED_BUILD_FILES).some((name) => !names.has(name))) return false;
    if (!Array.isArray(marker.files) || marker.files.length !== files.length
      || files.some((file, i) => marker.files[i]?.name !== file.name
        || marker.files[i]?.sha256 !== file.sha256 || marker.files[i]?.bytes !== file.bytes
        || Object.keys(marker.files[i]).sort().join(',') !== 'bytes,name,sha256')) return false;
    const identity = marker.exactBuild === 1 && marker.app?.id === app.id
      && marker.app.name === app.displayName && marker.manifestSha256 === appManifestDigest(app);
    // A JS-target build (LLP 1071) has no envelope; its plan names the app.
    if (js) return identity && planAppId(readFileSync(resolve(root, 'app.plan'))) === app.id;
    const found = await readStaticFileAsync(dist, '/exact.json');
    const plan = await readStaticFileAsync(dist, '/app.plan');
    if (!found || !plan) return false;
    const envelope = JSON.parse(found.body.toString('utf8'));
    const digest = createHash('sha256').update(plan.body).digest('hex');
    return envelope.exact === 1 && envelope.app?.id === app.id
      && envelope.app.name === app.displayName && planAppId(plan.body) === app.id
      && envelope.plan?.url === './app.plan'
      && envelope.plan.sha256 === digest && envelope.plan.bytes === plan.body.length
      && identity;
  } catch { return false; }
}

// A JS-target build (LLP 1071, `host/web-js/build.mjs`) is whatever its
// bundler wrote — the entry, content-named chunks, pages at their locations —
// so the local tools list and serve it as a tree: every regular file whose
// path has no dot component, never through a symlink. Its completion marker
// (`host/web/build.mjs`) says `target: 'js'`.
const JS_BUILD_FILES = ['app.js', 'app.plan', 'index.html'];
export function listBuildFiles(dist) {
  const root = realpathSync(dist), out = [];
  const walk = (sub) => {
    for (const entry of readdirSync(resolve(root, sub), { withFileTypes: true })) {
      if (entry.name.startsWith('.')) continue;
      const name = sub ? `${sub}/${entry.name}` : entry.name;
      if (entry.isDirectory()) walk(name);
      else if (entry.isFile()) out.push(name);
    }
  };
  walk('');
  return out.sort();
}
export const buildFileCards = (dist) => listBuildFiles(dist).map((name) => {
  const bytes = readFileSync(resolve(dist, name));
  return { name, sha256: sha256(bytes), bytes: bytes.length };
});
/** Whether the build at `dist` is the JS target's. */
export function jsTargetBuild(dist) {
  try { return JSON.parse(readFileSync(resolve(dist, '.exact-build.json'), 'utf8')).target === 'js'; }
  catch { return false; }
}
/** A file of a JS-target build for a request path: the file, a directory's
 * index.html (a page rendered at build), else the shell for an app
 * location. The previous build answers during a rebuild's rename. */
export function buildTreeFile(dist, pathname) {
  let path;
  try { path = installRoute(decodeURIComponent(pathname)); } catch { return null; }
  // No dot path, but the auth callback page's (LLP 1069.006 D4: `/.exact/auth/…`)
  // and the install pages and their data (LLP 1030.003 D6a).
  if (!path.startsWith('/') || path.includes('\\') || path.includes('\0') || (!INSTALL_PUBLIC.includes(path) && path.replace(/^\/\.exact\/auth\//, '/').split('/').some((part) => part.startsWith('.')))) return null;
  let root;
  try { root = realpathSync(dist); } catch { try { root = realpathSync(`${dist}.previous`); } catch { return null; } }
  const prefix = root.endsWith(sep) ? root : root + sep;
  for (const route of [path, path.replace(/\/?$/, '/index.html'), ...(appDocumentPath(pathname) ? ['/index.html'] : [])]) {
    const file = resolve(root, '.' + route);
    try { if (file.startsWith(prefix) && realpathSync(file) === file && statSync(file).isFile()) return { path: file, route }; } catch { /* next */ }
  }
  return null;
}
export function serveBuildTree(dist, req, res) {
  // The raw target is refused as serveStatic refuses it (an encoded dot
  // segment); a parsed URL would resolve one into another route.
  const target = webRequestURL(req.url);
  const found = target && buildTreeFile(dist, target.pathname);
  if (!found) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
  sendStaticBody(req, res, readFileSync(found.path), { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
}

/** The assets by digest (LLP 1023 D4's owed slice; 1026 D11; 1030 D10): every file under assets/, deck/, and shaders/ in a build, named by its path beside the page, so a client fetches by name and verifies by digest and a dev push names what changed. Sorted by name. */
export function listAssets(dir, rust = false) {
  const out = [];
  const walk = (sub) => {
    const abs = resolve(dir, sub);
    if (!existsSync(abs)) return;
    for (const entry of readdirSync(abs, { withFileTypes: true })) {
      const rel = `${sub}/${entry.name}`;
      if (entry.isDirectory()) walk(rel);
      else if (entry.isFile()) {
        const bytes = readFileSync(resolve(dir, rel));
        out.push({ name: rel, url: `./${rel}`, sha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length });
      }
    }
  };
  for (const tree of ['assets', 'deck', 'shaders', ...(rust ? ['rust'] : [])]) walk(tree);
  return out.sort((a, b) => (a.name < b.name ? -1 : 1));
}

/** The one web envelope producer for static builds and the live dev overlay.
 * App identity comes from the plan header; the human name comes from the
 * manifest's cross-platform `app.name`, never the internal crate slug. */
export function webEnvelope(app, bytes, assets, live = {}) {
  const appId = planAppId(bytes);
  if (!appId) throw new Error('the web envelope needs a valid Exact plan with a nonempty app id');
  if (appId !== app.id) throw new Error(`the web plan is for ${appId}, not manifest app ${app.id}`);
  return {
    exact: 1,
    app: { id: appId, name: app.displayName },
    plan: { url: './app.plan', sha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length,
      formatVersion: bytes.readUInt32LE(4), kernelSchema: bytes.readBigUInt64LE(16).toString(16).padStart(16, '0') },
    assets,
    ...live,
  };
}

export function webContentType(route) {
  if (route === '/exact.json') return 'application/vnd.exact.envelope+json';
  if (route === '/manifest.json') return 'application/manifest+json';
  if (route === '/.well-known/apple-app-site-association') return 'application/json';
  // The auth callback page (LLP 1069.006 D4): one static page at exactly this path.
  if (route === AUTH_CALLBACK) return 'text/html';
  return {
    '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json',
    '.wasm': 'application/wasm', '.plan': 'application/vnd.exact.plan',
    '.mp4': 'video/mp4', '.webm': 'video/webm', '.vtt': 'text/vtt',
    '.wav': 'audio/wav', '.mp3': 'audio/mpeg', '.m4a': 'audio/mp4', '.aac': 'audio/aac', '.ogg': 'audio/ogg', '.oga': 'audio/ogg',
    '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg',
    '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.woff2': 'font/woff2', '.wgsl': 'text/wgsl',
    // A game's baked assets (game/README "Assets"): binary, and compressible.
    // Warming brotli-11 variants of every family (rivals: 43 MiB) takes CPU
    // time at server start, off the request path; a request before its
    // variant exists gets identity bytes.
    '.model': 'application/vnd.exact.model', '.tex': 'application/vnd.exact.texture',
  }[extname(route).toLowerCase()] ?? 'application/octet-stream';
}

/** Cache policy is enforced by the supported origin server, not metadata
 * dropped on the floor by a directory copy. Content-addressed files are
 * immutable; the update protocol's heads and pointers under `.exact/` are
 * never stored; every other name — the page, its install pages, a local
 * build's canonical files — is revalidated against its ETag each load. */
export function webCacheControl(found) {
  if (found.immutable || /^\/\.exact\/blobs\/[0-9a-f]{64}$/.test(found.route)
    || /^\/\.exact\/[^/.]+\/[^/.]+\/releases\/[^/.]+\.json$/.test(found.route)) return 'public, max-age=31536000, immutable';
  return found.route.startsWith(UPDATE_TREE) && !INSTALL_PUBLIC.includes(found.route) ? 'no-store' : 'no-cache';
}

// Compressed representations for the production server (`serve.mjs` itself;
// the dev server and drivers send identity bytes). Brotli at quality 11 takes
// seconds for an app's wasm, so a variant is made once per body digest, off
// the request path — the server warms its tree at startup — and a request that
// arrives first gets the identity bytes. A variant no smaller is not kept.
const COMPRESSIBLE = new Set(['application/wasm', 'text/javascript', 'text/css', 'text/html', 'application/json',
  'application/manifest+json', 'application/vnd.exact.envelope+json', 'image/svg+xml', 'text/wgsl',
  'application/vnd.exact.model', 'application/vnd.exact.texture']);
export function compressionCache(limit = 256 * 1024 * 1024) {
  const variants = new Map(), pending = new Map();
  let held = 0;
  const make = (digest, body, encoding) => {
    const key = `${digest}:${encoding}`;
    if (variants.has(key)) return Promise.resolve();
    if (!pending.has(key)) pending.set(key, new Promise(done => {
      const finish = (error, out) => {
        pending.delete(key);
        if (!error && out.length < body.length && held + out.length <= limit) { variants.set(key, out); held += out.length; }
        done();
      };
      if (encoding === 'br') brotliCompress(body, { params: { [zlib.BROTLI_PARAM_QUALITY]: 11, [zlib.BROTLI_PARAM_SIZE_HINT]: body.length } }, finish);
      else gzip(body, { level: 9 }, finish);
    }));
    return pending.get(key);
  };
  return {
    get: (digest, encoding) => variants.get(`${digest}:${encoding}`) ?? null,
    warm: (digest, body) => Promise.all(['br', 'gzip'].map(encoding => make(digest, body, encoding))),
  };
}
const bodyDigest = body => createHash('sha256').update(body).digest('base64url').slice(0, 32);
/** The encodings a request accepts, per RFC 9110 §12.5.3 (a q of 0 refuses). */
function acceptedEncodings(header = '') {
  const accepted = new Set();
  for (const part of String(header).split(',')) {
    const [name, ...params] = part.trim().toLowerCase().split(';');
    const q = params.map(p => /^\s*q=([0-9.]+)\s*$/.exec(p)?.[1]).find(Boolean);
    if (name && (q === undefined || Number(q) > 0)) accepted.add(name);
  }
  return accepted;
}
const matchesETag = (header, etag) => typeof header === 'string'
  && header.split(',').some(tag => { const t = tag.trim(); return t === '*' || t.replace(/^W\//, '') === etag; });

/** Warm every compressible file the server will answer for: a local build's
 * public files, or a published root's current release. */
export async function warmCompression(dist, compression) {
  let names;
  try { names = listPublicFiles(dist); } catch { names = []; }
  if (!names.includes('app.wasm')) {
    try { names = parseWebRoot(readFileSync(resolve(dist, webRootPath))).files.map(card => card.name); } catch { return 0; }
  }
  let warmed = 0;
  await Promise.all(names.filter(name => COMPRESSIBLE.has(webContentType('/' + name))).map(async name => {
    const found = await readStaticFileAsync(dist, '/' + name);
    if (found) { await compression.warm(bodyDigest(found.body), found.body); warmed++; }
  }));
  return warmed;
}

// @ref LLP 1038 D7 — preserve file precedence and negotiate the app
// document after resolving the shared extensionless-location fallback.
export async function readWebRequest(dist, pathname, accept = '') {
  if (!webRequestURL(pathname)) return { found: null, index: false };
  let route = pathname;
  let found = await readStaticFileAsync(dist, route);
  if (!found && appDocumentPath(route)) {
    route = '/index.html';
    found = await readStaticFileAsync(dist, route);
  }
  const index = route === '/' || route === '/index.html' || found?.route === '/index.html';
  if (index && accept.includes('application/vnd.exact.envelope+json')) {
    const release = found?.immutable && /^\/\.exact\/root\/web\/releases\/[0-9a-f]{64}\//.exec(decodeURIComponent(pathname));
    route = release ? release[0] + 'exact.json' : '/exact.json';
    found = await readStaticFileAsync(dist, route);
    // An envelope resolves against its response URL, not HTML's <base>.
    // A deep location must still name the root's payloads (1023 D2).
    if (found && !found.published && route === '/exact.json' && pathname !== '/' && pathname !== '/index.html') {
      const body = JSON.stringify(JSON.parse(found.body), (key, value) => {
        if (key !== 'url' || typeof value !== 'string') return value;
        const url = new URL(value, 'http://exact.invalid/');
        return url.origin === 'http://exact.invalid' ? url.pathname + url.search + url.hash : value;
      });
      found = { ...found, body: Buffer.from(body) };
    }
  } else if (index && found && !found.published) {
    // Native HTML discovery scans the alternate link without executing HTML.
    found = { ...found, body: Buffer.from(found.body.toString().replace('href="./exact.json"', 'href="/exact.json"')) };
  }
  return { found, index };
}

/** The production directory origin and diagnostic server share actual HTTP
 * handling, including no-store deletions and the native envelope rung. */
// One byte range is enough for browser media seeking. Unknown/multipart ranges
// fall back to the full representation; If-Range without a validator does too.
export function sendStaticBody(req, res, body, headers = {}) {
  const bytes = Buffer.isBuffer(body) ? body : Buffer.from(body);
  const size = bytes.length;
  const range = req.method !== 'HEAD' && !req.headers['if-range'] && /^bytes=(\d*)-(\d*)$/.exec(req.headers.range ?? '');
  const common = { ...headers, 'accept-ranges': 'bytes' };
  if (range && (range[1] || range[2])) {
    const start = range[1] ? Number(range[1]) : Math.max(0, size - Number(range[2]));
    const end = range[1] && range[2] ? Math.min(size - 1, Number(range[2])) : size - 1;
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start >= size || end < start) {
      res.writeHead(416, { ...common, 'content-range': `bytes */${size}`, 'content-length': 0 }); res.end(); return;
    }
    res.writeHead(206, { ...common, 'content-range': `bytes ${start}-${end}/${size}`, 'content-length': end - start + 1 });
    res.end(bytes.subarray(start, end + 1)); return;
  }
  res.writeHead(200, { ...common, 'content-length': size });
  res.end(req.method === 'HEAD' ? undefined : bytes);
}

export async function serveStatic(dist, req, res, listener, compression = null) {
  const target = webRequestURL(req.url);
  if (!target) { res.writeHead(404, { 'cache-control': 'no-store' }); res.end(); return; }
  if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405, { 'cache-control': 'no-store' }); res.end(); return; }
  const route = target.pathname;
  const { found, index } = await readWebRequest(dist, route, req.headers.accept);
  if (!found) { res.writeHead(404, { 'cache-control': 'no-store', ...(index ? { vary: 'Accept' } : {}) }); res.end(); return; }
  let body = !found.immutable && INSTALL_FILES.includes(found.route) ? found.body.toString().replace('<!-- exact-serving -->Static hosting<!-- /exact-serving -->', found.published ? 'Hosted release' : 'Development server') : found.body;
  if (!found.immutable && !found.published && INSTALL_FILES.includes(found.route)) body = installNetworkPage(body, listener ?? {host:req.socket.localAddress,port:req.socket.localPort});
  body = Buffer.isBuffer(body) ? body : Buffer.from(body);
  const type = webContentType(found.route), digest = bodyDigest(body);
  const compressed = compression && COMPRESSIBLE.has(type);
  let encoding = null;
  if (compressed) {
    const accepted = acceptedEncodings(req.headers['accept-encoding']);
    encoding = ['br', 'gzip'].find(name => accepted.has(name) && compression.get(digest, name)) ?? null;
    void compression.warm(digest, body);
  }
  const vary = [index && 'Accept', compressed && 'Accept-Encoding'].filter(Boolean).join(', ');
  const headers = { 'content-type': type, 'cache-control': webCacheControl(found), etag: `"${digest}${encoding ? `-${encoding}` : ''}"`,
    ...(vary ? { vary } : {}), ...(encoding ? { 'content-encoding': encoding } : {}),
    ...(found.route.startsWith(AUTH_CALLBACK) ? { 'referrer-policy': 'no-referrer' } : {}) };
  if (matchesETag(req.headers['if-none-match'], headers.etag)) {
    const { 'content-type': _, 'content-encoding': __, ...validators } = headers;
    res.writeHead(304, validators); res.end(); return;
  }
  sendStaticBody(req, res, encoding ? compression.get(digest, encoding) : body, headers);
}

async function main() {
  const argv = process.argv.slice(2);
  const at = argv.indexOf('--origin');
  if (at >= 0 && (!argv[at + 1] || argv[at + 1].startsWith('--'))) throw new Error('--origin needs a directory');
  const dist = at >= 0 ? resolve(argv.splice(at, 2)[1]) : webDist();
  // A JS-target build (LLP 1071) is served as its tree, uncompressed.
  const js = jsTargetBuild(dist);
  if (!js && !await readStaticFileAsync(dist, '/app.wasm')) {
    // A build that is there but unreadable says why: the reader is a Cargo-built
    // helper, and "build first" misled when cargo was not on PATH (LLP 1054 O5).
    let why = null;
    if (existsSync(resolve(dist, 'app.wasm'))) {
      try { filesystem({ op: 'get', root: resolve(dist), path: 'app.wasm' }); } catch (error) { why = error.message; }
    }
    console.error(why ? `${dist}/app.wasm is built but could not be read: ${why}` : `no build in ${dist}: run bun host/web/build.mjs first, or serve a published --origin <dir>`);
    return 2;
  }
  const loopback = argv.includes('--loopback') || process.env.EXACT_LOOPBACK === '1';
  const port = Number(argv.find((a) => !a.startsWith('--')) ?? 8765);
  const host = loopback ? '127.0.0.1' : '0.0.0.0';
  const compression = compressionCache();
  const warming = js ? Promise.resolve(0) : warmCompression(dist, compression);
  createServer((req, res) => js ? serveBuildTree(dist, req, res) : serveStatic(dist, req, res, {host,port}, compression)).listen(port, host, () => {
    const urls = [`http://127.0.0.1:${port}/`];
    if (!loopback) {
      const priv = (a) => /^(192\.168\.|10\.|172\.(1[6-9]|2\d|3[01])\.)/.test(a);
      urls.push(...Object.values(networkInterfaces()).flat().filter((a) => a && !a.internal && a.family === 'IPv4').map((a) => a.address).sort((a, b) => priv(b) - priv(a)).map((a) => `http://${a}:${port}/`));
    }
    // Name the app, so a directory another build replaced is seen at once (LLP 1054 O5).
    let served = '';
    try { const { app } = JSON.parse(readFileSync(resolve(dist, js ? '.exact-build.json' : 'exact.json'), 'utf8')); served = ` ${app.name} (${app.id})${js ? ', the JS target' : ''}`; } catch { /* an older build has no envelope */ }
    console.log(urls.join('\n') + `\n  (serving${served} from ${dist}; ctrl-c to stop)`);
    warming.then(n => console.log(`  (${n} files compressed: brotli and gzip)`));
  });
  return 0;
}

/** A development page's trace (LLP 1079 D5): ⌥⇧T posts its journal, frames and
 * `perf`; this adds what only the server knows — the build's identity, as
 * stress-metrics.mjs writes it, the plan's digest and its source map — and
 * writes it under the app's target/traces/ for `agent.mjs trace <file>`. */
export async function saveTrace(req, res, { app, dist, root }) {
  const reply = (status, body) => { res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' }); res.end(JSON.stringify(body)); };
  try {
    const parts = [];
    let bytes = 0;
    for await (const chunk of req) { if ((bytes += chunk.length) > 64 << 20) return reply(413, { error: 'a trace over 64 MiB' }); parts.push(chunk); }
    const trace = JSON.parse(Buffer.concat(parts).toString('utf8'));
    const git = (...args) => spawnSync('git', args, { cwd: root, encoding: 'utf8' }).stdout?.trim() ?? '';
    const planFile = resolve(dist, 'app.plan'), mapFile = `${planFile}.map.json`;
    const plan = existsSync(planFile) ? createHash('sha256').update(readFileSync(planFile)).digest('hex') : null;
    trace.identity = { ...trace.identity, app: app.name, trust: 'development', commit: git('rev-parse', 'HEAD'), working_tree: git('status', '--porcelain'), platform: platform(), arch: arch(), cpu: cpus()[0]?.model };
    trace.plan = trace.perf?.plan ?? plan;
    if (trace.perf && !trace.perf.plan) trace.perf.plan = plan;
    if (trace.plan === plan && existsSync(mapFile)) trace.map = JSON.parse(readFileSync(mapFile, 'utf8'));
    mkdirSync(resolve(app.target, 'traces'), { recursive: true });
    const path = resolve(app.target, 'traces', `trace-${new Date().toISOString().replace(/[:.]/g, '-')}.json`);
    writeFileSync(path, JSON.stringify(trace));
    console.log(`trace: ${path}`);
    reply(200, { path });
  } catch (error) { reply(400, { error: error.message }); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = await main();

/** A dev loop its launcher started stops when that process is gone, however it ended: `EXACT_LAUNCHER_PID` names
 * it (an app's `exact.mjs`, or the JS loop for the resident producers it starts). A SIGKILL forwards no signal, and
 * an orphaned server keeps its port (chat and onboarding F20). Without the variable a loop runs until stopped. */
export function watchLauncher(stop, pid = Number(process.env.EXACT_LAUNCHER_PID)) {
  if (!Number.isInteger(pid) || pid <= 1) return null;
  const timer = setInterval(() => { try { process.kill(pid, 0); } catch (e) { if (e.code === 'ESRCH') { clearInterval(timer); stop(); } } }, 1000);
  timer.unref?.();
  return timer;
}
