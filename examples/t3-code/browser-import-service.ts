// browser-surface part 4 (profiles): listing the importable browsers and writing one profile's cookies into a T3 Code
// profile (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/desktop/src/preview/BrowserImport/BrowserImport.ts).
//
// The reference writes into the target profile's Electron partition with `session.cookies.set`; the clone's module writes
// into that environment's WebKit data store for the profile (`browserImportCookies`, T3BrowserSessions+Profiles.swift:
// `WKHTTPCookieStore.setCookie`), in batches rather than one IPC per cookie. WebKit has no `flushStore`: a written cookie
// is in the store, which persists it on its own schedule.
import { ImportFsError, named, posixPath, type ImportIO, type PlatformName } from './browser-import-io';
import type { BrowserImportFailureReason, BrowserImportInput, BrowserImportResult, BrowserImportSource, BrowserImportUnavailableReason } from './browser-import';
import { ChromiumCookieReadError, FirefoxCookieReadError, readChromiumCookies, readFirefoxCookies, type CookieReadResult, type ImportedCookie } from './browser-import-readers';
import { SafariCookieReadError, readSafariCookies, safariAccessDenied } from './browser-import-safari';
import {
  BROWSER_IMPORT_SOURCES, isSourceInstalled, isSourceRunning, listSourceProfiles, resolveCookieDatabase,
  type BrowserImportPathContext, type BrowserImportSourceDefinition,
} from './browser-import-sources';

export class BrowserImportFailedError extends Error {
  readonly _tag = 'BrowserImportFailedError';
  // The reason token is part of the message on purpose: the settings page maps it back to its copy (importFailureReason).
  constructor(readonly sourceId: string, readonly reason: BrowserImportFailureReason, readonly cause?: unknown) {
    super(`Importing cookies from ${sourceId} failed: ${reason}.`); named(this, 'BrowserImportFailedError');
  }
}

/** The roots a definition builds its paths from: the module names the home it may read (the fixture home in a
 *  development build) and the platform. */
export function sourcePathContext(home: string, platform: PlatformName = 'darwin'): BrowserImportPathContext {
  return { path: posixPath, platform, home, appData: undefined, localAppData: undefined };
}

export async function unavailableReason(io: ImportIO, definition: BrowserImportSourceDefinition, context: BrowserImportPathContext): Promise<BrowserImportUnavailableReason | undefined> {
  if (!definition.platforms.includes(context.platform)) return 'unsupportedPlatform';
  if (!(await isSourceInstalled(io, definition, context))) return 'notInstalled';
  if (await isSourceRunning(io, definition, context)) return 'browserRunning';
  // Safari's jar is found by `stat`, which TCC permits without Full Disk Access: probe the grant here, so the wizard opens
  // on the permission step and a recheck can tell granted from still denied.
  if (definition.engine === 'safari') {
    const jar = await resolveCookieDatabase(io, definition, context, '.');
    if (jar !== undefined && (await safariAccessDenied(io, jar))) return 'needsFullDiskAccess';
  }
  return undefined;
}

/** Every source with its availability; listing profiles touches the source's files, so an unusable one lists none. */
export async function listBrowserImportSources(io: ImportIO, context: BrowserImportPathContext): Promise<BrowserImportSource[]> {
  const sources: BrowserImportSource[] = [];
  for (const definition of BROWSER_IMPORT_SOURCES) {
    const unavailable = await unavailableReason(io, definition, context);
    sources.push({ id: definition.id, name: definition.name, profiles: unavailable === undefined ? await listSourceProfiles(io, definition, context) : [], ...(unavailable === undefined ? {} : { unavailable }) });
  }
  return sources;
}

/** The target store: writes a batch of cookies, one answer per cookie (false: the store refused it). */
export interface CookieWriter {
  set(cookies: readonly ImportedCookie[]): Promise<boolean[]>;
  flushStore(): Promise<void>;
}
const WRITE_BATCH = 200;
const cookieHost = (url: string): string => { try { return new URL(url).hostname; } catch { return url; } };
/** A rejected cookie costs only itself and is named; the skipped hosts are capped at 20. */
export async function writeCookies(writer: CookieWriter, read: CookieReadResult): Promise<BrowserImportResult> {
  let imported = 0, skipped = read.undecryptable;
  const skippedDomains = new Set(read.undecryptableHosts);
  for (let start = 0; start < read.cookies.length; start += WRITE_BATCH) {
    const batch = read.cookies.slice(start, start + WRITE_BATCH);
    let written: boolean[];
    try { written = await writer.set(batch); } catch { written = batch.map(() => false); }
    batch.forEach((cookie, index) => {
      if (written[index] === true) imported += 1;
      else { skipped += 1; skippedDomains.add(cookieHost(cookie.url)); }
    });
  }
  // Persisted before "Done"; a failed flush is not a failed import (the cookies are in the store either way).
  if (imported > 0) await writer.flushStore().catch(() => undefined);
  return { imported, skipped, skippedDomains: [...skippedDomains].slice(0, 20) };
}

/** BrowserImport.importCookies: the request is checked against what the source reports before anything is read. */
export async function importBrowserCookies(io: ImportIO, context: BrowserImportPathContext, input: BrowserImportInput, writerFor: () => Promise<CookieWriter>): Promise<BrowserImportResult> {
  const definition = BROWSER_IMPORT_SOURCES.find(candidate => candidate.id === input.sourceId);
  if (!definition) throw new BrowserImportFailedError(input.sourceId, 'unknownSource');
  const blocked = await unavailableReason(io, definition, context);
  if (blocked !== undefined) throw new BrowserImportFailedError(definition.id, blocked);
  // The directory came from the page: honoured only when the source itself reported it (no `..` walk out of it).
  const requestedProfile = (await listSourceProfiles(io, definition, context)).find(profile => profile.directory === input.sourceProfileDirectory);
  if (requestedProfile === undefined) throw new BrowserImportFailedError(definition.id, 'unknownSourceProfile');
  const databasePath = await resolveCookieDatabase(io, definition, context, requestedProfile.directory);
  if (databasePath === undefined) throw new BrowserImportFailedError(definition.id, 'readFailed');
  let result: CookieReadResult;
  try {
    result = definition.engine === 'safari' ? { cookies: await readSafariCookies(io, databasePath), undecryptable: 0, undecryptableHosts: [] }
      : definition.engine === 'firefox' ? { cookies: await readFirefoxCookies(io, databasePath), undecryptable: 0, undecryptableHosts: [] }
        : await readChromiumCookies(io, { cookieDatabasePath: databasePath, keychainService: definition.keychainService, keychainAccount: definition.keychainAccount,
          linuxSecretApplication: definition.linuxSecretApplication, platform: context.platform });
  } catch (cause) {
    if (cause instanceof ChromiumCookieReadError || cause instanceof SafariCookieReadError) throw new BrowserImportFailedError(definition.id, cause.reason, cause);
    if (cause instanceof FirefoxCookieReadError) throw new BrowserImportFailedError(definition.id, 'readFailed', cause);
    throw new BrowserImportFailedError(definition.id, cause instanceof ImportFsError && cause.code === 'EPERM' ? 'needsFullDiskAccess' : 'readFailed', cause);
  }
  let writer: CookieWriter;
  try { writer = await writerFor(); } catch (cause) { throw new BrowserImportFailedError(definition.id, 'sessionUnavailable', cause); }
  return writeCookies(writer, result);
}
