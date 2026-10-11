// browser-surface part 4 (profiles): the browsers the cookie import can read (MIT reference, see LICENSE-T3, T3 Code
// 1e2ecbd975: apps/desktop/src/preview/BrowserImport/Sources.ts), over the clone's ImportIO (browser-import-io.ts).
//
// Chromium-family browsers keep cookies in an encrypted SQLite database whose key lives in the Keychain; Firefox keeps
// them in plain SQLite; Safari in binary cookie files, with separate WebKit data stores for named profiles. Each entry
// pins its own paths and Keychain coordinates, because the forks do not agree. The clone runs on macOS: the Linux paths
// (and Firefox's Snap home) are kept as the reference has them, the Windows lock probes are not ported (no Windows host).
import type { BrowserImportSourceId, BrowserImportSourceProfile } from './browser-import';
import { ImportFsError, type ImportIO, type PathApi, type PlatformName } from './browser-import-io';

export type BrowserImportEngine = 'chromium' | 'firefox' | 'safari';
/** Directory roots a definition builds its paths from, passed in so other platforms stay testable. */
export interface BrowserImportPathContext {
  readonly path: PathApi;
  readonly platform: PlatformName;
  readonly home: string;
  readonly appData: string | undefined;
  readonly localAppData: string | undefined;
}
export interface BrowserImportSourceDefinition {
  readonly id: BrowserImportSourceId;
  readonly name: string;
  readonly engine: BrowserImportEngine;
  readonly platforms: readonly PlatformName[];
  readonly userDataDirectory: (context: BrowserImportPathContext) => string | undefined;
  readonly keychainService?: string;
  readonly keychainAccount?: string;
  readonly linuxSecretApplication?: string;
}

const macApplicationSupport = (context: BrowserImportPathContext, ...segments: string[]) => context.path.join(context.home, 'Library', 'Application Support', ...segments);

function chromiumSource(input: { id: BrowserImportSourceId; name: string; keychainService: string; keychainAccount: string; macSegments: string[];
  linuxSegments?: string[]; linuxSecretApplication?: string; windowsSegments?: string[] }): BrowserImportSourceDefinition {
  return {
    id: input.id, name: input.name, engine: 'chromium',
    platforms: ['darwin', ...(input.linuxSegments ? ['linux' as const] : []), ...(input.windowsSegments ? ['win32' as const] : [])],
    keychainService: input.keychainService, keychainAccount: input.keychainAccount,
    ...(input.linuxSecretApplication === undefined ? {} : { linuxSecretApplication: input.linuxSecretApplication }),
    userDataDirectory: context => {
      if (context.platform === 'darwin') return macApplicationSupport(context, ...input.macSegments);
      if (context.platform === 'win32') return input.windowsSegments && context.localAppData ? context.path.join(context.localAppData, ...input.windowsSegments) : undefined;
      return input.linuxSegments ? context.path.join(context.home, '.config', ...input.linuxSegments) : undefined;
    },
  };
}

/** No Chromium fork but Helium is importable on Windows (App-Bound Encryption); macOS and Linux keep working. */
export const BROWSER_IMPORT_SOURCES: readonly BrowserImportSourceDefinition[] = [
  chromiumSource({ id: 'chrome', name: 'Chrome', keychainService: 'Chrome Safe Storage', keychainAccount: 'Chrome', macSegments: ['Google', 'Chrome'], linuxSegments: ['google-chrome'], linuxSecretApplication: 'chrome' }),
  chromiumSource({ id: 'edge', name: 'Microsoft Edge', keychainService: 'Microsoft Edge Safe Storage', keychainAccount: 'Microsoft Edge', macSegments: ['Microsoft Edge'], linuxSegments: ['microsoft-edge'], linuxSecretApplication: 'msedge' }),
  chromiumSource({ id: 'brave', name: 'Brave', keychainService: 'Brave Safe Storage', keychainAccount: 'Brave', macSegments: ['BraveSoftware', 'Brave-Browser'], linuxSegments: ['BraveSoftware', 'Brave-Browser'], linuxSecretApplication: 'brave' }),
  chromiumSource({ id: 'vivaldi', name: 'Vivaldi', keychainService: 'Vivaldi Safe Storage', keychainAccount: 'Vivaldi', macSegments: ['Vivaldi'], linuxSegments: ['vivaldi'], linuxSecretApplication: 'vivaldi' }),
  chromiumSource({ id: 'opera', name: 'Opera', keychainService: 'Opera Safe Storage', keychainAccount: 'Opera', macSegments: ['com.operasoftware.Opera'], linuxSegments: ['opera'], linuxSecretApplication: 'opera' }),
  // Arc has no Linux build.
  chromiumSource({ id: 'arc', name: 'Arc', keychainService: 'Arc Safe Storage', keychainAccount: 'Arc', macSegments: ['Arc', 'User Data'] }),
  chromiumSource({ id: 'helium', name: 'Helium', keychainService: 'Helium Storage Key', keychainAccount: 'Helium', macSegments: ['net.imput.helium'], linuxSegments: ['net.imput.helium'],
    windowsSegments: ['imput', 'Helium', 'User Data'], linuxSecretApplication: 'chromium' }),
  {
    // The default jar lives here; named profiles use WebKit data stores beside it in the same container.
    id: 'safari', name: 'Safari', engine: 'safari', platforms: ['darwin'],
    userDataDirectory: context => context.platform === 'darwin' ? context.path.join(context.home, 'Library', 'Containers', 'com.apple.Safari', 'Data', 'Library', 'Cookies') : undefined,
  },
  {
    id: 'firefox', name: 'Firefox', engine: 'firefox', platforms: ['darwin', 'win32', 'linux'],
    userDataDirectory: context => {
      if (context.platform === 'darwin') return macApplicationSupport(context, 'Firefox');
      if (context.platform === 'win32') return context.appData ? context.path.join(context.appData, 'Mozilla', 'Firefox') : undefined;
      return context.path.join(context.home, '.mozilla', 'firefox');
    },
  },
];

/** Where a profile's cookie database may live, most current first (Chromium 96+ `Network/Cookies`, then the legacy one). */
export function cookieDatabaseCandidatePaths(definition: BrowserImportSourceDefinition, context: BrowserImportPathContext, profileDirectory: string): string[] {
  const root = definition.userDataDirectory(context);
  if (root === undefined) return [];
  const profilePath = context.path.isAbsolute(profileDirectory) ? profileDirectory : context.path.join(root, profileDirectory);
  if (definition.engine === 'firefox') return [context.path.join(profilePath, 'cookies.sqlite')];
  if (definition.engine === 'safari') return [context.path.join(profilePath, 'Cookies.binarycookies')];
  return [context.path.join(profilePath, 'Network', 'Cookies'), context.path.join(profilePath, 'Cookies')];
}

/** Whether a candidate is a regular file (a directory at the path would list and then fail the open). */
export async function databaseFileExists(io: ImportIO, path: string): Promise<boolean> {
  try { return (await io.stat(path)) === 'File'; } catch { return false; }
}

/** The first candidate that is a regular file, or undefined. */
export async function resolveCookieDatabase(io: ImportIO, definition: BrowserImportSourceDefinition, context: BrowserImportPathContext, profileDirectory: string): Promise<string | undefined> {
  for (const candidate of cookieDatabaseCandidatePaths(definition, context, profileDirectory)) if (await databaseFileExists(io, candidate)) return candidate;
  return undefined;
}

/** Firefox's `profiles.ini`: only `[ProfileN]` blocks count; relative paths must stay under the root. */
export function parseFirefoxProfiles(ini: string, path: PathApi, root: string): BrowserImportSourceProfile[] {
  const profiles: BrowserImportSourceProfile[] = [];
  let current: { name?: string; path?: string; isRelative?: string } | null = null;
  const flush = () => {
    if (current?.path) {
      const candidate = current.path;
      const isRelative = current.isRelative === undefined || current.isRelative === '1';
      const validIsRelative = current.isRelative === undefined || /^[01]$/.test(current.isRelative);
      if (!validIsRelative || candidate.includes('\u0000')) { current = null; return; }
      let directory: string | undefined;
      if (isRelative) {
        if (!path.isAbsolute(candidate)) {
          const resolved = path.resolve(root, candidate), relative = path.relative(root, resolved);
          const escapesRoot = relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative);
          if (!escapesRoot) directory = path.normalize(candidate);
        }
      } else if (path.isAbsolute(candidate)) directory = path.normalize(candidate);
      if (directory !== undefined) profiles.push({ directory, name: current.name?.trim() || directory });
    }
    current = null;
  };
  for (const rawLine of ini.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.startsWith('[')) { flush(); current = /^\[Profile\d+\]$/i.test(line) ? {} : null; continue; }
    if (!current) continue;
    const separator = line.indexOf('=');
    if (separator === -1) continue;
    const key = line.slice(0, separator).trim().toLowerCase(), value = line.slice(separator + 1).trim();
    if (key === 'name') current.name = value;
    if (key === 'path') current.path = value;
    if (key === 'isrelative') current.isRelative = value;
  }
  flush();
  return profiles;
}

/** A single plain path segment: no separators, no `.`/`..`, not empty. */
const isSafeProfileDirectory = (directory: string): boolean => directory.length > 0 && directory !== '.' && directory !== '..' && !/[\\/]/.test(directory) && !directory.includes('\u0000');

/** How many importable cookies a profile holds, counted without decrypting; undefined when it cannot be counted. */
async function countProfileCookies(io: ImportIO, definition: BrowserImportSourceDefinition, context: BrowserImportPathContext, directory: string): Promise<number | undefined> {
  const database = await resolveCookieDatabase(io, definition, context, directory);
  if (database === undefined) return undefined;
  try {
    const rows = definition.engine === 'firefox'
      ? await io.query(database, "select count(*) as count from moz_cookies where originAttributes = ''")
      : await io.query(database, 'select count(*) as count from cookies');
    const count = rows[0]?.count;
    return typeof count === 'number' ? count : undefined;
  } catch { return undefined; }
}
async function withCookieCounts(io: ImportIO, definition: BrowserImportSourceDefinition, context: BrowserImportPathContext, profiles: readonly BrowserImportSourceProfile[]) {
  const out: BrowserImportSourceProfile[] = [];
  for (const profile of profiles) {
    const cookieCount = await countProfileCookies(io, definition, context, profile.directory);
    out.push(cookieCount === undefined ? profile : { ...profile, cookieCount });
  }
  return out;
}

const isSafariProfileUuid = (value: string) => /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
async function listSafariProfiles(io: ImportIO, context: BrowserImportPathContext, root: string): Promise<BrowserImportSourceProfile[]> {
  const library = context.path.dirname(root), metadata = context.path.join(library, 'Safari', 'SafariTabs.db');
  let declared: { title: string | null; external_uuid: string }[] = [];
  try {
    const rows = await io.query(metadata, 'select title, external_uuid from bookmarks where parent = 0 and type = 1 and subtype = 2 and deleted = 0 order by order_index');
    declared = rows.flatMap(row => typeof row.external_uuid === 'string' && (row.title === null || typeof row.title === 'string') ? [{ title: row.title as string | null, external_uuid: row.external_uuid }] : []);
    if (declared.length !== rows.length) declared = [];
  } catch { declared = []; }
  const defaultProfile = declared.find(profile => profile.external_uuid === 'DefaultProfile');
  const profiles: BrowserImportSourceProfile[] = [{ directory: '.', name: defaultProfile ? defaultProfile.title?.trim() || 'Personal' : 'Safari' }];
  const stores = context.path.join(library, 'WebKit', 'WebsiteDataStore');
  const profileDirectory = (uuid: string) => context.path.join(stores, uuid.toLowerCase(), 'Cookies');
  for (const profile of declared) {
    if (!isSafariProfileUuid(profile.external_uuid)) continue;
    profiles.push({ directory: profileDirectory(profile.external_uuid), name: profile.title?.trim() || profile.external_uuid });
  }
  // Without readable metadata, recover the stores that hold cookies; with it, deleted profiles stay deleted.
  if (declared.length === 0) {
    let entries: string[] = [];
    try { entries = await io.readDirectory(stores); } catch { entries = []; }
    for (const entry of entries.filter(isSafariProfileUuid).sort()) {
      const directory = context.path.join(stores, entry, 'Cookies');
      if (await databaseFileExists(io, context.path.join(directory, 'Cookies.binarycookies'))) profiles.push({ directory, name: entry });
    }
  }
  return profiles;
}

async function readLocalStateProfiles(io: ImportIO, root: string, path: PathApi): Promise<BrowserImportSourceProfile[]> {
  try {
    const state = JSON.parse(await io.readFileString(path.join(root, 'Local State'))) as unknown;
    const profile = state && typeof state === 'object' ? (state as { profile?: unknown }).profile : undefined;
    const cache = profile && typeof profile === 'object' ? (profile as { info_cache?: unknown }).info_cache : undefined;
    // Decoded as the reference's schema: one malformed entry fails the whole file (and the directories are scanned).
    if (!state || typeof state !== 'object' || Array.isArray(state)) return [];
    if (profile !== undefined && (typeof profile !== 'object' || profile === null || Array.isArray(profile))) return [];
    if (cache !== undefined && (typeof cache !== 'object' || cache === null || Array.isArray(cache))) return [];
    const entries = Object.entries((cache ?? {}) as Record<string, unknown>);
    if (entries.some(([, info]) => !info || typeof info !== 'object' || Array.isArray(info) || !['string', 'undefined'].includes(typeof (info as { name?: unknown }).name))) return [];
    return entries.filter(([directory]) => isSafeProfileDirectory(directory))
      .map(([directory, info]) => ({ directory, name: ((info as { name?: string }).name ?? '').trim() || directory }));
  } catch { return []; }
}

/**
 * Profiles the source browser knows about: Firefox's `profiles.ini`, Chromium's `Local State`, Safari's tab-group
 * metadata. Where that metadata is missing or malformed, the directories that hold a cookie database are scanned.
 */
async function listSourceProfilesInDirectory(io: ImportIO, definition: BrowserImportSourceDefinition, context: BrowserImportPathContext): Promise<BrowserImportSourceProfile[]> {
  const root = definition.userDataDirectory(context);
  if (root === undefined) return [];
  if (definition.engine === 'safari') return listSafariProfiles(io, context, root);
  if (definition.engine === 'firefox') {
    let declared: BrowserImportSourceProfile[] = [];
    try { declared = parseFirefoxProfiles(await io.readFileString(context.path.join(root, 'profiles.ini')), context.path, root); } catch { declared = []; }
    if (declared.length > 0) {
      const withDatabase: BrowserImportSourceProfile[] = [];
      for (const profile of declared) {
        let found = false;
        for (const candidate of cookieDatabaseCandidatePaths(definition, context, profile.directory)) if (await databaseFileExists(io, candidate)) found = true;
        if (found) withDatabase.push(profile);
      }
      if (withDatabase.length > 0) return withCookieCounts(io, definition, context, withDatabase);
    }
    const fallbackDirectory = context.platform === 'linux' ? root : context.path.join(root, 'Profiles');
    let scanned: string[] = [];
    try { scanned = await io.readDirectory(fallbackDirectory); } catch { scanned = []; }
    const found: BrowserImportSourceProfile[] = [];
    for (const entry of scanned) {
      const directory = context.platform === 'linux' ? entry : context.path.join('Profiles', entry);
      if ((await resolveCookieDatabase(io, definition, context, directory)) !== undefined) found.push({ directory, name: entry });
    }
    return withCookieCounts(io, definition, context, found);
  }
  // The keys of `Local State` are directory names anything running as the user can write: anything but one plain
  // segment is dropped, so `..` cannot lead the read outside the user-data directory.
  const declared = await readLocalStateProfiles(io, root, context.path);
  if (declared.length > 0) return withCookieCounts(io, definition, context, declared);
  let entries: string[] = [];
  try { entries = await io.readDirectory(root); } catch { entries = []; }
  const found: BrowserImportSourceProfile[] = [];
  for (const directory of entries.filter(isSafeProfileDirectory)) if ((await resolveCookieDatabase(io, definition, context, directory)) !== undefined) found.push({ directory, name: directory });
  return withCookieCounts(io, definition, context, found);
}

/** Firefox's Snap home on Linux beside its native one; Snap profiles keep absolute directories. */
export async function listSourceProfiles(io: ImportIO, definition: BrowserImportSourceDefinition, context: BrowserImportPathContext): Promise<BrowserImportSourceProfile[]> {
  if (definition.engine !== 'firefox' || context.platform !== 'linux') return listSourceProfilesInDirectory(io, definition, context);
  const root = definition.userDataDirectory(context);
  if (root === undefined) return [];
  const roots = [root, context.path.join(context.home, 'snap', 'firefox', 'common', '.mozilla', 'firefox')];
  const profiles = new Map<string, BrowserImportSourceProfile>();
  for (const directory of roots) {
    for (const profile of await listSourceProfilesInDirectory(io, { ...definition, userDataDirectory: () => directory }, context)) {
      const absolute = context.path.resolve(directory, profile.directory);
      if (!profiles.has(absolute)) profiles.set(absolute, directory === root ? profile : { ...profile, directory: absolute });
    }
  }
  return [...profiles.values()];
}

type ProcessLivenessProbe = (pid: number) => Promise<boolean>;
/** Signal 0 is a read-only existence check; only ESRCH proves the process is gone. */
export async function chromiumProcessIsAlive(pid: number, signalProcess: (pid: number, signal: 0) => unknown): Promise<boolean> {
  try { await signalProcess(pid, 0); return true; }
  catch (cause) { return !(typeof cause === 'object' && cause !== null && 'code' in cause && (cause as { code: unknown }).code === 'ESRCH'); }
}
const processIsAlive = (io: ImportIO): ProcessLivenessProbe => pid => chromiumProcessIsAlive(pid, target => io.signal0(target));

/** Whether a Chromium `<host>-<pid>` lock target may still name its owner (a foreign host is never declared stale). */
export async function chromiumSingletonLockIsHeld(target: string, currentHost: string, isProcessAlive: ProcessLivenessProbe): Promise<boolean> {
  const separator = target.lastIndexOf('-');
  if (separator <= 0) return true;
  const host = target.slice(0, separator), pidText = target.slice(separator + 1);
  if (!/^\d+$/.test(pidText)) return true;
  const pid = Number(pidText);
  if (!Number.isSafeInteger(pid) || pid <= 0) return true;
  if (host !== currentHost) return true;
  return isProcessAlive(pid);
}

/** Whether a Firefox `lock` symlink's `<ip>:[+]<pid>` target still names a live local owner. */
export async function firefoxSymlinkLockIsHeld(target: string, localAddresses: ReadonlySet<string>, isProcessAlive: ProcessLivenessProbe): Promise<boolean> {
  const separator = target.lastIndexOf(':');
  if (separator < 0) return true;
  const owner = target.slice(0, separator);
  if (!localAddresses.has(owner)) return true;
  const pidText = target.slice(separator + 1).replace(/^\+/, '');
  if (!/^\d+$/.test(pidText)) return true;
  const pid = Number(pidText);
  if (!Number.isSafeInteger(pid) || pid <= 0) return true;
  return isProcessAlive(pid);
}

/**
 * Whether Firefox holds a profile: a live pid in the Linux `lock` symlink, else the kernel lock on `.parentlock` (left on
 * disk after a clean exit as a last-used marker, so only its fcntl lock is evidence). The reference probes that lock
 * with a python3 child; the clone's module asks fcntl itself (`lockHeld`).
 */
async function firefoxProfileIsHeld(io: ImportIO, directory: string, context: BrowserImportPathContext, localAddresses: ReadonlySet<string>): Promise<boolean> {
  let symlinkHeld = false;
  try { symlinkHeld = await firefoxSymlinkLockIsHeld(await io.readLink(context.path.join(directory, 'lock')), localAddresses, processIsAlive(io)); } catch { symlinkHeld = false; }
  if (symlinkHeld) return true;
  const parentLock = context.path.join(directory, '.parentlock');
  if (!(await databaseFileExists(io, parentLock))) return false;
  try { return await io.lockHeld(parentLock); } catch { return false; }
}

/** Whether the browser is running, which leaves its cookie database mid-write. Safari keeps no lock and writes atomically. */
export async function isSourceRunning(io: ImportIO, definition: BrowserImportSourceDefinition, context: BrowserImportPathContext): Promise<boolean> {
  const root = definition.userDataDirectory(context);
  if (root === undefined || definition.engine === 'safari') return false;
  if (definition.engine !== 'firefox') {
    // Chromium's POSIX SingletonLock is a dangling symlink to `<host>-<pid>`; anything but its absence counts as held.
    try { return await chromiumSingletonLockIsHeld(await io.readLink(context.path.join(root, 'SingletonLock')), await io.hostname(), processIsAlive(io)); }
    catch (error) { return !(error instanceof ImportFsError && error.code === 'ENOENT'); }
  }
  const profiles = await listSourceProfiles(io, definition, context);
  const localAddresses = new Set(await io.localAddresses());
  for (const profile of profiles) {
    const directory = context.path.isAbsolute(profile.directory) ? profile.directory : context.path.join(root, profile.directory);
    if (await firefoxProfileIsHeld(io, directory, context, localAddresses)) return true;
  }
  return false;
}

/** Whether the source has cookies to import: keyed off the cookie database (installers create empty user-data folders),
 *  found by `stat`, which TCC permits for Safari's jar without Full Disk Access. */
export async function isSourceInstalled(io: ImportIO, definition: BrowserImportSourceDefinition, context: BrowserImportPathContext): Promise<boolean> {
  for (const profile of await listSourceProfiles(io, definition, context)) if ((await resolveCookieDatabase(io, definition, context, profile.directory)) !== undefined) return true;
  return false;
}
