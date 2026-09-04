// The origin adapter `exact deploy` publishes through (LLP 1030.000 D3 item
// 5, D7; LLP 1030 D3a): the static host that serves the web app at its root,
// content-addressed blobs under `.exact/blobs/<sha256>`, and one directory
// per stream — `.exact/<channel>/<compatibilityId>/` — holding the signed
// head `exact.json` and `releases/<release>.json` (the immutable prepared
// record of each publish). A client resolves the head's relative `url`s to
// the immutable blob tree; no mutable payload copy lives under a stream.
//
// Two adapters. A **directory** is v1's writable origin: any directory a
// supported serve.mjs origin handler serves. Every file lands whole or absent (a temp file beside
// it, then a rename), blobs are immutable and skipped when present by
// digest, and the head is a **conditional put**: one writer per stream at a
// time through an OS lock on a permanent `.lock` inode (never stolen on
// elapsed time or a host's interpretation of another host's pid), and the current head's digest compared with the one the publisher
// read before the new head is renamed into place. An **https** origin is
// read-only here — enough for the classifier to read live heads and root
// files; an object-store adapter (`If-Match` on the ETag, which S3, GCS, R2,
// and Azure all have) is owed, and `--yes` against https refuses by name.
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { filesystem, filesystemLock } from './filesystem.mjs';

/** SHA-256, lowercase hex — the digest every card and blob name carries (LLP 1023 D2). */
export const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** A transport or filesystem availability failure. Classifiers turn only
 * this typed failure into an `unavailable` row; malformed heads and broken
 * invariants still fail closed. */
export class OriginUnavailable extends Error {}

/** The stream's directory on the origin, relative to its root (LLP 1030.000 D7). */
export const streamPath = ({ channel, compatibilityId }) => `.exact/${channel}/${compatibilityId}`;

/** The blob's path on the origin: content-addressed, immutable. */
export const blobPath = (digest) => `.exact/blobs/${digest}`;

// The web release has one complete public inventory and one guarded pointer.
// It uses the same OS lock and conditional write as native stream heads.
export const webRootStream = { channel: 'root', compatibilityId: 'web' };
export const webRootPath = `${streamPath(webRootStream)}/exact.json`;
export const webReleasePath = (id) => `.exact/web/${id}`;
export function parseWebRoot(bytes) {
  const root = JSON.parse(bytes.toString('utf8'));
  if (root.webRoot !== 1 || !/^[0-9a-f]{64}$/.test(root.id ?? '') || !Array.isArray(root.files)) throw new Error('invalid web root pointer');
  const names = new Set();
  for (const file of root.files) {
    safeRelative(file.name);
    if (names.has(file.name) || !/^[0-9a-f]{64}$/.test(file.sha256 ?? '')
      || !/^[0-9a-f]{64}$/.test(file.sourceSha256 ?? '') || !Number.isSafeInteger(file.bytes) || file.bytes < 0) throw new Error('invalid web root inventory');
    names.add(file.name);
  }
  return root;
}

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

/** A directory served by host/web/serve.mjs --origin: the one writable origin in v1, one publisher per stream at a time. */
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

  async operation(input) {
    try { return this.locked ? await this.locked(input) : filesystem({ root: this.dir, ...input }); }
    catch (error) {
      if (['EACCES', 'EPERM', 'EIO', 'EMFILE', 'ENFILE'].includes(error.code)) throw new OriginUnavailable(`${this.dir}: ${error.code}`);
      throw error;
    }
  }

  /** Read through owned directory handles; only a genuinely absent file is null. */
  async get(rel) {
    const value = await this.operation({ op: 'get', path: safeRelative(rel) });
    return value === null ? null : Buffer.from(value, 'base64');
  }

  async list(rel, { includeHidden = false } = {}) {
    const value = await this.operation({ op: 'list', path: safeRelative(rel) });
    return value?.filter((name) => includeHidden || !name.startsWith('.')) ?? null;
  }

  /** A complete sibling-temp rename, or an exclusive immutable link. */
  async put(rel, bytes, { immutable = false } = {}) {
    return this.operation({ op: 'put', path: safeRelative(rel), bytes: Buffer.from(bytes).toString('base64'), immutable });
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

  /** Hold an OS lock on a permanent, no-follow inode. Age, PID and host
   * identity never authorize stealing it; closing the helper releases it. */
  async withLock(stream, fn) {
    if (this.lockPending) throw new Error('this origin already holds or is acquiring a stream lock');
    const path = safeRelative(`${streamPath(stream)}/.lock`);
    this.lockPending = true;
    try {
      return await filesystemLock(this.dir, path, async (send) => {
        this.locked = send;
        try { return await fn(); } finally { this.locked = null; }
      });
    } finally { this.lockPending = false; }
  }

  /** The digest check and head rename run inside the same lock-owning
   * helper. A standalone call acquires that stream lock too. */
  async putHead(stream, bytes, { previousDigest = null } = {}) {
    if (!this.locked) return this.withLock(stream, () => this.putHead(stream, bytes, { previousDigest }));
    return this.operation({ op: 'head', path: safeRelative(`${streamPath(stream)}/exact.json`),
      bytes: Buffer.from(bytes).toString('base64'), previousDigest });
  }
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
