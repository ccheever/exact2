// The origin adapter `exact deploy` publishes through (LLP 1030.000 D3 item
// 5, D7; LLP 1030 D3a): the static host that serves the web app at its root,
// content-addressed blobs under `.exact/blobs/<sha256>`, and one directory
// per stream — `.exact/<channel>/<compatibilityId>/` — holding the signed
// head `exact.json` and `releases/<release>.json` (the immutable prepared
// record of each publish). A client resolves the head's relative `url`s to
// the immutable blob tree; no mutable payload copy lives under a stream.
//
// Two adapters. A **directory** is v1's writable origin: any directory a
// static host serves. Every file lands whole or absent (a temp file beside
// it, then a rename), blobs are immutable and skipped when present by
// digest, and the head is a **conditional put**: one writer per stream at a
// time through a lock file (`.lock` in the stream directory, created
// `O_EXCL`, the pid and time recorded, stale after 60 s or when the pid is
// gone), and the current head's digest compared with the one the publisher
// read before the new head is renamed into place. An **https** origin is
// read-only here — enough for the classifier to read live heads and root
// files; an object-store adapter (`If-Match` on the ETag, which S3, GCS, R2,
// and Azure all have) is owed, and `--yes` against https refuses by name.
import { createHash, randomBytes } from 'node:crypto';
import { closeSync, linkSync, mkdirSync, openSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

/** SHA-256, lowercase hex — the digest every card and blob name carries (LLP 1023 D2). */
export const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** A lock older than this is a dead publisher's and is removed (with its pid checked first). */
export const LOCK_STALE_MS = 60_000;

/** A transport or filesystem availability failure. Classifiers turn only
 * this typed failure into an `unavailable` row; malformed heads and broken
 * invariants still fail closed. */
export class OriginUnavailable extends Error {}

/** The stream's directory on the origin, relative to its root (LLP 1030.000 D7). */
export const streamPath = ({ channel, compatibilityId }) => `.exact/${channel}/${compatibilityId}`;

/** The blob's path on the origin: content-addressed, immutable. */
export const blobPath = (digest) => `.exact/blobs/${digest}`;

/** A relative origin path is segments with no `..`, no empty segment, no backslash, no drive. */
export function safeRelative(rel) {
  if (typeof rel !== 'string' || rel === '' || rel.startsWith('/') || rel.includes('\\') || rel.includes('\0')) throw new Error(`not a relative origin path: ${JSON.stringify(rel)}`);
  for (const segment of rel.split('/')) if (segment === '' || segment === '.' || segment === '..') throw new Error(`not a relative origin path: ${JSON.stringify(rel)}`);
  return rel;
}

/** The origin `spec` names: a directory (created on first write) or an https URL (read-only). */
export function openOrigin(spec) {
  if (/^https?:\/\//.test(spec)) return new HttpsOrigin(spec);
  return new DirectoryOrigin(spec);
}

/** A directory an https static host serves: the one writable origin in v1, one publisher per stream at a time. */
export class DirectoryOrigin {
  constructor(dir) {
    this.dir = resolve(dir);
    this.kind = 'directory';
    this.writable = true;
  }

  /** How the table names it. */
  describe() { return this.dir; }

  /** The absolute path of `rel`, refused when it would leave the origin. */
  path(rel) {
    const abs = resolve(this.dir, safeRelative(rel));
    if (abs !== this.dir && !abs.startsWith(this.dir + '/')) throw new Error(`${rel} leaves the origin`);
    return abs;
  }

  /** The bytes at `rel`, or null when there is no such file. */
  async get(rel) {
    try { return readFileSync(this.path(rel)); } catch (e) {
      if (e.code === 'ENOENT' || e.code === 'EISDIR' || e.code === 'ENOTDIR') return null;
      if (['EACCES', 'EPERM', 'EIO', 'EMFILE', 'ENFILE'].includes(e.code)) throw new OriginUnavailable(`${this.path(rel)}: ${e.code}`);
      throw e;
    }
  }

  /** The names under the directory `rel` (files and directories), or null when it does not exist. */
  async list(rel) {
    try { return readdirSync(this.path(rel)).filter((n) => !n.startsWith('.')).sort(); } catch (e) {
      if (e.code === 'ENOENT' || e.code === 'ENOTDIR') return null;
      if (['EACCES', 'EPERM', 'EIO', 'EMFILE', 'ENFILE'].includes(e.code)) throw new OriginUnavailable(`${this.path(rel)}: ${e.code}`);
      throw e;
    }
  }

  /** Write `bytes` at `rel`, whole or absent (a temp file beside it, renamed into place). An `immutable` file is content-addressed or an audit record: present with the same bytes, it is skipped (`'present'`); present with other bytes, refused. Returns `'written'` or `'present'`. */
  async put(rel, bytes, { immutable = false } = {}) {
    const abs = this.path(rel);
    const existing = immutable ? await this.get(rel) : null;
    if (existing) {
      const have = existing;
      if (have.equals(bytes)) return 'present';
      throw new Error(`the immutable file ${rel} is on the origin with other bytes (${sha256(have)} on the origin, ${sha256(bytes)} here): it never changes under its name`);
    }
    mkdirSync(dirname(abs), { recursive: true });
    const temp = resolve(dirname(abs), `.tmp-${process.pid}-${randomBytes(4).toString('hex')}`);
    try {
      writeFileSync(temp, bytes, { flag: 'wx' });
      if (immutable) {
        try { linkSync(temp, abs); }
        catch (error) {
          if (error.code !== 'EEXIST') throw error;
          const have = await this.get(rel);
          if (have?.equals(bytes)) { rmSync(temp, { force: true }); return 'present'; }
          throw new Error(`the immutable file ${rel} appeared with other bytes while it was being published: it never changes under its name`);
        }
        rmSync(temp, { force: true });
      } else {
        renameSync(temp, abs);
      }
    } catch (e) {
      rmSync(temp, { force: true });
      throw e;
    }
    return 'written';
  }

  /** The stream's current head: `{bytes, sha256, json}`, or null when the stream has none. A head that is not JSON is an error naming the path — a corrupt head is not a missing one. */
  async head(stream) {
    const rel = `${streamPath(stream)}/exact.json`;
    const bytes = await this.get(rel);
    if (!bytes) return null;
    let json;
    try { json = JSON.parse(bytes.toString('utf8')); } catch (e) { throw new Error(`the head at ${this.path(rel)} is not JSON: ${e.message}`); }
    return { bytes, sha256: sha256(bytes), json };
  }

  /** Hold the stream's lock for the duration of `fn`. Refuses, naming the lock file and the pid, when another live publisher holds it; a lock whose pid is gone or that is older than `LOCK_STALE_MS` is a dead publisher's and is removed first. */
  async withLock(stream, fn) {
    const lock = this.path(`${streamPath(stream)}/.lock`);
    mkdirSync(dirname(lock), { recursive: true });
    const note = JSON.stringify({ pid: process.pid, at: new Date().toISOString(), host: process.env.HOSTNAME ?? null });
    for (let attempt = 0; ; attempt++) {
      try {
        const fd = openSync(lock, 'wx', 0o644);
        writeFileSync(fd, note);
        closeSync(fd);
        break;
      } catch (e) {
        if (e.code !== 'EEXIST') throw e;
        const holder = lockHolder(lock);
        if (attempt === 0 && holder.stale) { rmSync(lock, { force: true }); continue; }
        throw new Error(`the stream ${streamPath(stream)} is locked by ${holder.who} (${lock}): another publisher is on it; a lock older than ${LOCK_STALE_MS / 1000} s or whose process is gone is removed automatically`);
      }
    }
    try { return await fn(); } finally { rmSync(lock, { force: true }); }
  }

  /** The conditional put of the head (LLP 1030.000 D3 item 5): the current head's digest must be `previousDigest` (null: no head), or the put refuses and nothing changes. Called under `withLock`; the compare is the check that holds even when it is not. */
  async putHead(stream, bytes, { previousDigest = null } = {}) {
    const current = await this.head(stream);
    const found = current?.sha256 ?? null;
    if (found !== previousDigest) {
      throw new Error(`the head of ${streamPath(stream)} changed underneath: expected ${previousDigest ?? 'no head'}, found ${found ?? 'no head'}; classify again`);
    }
    return this.put(`${streamPath(stream)}/exact.json`, bytes);
  }
}

/** Who holds a lock file and whether it is stale: its pid gone, or its mtime older than `LOCK_STALE_MS`. */
function lockHolder(lock) {
  let pid = null;
  let at = null;
  try { ({ pid = null, at = null } = JSON.parse(readFileSync(lock, 'utf8'))); } catch { /* an unreadable note is still a lock */ }
  let age = 0;
  try { age = Date.now() - statSync(lock).mtimeMs; } catch { return { who: 'nobody', stale: true }; }
  let alive = true;
  if (typeof pid === 'number' && pid > 0) {
    try { process.kill(pid, 0); } catch (e) { if (e.code === 'ESRCH') alive = false; }
  }
  return { who: `pid ${pid ?? '?'}${at ? ` since ${at}` : ''}${alive ? '' : ' (gone)'}`, stale: !alive || age > LOCK_STALE_MS };
}

/** An https origin: what a client sees. Reads heads and root files for the classifier; every write refuses — an object-store adapter is owed. */
export class HttpsOrigin {
  constructor(url) {
    this.url = url.replace(/\/+$/, '');
    this.kind = 'https';
    this.writable = false;
  }

  describe() { return this.url; }

  /** The body at `rel`, or null on 404. Any other failure is an error naming the URL. */
  async get(rel) {
    const url = `${this.url}/${safeRelative(rel)}`;
    let response;
    try {
      response = await fetch(url, { headers: { 'cache-control': 'no-cache' }, redirect: 'follow' });
    } catch (error) {
      throw new OriginUnavailable(`${url}: ${error.cause?.code ?? error.message}`);
    }
    if (response.status === 404) return null;
    if (!response.ok) throw new OriginUnavailable(`${url}: HTTP ${response.status}`);
    return Buffer.from(await response.arrayBuffer());
  }

  /** An https origin cannot be listed: null, and the caller says so. */
  async list() { return null; }

  async head(stream) {
    const bytes = await this.get(`${streamPath(stream)}/exact.json`);
    if (!bytes) return null;
    let json;
    try { json = JSON.parse(bytes.toString('utf8')); } catch (e) { throw new Error(`the head at ${this.url}/${streamPath(stream)}/exact.json is not JSON: ${e.message}`); }
    return { bytes, sha256: sha256(bytes), json };
  }

  readOnly() { throw new Error(`${this.url} is an https origin, read-only in v1: point --origin at the directory the host serves (an object-store adapter with a conditional put is owed)`); }
  async put() { this.readOnly(); }
  async withLock() { this.readOnly(); }
  async putHead() { this.readOnly(); }
}
