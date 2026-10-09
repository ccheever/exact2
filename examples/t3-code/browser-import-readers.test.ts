// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names: apps/desktop/src/preview/BrowserImport/
// ChromiumCookies.test.ts (14 of 14; "reports the missing key …" reads the macOS key, whose unavailable binding is the
// clone's KeychainUnavailableError, where the reference used Linux's missing helper), ChromiumKeys.test.ts's macOS
// secrets (3 of 15: the Linux helper and Windows DPAPI tests are not ported, the clone runs on macOS),
// ChromiumKeys.module.test.ts (1) and FirefoxCookies.test.ts (9 of 10: the Windows path rules are not ported).
// The readers run on the Bun ImportIO (browser-import-fixture.ts) against fixture databases built here with bun:sqlite,
// as the reference builds them with node:sqlite. CookieDatabase.test.ts's snapshot rows test the module's IO
// (macos/tests/browser-profiles).
import { afterEach, describe, expect, it } from 'bun:test';
import { Database } from 'bun:sqlite';
import * as NodeCrypto from 'node:crypto';
import * as NodeFs from 'node:fs';
import { KeychainUnavailableError, posixPath } from './browser-import-io';
import {
  ChromiumKeyError, cookieScope, decryptChromiumValue, readChromiumCookieDatabase, readChromiumCookies, readFirefoxCookies, resolveChromiumKeys,
} from './browser-import-readers';
import { parseFirefoxProfiles } from './browser-import-sources';
import { bunImportIO, tempDirectory } from './browser-import-fixture';

const io = bunImportIO();
const scratch: { cleanup(): void }[] = [];
afterEach(() => { for (const entry of scratch.splice(0)) entry.cleanup(); });
const temp = (prefix: string) => { const directory = tempDirectory(prefix); scratch.push(directory); return directory.path; };

const encryptChromium = (prefix: 'v10' | 'v11', value: string | Buffer, key: Buffer): Uint8Array => {
  const cipher = NodeCrypto.createCipheriv('aes-128-cbc', key, Buffer.alloc(16, 0x20));
  return Buffer.concat([Buffer.from(prefix), cipher.update(value), cipher.final()]);
};
const encryptV10 = (value: string | Buffer, key: Buffer): Uint8Array => encryptChromium('v10', value, key);
const encryptWindowsV10 = (value: string | Buffer, key: Buffer): Uint8Array => {
  const nonce = Buffer.from('0123456789ab');
  const cipher = NodeCrypto.createCipheriv('aes-256-gcm', key, nonce);
  const encrypted = Buffer.concat([cipher.update(value), cipher.final()]);
  return Buffer.concat([Buffer.from('v10'), nonce, encrypted, cipher.getAuthTag()]);
};
const boundValue = (host: string, value: string) => Buffer.concat([NodeCrypto.createHash('sha256').update(host).digest(), Buffer.from(value)]);
const COOKIES_TABLE = `create table cookies (
  host_key text not null, name text not null, value text not null, encrypted_value blob not null, path text not null, expires_utc integer not null,
  is_secure integer not null, is_httponly integer not null, samesite integer not null, top_frame_site_key text not null default '')`;
const INSERT = 'insert into cookies (host_key, name, value, encrypted_value, path, expires_utc, is_secure, is_httponly, samesite) values (?, ?, ?, ?, ?, ?, ?, ?, ?)';
function chromiumDb(file: string, version: number | string, build: (db: Database) => void, metaValueType = 'text') {
  const db = new Database(file);
  db.run(`create table meta (key text primary key, value ${metaValueType} not null)`);
  db.run('insert into meta values (?, ?)', ['version', version]);
  build(db);
  db.close();
}

describe('cookieScope', () => {
  it('keeps a host-only cookie host-only', () => {
    expect(cookieScope('example.test', '/', true)).toEqual({ url: 'https://example.test/', domain: undefined });
  });
  it("preserves a domain cookie's leading dot", () => {
    expect(cookieScope('.example.test', '/app', true)).toEqual({ url: 'https://example.test/app', domain: '.example.test' });
  });
  it('matches the scheme to the secure flag', () => {
    expect(cookieScope('example.test', '/', false).url).toBe('http://example.test/');
  });
  it('brackets bare IPv6 hosts without duplicating existing brackets', () => {
    expect(cookieScope('::1', '/', false)).toEqual({ url: 'http://[::1]/', domain: undefined });
    expect(cookieScope('[::1]', '/app', true)).toEqual({ url: 'https://[::1]/app', domain: undefined });
  });
});

describe('readChromiumCookieDatabase', () => {
  it('decrypts Windows v10 AES-GCM records and rejects app-bound v20 records', async () => {
    const key = Buffer.from('0123456789abcdef0123456789abcdef'), host = '.example.test', bound = boundValue(host, 'windows value');
    expect(await decryptChromiumValue(io, encryptWindowsV10(bound, key), { gcmV10: key }, host, 24, 'win32')).toBe('windows value');
    expect(await decryptChromiumValue(io, Buffer.from('v20app-bound'), { gcmV10: key }, host, 24, 'win32')).toBeNull();
    expect(await decryptChromiumValue(io, encryptWindowsV10(bound, Buffer.alloc(32, 1)), { gcmV10: key }, host, 24, 'win32')).toBeNull();
  });

  it('reports the missing key when no cookies can be read, while preserving partial imports', async () => {
    const filename = `${temp('t3code-missing-key-')}/Cookies`, key = Buffer.from('0123456789abcdef');
    chromiumDb(filename, 23, db => { db.run(COOKIES_TABLE); db.run("insert into cookies values ('v11.example', 'session', '', ?, '/', 0, 1, 1, 1, '')", [encryptChromium('v11', 'secret', key)]); });
    // The clone reads the macOS key; a Keychain that cannot be asked is keychainUnavailable (the reference: Linux's missing helper).
    const unavailable = bunImportIO({ keychainPassword: async () => { throw new KeychainUnavailableError(); } });
    const error = await readChromiumCookies(unavailable, { cookieDatabasePath: filename, platform: 'darwin', linuxSecretApplication: undefined, keychainService: 'Chrome Safe Storage', keychainAccount: 'Chrome' }).catch(cause => cause);
    expect(error.reason).toBe('keychainUnavailable');

    const db = new Database(filename);
    db.run("insert into cookies values ('v10.example', 'readable', '', ?, '/', 0, 1, 1, 1, '')", [encryptV10('kept', key)]);
    db.close();
    const keys = { cbcV10: key, cbcV11Error: new ChromiumKeyError('keychainUnavailable') };
    const partial = await readChromiumCookieDatabase(io, filename, keys, 'linux');
    expect(partial.cookies.map(cookie => cookie.value)).toEqual(['kept']);
    expect(partial.undecryptable).toBe(1);

    // A partitioned-only jar does not need its key: it is skipped separately.
    const again = new Database(filename);
    again.run("delete from cookies where name = 'readable'");
    again.run("update cookies set top_frame_site_key = 'https://top.example'");
    again.close();
    const partitioned = await readChromiumCookieDatabase(io, filename, keys, 'linux');
    expect(partitioned.cookies).toEqual([]);
    expect(partitioned.undecryptable).toBe(1);
  });

  it('reads plaintext, encrypted, and genuinely empty cookie values', async () => {
    const filename = `${temp('t3code-chromium-cookies-')}/Cookies`, key = Buffer.from('0123456789abcdef');
    chromiumDb(filename, 23, db => {
      db.run(COOKIES_TABLE);
      db.run(INSERT, ['plain.example', 'plain', 'stored plaintext', new Uint8Array(), '/', 0, 0, 0, -1]);
      db.run(INSERT, ['secure.example', 'encrypted', '', encryptV10('stored encrypted', key), '/', 0, 1, 1, 2]);
      db.run(INSERT, ['empty.example', 'empty', '', new Uint8Array(), '/', 0, 0, 0, 0]);
    });
    const result = await readChromiumCookieDatabase(io, filename, { cbcV10: key }, 'darwin');
    expect(result.undecryptable).toBe(0);
    expect(result.cookies.map(({ name, value }) => ({ name, value }))).toEqual([
      { name: 'plain', value: 'stored plaintext' }, { name: 'encrypted', value: 'stored encrypted' }, { name: 'empty', value: '' },
    ]);
  });

  it('enforces domain binding only for schema 24 and newer', async () => {
    const filename = `${temp('t3code-chromium-cookies-')}/Cookies`, key = Buffer.from('0123456789abcdef');
    chromiumDb(filename, 24, db => {
      db.run(COOKIES_TABLE);
      db.run(INSERT, ['bound.example', 'valid', '', encryptV10(boundValue('bound.example', 'kept'), key), '/', 0, 1, 0, 0]);
      db.run(INSERT, ['wrong.example', 'mismatch', '', encryptV10(boundValue('another.example', 'drop'), key), '/', 0, 1, 0, 0]);
      db.run(INSERT, ['short.example', 'short', '', encryptV10('short value', key), '/', 0, 1, 0, 0]);
    });
    const result = await readChromiumCookieDatabase(io, filename, { cbcV10: key }, 'darwin');
    expect(result.cookies.map(({ name, value }) => ({ name, value }))).toEqual([{ name: 'valid', value: 'kept' }]);
    expect(result.undecryptable).toBe(2);
    expect(result.undecryptableHosts).toEqual(['wrong.example', 'short.example']);
  });

  it('decrypts mixed v10 and v11 cookies with their respective keys', async () => {
    const filename = `${temp('t3code-chromium-cookies-')}/Cookies`, cbcV10 = Buffer.from('0123456789abcdef'), cbcV11 = Buffer.from('fedcba9876543210');
    chromiumDb(filename, '24', db => {
      db.run(COOKIES_TABLE);
      db.run(INSERT, ['v10.example', 'v10-cookie', '', encryptChromium('v10', boundValue('v10.example', 'v10 value'), cbcV10), '/', 0, 1, 0, 0]);
      db.run(INSERT, ['v11.example', 'v11-cookie', '', encryptChromium('v11', boundValue('v11.example', 'v11 value'), cbcV11), '/', 0, 1, 0, 0]);
    });
    const complete = await readChromiumCookieDatabase(io, filename, { cbcV10, cbcV11 }, 'linux');
    expect(complete.cookies.map(({ name, value }) => ({ name, value }))).toEqual([{ name: 'v10-cookie', value: 'v10 value' }, { name: 'v11-cookie', value: 'v11 value' }]);
    expect(complete.undecryptable).toBe(0);
    const v10Only = await readChromiumCookieDatabase(io, filename, { cbcV10 }, 'linux');
    expect(v10Only.cookies.map(({ name, value }) => ({ name, value }))).toEqual([{ name: 'v10-cookie', value: 'v10 value' }]);
    expect(v10Only.undecryptable).toBe(1);
    expect(v10Only.undecryptableHosts).toEqual(['v11.example']);
  });

  it('recovers records written with the empty-passphrase key', async () => {
    const filename = `${temp('t3code-chromium-cookies-')}/Cookies`, cbcV10 = Buffer.from('0123456789abcdef'), cbcV11 = Buffer.from('fedcba9876543210');
    const cbcEmpty = NodeCrypto.pbkdf2Sync('', 'saltysalt', 1, 16, 'sha1');
    chromiumDb(filename, '23', db => {
      db.run(COOKIES_TABLE);
      db.run(INSERT, ['ev10.example', 'empty-v10', '', encryptChromium('v10', 'empty v10 value', cbcEmpty), '/', 0, 1, 0, 0]);
      db.run(INSERT, ['ev11.example', 'empty-v11', '', encryptChromium('v11', 'empty v11 value', cbcEmpty), '/', 0, 1, 0, 0]);
    });
    const recovered = await readChromiumCookieDatabase(io, filename, { cbcV10, cbcV11, cbcEmpty }, 'linux');
    expect(recovered.cookies.map(({ name, value }) => ({ name, value }))).toEqual([{ name: 'empty-v10', value: 'empty v10 value' }, { name: 'empty-v11', value: 'empty v11 value' }]);
    expect(recovered.undecryptable).toBe(0);
    // Matching Chromium: a record whose own key is missing entirely is not retried with the empty key.
    const noV11 = await readChromiumCookieDatabase(io, filename, { cbcV10, cbcEmpty }, 'linux');
    expect(noV11.cookies.map(({ name }) => name)).toEqual(['empty-v10']);
    expect(noV11.undecryptableHosts).toEqual(['ev11.example']);
  });

  it('preserves arbitrary long encrypted values from pre-24 schemas', async () => {
    const filename = `${temp('t3code-chromium-cookies-')}/Cookies`, key = Buffer.from('0123456789abcdef'), value = 'x'.repeat(32) + ' legacy value';
    chromiumDb(filename, 23, db => { db.run(COOKIES_TABLE); db.run(INSERT, ['legacy.example', 'legacy', '', encryptV10(value, key), '/', 0, 0, 0, 0]); });
    const result = await readChromiumCookieDatabase(io, filename, { cbcV10: key }, 'darwin');
    expect(result.cookies[0]?.value).toBe(value);
    expect(result.undecryptable).toBe(0);
  });

  it('rejects a malformed text schema version', async () => {
    const filename = `${temp('t3code-chromium-cookies-')}/Cookies`;
    chromiumDb(filename, 'not-a-version', () => undefined);
    const error = await readChromiumCookieDatabase(io, filename, { cbcV10: Buffer.from('0123456789abcdef') }, 'darwin').catch(cause => cause);
    expect(error._tag).toBe('SchemaError');
  });

  it('treats unversioned encrypted values as legacy plaintext on macOS and Linux', async () => {
    const filename = `${temp('t3code-chromium-cookies-')}/Cookies`, key = Buffer.from('0123456789abcdef');
    chromiumDb(filename, 23, db => { db.run(COOKIES_TABLE); db.run(INSERT, ['legacy.example', 'legacy', '', Buffer.from('legacy cleartext'), '/', 0, 0, 0, 0]); }, 'integer');
    const mac = await readChromiumCookieDatabase(io, filename, { cbcV10: key }, 'darwin');
    const linux = await readChromiumCookieDatabase(io, filename, { cbcV10: key }, 'linux');
    expect(mac.cookies[0]?.value).toBe('legacy cleartext');
    expect(mac.undecryptable).toBe(0);
    expect(linux.cookies[0]?.value).toBe('legacy cleartext');
    expect(linux.undecryptable).toBe(0);
  });

  it('skips partitioned cookies without breaking pre-CHIPS schemas', async () => {
    const directory = temp('t3code-chromium-cookies-'), legacyFilename = `${directory}/LegacyCookies`, chipsFilename = `${directory}/ChipsCookies`, key = Buffer.from('0123456789abcdef');
    chromiumDb(legacyFilename, 14, db => {
      db.run('create table cookies (host_key text not null, name text not null, value text not null, encrypted_value blob not null, path text not null, expires_utc integer not null, is_secure integer not null, is_httponly integer not null, samesite integer not null)');
      db.run("insert into cookies values ('legacy.example', 'legacy', 'kept', ?, '/', 0, 0, 0, 0)", [new Uint8Array()]);
    }, 'integer');
    chromiumDb(chipsFilename, 23, db => {
      db.run('create table cookies (host_key text not null, name text not null, value text not null, encrypted_value blob not null, path text not null, expires_utc integer not null, is_secure integer not null, is_httponly integer not null, samesite integer not null, top_frame_site_key text not null)');
      db.run("insert into cookies values ('plain.example', 'plain', 'kept', ?, '/', 0, 0, 0, 0, '')", [new Uint8Array()]);
      db.run("insert into cookies values ('partitioned.example', 'partitioned', 'must skip', ?, '/', 0, 1, 0, 0, 'https://top.example')", [new Uint8Array()]);
    }, 'integer');
    const legacy = await readChromiumCookieDatabase(io, legacyFilename, { cbcV10: key }, 'darwin');
    const chips = await readChromiumCookieDatabase(io, chipsFilename, { cbcV10: key }, 'darwin');
    expect(legacy.cookies.map(({ name }) => name)).toEqual(['legacy']);
    expect(legacy.undecryptable).toBe(0);
    expect(chips.cookies.map(({ name }) => name)).toEqual(['plain']);
    expect(chips.undecryptable).toBe(1);
    expect(chips.undecryptableHosts).toEqual(['partitioned.example']);
  });
});

describe('macOS Chromium secrets', () => {
  const request = { platform: 'darwin', keychainService: 'Chrome Safe Storage', keychainAccount: 'Chrome', linuxSecretApplication: undefined } as const;
  it('derives the cookie key from the keychain secret', async () => {
    const keys = await resolveChromiumKeys(bunImportIO({ keychainPassword: async () => 'macos-secret' }), request);
    expect(Buffer.from(keys.cbcV10!).toString('hex')).toBe('3df7306fb1eac353289565a2f6b64f74');
  });
  it('reports a missing keychain entry', async () => {
    const error = await resolveChromiumKeys(bunImportIO({ keychainPassword: async () => null }), request).catch(cause => cause);
    expect(error.reason).toBe('keychainItemMissing');
  });
  it('preserves a denied keychain approval', async () => {
    const denied = new Error('User denied access');
    const error = await resolveChromiumKeys(bunImportIO({ keychainPassword: async () => { throw denied; } }), request).catch(cause => cause);
    expect(error.reason).toBe('needsKeychainApproval');
    expect(error.cause).toBe(denied);
  });
});

it('reports an unavailable keychain when the macOS binding cannot load', async () => {
  const error = await resolveChromiumKeys(bunImportIO({ keychainPassword: async () => { throw new KeychainUnavailableError('Cannot find native binding'); } }),
    { platform: 'darwin', keychainService: 'Chrome Safe Storage', keychainAccount: 'Chrome', linuxSecretApplication: undefined }).catch(cause => cause);
  expect(error).toBeInstanceOf(ChromiumKeyError);
  expect(error.reason).toBe('keychainUnavailable');
  expect(error.cause).toBeInstanceOf(Error);
});

/** Builds a `cookies.sqlite` with Firefox's real `moz_cookies` shape. */
type FirefoxRow = { host: string; name: string; value: string; path: string; expiry: number; isSecure: number; isHttpOnly: number; sameSite: number | null; rawSameSite?: number; originAttributes?: string };
function writeFirefoxCookieDatabase(rows: readonly FirefoxRow[], schemaVersion = 15): string {
  const file = `${temp('t3code-firefox-test-')}/cookies.sqlite`, db = new Database(file), hasRawSameSite = schemaVersion >= 10 && schemaVersion <= 14;
  db.run(`pragma user_version = ${schemaVersion}`);
  db.run(`create table moz_cookies (id integer primary key, host text, name text, value text, path text, expiry integer, isSecure integer, isHttpOnly integer, sameSite integer,
    ${hasRawSameSite ? 'rawSameSite integer,' : ''} originAttributes text not null default '')`);
  for (const row of rows) {
    db.run(`insert into moz_cookies (host, name, value, path, expiry, isSecure, isHttpOnly, sameSite, ${hasRawSameSite ? 'rawSameSite,' : ''} originAttributes) values (?, ?, ?, ?, ?, ?, ?, ?, ${hasRawSameSite ? '?,' : ''} ?)`,
      [row.host, row.name, row.value, row.path, row.expiry, row.isSecure, row.isHttpOnly, row.sameSite, ...(hasRawSameSite ? [row.rawSameSite ?? row.sameSite] : []), row.originAttributes ?? '']);
  }
  db.close();
  return file;
}

describe('readFirefoxCookies', () => {
  it('converts millisecond expiries from schema 16 and newer', async () => {
    const row = { host: 'example.test', name: 'c', value: 'v', path: '/', expiry: 1_800_000_000_000, isSecure: 0, isHttpOnly: 0, sameSite: 0 };
    expect((await readFirefoxCookies(io, writeFirefoxCookieDatabase([row], 16)))[0]?.expirationDate).toBe(1_800_000_000);
    expect((await readFirefoxCookies(io, writeFirefoxCookieDatabase([{ ...row, expiry: 1_800_000_000 }], 15)))[0]?.expirationDate).toBe(1_800_000_000);
  });

  it('maps moz_cookies onto the shape Electron accepts', async () => {
    const file = writeFirefoxCookieDatabase([
      { host: '.github.com', name: 'session', value: 'abc', path: '/', expiry: 1_800_000_000, isSecure: 1, isHttpOnly: 1, sameSite: 1 },
      { host: 'example.test', name: 'plain', value: 'v', path: '/app', expiry: 0, isSecure: 0, isHttpOnly: 0, sameSite: 0 },
    ]);
    expect(await readFirefoxCookies(io, file)).toEqual([
      { url: 'https://github.com/', name: 'session', value: 'abc', domain: '.github.com', path: '/', secure: true, httpOnly: true, expirationDate: 1_800_000_000, sameSite: 'lax' },
      { url: 'http://example.test/app', name: 'plain', value: 'v', domain: undefined, path: '/app', secure: false, httpOnly: false, expirationDate: undefined, sameSite: 'no_restriction' },
    ]);
  });

  it('keeps an unset SameSite unspecified instead of widening it to none', async () => {
    const row = { host: 'example.test', name: 'c', value: 'v', path: '/', expiry: 0, isSecure: 0, isHttpOnly: 0 };
    const cookies = await readFirefoxCookies(io, writeFirefoxCookieDatabase([{ ...row, name: 'unset', sameSite: 256 }, { ...row, name: 'none', sameSite: 0 }]));
    expect(cookies.map(({ name, sameSite }) => ({ name, sameSite }))).toEqual([{ name: 'unset', sameSite: 'unspecified' }, { name: 'none', sameSite: 'no_restriction' }]);
  });

  it('imports rows whose SameSite was never written', async () => {
    const row = { host: 'example.test', name: 'c', value: 'v', path: '/', expiry: 0, isSecure: 0, isHttpOnly: 0 };
    const cookies = await readFirefoxCookies(io, writeFirefoxCookieDatabase([{ ...row, name: 'legacy', sameSite: null }, { ...row, name: 'strict', sameSite: 2 }], 9));
    expect(cookies.map(({ name, sameSite }) => ({ name, sameSite }))).toEqual([{ name: 'legacy', sameSite: 'unspecified' }, { name: 'strict', sameSite: 'strict' }]);
  });

  it('applies the schema-15 rawSameSite rule to older databases', async () => {
    const row = { host: 'example.test', name: 'c', value: 'v', path: '/', expiry: 0, isSecure: 0, isHttpOnly: 0 };
    const cookies = await readFirefoxCookies(io, writeFirefoxCookieDatabase([
      { ...row, name: 'defaulted', sameSite: 1, rawSameSite: 0 }, { ...row, name: 'declared', sameSite: 1, rawSameSite: 1 }, { ...row, name: 'none', sameSite: 0, rawSameSite: 0 },
    ], 14));
    expect(cookies.map(({ name, sameSite }) => ({ name, sameSite }))).toEqual([
      { name: 'defaulted', sameSite: 'unspecified' }, { name: 'declared', sameSite: 'lax' }, { name: 'none', sameSite: 'no_restriction' },
    ]);
  });

  it('imports only the default container', async () => {
    const base = { host: 'mail.test', name: 'session', path: '/', expiry: 1_800_000_000, isSecure: 1, isHttpOnly: 0, sameSite: 1 };
    const file = writeFirefoxCookieDatabase([
      { ...base, value: 'default-container' },
      { ...base, value: 'work-container', originAttributes: '^userContextId=2' },
      { ...base, name: 'private', value: 'private-window', originAttributes: '^privateBrowsingId=1' },
    ]);
    expect((await readFirefoxCookies(io, file)).map(cookie => cookie.value)).toEqual(['default-container']);
  });

  it('reads without mutating the source database', async () => {
    const file = writeFirefoxCookieDatabase([{ host: 'a.test', name: 'n', value: 'v', path: '/', expiry: 1_800_000_000, isSecure: 1, isHttpOnly: 0, sameSite: 2 }]);
    const before = NodeFs.statSync(file);
    await readFirefoxCookies(io, file);
    const after = NodeFs.statSync(file);
    expect(after.mtimeMs).toEqual(before.mtimeMs);
    expect(after.size).toBe(before.size);
  });
});

describe('parseFirefoxProfiles', () => {
  const parse = (ini: string, root = '/home/user/.mozilla/firefox') => parseFirefoxProfiles(ini, posixPath, root);
  it('reads named profiles and ignores Install sections', () => {
    expect(parse(['[Install4F96D1932A9F858E]', 'Default=Profiles/abcd1234.default-release', 'Locked=1', '', '[Profile0]', 'Name=default-release', 'IsRelative=1',
      'Path=Profiles/abcd1234.default-release', '', '[Profile1]', 'Name=Work', 'IsRelative=0', 'Path=/Volumes/External/firefox-work', '', '[General]', 'StartWithLastProfile=1'].join('\n')))
      .toEqual([{ directory: 'Profiles/abcd1234.default-release', name: 'default-release' }, { directory: '/Volumes/External/firefox-work', name: 'Work' }]);
  });
  it('falls back to the path when a profile has no name', () => {
    expect(parse(['[Profile0]', 'Path=Profiles/x.default'].join('\n'))).toEqual([{ directory: 'Profiles/x.default', name: 'Profiles/x.default' }]);
  });
  it.each([
    { platform: 'Linux', root: '/home/user/.mozilla/firefox' },
    { platform: 'macOS', root: '/Users/user/Library/Application Support/Firefox' },
  ])('validates relative and absolute $platform profile paths', ({ root }) => {
    expect(parse(['[Profile0]', 'Name=Relative', 'IsRelative=1', 'Path=Profiles/relative.default', '[Profile1]', 'Name=Custom', 'IsRelative=0', 'Path=/mnt/custom/firefox-profile',
      '[Profile2]', 'IsRelative=1', 'Path=../../escape', '[Profile3]', 'IsRelative=1', 'Path=/absolute-marked-relative', '[Profile4]', 'IsRelative=0', 'Path=relative-marked-absolute',
      '[Profile5]', 'IsRelative=1', 'Path=Profiles/nul\u0000escape'].join('\n'), root))
      .toEqual([{ directory: 'Profiles/relative.default', name: 'Relative' }, { directory: '/mnt/custom/firefox-profile', name: 'Custom' }]);
  });
});

// Clone row: the app's JS runtime freezes Error.prototype, so an error class that assigned `this.name` threw a TypeError
// there (found by the live drive: every browser read as running), while Bun passed. Each import error constructs under
// a frozen Error.prototype, in a child Bun.
it('constructs every import error under a frozen Error.prototype, as the app runtime has it', () => {
  const here = import.meta.dir;
  const script = `Object.freeze(Error.prototype);
    const io = await import('${here}/browser-import-io.ts'), readers = await import('${here}/browser-import-readers.ts');
    const safari = await import('${here}/browser-import-safari.ts'), service = await import('${here}/browser-import-service.ts'), profiles = await import('${here}/browser-profiles.ts');
    const made = [new io.ImportFsError('ENOENT'), new io.ImportSqlError('x'), new io.KeychainUnavailableError(), new readers.ChromiumKeyError('readFailed'),
      new readers.ChromiumCookieReadError('readFailed', '/p'), new readers.SchemaError('x'), new readers.FirefoxCookieReadError('/p', null),
      new safari.SafariCookieReadError('readFailed'), new service.BrowserImportFailedError('chrome', 'readFailed'), new profiles.BrowserSettingsReadError()];
    console.log(made.map(error => error.name).join(','));`;
  const run = Bun.spawnSync([process.execPath, '-e', script]);
  expect(run.stderr.toString()).toBe('');
  expect(run.stdout.toString().trim()).toBe('ImportFsError,SqlError,KeychainUnavailableError,ChromiumKeyError,ChromiumCookieReadError,SchemaError,FirefoxCookieReadError,SafariCookieReadError,BrowserImportFailedError,BrowserSettingsReadError');
});
