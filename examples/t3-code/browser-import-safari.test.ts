// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names: apps/desktop/src/preview/BrowserImport/
// SafariCookies.test.ts (23 of 23) and SafariPermission.test.ts (2 of 2). The reference's EPERM-tagged PlatformError is
// the clone's ImportFsError('EPERM') (the module reads errno itself, T3BrowserImportIO.swift); FileSystem.layerNoop's
// failing open/readFile is an ImportIO override. The jar is hand-built byte by byte as the reference builds it.
import { afterEach, describe, expect, it } from 'bun:test';
import * as NodeFs from 'node:fs';
import { ImportFsError } from './browser-import-io';
import { isPermissionDenied, parseBinaryCookies, readSafariCookies, safariAccessDenied, safariAccessGranted, safariPermissionCheck, SafariCookieReadError } from './browser-import-safari';
import { sourcePathContext } from './browser-import-service';
import { bunImportIO, tempDirectory } from './browser-import-fixture';

const io = bunImportIO();
const scratch: { cleanup(): void }[] = [];
afterEach(() => { for (const entry of scratch.splice(0)) entry.cleanup(); });
const temp = (prefix: string) => { const directory = tempDirectory(prefix); scratch.push(directory); return directory.path; };
const APPLE_EPOCH_OFFSET_SECONDS = 978_307_200;

interface FixtureCookie { readonly domain: string; readonly name: string; readonly path: string; readonly value: string; readonly flags: number; readonly expiry: number }
/** Encodes one cookie exactly as Safari lays it out. */
function encodeCookie(cookie: FixtureCookie): Buffer {
  const strings = [cookie.domain, cookie.name, cookie.path, cookie.value], offsets: number[] = [];
  let cursor = 56;
  for (const value of strings) { offsets.push(cursor); cursor += Buffer.byteLength(value) + 1; }
  const buffer = Buffer.alloc(cursor);
  buffer.writeUInt32LE(cursor, 0); buffer.writeUInt32LE(0, 4); buffer.writeUInt32LE(cookie.flags, 8); buffer.writeUInt32LE(0, 12);
  offsets.forEach((offset, index) => buffer.writeUInt32LE(offset, 16 + index * 4));
  buffer.writeUInt32LE(0, 32); buffer.writeUInt32LE(0, 36); buffer.writeDoubleLE(cookie.expiry, 40); buffer.writeDoubleLE(0, 48);
  strings.forEach((value, index) => buffer.write(value, offsets[index]!, 'utf8'));
  return buffer;
}
/** Builds a single-page `Cookies.binarycookies` file. */
function encodeBinaryCookies(cookies: readonly FixtureCookie[]): Buffer {
  const encoded = cookies.map(encodeCookie), offsets: number[] = [];
  let cursor = 12 + encoded.length * 4;
  for (const cookie of encoded) { offsets.push(cursor); cursor += cookie.length; }
  const page = Buffer.alloc(cursor);
  page.writeUInt32BE(0x0000_0100, 0); page.writeUInt32LE(encoded.length, 4);
  offsets.forEach((offset, index) => page.writeUInt32LE(offset, 8 + index * 4));
  encoded.forEach((cookie, index) => cookie.copy(page, offsets[index]!));
  const header = Buffer.alloc(12);
  header.write('cook', 0, 'latin1'); header.writeUInt32BE(1, 4); header.writeUInt32BE(page.length, 8);
  return Buffer.concat([header, page]);
}
const eperm = () => new ImportFsError('EPERM', 'operation not permitted');

describe('parseBinaryCookies', () => {
  it("reads Safari's format and rebases its 2001 epoch", () => {
    const file = encodeBinaryCookies([
      { domain: '.apple.com', name: 'session', path: '/', value: 'abc', flags: 0x1 | 0x4, expiry: 800_000_000 },
      { domain: 'example.test', name: 'plain', path: '/app', value: 'v', flags: 0, expiry: 0 },
    ]);
    expect(parseBinaryCookies(file)).toEqual([
      { url: 'https://apple.com/', name: 'session', value: 'abc', domain: '.apple.com', path: '/', secure: true, httpOnly: true, expirationDate: 800_000_000 + APPLE_EPOCH_OFFSET_SECONDS, sameSite: 'lax' },
      { url: 'http://example.test/app', name: 'plain', value: 'v', domain: undefined, path: '/app', secure: false, httpOnly: false, expirationDate: undefined, sameSite: 'lax' },
    ]);
  });
  it('keeps __Host- cookies host-only so Electron accepts them', () => {
    expect(parseBinaryCookies(encodeBinaryCookies([{ domain: 'example.test', name: '__Host-id', path: '/', value: 'v', flags: 0x1, expiry: 0 }]))[0])
      .toMatchObject({ url: 'https://example.test/', name: '__Host-id', domain: undefined });
  });
  it('brackets IPv6 hosts in the cookie URL', () => {
    expect(parseBinaryCookies(encodeBinaryCookies([{ domain: '::1', name: 'local', path: '/', value: 'v', flags: 0, expiry: 0 }]))[0]).toMatchObject({ url: 'http://[::1]/', domain: undefined });
  });
  const twoPages = (firstLength?: (first: Buffer, second: Buffer) => number) => {
    const firstPage = encodeBinaryCookies([{ domain: 'a.test', name: 'one', path: '/', value: '1', flags: 0, expiry: 1 }]).subarray(12);
    const secondPage = encodeBinaryCookies([{ domain: 'b.test', name: 'two', path: '/', value: '2', flags: 0, expiry: 1 }]).subarray(12);
    const header = Buffer.alloc(16);
    header.write('cook', 0, 'latin1'); header.writeUInt32BE(2, 4);
    header.writeUInt32BE(firstLength ? firstLength(firstPage, secondPage) : firstPage.length, 8); header.writeUInt32BE(secondPage.length, 12);
    return Buffer.concat([header, firstPage, secondPage]);
  };
  it('reads cookies spread across multiple pages', () => {
    expect(parseBinaryCookies(twoPages()).map(cookie => cookie.name)).toEqual(['one', 'two']);
  });
  it('rejects a page that runs past the end of the file', () => {
    expect(() => parseBinaryCookies(twoPages((first, second) => first.length + second.length + 32))).toThrow(SafariCookieReadError);
  });
  const single = () => encodeBinaryCookies([{ domain: 'a.test', name: 'n', path: '/', value: 'v', expiry: 1_000, flags: 0 }]);
  it('rejects a record whose declared size runs past its page', () => {
    const valid = single(), recordStart = 12 + valid.readUInt32LE(12 + 8), corrupt = Buffer.from(valid);
    corrupt.writeUInt32LE(0xffff, recordStart);
    expect(() => parseBinaryCookies(corrupt)).toThrow(SafariCookieReadError);
  });
  it('rejects records truncated inside the 56-byte header', () => {
    const valid = single(), recordStart = 12 + valid.readUInt32LE(12 + 8);
    for (let size = 48; size < 56; size += 1) {
      const corrupt = Buffer.from(valid);
      corrupt.writeUInt32LE(size, recordStart);
      expect(() => parseBinaryCookies(corrupt)).toThrow(SafariCookieReadError);
    }
  });
  it('rejects record offsets that point into the page header or an earlier record', () => {
    const valid = encodeBinaryCookies([{ domain: 'a.test', name: 'n', path: '/', value: 'v', expiry: 1_000, flags: 0 }, { domain: 'b.test', name: 'm', path: '/', value: 'w', expiry: 1_000, flags: 0 }]);
    const firstRecord = valid.readUInt32LE(12 + 8);
    const intoTable = Buffer.from(valid);
    intoTable.writeUInt32LE(4, 12 + 12);
    expect(() => parseBinaryCookies(intoTable)).toThrow(SafariCookieReadError);
    const overlapping = Buffer.from(valid);
    overlapping.writeUInt32LE(firstRecord, 12 + 12);
    expect(() => parseBinaryCookies(overlapping)).toThrow(SafariCookieReadError);
    expect(parseBinaryCookies(valid)).toHaveLength(2);
  });
  it('rejects string offsets that point into the record header', () => {
    const valid = single(), recordStart = 12 + valid.readUInt32LE(12 + 8);
    for (const offsetField of [16, 20, 24, 28]) {
      const corrupt = Buffer.from(valid);
      corrupt.writeUInt32LE(55, recordStart + offsetField);
      expect(() => parseBinaryCookies(corrupt)).toThrow(SafariCookieReadError);
    }
  });
  it('accepts the checksum and property-list trailer Safari writes', () => {
    const file = encodeBinaryCookies([{ domain: 'a.test', name: 'c', path: '/', value: 'v', flags: 0, expiry: 0 }]);
    const checksum = Buffer.alloc(8), plist = Buffer.from('bplist00 stub'), plistLength = Buffer.alloc(4);
    plistLength.writeUInt32BE(plist.length, 0);
    expect(parseBinaryCookies(Buffer.concat([file, checksum]))).toHaveLength(1);
    expect(parseBinaryCookies(Buffer.concat([file, checksum, plistLength, plist]))).toHaveLength(1);
  });
  it('rejects a jar whose page table stops short of its contents', () => {
    const first = encodeBinaryCookies([{ domain: 'a.test', name: 'c', path: '/', value: 'v', flags: 0, expiry: 0 }]);
    const extraPage = encodeBinaryCookies([{ domain: 'b.test', name: 'd', path: '/', value: 'w', flags: 0, expiry: 0 }]).subarray(12);
    expect(() => parseBinaryCookies(Buffer.concat([first, extraPage]))).toThrow(SafariCookieReadError);
    const badLength = Buffer.alloc(4);
    badLength.writeUInt32BE(99, 0);
    expect(() => parseBinaryCookies(Buffer.concat([first, Buffer.alloc(8), badLength, Buffer.from('x')]))).toThrow(SafariCookieReadError);
  });
  it('rejects a file that is not binarycookies', () => {
    expect(() => parseBinaryCookies(Buffer.from('not a cookie jar'))).toThrow(SafariCookieReadError);
  });
});

describe('readSafariCookies', () => {
  it('adds the cookie path and parser cause to malformed jar failures', async () => {
    const jar = `${temp('t3code-safari-')}/Cookies.binarycookies`;
    NodeFs.writeFileSync(jar, 'not a cookie jar');
    const error = await readSafariCookies(io, jar).catch(cause => cause);
    expect(error.reason).toBe('readFailed');
    expect(error.cookieDatabasePath).toBe(jar);
    expect(error.cause).toBeInstanceOf(SafariCookieReadError);
  });
  it('reports a TCC denial as a permission the user can grant', async () => {
    const error = await readSafariCookies(bunImportIO({ readFile: async () => { throw eperm(); } }), '/protected/Cookies.binarycookies').catch(cause => cause);
    expect(error.reason).toBe('needsFullDiskAccess');
  });
  it('reports an ordinary permission failure as a plain read failure', async () => {
    const jar = `${temp('t3code-safari-')}/Cookies.binarycookies`;
    NodeFs.writeFileSync(jar, new Uint8Array([0x63, 0x6f, 0x6f, 0x6b]));
    NodeFs.chmodSync(jar, 0o000);
    const error = await readSafariCookies(io, jar).catch(cause => cause);
    expect(error.reason).toBe('readFailed');
  });
  it('reports a missing jar as a plain read failure', async () => {
    const error = await readSafariCookies(io, `${temp('t3code-safari-')}/absent.binarycookies`).catch(cause => cause);
    expect(error.reason).toBe('readFailed');
  });
});

describe('safariAccessDenied', () => {
  it("reports TCC's EPERM as a missing Full Disk Access grant", async () => {
    expect(await safariAccessDenied(bunImportIO({ open: async () => { throw eperm(); } }), '/protected/Cookies.binarycookies')).toBe(true);
  });
  it('does not read a readable jar, or any other failure, as denied', async () => {
    const directory = temp('t3code-safari-'), jar = `${directory}/Cookies.binarycookies`;
    NodeFs.writeFileSync(jar, new Uint8Array([0x63, 0x6f, 0x6f, 0x6b]));
    expect(await safariAccessDenied(io, jar)).toBe(false);
    expect(await safariAccessDenied(io, `${directory}/absent.binarycookies`)).toBe(false);
  });
});

describe('isPermissionDenied', () => {
  it('treats a TCC EPERM denial as permission denied', () => {
    expect(isPermissionDenied(new ImportFsError('EPERM'))).toBe(true);
  });
  it('does not send an ordinary EACCES failure to the Full Disk Access grant', () => {
    expect(isPermissionDenied(new ImportFsError('EACCES'))).toBe(false);
  });
  it('does not treat an unrelated failure as permission denied', () => {
    expect(isPermissionDenied(new ImportFsError('EIO'))).toBe(false);
  });
});

describe('safariAccessGranted', () => {
  it('only reports a successful read-only open as granted', async () => {
    const jar = `${temp('t3-safari-permission-')}/Cookies.binarycookies`;
    expect(await safariAccessGranted(io, jar)).toBe(false);
    NodeFs.writeFileSync(jar, 'no cookie parsing needed');
    expect(await safariAccessGranted(io, jar)).toBe(true);
    NodeFs.rmSync(jar);
    expect(await safariAccessGranted(io, jar)).toBe(false);
  });
  it('does not mistake TCC denial for a grant', async () => {
    expect(await safariAccessGranted(bunImportIO({ open: async () => { throw eperm(); } }), '/protected/Cookies.binarycookies')).toBe(false);
  });
});

it('detects Safari access becoming available without reading or importing cookies', async () => {
  const home = temp('t3-safari-access-'), directory = `${home}/Library/Containers/com.apple.Safari/Data/Library/Cookies`;
  NodeFs.mkdirSync(directory, { recursive: true });
  const jar = `${directory}/Cookies.binarycookies`;
  NodeFs.writeFileSync(jar, 'not a valid cookie database');
  let allowed = false;
  const guarded = bunImportIO({ open: async path => { if (!allowed) throw eperm(); await io.open(path); }, readFile: async () => { throw new Error('must not read'); } });
  const check = safariPermissionCheck(guarded, sourcePathContext(home));
  expect(await check()).toBe(false);
  allowed = true;
  expect(await check()).toBe(true);
  NodeFs.rmSync(jar);
  expect(await check()).toBe(false);
});

it('recognizes access when cookies exist only in a named Safari profile', async () => {
  const home = temp('t3-safari-named-access-'), check = safariPermissionCheck(io, sourcePathContext(home));
  expect(await check()).toBe(false);
  const directory = `${home}/Library/Containers/com.apple.Safari/Data/Library/WebKit/WebsiteDataStore/12345678-1234-1234-1234-123456789abc/Cookies`;
  NodeFs.mkdirSync(directory, { recursive: true });
  NodeFs.writeFileSync(`${directory}/Cookies.binarycookies`, 'not parsed');
  expect(await check()).toBe(true);
});
