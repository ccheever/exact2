// browser-surface part 4 (profiles): reading Safari's cookies (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/desktop/src/preview/BrowserImport/{SafariCookies,SafariPermission}.ts), over the clone's ImportIO.
//
// Safari does not encrypt its cookies; it keeps them in `Cookies.binarycookies` inside its container, a path only apps
// with Full Disk Access may read. The protection is TCC: `stat` finds the jar, opening it fails with EPERM. The format is
// big-endian except the page bodies: magic "cook", u32 pageCount, u32 pageSize[pageCount], then each page: u32 0x100,
// u32le cookieCount, u32le cookieOffset[cookieCount], then each cookie: u32le size, u32le, u32le flags, u32le, u32le
// url/name/path/value offsets, u64 end of header, f64 expiry, f64 creation, then NUL-terminated strings.
import { ImportFsError, latin1, named, utf8, type ImportIO } from './browser-import-io';
import { cookieScope, type ImportedCookie } from './browser-import-readers';
import { BROWSER_IMPORT_SOURCES, listSourceProfiles, resolveCookieDatabase, type BrowserImportPathContext } from './browser-import-sources';

/** Safari's timestamps count seconds from 2001-01-01. */
const APPLE_EPOCH_OFFSET_SECONDS = 978_307_200;
const COOKIE_PAGE_HEADER_SIZE = 12, COOKIE_RECORD_HEADER_SIZE = 56;
const FLAG_SECURE = 0x1, FLAG_HTTP_ONLY = 0x4;

export type SafariCookieReadFailure = 'needsFullDiskAccess' | 'readFailed';
export class SafariCookieReadError extends Error {
  readonly _tag = 'SafariCookieReadError';
  constructor(readonly reason: SafariCookieReadFailure, readonly cookieDatabasePath?: string, readonly cause?: unknown) {
    super(cookieDatabasePath === undefined ? `Could not read Safari cookies: ${reason}.` : `Could not read Safari cookies at ${cookieDatabasePath}: ${reason}.`);
    named(this, 'SafariCookieReadError');
  }
}

function readCString(bytes: Uint8Array, start: number): string {
  const end = bytes.indexOf(0, start);
  return utf8(bytes.subarray(start, end === -1 ? bytes.length : end));
}

/** Every declared structure is bounds-checked against the file, and a mismatch fails the read rather than importing a
 *  set that is quietly missing cookies or carrying another record's bytes. */
export function parseBinaryCookies(buffer: Uint8Array): ImportedCookie[] {
  const fail = () => new SafariCookieReadError('readFailed');
  if (buffer.length < 8 || latin1(buffer.subarray(0, 4)) !== 'cook') throw fail();
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  const pageCount = view.getUint32(4, false);
  if (8 + pageCount * 4 > buffer.length) throw fail();
  const pageSizes: number[] = [];
  for (let index = 0; index < pageCount; index += 1) pageSizes.push(view.getUint32(8 + index * 4, false));
  const cookies: ImportedCookie[] = [];
  let pageStart = 8 + pageCount * 4;
  for (const pageSize of pageSizes) {
    if (pageSize < COOKIE_PAGE_HEADER_SIZE || pageStart + pageSize > buffer.length) throw fail();
    const page = buffer.subarray(pageStart, pageStart + pageSize), pageView = new DataView(page.buffer, page.byteOffset, page.byteLength);
    pageStart += pageSize;
    const cookieCount = pageView.getUint32(4, true), offsetTableEnd = COOKIE_PAGE_HEADER_SIZE + cookieCount * 4;
    if (offsetTableEnd > page.length) throw fail();
    // A later offset may not point back into the header, the table or an accepted record.
    const accepted: [number, number][] = [];
    for (let index = 0; index < cookieCount; index += 1) {
      const cookieStart = pageView.getUint32(8 + index * 4, true);
      if (cookieStart < offsetTableEnd || cookieStart + COOKIE_RECORD_HEADER_SIZE > page.length) throw fail();
      const recordSize = pageView.getUint32(cookieStart, true), cookieEnd = cookieStart + recordSize;
      if (recordSize < COOKIE_RECORD_HEADER_SIZE || cookieEnd > page.length || accepted.some(([start, end]) => cookieStart < end && cookieEnd > start)) throw fail();
      accepted.push([cookieStart, cookieEnd]);
      const cookie = page.subarray(cookieStart, cookieEnd), cookieView = new DataView(cookie.buffer, cookie.byteOffset, cookie.byteLength);
      const flags = cookieView.getUint32(8, true);
      const offsets = [16, 20, 24, 28].map(at => cookieView.getUint32(at, true));
      const expiry = cookieView.getFloat64(40, true);
      if (offsets.some(offset => offset < COOKIE_RECORD_HEADER_SIZE || offset >= cookie.length)) throw fail();
      const [domain, name, path, value] = offsets.map(offset => readCString(cookie, offset)) as [string, string, string, string];
      if (domain === '' || name === '') continue;
      const secure = (flags & FLAG_SECURE) !== 0;
      cookies.push({
        ...cookieScope(domain, path || '/', secure), name, value, path: path || '/', secure, httpOnly: (flags & FLAG_HTTP_ONLY) !== 0,
        expirationDate: expiry > 0 ? Math.floor(expiry) + APPLE_EPOCH_OFFSET_SECONDS : undefined,
        // No public description of the SameSite bits agrees with real jars; Lax is the modern default and never widens.
        sameSite: 'lax',
      });
    }
  }
  // After the pages: nothing, the 8-byte checksum, or the checksum and a length-prefixed property list.
  const trailer = buffer.length - pageStart;
  const validTrailer = trailer === 0 || trailer === 8 || (trailer >= 12 && trailer === 8 + 4 + view.getUint32(pageStart + 8, false));
  if (!validTrailer) throw fail();
  return cookies;
}

/** TCC denies with EPERM; EACCES is an ordinary permission failure Full Disk Access cannot fix. */
export const isPermissionDenied = (error: unknown): boolean => error instanceof ImportFsError && error.code === 'EPERM';

/** Whether reading the jar is refused by TCC (an open, not a read: no cookie is read). */
export async function safariAccessDenied(io: ImportIO, cookiePath: string): Promise<boolean> {
  try { await io.open(cookiePath); return false; } catch (cause) { return isPermissionDenied(cause); }
}
/** A missing or unreadable jar is never evidence that access was granted. */
export async function safariAccessGranted(io: ImportIO, cookiePath: string): Promise<boolean> {
  try { await io.open(cookiePath); return true; } catch { return false; }
}

export async function readSafariCookies(io: ImportIO, cookiePath: string): Promise<ImportedCookie[]> {
  let contents: Uint8Array;
  try { contents = await io.readFile(cookiePath); }
  catch (cause) { throw new SafariCookieReadError(isPermissionDenied(cause) ? 'needsFullDiskAccess' : 'readFailed', cookiePath, cause); }
  try { return parseBinaryCookies(contents); }
  catch (cause) { throw new SafariCookieReadError(cause instanceof SafariCookieReadError ? cause.reason : 'readFailed', cookiePath, cause); }
}

/** SafariPermission.safariPermissionCheck: whether the jar (or, without a default jar, a named profile's) opens, without
 *  reading a cookie. The wizard's Full Disk Access step re-asks it. */
export function safariPermissionCheck(io: ImportIO, context: BrowserImportPathContext): () => Promise<boolean> {
  const safari = BROWSER_IMPORT_SOURCES.find(source => source.engine === 'safari');
  return async () => {
    if (!safari || context.platform !== 'darwin') return false;
    const defaultJar = await resolveCookieDatabase(io, safari, context, '.');
    if (defaultJar !== undefined) return safariAccessGranted(io, defaultJar);
    for (const profile of await listSourceProfiles(io, safari, context)) {
      const jar = await resolveCookieDatabase(io, safari, context, profile.directory);
      if (jar !== undefined && (await safariAccessGranted(io, jar))) return true;
    }
    return false;
  };
}
