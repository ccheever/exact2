#!/usr/bin/env node
// Serve the current web build for a browser — and, through exact.json, for
// a native client (LLP 1023 D1). LAN by default (D8); --loopback (or
// EXACT_LOOPBACK=1) binds 127.0.0.1 only.
// Usage: node host/web/serve.mjs [port=8765] [--loopback]
import { createServer } from 'node:http';
import { createHash, randomBytes } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, statSync, watch, writeFileSync } from 'node:fs';
import { networkInterfaces } from 'node:os';
import { basename, dirname, extname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { filesystem } from '../../scripts/filesystem.mjs';
import { parseWebRoot, sha256, webReleasePath, webRootPath } from '../../scripts/origin.mjs';

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
 * configured static tree. A root event has an empty `relative` path; a null
 * filename conservatively reconciles every tree. */
export function staticWatchChanges(watchRoot, trees, filename) {
  if (filename === null || filename === undefined || filename === '') return trees.map(([root, targetRoot]) => ({ root: resolve(root), targetRoot, relative: '', name: targetRoot, tree: true }));
  const path = resolve(watchRoot, String(filename));
  const changes = [];
  for (const [tree, targetRoot] of trees) {
    const root = resolve(tree);
    const rel = relative(root, path);
    if (rel === '') changes.push({ root, targetRoot, relative: '', name: targetRoot, tree: true });
    else if (rel !== '..' && !rel.startsWith(`..${sep}`) && !isAbsolute(rel)) {
      const portable = rel.split(sep).join('/');
      changes.push({ root, targetRoot, relative: portable, name: `${targetRoot}/${portable}`, tree: false });
    }
  }
  return changes;
}

/** Watch the stable app directory rather than only roots present at startup.
 * This observes first creation, whole-root deletion, and recreation. */
export function watchStaticTrees(watchRoot, trees, onChange) {
  return watch(watchRoot, { recursive: true }, (event, filename) => {
    for (const change of staticWatchChanges(watchRoot, trees, filename)) onChange(change, event);
  });
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
function publishedFile(dist, pathname) {
  let route;
  try { route = decodeURIComponent(pathname === '/' ? '/index.html' : pathname); }
  catch { return null; }
  // Immutable generation URLs never consult the current pointer: readers
  // that already opened an older index keep all of that generation's files.
  const release = /^\/\.exact\/web\/([0-9a-f]{64})\/(.*)$/.exec(route);
  if (release) {
    const name = release[2] || 'index.html';
    if (!PUBLIC_FILES.has('/' + name) && !PUBLIC_TREES.some((tree) => ('/' + name).startsWith(tree))) return null;
    staticRelative(name);
    const rel = `${webReleasePath(release[1])}/${name}`;
    const body = filesystem({ op: 'get', root: resolve(dist), path: rel });
    return body === null ? null : { path: resolve(dist, rel), route: '/' + name, body: Buffer.from(body, 'base64'), immutable: true };
  }
  if (route.startsWith('/.exact/')) return undefined; // native heads/blobs and the web pointer
  const raw = filesystem({ op: 'get', root: resolve(dist), path: webRootPath });
  if (raw === null) return undefined; // a local build, not a deployed root
  const root = parseWebRoot(Buffer.from(raw, 'base64'));
  const card = root.files.find((file) => '/' + file.name === route);
  if (!card || !PUBLIC_FILES.has(route) && !PUBLIC_TREES.some((tree) => route.startsWith(tree))) return null;
  const rel = `${webReleasePath(root.id)}/${card.name}`;
  const value = filesystem({ op: 'get', root: resolve(dist), path: rel });
  if (value === null) return null;
  const body = Buffer.from(value, 'base64');
  if (body.length !== card.bytes || sha256(body) !== card.sha256) return null;
  return { path: resolve(dist, rel), route, body, immutable: false };
}

export function readStaticFile(dist, pathname) {
  try {
    const published = publishedFile(dist, pathname);
    if (published !== undefined) return published;
  } catch { return null; } // a malformed pointer or unsafe path never falls back to stale files
  for (let attempt = 0; attempt < 4; attempt++) {
    const found = staticFile(dist, pathname);
    if (!found) continue;
    try {
      const bytes = filesystem({ op: 'get', root: dirname(found.path), path: basename(found.path) });
      if (bytes !== null) return { ...found, body: Buffer.from(bytes, 'base64') };
    }
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

/** Cache policy is enforced by the supported origin server, not metadata
 * dropped on the floor by a directory copy. Canonical names always revalidate. */
export function webCacheControl(found) {
  return found.immutable || /^\/\.exact\/blobs\/[0-9a-f]{64}$/.test(found.route)
    || /^\/\.exact\/[^/.]+\/[^/.]+\/releases\/[^/.]+\.json$/.test(found.route)
    ? 'public, max-age=31536000, immutable' : 'no-store';
}

/** The production directory origin and diagnostic server share actual HTTP
 * handling, including no-store deletions and the native envelope rung. */
export function serveStatic(dist, req, res) {
  if (req.method !== 'GET' && req.method !== 'HEAD') { res.writeHead(405, { 'cache-control': 'no-store' }); res.end(); return; }
  let route = new URL(req.url, 'http://exact.invalid').pathname;
  const index = route === '/' || route.endsWith('/index.html');
  if (index && (req.headers.accept ?? '').includes('application/vnd.exact.envelope+json')) route = route === '/' ? '/exact.json' : route.slice(0, -10) + 'exact.json';
  const found = readStaticFile(dist, route);
  if (!found) { res.writeHead(404, { 'cache-control': 'no-store', ...(index ? { vary: 'Accept' } : {}) }); res.end(); return; }
  res.writeHead(200, { 'content-type': webContentType(found.route), 'cache-control': webCacheControl(found), ...(index ? { vary: 'Accept' } : {}) });
  res.end(req.method === 'HEAD' ? undefined : found.body);
}

function main() {
  const argv = process.argv.slice(2);
  const at = argv.indexOf('--origin');
  if (at >= 0 && (!argv[at + 1] || argv[at + 1].startsWith('--'))) throw new Error('--origin needs a directory');
  const dist = at >= 0 ? resolve(argv.splice(at, 2)[1]) : resolve(process.env.EXACT_WEB_DIST ?? new URL('./dist', import.meta.url).pathname);
  if (!readStaticFile(dist, '/app.wasm')) { console.error('run node host/web/build.mjs first, or serve a published --origin <dir>'); return 2; }
  const loopback = argv.includes('--loopback') || process.env.EXACT_LOOPBACK === '1';
  const port = Number(argv.find((a) => !a.startsWith('--')) ?? 8765);
  const host = loopback ? '127.0.0.1' : '0.0.0.0';
  createServer((req, res) => serveStatic(dist, req, res)).listen(port, host, () => {
    const urls = [`http://127.0.0.1:${port}/`];
    if (!loopback) {
      const priv = (a) => /^(192\.168\.|10\.|172\.(1[6-9]|2\d|3[01])\.)/.test(a);
      urls.push(...Object.values(networkInterfaces()).flat().filter((a) => a && !a.internal && a.family === 'IPv4').map((a) => a.address).sort((a, b) => priv(b) - priv(a)).map((a) => `http://${a}:${port}/`));
    }
    console.log(urls.join('\n') + `\n  (serving ${dist}; ctrl-c to stop)`);
  });
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = main();
