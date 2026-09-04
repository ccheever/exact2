#!/usr/bin/env node
// Serve the current web build for a browser — and, through exact.json, for
// a native client (LLP 1023 D1). LAN by default (D8); --loopback (or
// EXACT_LOOPBACK=1) binds 127.0.0.1 only.
// Usage: node host/web/serve.mjs [port=8765] [--loopback]
import { createServer } from 'node:http';
import { createHash, randomBytes } from 'node:crypto';
import { closeSync, constants, existsSync, fstatSync, lstatSync, mkdirSync, openSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { networkInterfaces } from 'node:os';
import { dirname, extname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const PUBLIC_FILES = new Set([
  '/app.plan', '/app.wasm', '/exact.json', '/glue.js', '/gpu-glue.js',
  '/gpu.js', '/gpu_bg.wasm', '/index.html', '/manifest.json',
  // The one dot path a static origin serves: the deep-link association
  // file bake generates (LLP 1030 D1), read by Apple's CDN over HTTPS.
  '/.well-known/apple-app-site-association',
]);
const PUBLIC_TREES = ['/assets/', '/deck/', '/shaders/'];
const REQUIRED_BUILD_FILES = ['app.plan', 'app.wasm', 'exact.json', 'glue.js', 'index.html', 'manifest.json'];
// An origin's update streams (LLP 1030.000 D7; `scripts/origin.mjs`):
// `.exact/blobs/<sha256>` and `.exact/<channel>/<compatibility id>/…` — the
// one dot path a client fetches. Inside it every other dot name (the
// stream's `.lock`) stays private.
const UPDATE_TREE = '/.exact/';

function staticRelative(name) {
  if (typeof name !== 'string' || !name || name.startsWith('/') || name.includes('\\') || name.includes('\0')) throw new Error(`not a relative static-file path: ${JSON.stringify(name)}`);
  if (name.split('/').some((part) => !part || part === '.' || part === '..')) throw new Error(`not a relative static-file path: ${JSON.stringify(name)}`);
  return name;
}

/** Every regular file under a static source tree, sorted and refused when
 * the root or any entry is a symlink or another special filesystem object. */
export function listStaticFiles(source) {
  const root = resolve(source);
  const rootInfo = lstatSync(root);
  if (rootInfo.isSymbolicLink() || !rootInfo.isDirectory()) throw new Error(`static app tree must be a real directory: ${root}`);
  const out = [];
  const walk = (dir, prefix) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const name = prefix ? `${prefix}/${entry.name}` : entry.name;
      const path = resolve(root, name);
      const info = lstatSync(path);
      if (info.isSymbolicLink()) throw new Error(`static app files cannot be symlinks: ${path}`);
      if (info.isDirectory()) walk(path, name);
      else if (info.isFile()) out.push(name);
      else throw new Error(`static app files must be regular files or directories: ${path}`);
    }
  };
  walk(root, '');
  return out.sort();
}

/** Read one candidate through a no-follow fd and prove it is the same
 * regular inode the source tree walk inspected. */
export function readStaticCandidate(source, name) {
  const root = resolve(source);
  const rootInfo = lstatSync(root);
  if (rootInfo.isSymbolicLink() || !rootInfo.isDirectory()) throw new Error(`static app tree must be a real directory: ${root}`);
  let path = root;
  let expected = rootInfo;
  const parts = staticRelative(name).split('/');
  for (let i = 0; i < parts.length; i++) {
    path = resolve(path, parts[i]);
    expected = lstatSync(path);
    if (expected.isSymbolicLink()) throw new Error(`static app files cannot be symlinks: ${path}`);
    if (i + 1 < parts.length && !expected.isDirectory()) throw new Error(`static app path is not a directory: ${path}`);
  }
  if (!expected.isFile()) throw new Error(`static app file is not regular: ${path}`);
  const fd = openSync(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
  try {
    const opened = fstatSync(fd);
    if (!opened.isFile() || opened.dev !== expected.dev || opened.ino !== expected.ino) throw new Error(`static app file changed while opening: ${path}`);
    return readFileSync(fd);
  } finally { closeSync(fd); }
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

/** Copy one complete static source tree under the same no-symlink policy
 * used by the live candidate path. Intended for private build stages. */
export function copyStaticTree(source, target) {
  const names = listStaticFiles(source);
  mkdirSync(target, { recursive: true });
  for (const name of names) installStaticCandidate(source, name, resolve(target, name));
}

function optionalInfo(path) {
  try { return lstatSync(path); }
  catch (error) { if (error.code === 'ENOENT') return null; throw error; }
}

/** Copy a tree when it is genuinely absent, while still sending a dangling
 * root link through the static-tree refusal. `existsSync` cannot make that
 * distinction and must not guard an app-visible copy. */
export function copyStaticTreeIfPresent(source, target) {
  if (!optionalInfo(resolve(source))) return false;
  copyStaticTree(source, target);
  return true;
}

/** Mirror one complete source tree into a live dist at startup. A missing
 * source removes the formerly served tree. A present tree is first copied
 * into a private sibling and only then replaces the target, so a refused
 * link or read race preserves the last-good tree. */
export function syncStaticTree(source, target) {
  const sourcePath = resolve(source);
  const targetPath = resolve(target);
  if (!optionalInfo(sourcePath)) {
    rmSync(targetPath, { recursive: true, force: true });
    return false;
  }
  const parent = dirname(targetPath);
  mkdirSync(parent, { recursive: true });
  const candidate = resolve(parent, `.candidate-tree-${process.pid}-${randomBytes(4).toString('hex')}`);
  const previous = resolve(parent, `.previous-tree-${process.pid}-${randomBytes(4).toString('hex')}`);
  try {
    copyStaticTree(sourcePath, candidate);
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
  try { route = decodeURIComponent(pathname === '/' ? '/index.html' : pathname); }
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
    const path = resolve(root, '.' + route);
    if (!path.startsWith(root + '/')) return null;
    const real = realpathSync(path);
    // Reject both a symlink file and a file reached through a symlink dir.
    if (real !== path || !real.startsWith(root + '/') || !statSync(real).isFile()) return null;
    return { path: real, route };
  } catch { return null; }
}

/** Resolve and read together, retrying when a build rename moved the path
 * between those operations. The opened response is wholly old or wholly
 * new; a request never observes the rename window as a synthetic 404. */
export function readStaticFile(dist, pathname) {
  for (let attempt = 0; attempt < 4; attempt++) {
    const found = staticFile(dist, pathname);
    if (!found) continue;
    try { return { ...found, body: readFileSync(found.path) }; }
    catch { /* retry against dist or dist.previous */ }
  }
  return null;
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
  return out.sort();
}

/** Digest cards for the complete public web build. The private completion
 * marker records these after every generated/optional artifact exists. */
export function publicFileCards(dist) {
  return listPublicFiles(dist).map((name) => {
    const found = readStaticFile(dist, `/${name}`);
    if (!found) throw new Error(`public web file changed while inventorying: ${resolve(dist, name)}`);
    return { name, sha256: createHash('sha256').update(found.body).digest('hex'), bytes: found.body.length };
  });
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
export function builtAppMatches(dist, app) {
  try {
    if (!app?.id || !app?.displayName) return false;
    const root = realpathSync(dist);
    const markerPath = resolve(root, '.exact-build.json');
    if (realpathSync(markerPath) !== markerPath || !statSync(markerPath).isFile()) return false;
    const marker = JSON.parse(readFileSync(markerPath, 'utf8'));
    const files = publicFileCards(root);
    const names = new Set(files.map((file) => file.name));
    if (REQUIRED_BUILD_FILES.some((name) => !names.has(name))) return false;
    if (!Array.isArray(marker.files) || marker.files.length !== files.length
      || files.some((file, i) => marker.files[i]?.name !== file.name
        || marker.files[i]?.sha256 !== file.sha256 || marker.files[i]?.bytes !== file.bytes
        || Object.keys(marker.files[i]).sort().join(',') !== 'bytes,name,sha256')) return false;
    const found = readStaticFile(dist, '/exact.json');
    const plan = readStaticFile(dist, '/app.plan');
    if (!found || !plan) return false;
    const envelope = JSON.parse(found.body.toString('utf8'));
    const digest = createHash('sha256').update(plan.body).digest('hex');
    return envelope.exact === 1 && envelope.app?.id === app.id
      && envelope.app.name === app.displayName && planAppId(plan.body) === app.id
      && envelope.plan?.url === './app.plan'
      && envelope.plan.sha256 === digest && envelope.plan.bytes === plan.body.length
      && marker.exactBuild === 1 && marker.app?.id === app.id
      && marker.app.name === app.displayName
      && marker.manifestSha256 === appManifestDigest(app);
  } catch { return false; }
}

/** The assets by digest (LLP 1023 D4's owed slice; 1026 D11; 1030 D10): every file under assets/, deck/, and shaders/ in a build, named by its path beside the page, so a client fetches by name and verifies by digest and a dev push names what changed. Sorted by name. */
export function listAssets(dir) {
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
  for (const tree of ['assets', 'deck', 'shaders']) walk(tree);
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
  return {
    '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json',
    '.wasm': 'application/wasm', '.plan': 'application/vnd.exact.plan',
    '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg',
    '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.woff2': 'font/woff2', '.wgsl': 'text/wgsl',
  }[extname(route).toLowerCase()] ?? 'application/octet-stream';
}

function main() {
  const dist = resolve(new URL('./dist', import.meta.url).pathname);
  if (!staticFile(dist, '/app.wasm')) { console.error('run node host/web/build.mjs first'); return 2; }
  const argv = process.argv.slice(2);
  const loopback = argv.includes('--loopback') || process.env.EXACT_LOOPBACK === '1';
  const port = Number(argv.find((a) => !a.startsWith('--')) ?? 8765);
  const host = loopback ? '127.0.0.1' : '0.0.0.0';
  createServer((req, res) => {
    if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405); res.end(); return; }
    const found = readStaticFile(dist, new URL(req.url, 'http://exact.invalid').pathname);
    if (!found) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
    res.end(req.method === 'HEAD' ? undefined : found.body);
  }).listen(port, host, () => {
    const urls = [`http://127.0.0.1:${port}/`];
    if (!loopback) {
      const priv = (a) => /^(192\.168\.|10\.|172\.(1[6-9]|2\d|3[01])\.)/.test(a);
      urls.push(...Object.values(networkInterfaces()).flat().filter((a) => a && !a.internal && a.family === 'IPv4').map((a) => a.address).sort((a, b) => priv(b) - priv(a)).map((a) => `http://${a}:${port}/`));
    }
    console.log(urls.join('\n') + '\n  (serving host/web/dist; ctrl-c to stop)');
  });
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = main();
}
