// browser-surface part 4 (profiles): what the cookie import reads the other browser with. The reference's readers run on
// Effect's FileSystem, Path, SqlClient (node:sqlite), node:crypto, the keyring binding and child processes
// (apps/desktop/src/preview/BrowserImport/*). The clone's port (browser-import-*.ts) is the same logic over this one
// interface: in the app the clone's module answers it (T3BrowserImportIO.swift: FileManager, SQLite, CommonCrypto,
// CryptoKit, the Keychain, fcntl), in the tests a Bun implementation over fixture stores (browser-import-fixture.ts).
//
// A development build reads only under the fixture home named by T3_BROWSER_IMPORT_HOME and never the Keychain (the
// module refuses every other path and answers the Keychain from the fixture's own file); only the packaged build reads
// the user's browsers. MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975.
import { obj, str, type Json, type Obj } from './domain';
import { letGo } from './let-go';

export type FileKind = 'File' | 'Directory' | 'SymbolicLink' | 'Other';
/** An error's own `name`. The app's JS runtime freezes Error.prototype, whose `name` is then read-only, so `this.name = …`
 *  throws a TypeError there (the override mistake) while Bun passes; a defined own property does not. */
export function named<T extends Error>(error: T, name: string): T {
  Object.defineProperty(error, 'name', { value: name, writable: true, configurable: true, enumerable: false });
  return error;
}
/** A file-system failure with its errno name (`ENOENT`, `EPERM`, `EACCES`, `EBUSY`, …), as Node's PlatformError cause. */
export class ImportFsError extends Error {
  constructor(readonly code: string, message = code) { super(message); named(this, 'ImportFsError'); }
}
/** A SQLite failure (Effect's SqlError). */
export class ImportSqlError extends Error {
  constructor(message: string) { super(message); named(this, 'SqlError'); }
}
/** The Keychain cannot be asked at all (the reference's binding that failed to load). */
export class KeychainUnavailableError extends Error {
  constructor(message = 'The Keychain is unavailable.') { super(message); named(this, 'KeychainUnavailableError'); }
}
export type SqlValue = string | number | null | Uint8Array;
export type SqlRow = Record<string, SqlValue>;
export type DecryptItem =
  | { mode: 'cbc'; key: Uint8Array; iv: Uint8Array; data: Uint8Array }
  | { mode: 'gcm'; key: Uint8Array; nonce: Uint8Array; data: Uint8Array; tag: Uint8Array };
export type PlatformName = 'darwin' | 'linux' | 'win32';

export interface ImportIO {
  /** The kind of what the path names, following symlinks (Effect `stat`); throws ImportFsError. */
  stat(path: string): Promise<FileKind>;
  readDirectory(path: string): Promise<string[]>;
  readFileString(path: string): Promise<string>;
  readFile(path: string): Promise<Uint8Array>;
  /** Opens the file read-only and closes it again (TCC gates the open, not `stat`). */
  open(path: string): Promise<void>;
  readLink(path: string): Promise<string>;
  /** A read-only query; rows by column name; throws ImportSqlError. */
  query(database: string, sql: string, params?: SqlValue[]): Promise<SqlRow[]>;
  /** `VACUUM INTO` a fresh temporary directory (CookieDatabase.snapshotCookieDatabase); `release` removes it. */
  snapshot(database: string): Promise<{ path: string; release(): Promise<void> }>;
  /** The generic password; null when there is none; throws on a refusal (its message) or KeychainUnavailableError. */
  keychainPassword(service: string, account: string): Promise<string | null>;
  pbkdf2Sha1(passphrase: string, salt: string, iterations: number, length: number): Promise<Uint8Array>;
  /** AES-128/256-CBC with PKCS#7 padding, or AES-256-GCM; null where a record does not decrypt. */
  decrypt(items: DecryptItem[]): Promise<(Uint8Array | null)[]>;
  sha256(items: Uint8Array[]): Promise<Uint8Array[]>;
  /** `kill(pid, 0)`: resolves when the process exists, throws ImportFsError (`ESRCH`, `EPERM`) otherwise. */
  signal0(pid: number): Promise<void>;
  hostname(): Promise<string>;
  localAddresses(): Promise<string[]>;
  /** Whether another process holds an fcntl write lock on the file (Firefox's `.parentlock`). */
  lockHeld(path: string): Promise<boolean>;
}

// ── Paths (Effect's posix Path, the subset the readers use) ─────────────────────────────────────
function normalizeParts(path: string, absolute: boolean): string[] {
  const out: string[] = [];
  for (const part of path.split('/')) {
    if (part === '' || part === '.') continue;
    if (part === '..') { if (out.length && out[out.length - 1] !== '..') out.pop(); else if (!absolute) out.push('..'); continue; }
    out.push(part);
  }
  return out;
}
export const posixPath = {
  sep: '/',
  isAbsolute: (path: string) => path.startsWith('/'),
  normalize(path: string): string {
    if (path === '') return '.';
    const absolute = path.startsWith('/'), trailing = path.endsWith('/');
    const joined = normalizeParts(path, absolute).join('/');
    const body = joined === '' ? (absolute ? '' : '.') : joined;
    return `${absolute ? '/' : ''}${body}${trailing && joined !== '' ? '/' : ''}`;
  },
  join(...parts: string[]): string {
    const joined = parts.filter(part => part !== '').join('/');
    return joined === '' ? '.' : posixPath.normalize(joined);
  },
  resolve(...parts: string[]): string {
    let resolved = '';
    for (let index = parts.length - 1; index >= 0 && !resolved.startsWith('/'); index--) if (parts[index]) resolved = resolved ? `${parts[index]}/${resolved}` : parts[index]!;
    const out = normalizeParts(resolved, true).join('/');
    return `/${out}`;
  },
  relative(from: string, to: string): string {
    const a = normalizeParts(posixPath.resolve(from), true), b = normalizeParts(posixPath.resolve(to), true);
    let shared = 0;
    while (shared < a.length && shared < b.length && a[shared] === b[shared]) shared++;
    return [...a.slice(shared).map(() => '..'), ...b.slice(shared)].join('/');
  },
  dirname(path: string): string {
    const trimmed = path.length > 1 ? path.replace(/\/+$/, '') : path;
    const index = trimmed.lastIndexOf('/');
    return index < 0 ? '.' : index === 0 ? '/' : trimmed.slice(0, index);
  },
  basename(path: string): string {
    const trimmed = path.length > 1 ? path.replace(/\/+$/, '') : path;
    return trimmed.slice(trimmed.lastIndexOf('/') + 1);
  },
};
export type PathApi = typeof posixPath;

// ── Bytes ───────────────────────────────────────────────────────────────────────────────────────
const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
const LOOKUP = new Map([...ALPHABET].map((char, index) => [char, index]));
export function toBase64(bytes: Uint8Array): string {
  const out: string[] = [];
  for (let index = 0; index < bytes.length; index += 3) {
    const a = bytes[index]!, b = bytes[index + 1], c = bytes[index + 2];
    const triple = (a << 16) | ((b ?? 0) << 8) | (c ?? 0);
    out.push(ALPHABET[(triple >> 18) & 63]! + ALPHABET[(triple >> 12) & 63]! + (b === undefined ? '=' : ALPHABET[(triple >> 6) & 63]!) + (c === undefined ? '=' : ALPHABET[triple & 63]!));
  }
  return out.join('');
}
export function fromBase64(text: string): Uint8Array {
  const clean = text.replace(/[^A-Za-z0-9+/]/g, '');
  const out = new Uint8Array(Math.floor(clean.length * 3 / 4));
  let bits = 0, value = 0, at = 0;
  for (const char of clean) {
    value = (value << 6) | (LOOKUP.get(char) ?? 0); bits += 6;
    if (bits >= 8) { bits -= 8; out[at++] = (value >> bits) & 255; }
  }
  return out.subarray(0, at);
}
export const latin1 = (bytes: Uint8Array): string => String.fromCharCode(...bytes);
export const utf8 = (bytes: Uint8Array): string => new TextDecoder().decode(bytes);
export const utf8Bytes = (text: string): Uint8Array => new TextEncoder().encode(text);
export const bytesEqual = (a: Uint8Array, b: Uint8Array): boolean => a.length === b.length && a.every((byte, index) => byte === b[index]);
export const concatBytes = (...parts: Uint8Array[]): Uint8Array => {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let at = 0;
  for (const part of parts) { out.set(part, at); at += part.length; }
  return out;
};

// ── The module's half (T3BrowserImportIO.swift, op `browserImportIO`) ─────────────────────────────────
type Raw = (request: Obj) => Promise<{ ok: boolean; value?: unknown; error?: unknown }>;
const PAGE_ROWS = 300, CHUNK_BYTES = 256 * 1024;
const encodeValue = (value: SqlValue): Json => value instanceof Uint8Array ? { $b: toBase64(value) } : value;
const decodeValue = (value: unknown): SqlValue => {
  if (value && typeof value === 'object' && typeof (value as Obj).$b === 'string') return fromBase64(str((value as Obj).$b));
  return typeof value === 'string' || typeof value === 'number' ? value : null;
};

/** The module's file, SQLite, crypto and Keychain primitives. Large answers come in pages (select queries by
 *  LIMIT/OFFSET, files in chunks), so no reply passes the bridge's size. The readers treat a failed read as an answer
 *  (absent, not installed, held); a call Exact let go (its answer was replaced) is not one, so `letGoSeen` tells the caller
 *  to discard whatever the readers made of it (docs/agent-pitfalls.md "A superseded send's fetch rejects natively"). */
export function nativeImportIO(raw: Raw): ImportIO & { letGoSeen(): boolean } {
  let lost = false;
  const call = async (name: string, args: Obj = {}): Promise<Obj> => {
    let reply: Awaited<ReturnType<Raw>>;
    try { reply = await raw({ op: 'browserImportIO', call: name, ...args }); }
    catch (error) { if (letGo(error)) lost = true; throw error; }
    const error = obj(reply.error), value = obj(reply.value);
    if (!reply.ok) throw new ImportFsError('EIO', str(error.message, 'The import could not reach the module.'));
    if (str(value.code)) {
      if (value.kind === 'sql') throw new ImportSqlError(str(value.message, value.code as string));
      if (value.kind === 'keychainUnavailable') throw new KeychainUnavailableError(str(value.message));
      if (value.kind === 'keychain') throw new Error(str(value.message, 'The Keychain refused the request.'));
      throw new ImportFsError(str(value.code), str(value.message, str(value.code)));
    }
    return value;
  };
  return {
    letGoSeen: () => lost,
    stat: async path => str((await call('stat', { path })).kind, 'Other') as FileKind,
    readDirectory: async path => { const entries = (await call('readDirectory', { path })).entries; return Array.isArray(entries) ? entries.map(String) : []; },
    readFileString: async path => utf8(await nativeReadFile(call, path)),
    readFile: path => nativeReadFile(call, path),
    open: async path => { await call('open', { path }); },
    readLink: async path => str((await call('readLink', { path })).target),
    async query(database, sql, params = []) {
      if (!/^\s*select\b/i.test(sql)) return ((await call('query', { path: database, sql, params: params.map(encodeValue) })).rows as Obj[] ?? []).map(decodeRow);
      const rows: SqlRow[] = [];
      for (let offset = 0; ; offset += PAGE_ROWS) {
        const page = ((await call('query', { path: database, sql: `select * from (${sql}) limit ${PAGE_ROWS} offset ${offset}`, params: params.map(encodeValue) })).rows as Obj[] ?? []).map(decodeRow);
        rows.push(...page);
        if (page.length < PAGE_ROWS) return rows;
      }
    },
    async snapshot(database) {
      const path = str((await call('snapshot', { path: database })).path);
      return { path, release: async () => { await call('release', { path }); } };
    },
    async keychainPassword(service, account) {
      const value = await call('keychain', { service, account });
      return typeof value.secret === 'string' ? value.secret : null;
    },
    pbkdf2Sha1: async (passphrase, salt, iterations, length) => fromBase64(str((await call('pbkdf2', { passphrase, salt, iterations, length })).key)),
    async decrypt(items) {
      const out: (Uint8Array | null)[] = [];
      for (let start = 0; start < items.length; start += PAGE_ROWS) {
        const page = items.slice(start, start + PAGE_ROWS).map(item => item.mode === 'cbc'
          ? { mode: 'cbc', key: toBase64(item.key), iv: toBase64(item.iv), data: toBase64(item.data) }
          : { mode: 'gcm', key: toBase64(item.key), nonce: toBase64(item.nonce), data: toBase64(item.data), tag: toBase64(item.tag) });
        const results = (await call('decrypt', { items: page })).results;
        out.push(...(Array.isArray(results) ? results : []).map(result => typeof result === 'string' ? fromBase64(result) : null));
      }
      return out;
    },
    async sha256(items) {
      const results = (await call('sha256', { items: items.map(toBase64) })).results;
      return (Array.isArray(results) ? results : []).map(result => fromBase64(String(result)));
    },
    signal0: async pid => { await call('signal0', { pid }); },
    hostname: async () => str((await call('hostname')).hostname),
    localAddresses: async () => { const value = (await call('localAddresses')).addresses; return Array.isArray(value) ? value.map(String) : []; },
    lockHeld: async path => (await call('lockHeld', { path })).held === true,
  };
}
const decodeRow = (row: Obj): SqlRow => Object.fromEntries(Object.entries(row).map(([key, value]) => [key, decodeValue(value)]));
async function nativeReadFile(call: (name: string, args?: Obj) => Promise<Obj>, path: string): Promise<Uint8Array> {
  const staged = await call('readFile', { path });
  const id = str(staged.id), size = Number(staged.size) || 0, parts: Uint8Array[] = [];
  try {
    for (let offset = 0; offset < size; offset += CHUNK_BYTES) parts.push(fromBase64(str((await call('readChunk', { id, offset, length: CHUNK_BYTES })).data)));
  } finally { await call('release', { id }).catch(() => undefined); }
  return concatBytes(...parts);
}
