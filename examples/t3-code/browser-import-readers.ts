// browser-surface part 4 (profiles): reading Chromium and Firefox cookie databases (MIT reference, see LICENSE-T3, T3 Code
// 1e2ecbd975: apps/desktop/src/preview/BrowserImport/{CookieDatabase,ChromiumKeys,ChromiumCookies,FirefoxCookies}.ts),
// over the clone's ImportIO (browser-import-io.ts).
//
// Chromium (OSCrypt): on macOS one key in the login Keychain, read in-process so the consent prompt names this app; each
// record decrypted with the key its prefix calls for (`v10` AES-128-CBC with 16 spaces as the IV; Windows' `v10` is
// AES-256-GCM; `v20` app-bound records have no key); Chromium 127+ binds the plaintext to SHA-256 of the host. A record
// no key covers is skipped, not the import. Firefox stores plaintext; only the default container is imported.
// Every reader works on a `VACUUM INTO` snapshot, never the browser's own file. The Linux keyring helper and Windows'
// DPAPI unwrap are not ported (the clone runs on macOS); the decryption keeps every scheme, as the reference does.
import {
  KeychainUnavailableError, bytesEqual, named, latin1, utf8, utf8Bytes,
  type DecryptItem, type ImportIO, type PlatformName, type SqlRow,
} from './browser-import-io';

// ── CookieDatabase.ts ──────────────────────────────────────────────────────────────────────────────
/** A cookie in the shape the reference hands Electron's `session.cookies.set` (the module writes it to WebKit). */
export interface ImportedCookie {
  readonly url: string;
  readonly name: string;
  readonly value: string;
  /** Set only for domain cookies (a leading dot); a host-only cookie leaves it undefined, so it is never widened. */
  readonly domain: string | undefined;
  readonly path: string;
  readonly secure: boolean;
  readonly httpOnly: boolean;
  /** Seconds since the UNIX epoch, or undefined for a session cookie. */
  readonly expirationDate: number | undefined;
  readonly sameSite: 'unspecified' | 'no_restriction' | 'lax' | 'strict';
}
export interface CookieReadResult {
  readonly cookies: readonly ImportedCookie[];
  readonly undecryptable: number;
  readonly undecryptableHosts: readonly string[];
}
/** A host without the leading dot both engines put on a domain cookie. */
export const bareHost = (host: string): string => (host.startsWith('.') ? host.slice(1) : host);
/** The URL and domain a stored row is registered under: the dot comes off for the URL; `domain` only for domain cookies. */
export function cookieScope(host: string, path: string, secure: boolean): { url: string; domain: string | undefined } {
  const isDomainCookie = host.startsWith('.'), unwrappedHost = bareHost(host);
  const authority = unwrappedHost.includes(':') && !(unwrappedHost.startsWith('[') && unwrappedHost.endsWith(']')) ? `[${unwrappedHost}]` : unwrappedHost;
  return { url: `${secure ? 'https' : 'http'}://${authority}${path}`, domain: isDomainCookie ? host : undefined };
}
/** A consistent copy of a live database (WAL included) in a temporary directory; `release` removes it. */
export const snapshotCookieDatabase = (io: ImportIO, cookiePath: string) => io.snapshot(cookiePath);

// ── ChromiumKeys.ts (macOS) ───────────────────────────────────────────────────────────────────────
const KEY_SALT = 'saltysalt', KEY_LENGTH = 16, MAC_KEY_ITERATIONS = 1003;
export type ChromiumKeyFailure = 'needsKeychainApproval' | 'keychainItemMissing' | 'keychainUnavailable' | 'unsupportedPlatform' | 'readFailed';
export class ChromiumKeyError extends Error {
  readonly _tag = 'ChromiumKeyError';
  constructor(readonly reason: ChromiumKeyFailure, readonly cause?: unknown) { super(`Could not obtain the Chromium cookie key: ${reason}.`); named(this, 'ChromiumKeyError'); }
}
/** Keys to try, by the record prefix they decrypt; a missing one means those records are skipped. */
export interface ChromiumKeyMaterial {
  readonly cbcV10?: Uint8Array;
  readonly cbcV11?: Uint8Array;
  readonly cbcV11Error?: ChromiumKeyError;
  readonly cbcEmpty?: Uint8Array;
  readonly gcmV10?: Uint8Array;
}
/** The macOS OSCrypt secret from the login Keychain; a refusal is consent the user can give on a retry. */
async function readKeychainSecret(io: ImportIO, service: string, account: string): Promise<string> {
  let secret: string | null;
  try { secret = await io.keychainPassword(service, account); }
  catch (cause) {
    if (cause instanceof KeychainUnavailableError) throw new ChromiumKeyError('keychainUnavailable', cause);
    const message = String((cause as { message?: unknown } | undefined)?.message ?? '');
    throw new ChromiumKeyError(/no (matching )?entry|not found/i.test(message) ? 'keychainItemMissing' : 'needsKeychainApproval', cause);
  }
  if (secret === null || secret === '') throw new ChromiumKeyError('keychainItemMissing');
  return secret;
}
export interface ChromiumKeyRequest {
  readonly platform: PlatformName;
  readonly keychainService: string | undefined;
  readonly keychainAccount: string | undefined;
  readonly linuxSecretApplication: string | undefined;
}
export async function resolveChromiumKeys(io: ImportIO, request: ChromiumKeyRequest): Promise<ChromiumKeyMaterial> {
  if (request.platform !== 'darwin' || !request.keychainService || !request.keychainAccount) throw new ChromiumKeyError('unsupportedPlatform');
  const secret = await readKeychainSecret(io, request.keychainService, request.keychainAccount);
  return { cbcV10: await io.pbkdf2Sha1(secret, KEY_SALT, MAC_KEY_ITERATIONS, KEY_LENGTH) };
}

// ── ChromiumCookies.ts ────────────────────────────────────────────────────────────────────────────
const AES_CBC_IV = new Uint8Array(16).fill(0x20), AES_GCM_NONCE_LENGTH = 12, AES_GCM_TAG_LENGTH = 16;
export type ChromiumCookieReadReason = ChromiumKeyFailure | 'browserRunning';
export class ChromiumCookieReadError extends Error {
  readonly _tag = 'ChromiumCookieReadError';
  constructor(readonly reason: ChromiumCookieReadReason, readonly cookieDatabasePath: string, readonly cause?: unknown) {
    super(`Could not read Chromium cookies at ${cookieDatabasePath}: ${reason}.`); named(this, 'ChromiumCookieReadError');
  }
}
/** Effect's SchemaError: a row or the schema version that does not decode. */
export class SchemaError extends Error {
  readonly _tag = 'SchemaError';
  constructor(message: string) { super(message); named(this, 'SchemaError'); }
}

/** Chromium's SameSite column: -1 unspecified, 0 none, 1 lax, 2 strict; anything else unspecified (never widened). */
const chromiumSameSite = (value: number): ImportedCookie['sameSite'] => value === 0 ? 'no_restriction' : value === 1 ? 'lax' : value === 2 ? 'strict' : 'unspecified';
/** Chromium counts microseconds from 1601; the SQL divides to seconds, this rebases them. */
const WEBKIT_EPOCH_OFFSET_SECONDS = 11_644_473_600;
const toUnixSeconds = (webkitSeconds: number): number | undefined => webkitSeconds <= 0 ? undefined : webkitSeconds - WEBKIT_EPOCH_OFFSET_SECONDS;

type Attempt = { mode: 'cbc'; key: Uint8Array } | { mode: 'gcm'; key: Uint8Array } | { mode: 'plain' };
/**
 * Decrypts stored values, choosing the scheme from each prefix, in batches through the module (one `decrypt` call per
 * round, not per cookie). A failed record is retried with the empty-passphrase key, as Chromium does; one whose own key
 * is missing stays skipped. Unprefixed data on macOS and Linux is legacy plaintext. Null where nothing decrypts.
 */
export async function decryptChromiumValues(io: ImportIO, items: readonly { encrypted: Uint8Array; domain: string }[], keys: ChromiumKeyMaterial,
  schemaVersion = 23, platform: PlatformName = 'linux'): Promise<(string | null)[]> {
  const hashes = new Map<string, Uint8Array>();
  if (schemaVersion >= 24) {
    const domains = [...new Set(items.map(item => item.domain))];
    const digests = await io.sha256(domains.map(utf8Bytes));
    domains.forEach((domain, index) => hashes.set(domain, digests[index]!));
  }
  // Chromium >= 127 prefixes the plaintext with SHA-256 of the host key: strip it when present, refuse it when wrong.
  const strip = (plaintext: Uint8Array, domain: string): Uint8Array | null => {
    if (schemaVersion < 24) return plaintext;
    const hash = hashes.get(domain)!;
    return plaintext.length >= 32 && bytesEqual(plaintext.subarray(0, 32), hash) ? plaintext.subarray(32) : null;
  };
  const results: (string | null)[] = items.map(() => null);
  const plans = items.map(item => {
    const buffer = item.encrypted;
    if (buffer.length === 0) return [] as Attempt[];
    const prefix = latin1(buffer.subarray(0, 3));
    if (platform === 'win32') return prefix === 'v10' && keys.gcmV10 ? [{ mode: 'gcm', key: keys.gcmV10 } as Attempt] : [];
    const own = prefix === 'v10' ? keys.cbcV10 : prefix === 'v11' ? keys.cbcV11 : undefined;
    if (prefix === 'v10' || prefix === 'v11') return own ? [{ mode: 'cbc', key: own } as Attempt, ...(keys.cbcEmpty ? [{ mode: 'cbc', key: keys.cbcEmpty } as Attempt] : [])] : [];
    return platform === 'darwin' || platform === 'linux' ? [{ mode: 'plain' } as Attempt] : [];
  });
  items.forEach((item, index) => { if (item.encrypted.length === 0) results[index] = ''; });
  for (let round = 0; round < 2; round++) {
    const pending = plans.flatMap((plan, index) => plan[round] && results[index] === null ? [index] : []);
    if (!pending.length) break;
    const requests: { index: number; item: DecryptItem }[] = [];
    for (const index of pending) {
      const attempt = plans[index]![round]!, buffer = items[index]!.encrypted;
      if (attempt.mode === 'plain') { const stripped = strip(buffer, items[index]!.domain); if (stripped) results[index] = utf8(stripped); continue; }
      const payload = buffer.subarray(3);
      if (attempt.mode === 'cbc') requests.push({ index, item: { mode: 'cbc', key: attempt.key, iv: AES_CBC_IV, data: payload } });
      else if (payload.length >= AES_GCM_NONCE_LENGTH + AES_GCM_TAG_LENGTH) {
        requests.push({ index, item: { mode: 'gcm', key: attempt.key, nonce: payload.subarray(0, AES_GCM_NONCE_LENGTH), data: payload.subarray(AES_GCM_NONCE_LENGTH, payload.length - AES_GCM_TAG_LENGTH), tag: payload.subarray(payload.length - AES_GCM_TAG_LENGTH) } });
      }
    }
    const plaintexts = requests.length ? await io.decrypt(requests.map(request => request.item)) : [];
    requests.forEach((request, at) => {
      const plaintext = plaintexts[at];
      const stripped = plaintext ? strip(plaintext, items[request.index]!.domain) : null;
      if (stripped) results[request.index] = utf8(stripped);
    });
  }
  return results;
}
export async function decryptChromiumValue(io: ImportIO, encrypted: Uint8Array, keys: ChromiumKeyMaterial, domain: string, schemaVersion = 23, platform: PlatformName = 'linux'): Promise<string | null> {
  return (await decryptChromiumValues(io, [{ encrypted, domain }], keys, schemaVersion, platform))[0] ?? null;
}

type ChromiumRow = { host_key: string; name: string; value: string; encrypted_value: Uint8Array; path: string; expires_seconds: number; is_secure: number; is_httponly: number; samesite: number; top_frame_site_key: string };
function decodeChromiumRows(raw: SqlRow[]): ChromiumRow[] {
  return raw.map((row, index) => {
    const ok = typeof row.host_key === 'string' && typeof row.name === 'string' && typeof row.value === 'string' && row.encrypted_value instanceof Uint8Array
      && typeof row.path === 'string' && typeof row.expires_seconds === 'number' && typeof row.is_secure === 'number' && typeof row.is_httponly === 'number'
      && typeof row.samesite === 'number' && typeof row.top_frame_site_key === 'string';
    if (!ok) throw new SchemaError(`Expected a cookie row at [${index}].`);
    return row as unknown as ChromiumRow;
  });
}
function decodeSchemaVersion(rows: SqlRow[]): number {
  const value = rows.length === 1 ? rows[0]!.value : undefined;
  const parsed = typeof value === 'number' ? value : typeof value === 'string' && value.trim() !== '' ? Number(value) : NaN;
  if (!Number.isInteger(parsed) || parsed < 0) throw new SchemaError(`Expected a non-negative integer schema version, got ${String(value)}.`);
  return parsed;
}

/** Reads and decodes one snapshotted Chromium cookie database. */
export async function readChromiumCookieDatabase(io: ImportIO, snapshotPath: string, keys: ChromiumKeyMaterial, platform: PlatformName): Promise<CookieReadResult> {
  const schemaVersion = decodeSchemaVersion(await io.query(snapshotPath, "select value from meta where key = 'version' limit 1"));
  const rows = decodeChromiumRows(schemaVersion >= 15
    ? await io.query(snapshotPath, 'select host_key, name, value, encrypted_value, path, expires_utc / 1000000 as expires_seconds, is_secure, is_httponly, samesite, top_frame_site_key from cookies')
    : await io.query(snapshotPath, "select host_key, name, value, encrypted_value, path, expires_utc / 1000000 as expires_seconds, is_secure, is_httponly, samesite, '' as top_frame_site_key from cookies"));
  // Partitioned (CHIPS) cookies have no WebKit equivalent: skipped and named.
  const encrypted = rows.flatMap((row, index) => row.top_frame_site_key === '' && row.encrypted_value.length > 0 ? [index] : []);
  const decrypted = await decryptChromiumValues(io, encrypted.map(index => ({ encrypted: rows[index]!.encrypted_value, domain: rows[index]!.host_key })), keys, schemaVersion, platform);
  const values = new Map(encrypted.map((index, at) => [index, decrypted[at] ?? null]));
  const cookies: ImportedCookie[] = [];
  let undecryptable = 0;
  const undecryptableHosts = new Set<string>();
  rows.forEach((row, index) => {
    const value = row.top_frame_site_key !== '' ? null : row.encrypted_value.length === 0 ? row.value : values.get(index) ?? null;
    if (value === null) { undecryptable += 1; undecryptableHosts.add(bareHost(row.host_key)); return; }
    const secure = row.is_secure === 1, scope = cookieScope(row.host_key, row.path, secure);
    cookies.push({ url: scope.url, name: row.name, value, domain: scope.domain, path: row.path, secure, httpOnly: row.is_httponly === 1,
      expirationDate: toUnixSeconds(row.expires_seconds), sameSite: chromiumSameSite(row.samesite) });
  });
  // A missing key that kept every importable cookie out is the reason, not an empty import.
  if (cookies.length === 0 && keys.cbcV11Error !== undefined && rows.some(row => row.top_frame_site_key === '' && latin1(row.encrypted_value.subarray(0, 3)) === 'v11')) throw keys.cbcV11Error;
  return { cookies, undecryptable, undecryptableHosts: [...undecryptableHosts] };
}

export interface ChromiumCookieSource {
  readonly cookieDatabasePath: string;
  readonly keychainService: string | undefined;
  readonly keychainAccount: string | undefined;
  readonly linuxSecretApplication: string | undefined;
  readonly platform: PlatformName;
}
export async function readChromiumCookies(io: ImportIO, source: ChromiumCookieSource): Promise<CookieReadResult> {
  let keys: ChromiumKeyMaterial;
  try { keys = await resolveChromiumKeys(io, source); }
  catch (cause) { throw new ChromiumCookieReadError(cause instanceof ChromiumKeyError ? cause.reason : 'readFailed', source.cookieDatabasePath, cause); }
  let snapshot: { path: string; release(): Promise<void> };
  try { snapshot = await snapshotCookieDatabase(io, source.cookieDatabasePath); }
  catch (cause) { throw new ChromiumCookieReadError('readFailed', source.cookieDatabasePath, cause); }
  try { return await readChromiumCookieDatabase(io, snapshot.path, keys, source.platform); }
  catch (cause) { throw new ChromiumCookieReadError(cause instanceof ChromiumKeyError ? cause.reason : 'readFailed', source.cookieDatabasePath, cause); }
  finally { await snapshot.release().catch(() => undefined); }
}

// ── FirefoxCookies.ts ─────────────────────────────────────────────────────────────────────────────
export class FirefoxCookieReadError extends Error {
  readonly _tag = 'FirefoxCookieReadError';
  constructor(readonly cookieDatabasePath: string, readonly cause: unknown) { super(`Could not read Firefox cookies at ${cookieDatabasePath}.`); named(this, 'FirefoxCookieReadError'); }
}
/** nsICookie: 0 None, 1 Lax, 2 Strict, 256 Unset; NULL before schema 9; schemas 10–14 fold "Lax, None declared" to Unset. */
const SAMESITE_NONE = 0, SAMESITE_LAX = 1, SAMESITE_STRICT = 2;
function firefoxSameSite(value: number | null, rawValue: number | null): ImportedCookie['sameSite'] {
  if (value === null) return 'unspecified';
  if (value === SAMESITE_LAX && rawValue === SAMESITE_NONE) return 'unspecified';
  return value === SAMESITE_NONE ? 'no_restriction' : value === SAMESITE_LAX ? 'lax' : value === SAMESITE_STRICT ? 'strict' : 'unspecified';
}
/** Schema 16 (Firefox 129) moved `expiry` from seconds to milliseconds. */
const expiryToSeconds = (expiry: number, schemaVersion: number): number | undefined => expiry <= 0 ? undefined : schemaVersion >= 16 ? Math.floor(expiry / 1000) : expiry;

export async function readFirefoxCookies(io: ImportIO, cookieDatabasePath: string): Promise<ImportedCookie[]> {
  let snapshot: { path: string; release(): Promise<void> } | null = null;
  try {
    snapshot = await snapshotCookieDatabase(io, cookieDatabasePath);
    const versionRows = await io.query(snapshot.path, 'pragma user_version');
    const userVersion = versionRows[0]?.user_version;
    if (versionRows.length > 0 && typeof userVersion !== 'number') throw new SchemaError('Expected a numeric user_version.');
    const schemaVersion = typeof userVersion === 'number' ? userVersion : 0;
    const hasRawSameSite = schemaVersion >= 10 && schemaVersion <= 14;
    // Only the default container: Firefox isolates containers and private windows by originAttributes; WebKit cannot.
    const raw = await io.query(snapshot.path, hasRawSameSite
      ? "select host, name, value, path, expiry, isSecure, isHttpOnly, sameSite, rawSameSite from moz_cookies where originAttributes = ''"
      : "select host, name, value, path, expiry, isSecure, isHttpOnly, sameSite, null as rawSameSite from moz_cookies where originAttributes = ''");
    return raw.map((row, index) => {
      const nullableNumber = (value: unknown) => value === null || typeof value === 'number';
      if (!(typeof row.host === 'string' && typeof row.name === 'string' && typeof row.value === 'string' && typeof row.path === 'string' && typeof row.expiry === 'number'
        && typeof row.isSecure === 'number' && typeof row.isHttpOnly === 'number' && nullableNumber(row.sameSite) && nullableNumber(row.rawSameSite))) throw new SchemaError(`Expected a cookie row at [${index}].`);
      const secure = row.isSecure === 1, scope = cookieScope(row.host, row.path, secure);
      return { url: scope.url, name: row.name, value: row.value, domain: scope.domain, path: row.path, secure, httpOnly: row.isHttpOnly === 1,
        expirationDate: expiryToSeconds(row.expiry, schemaVersion), sameSite: firefoxSameSite(row.sameSite as number | null, row.rawSameSite as number | null) };
    });
  } catch (cause) { throw new FirefoxCookieReadError(cookieDatabasePath, cause); }
  finally { await snapshot?.release().catch(() => undefined); }
}

