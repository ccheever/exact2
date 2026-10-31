// Builds kept by generation, so a page is served from the build it booted (QUEUE: "a request across a dist swap
// can mix two builds", "a drive's page can lose a lazy chunk"; theme2's ruling 2026-10-24, dtool2-buildswap-plan.md).
// `dist` stays the current build, a real directory, as every reader expects; `replaceBuild` (serve.mjs) moves the
// tree it replaces to `<dist>.gen/<id>/`, a real directory too (the filesystem helper refuses links), where its pages
// still find it. A build's generation is its receipt's file cards' digest, so a byte-identical rebuild is the same one.
// A page's document names its generation in its base (`/.exact/gen/<id>/`), so its scripts, chunks, plan and assets
// come from that build; a drive pins one at open under a lease. Kept: the newest 3, any leased by a live process, any
// put aside under 15 minutes ago, within 2 GiB (the oldest unleased first; leased ones are never reaped).
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { BESIDE } from './beside.mjs';
import { lockHeld } from './build-lock.mjs';
import { buildTreeFileAsync } from './serve.mjs';

/** A generation's form: 32 lowercase hex digits, nothing else. */
export const GENERATION = /^[0-9a-f]{32}$/;
export const KEEP = { newest: 3, ageMs: 15 * 60_000, bytes: 2 * 2 ** 30 };
const PREFIX = '/.exact/gen/';
const realDirectory = (path) => { try { return lstatSync(path).isDirectory(); } catch { return false; } };
const realFile = (path) => { try { return lstatSync(path).isFile(); } catch { return false; } };
const generationsOf = (dist) => `${dist}.gen`, leasesOf = (dist) => `${dist}.leases`, reapedOf = (dist) => `${dist}.gen.reaped`;

/** What beside `dist` a build there would replace, remove or overwrite though it is not one of its own (an author's
 * `--out` directory may sit beside anything): each name of beside.mjs's `BESIDE` that is there and is not what a build
 * writes there, with what it should be, else null. A stage is a fresh directory each build makes and removes, so
 * another is never touched. */
export function foreignBeside(dist) {
  const receipt = (dir) => realDirectory(dir) && realFile(resolve(dir, '.exact-build.json'));
  const entries = (dir, each) => realDirectory(dir) && readdirSync(dir).every((name) => each(name, resolve(dir, name)));
  const lines = (file, form) => realFile(file) && readFileSync(file, 'utf8').split('\n').every((line) => line === '' || form.test(line));
  const owned = {
    previous: [receipt, 'a web build (its .exact-build.json)'],
    gen: [(dir) => entries(dir, (name, at) => GENERATION.test(name) && receipt(at)), 'builds kept by generation'],
    'gen.reaped': [(file) => lines(file, GENERATION), 'the generations reaped'],
    leases: [(dir) => entries(dir, (name, at) => GENERATION.test(name) && entries(at, (lease, file) => /^\d+-\d+$/.test(lease) && realFile(file))), 'leases on kept builds'],
    lock: [(file) => lines(file, /^\d+$/), 'a build\'s lock'],
  };
  for (const name of BESIDE) {
    const path = `${dist}.${name}`, [ours, what] = owned[name];
    try { lstatSync(path); } catch { continue; }
    if (!ours(path)) return `${path} is there and is not ${what}, which a build in ${dist} would replace or remove`;
  }
  return null;
}
/** The generations reaped at `dist` (the last 256): a page asking for one is told it is gone (410), unlike an id that
 * names nothing here (404). */
const reapedIds = (dist) => { try { return new Set(readFileSync(reapedOf(dist), 'utf8').split('\n').filter((l) => GENERATION.test(l))); } catch { return new Set(); } };

const known = new Map(); // receipt path -> { key, id }
/** The generation of the build at `dir`: its receipt's file cards' digest (cached by the receipt's identity), or null
 * for a build with no receipt (served as before, unpinned). */
export function generationOf(dir) {
  const receipt = resolve(dir, '.exact-build.json');
  let st;
  try { st = statSync(receipt); } catch { return null; }
  const key = `${st.dev}:${st.ino}:${st.size}:${st.mtimeMs}`, hit = known.get(receipt);
  if (hit?.key === key) return hit.id;
  let id = null;
  try { const r = JSON.parse(readFileSync(receipt, 'utf8')); id = createHash('sha256').update(JSON.stringify(r.files ?? r)).digest('hex').slice(0, 32); } catch { /* none */ }
  known.set(receipt, { key, id });
  return id;
}

/** The build `old` replaced at `dist` kept as `<dist>.gen/<id>/` (replaceBuild, under its lock): put aside now (its
 * time is its age), or removed when it has no generation or one is kept already (the same bytes). */
export function keepGeneration(old, dist) {
  const id = generationOf(old), to = id && resolve(generationsOf(dist), id);
  if (!to || existsSync(to) || id === generationOf(dist)) { rmSync(old, { recursive: true, force: true }); return null; }
  mkdirSync(generationsOf(dist), { recursive: true });
  renameSync(old, to);
  const now = new Date(); utimesSync(to, now, now);
  return id;
}

/** A process's start, as `ps` says it (a pid is reused; with its start it names one process), or '' when `ps` can't
 * say (none here, or it failed). */
const startOf = (pid) => String(spawnSync('ps', ['-o', 'lstart=', '-p', String(pid)], { encoding: 'utf8', env: { ...process.env, LC_ALL: 'C' } }).stdout ?? '').trim();
/** Whether lease file `name` (`<pid>-<n>`, holding the process's start) still names a live process: one that exists
 * (EPERM: it does, another user's) and whose start, when `ps` can say it, is the one written. What can't be told is
 * kept, as the repo's other claims keep it (app.mjs `claimLive`). */
function live(dir, name) {
  const pid = Number.parseInt(name, 10);
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try { process.kill(pid, 0); } catch (e) { if (e.code !== 'EPERM') return false; }
  let written;
  try { written = readFileSync(resolve(dir, name), 'utf8'); } catch { return false; }
  const now = startOf(pid);
  return !written || !now || written === now;
}
/** The generation a drive boots on `dist`: asked again, for up to a second, between a swap's two renames (no dist
 * then, or a build with no receipt yet); null for a build that has none (served unpinned, as before). */
export async function bootedGeneration(dist) {
  for (const end = Date.now() + 1000; ; await sleep(10)) {
    const id = generationOf(dist);
    if (id || (realDirectory(dist) && !swapping(dist)) || Date.now() > end) return id;
  }
}
let leases = 0;
/** A lease on generation `id` of `dist` (a drive's, for as long as it runs; one each, so two drives of one process
 * hold two): the release. */
export function leaseGeneration(dist, id) {
  if (!GENERATION.test(id ?? '')) return () => {};
  const dir = resolve(leasesOf(dist), id), file = resolve(dir, `${process.pid}-${++leases}`), start = startOf(process.pid);
  // Made and written together, again if another process's cleanup took the empty directory between the two.
  for (let attempt = 0; ; attempt++) {
    try { mkdirSync(dir, { recursive: true }); writeFileSync(file, start); break; }
    catch (e) { if (e.code !== 'ENOENT' || attempt >= 5) throw e; }
  }
  return () => { rmSync(file, { force: true }); try { if (!readdirSync(dir).length) rmSync(dir, { recursive: true, force: true }); } catch { /* gone */ } };
}
/** The generations of `dist` some live process leases (dead ones' leases removed). */
export function leasedGenerations(dist) {
  const leased = new Set();
  let ids = [];
  try { ids = readdirSync(leasesOf(dist)); } catch { return leased; }
  for (const id of ids) {
    const dir = resolve(leasesOf(dist), id);
    let names = [];
    try { names = readdirSync(dir); } catch { continue; }
    for (const name of names) if (live(dir, name)) leased.add(id); else rmSync(resolve(dir, name), { force: true });
    try { if (!readdirSync(dir).length) rmSync(dir, { recursive: true, force: true }); } catch { /* gone */ }
  }
  return leased;
}

const bytesIn = (dir) => {
  let total = 0;
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = resolve(dir, e.name);
    if (e.isDirectory()) total += bytesIn(p); else if (e.isFile()) total += statSync(p).size;
  }
  return total;
};
/** Kept generations of `dist` removed as the ruling keeps them (replaceBuild, under its lock): what was removed. A
 * cap leased generations alone exceed is said once and kept. */
let overSaid = false;
export function reapGenerations(dist, keep = KEEP, now = Date.now()) {
  let names = [];
  try { names = readdirSync(generationsOf(dist)); } catch { return []; }
  const leased = leasedGenerations(dist);
  const kept = names.filter((n) => GENERATION.test(n) && realDirectory(resolve(generationsOf(dist), n)))
    .map((id) => ({ id, at: statSync(resolve(generationsOf(dist), id)).mtimeMs })).sort((a, b) => b.at - a.at);
  const reaped = [], gone = (g) => { rmSync(resolve(generationsOf(dist), g.id), { recursive: true, force: true }); reaped.push(g.id); };
  const told = () => { if (reaped.length) writeFileSync(reapedOf(dist), [...[...reapedIds(dist)].filter((id) => !reaped.includes(id)), ...reaped].slice(-256).join('\n') + '\n'); };
  const left = [];
  for (const [i, g] of kept.entries()) {
    if (leased.has(g.id) || i < keep.newest || now - g.at < keep.ageMs) left.push(g); else gone(g);
  }
  // The cap: the oldest unleased first, never a leased one.
  let total = left.reduce((sum, g) => sum + (g.bytes = bytesIn(resolve(generationsOf(dist), g.id))), 0);
  for (const g of [...left].reverse()) {
    if (total <= keep.bytes) break;
    if (leased.has(g.id)) continue;
    gone(g); total -= g.bytes;
  }
  if (total > keep.bytes && !overSaid) { overSaid = true; console.log(`${generationsOf(dist)}: leased builds alone hold ${Math.round(total / 2 ** 20)} MiB, past the ${Math.round(keep.bytes / 2 ** 20)} MiB cap; kept while their drives run`); }
  // Anything else there (a half-moved tree, a stray name) is not a generation.
  for (const n of names) if (!GENERATION.test(n)) rmSync(resolve(generationsOf(dist), n), { recursive: true, force: true });
  told();
  return reaped;
}

/** A request path under a generation (`<base>.exact/gen/<id>/<path>`): `{ id, path }`, 'bad' for a malformed one (an
 * id of another form, an encoded slash or dot segment in it), or null for any other path. */
export function generationPath(pathname, base = '/') {
  const head = base.replace(/\/?$/, '/') + PREFIX.slice(1);
  if (!pathname.startsWith(head)) return null;
  const rest = pathname.slice(head.length), slash = rest.indexOf('/');
  const id = slash < 0 ? rest : rest.slice(0, slash);
  if (!GENERATION.test(id) || slash < 0) return 'bad';
  const path = rest.slice(slash);
  if (/%2f|%5c|%2e/i.test(path) || path.split('/').some((part) => part === '..' || part === '.')) return 'bad';
  return { id, path };
}

const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
/** Whether a swap holds `dist.lock` now (build-lock.mjs: a lock a killed build left counts for nothing). */
const swapping = (dist) => lockHeld(`${dist}.lock`);
/** A file of generation `id` of `dist` for a request path, read whole from that build: the current one when it is
 * still `id` after the read, else the kept tree (asked again, for up to a second, while a swap holds `dist.lock`:
 * its rename lands under it); 'gone' for a generation reaped, 'busy' while a swap still holds the lock past that
 * second, null for no such file or an id that names nothing. With `miss`, no such file is the build's page for a
 * miss where it has one (`buildTreeFile`). */
export async function generationFile(dist, id, pathname, accept = '', { miss = false } = {}) {
  for (const end = Date.now() + 1000; ;) {
    if (generationOf(dist) === id) {
      const found = await buildTreeFileAsync(dist, pathname, accept, { miss });
      if (found?.status === 503) return 'busy'; // (its route table unreadable: serve.mjs `buildTreeFile`)
      let body = null;
      try { body = found && readFileSync(found.path); } catch { /* moved */ }
      if (generationOf(dist) === id) return found && body ? { ...found, body, id } : null;
      continue;
    }
    const dir = resolve(generationsOf(dist), id);
    if (realDirectory(dir)) {
      const found = await buildTreeFileAsync(dir, pathname, accept, { miss });
      if (found?.status === 503) return 'busy';
      let body = null;
      try { body = found && readFileSync(found.path); } catch { /* reaped meanwhile */ }
      if (body) return { ...found, body, id };
      // No such file there, unless the tree is being reaped under the swap's lock (its files go before it does).
      if (!found && realDirectory(dir) && !swapping(dist)) return null;
    }
    if (!swapping(dist)) return reapedIds(dist).has(id) ? 'gone' : null;
    // A swap still under way past the second (a slow reap): not known yet, said as such (503), never as absent.
    if (Date.now() > end) return 'busy';
    await sleep(10);
  }
}

/** A document's base set to its generation's (`<base>.exact/gen/<id>/`): its relative scripts, chunks, plan and
 * assets resolve there. A document with no `<base href="/">` (or a base of its own) is left as it is. */
export function withGenerationBase(html, id, base = '/') {
  const text = typeof html === 'string' ? html : html.toString();
  const from = `<base href="${base}">`;
  return text.includes(from) ? text.replace(from, `<base href="${base}${PREFIX.slice(1)}${id}/">`) : text;
}

/** What a server answers for a request path on `dist` (`base`, the path it serves the build under): a generation's
 * file (a path under one, or any path for a drive's `pinned` one), else the current build's, read whole from one
 * build; a document's base set to its generation. `{ status, route?, body?, id?, render? }`: 200, 400 for a malformed
 * generation path, 404 (with the build's page for a miss where it has one), or 410 for a generation kept no more;
 * `render`, for the shell, is the render policy of the route it is served for. */
export async function servedFile(dist, pathname, accept = '', { base = '/', pinned = null } = {}) {
  const under = generationPath(pathname, base);
  if (under === 'bad') return { status: 400 };
  // The path in the app: a request under a generation names its page without that prefix.
  const location = under ? under.path : pathname.slice(base.length - 1) || '/';
  let found;
  if (under || pinned) {
    const id = under ? under.id : pinned, path = under ? under.path : pathname.slice(base.length - 1) || '/';
    found = await generationFile(dist, id, path, accept, { miss: true });
    if (found === 'gone') return { status: 410, id };
    if (found === 'busy') return { status: 503 };
  } else {
    // Read whole from one build: its generation the same before and after. Between a swap's two renames (no one-step
    // exchange) there is no dist, and `.previous` answers with no generation: asked again until the swap is done.
    for (const end = Date.now() + 1000; found === undefined; ) {
      const id = generationOf(dist), at = await buildTreeFileAsync(dist, pathname.slice(base.length - 1) || '/', accept, { miss: true });
      if (at?.status === 503) return { status: 503 };
      let body = null;
      try { body = at && readFileSync(at.path); } catch { /* moved */ }
      // (No dist and no swap under way: no build yet, or one a killed swap left as `.previous`, served as before.)
      const settled = realDirectory(dist) || !swapping(dist);
      if (generationOf(dist) === id && settled && (!at || body)) found = at ? { ...at, body, id } : null;
      else if (Date.now() > end) return { status: 503 };
      else await sleep(10);
    }
  }
  if (!found) return { status: 404 };
  const body = found.route.endsWith('.html') && found.id ? withGenerationBase(found.body, found.id, base) : found.body;
  return { status: found.status ?? 200, route: found.route, body, id: found.id, render: found.render, location };
}
