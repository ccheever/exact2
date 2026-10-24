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
import { buildTreeFileAsync } from './serve.mjs';

/** A generation's form: 32 lowercase hex digits, nothing else. */
export const GENERATION = /^[0-9a-f]{32}$/;
export const KEEP = { newest: 3, ageMs: 15 * 60_000, bytes: 2 * 2 ** 30 };
const PREFIX = '/.exact/gen/';
const realDirectory = (path) => { try { return lstatSync(path).isDirectory(); } catch { return false; } };
const generationsOf = (dist) => `${dist}.gen`, leasesOf = (dist) => `${dist}.leases`, reapedOf = (dist) => `${dist}.gen.reaped`;
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

/** A process's start, as `ps` says it (a pid is reused; with its start it names one process). */
const startOf = (pid) => spawnSync('ps', ['-o', 'lstart=', '-p', String(pid)], { encoding: 'utf8', env: { ...process.env, LC_ALL: 'C' } }).stdout.trim();
/** Whether lease file `name` (`<pid>`) with `start` still names a live process. */
function live(dir, name) {
  const pid = Number(name);
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try { process.kill(pid, 0); } catch (e) { if (e.code !== 'EPERM') return false; }
  try { return readFileSync(resolve(dir, name), 'utf8') === startOf(pid); } catch { return false; }
}
/** This process's lease on generation `id` of `dist` (a drive's, for as long as it runs): the release. */
export function leaseGeneration(dist, id) {
  if (!GENERATION.test(id ?? '')) return () => {};
  const dir = resolve(leasesOf(dist), id), file = resolve(dir, String(process.pid));
  mkdirSync(dir, { recursive: true });
  writeFileSync(file, startOf(process.pid));
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
/** A file of generation `id` of `dist` for a request path, read whole from that build: the current one when it is
 * still `id` after the read, else the kept tree (asked again, for up to a second, while a swap holds `dist.lock`:
 * its rename lands under it); 'gone' for a generation reaped, null for no such file or an id that names nothing. */
export async function generationFile(dist, id, pathname, accept = '') {
  for (const end = Date.now() + 1000; ;) {
    if (generationOf(dist) === id) {
      const found = await buildTreeFileAsync(dist, pathname, accept);
      let body = null;
      try { body = found && readFileSync(found.path); } catch { /* moved */ }
      if (generationOf(dist) === id) return found && body ? { ...found, body, id } : null;
      continue;
    }
    const dir = resolve(generationsOf(dist), id);
    if (realDirectory(dir)) {
      const found = await buildTreeFileAsync(dir, pathname, accept);
      if (!found) return realDirectory(dir) ? null : 'gone';
      try { return { ...found, body: readFileSync(found.path), id }; } catch { /* reaped meanwhile */ }
    }
    if (!existsSync(`${dist}.lock`) || Date.now() > end) return reapedIds(dist).has(id) ? 'gone' : null;
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
 * build; a document's base set to its generation. `{ status, route?, body?, id? }`: 200, 400 for a malformed
 * generation path, 404, or 410 for a generation kept no more. */
export async function servedFile(dist, pathname, accept = '', { base = '/', pinned = null } = {}) {
  const under = generationPath(pathname, base);
  if (under === 'bad') return { status: 400 };
  let found;
  if (under || pinned) {
    const id = under ? under.id : pinned, path = under ? under.path : pathname.slice(base.length - 1) || '/';
    found = await generationFile(dist, id, path, accept);
    if (found === 'gone') return { status: 410, id };
  } else {
    for (let attempt = 0; attempt < 4 && found === undefined; attempt++) {
      const id = generationOf(dist), at = await buildTreeFileAsync(dist, pathname.slice(base.length - 1) || '/', accept);
      let body = null;
      try { body = at && readFileSync(at.path); } catch { continue; }
      if (generationOf(dist) === id) found = at ? { ...at, body, id } : null;
    }
  }
  if (!found) return { status: 404 };
  const body = found.route.endsWith('.html') && found.id ? withGenerationBase(found.body, found.id, base) : found.body;
  return { status: 200, route: found.route, body, id: found.id };
}
