// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names: apps/desktop/src/preview/BrowserImport/
// Sources.test.ts (28 of 35; not ported: the three Windows lock-error rows, Helium on Windows, the Windows running signal,
// Firefox's win32 fallback row and stale `parent.lock` (the clone runs on macOS), and the python3 shim row: the clone's
// module asks fcntl itself, tested in macos/tests/browser-profiles) and BrowserImport.test.ts (3 of 4: interruption is an
// Effect fiber's, which a promise has not; the batch write replaces one `set` per cookie). HostProcessHostname and
// HostProcessPlatform are an ImportIO override and the path context's platform.
import { afterEach, describe, expect, it } from 'bun:test';
import { Database } from 'bun:sqlite';
import * as NodeFs from 'node:fs';
import * as NodePath from 'node:path';
import { spawn } from 'node:child_process';
import { posixPath } from './browser-import-io';
import {
  BROWSER_IMPORT_SOURCES, chromiumProcessIsAlive, chromiumSingletonLockIsHeld, cookieDatabaseCandidatePaths, firefoxSymlinkLockIsHeld,
  isSourceInstalled, isSourceRunning, listSourceProfiles, resolveCookieDatabase, type BrowserImportPathContext,
} from './browser-import-sources';
import { BrowserImportFailedError, importBrowserCookies, sourcePathContext, writeCookies, type CookieWriter } from './browser-import-service';
import { bunImportIO, tempDirectory } from './browser-import-fixture';

const io = bunImportIO();
const scratch: { cleanup(): void }[] = [];
afterEach(() => { for (const entry of scratch.splice(0)) entry.cleanup(); });
const temp = (prefix: string) => { const directory = tempDirectory(prefix); scratch.push(directory); return directory.path; };
const helium = BROWSER_IMPORT_SOURCES.find(source => source.id === 'helium')!;
const firefox = BROWSER_IMPORT_SOURCES.find(source => source.id === 'firefox')!;
const safari = BROWSER_IMPORT_SOURCES.find(source => source.id === 'safari')!;
const mkdir = (path: string) => NodeFs.mkdirSync(path, { recursive: true });
const write = (path: string, text: string) => NodeFs.writeFileSync(path, text);

describe('Linux Chromium secret applications', () => {
  it('pins the libsecret application attribute for each supported fork', () => {
    expect(Object.fromEntries(BROWSER_IMPORT_SOURCES.filter(source => source.platforms.includes('linux')).map(source => [source.id, source.linuxSecretApplication])))
      .toEqual({ chrome: 'chrome', edge: 'msedge', brave: 'brave', vivaldi: 'vivaldi', opera: 'opera', helium: 'chromium', firefox: undefined });
  });
});

/** A scratch home with the source's user-data directory already created (darwin). */
function withSourceHome(): BrowserImportPathContext {
  const context = sourcePathContext(temp('t3code-sources-'), 'darwin');
  mkdir(userDataDirectory(context));
  return context;
}
const userDataDirectory = (context: BrowserImportPathContext) => helium.userDataDirectory(context)!;
/** Writes a Chromium-shaped cookie table with `count` rows. */
function writeCookieDatabase(file: string, count: number) {
  const db = new Database(file);
  db.run('create table cookies (host_key text, name text)');
  for (let index = 0; index < count; index += 1) db.run('insert into cookies (host_key, name) values (?, ?)', ['example.test', `c${index}`]);
  db.close();
}
function writeFirefoxCookieDatabase(file: string, defaultContainerCount: number, containerCount: number) {
  const db = new Database(file);
  db.run('create table moz_cookies (originAttributes text not null)');
  for (let index = 0; index < defaultContainerCount; index += 1) db.run("insert into moz_cookies (originAttributes) values ('')");
  for (let index = 0; index < containerCount; index += 1) db.run("insert into moz_cookies (originAttributes) values ('^userContextId=2')");
  db.close();
}

describe('Helium on Linux', () => {
  it('discovers its profiles and checks the user-data lock', async () => {
    const home = temp('t3code-helium-linux-'), context = sourcePathContext(home, 'linux'), root = `${home}/.config/net.imput.helium`;
    mkdir(`${root}/Default`);
    writeCookieDatabase(`${root}/Default/Cookies`, 3);
    write(`${root}/Local State`, '{"profile":{"info_cache":{"Default":{"name":"Personal"}}}}');
    expect(helium.platforms).toContain('linux');
    expect(await isSourceInstalled(io, helium, context)).toBe(true);
    expect(await listSourceProfiles(io, helium, context)).toEqual([{ directory: 'Default', name: 'Personal', cookieCount: 3 }]);
    expect(await isSourceRunning(io, helium, context)).toBe(false);
    NodeFs.symlinkSync('foreign-host-4242', `${root}/SingletonLock`);
    expect(await isSourceRunning(io, helium, context)).toBe(true);
  });
});

describe('isSourceRunning', () => {
  it("reads Chromium's dangling SingletonLock symlink as a running browser", async () => {
    const context = withSourceHome();
    expect(await isSourceRunning(io, helium, context)).toBe(false);
    NodeFs.symlinkSync('host-that-does-not-exist-1234', `${userDataDirectory(context)}/SingletonLock`);
    expect(await isSourceRunning(io, helium, context)).toBe(true);
  });
  it('uses the provided hostname to classify Chromium locks', async () => {
    const paths = withSourceHome();
    NodeFs.symlinkSync('lock-owner-99999999', `${helium.userDataDirectory(paths)}/SingletonLock`);
    expect(await isSourceRunning(bunImportIO({ hostname: async () => 'another-host' }), helium, paths)).toBe(true);
    expect(await isSourceRunning(bunImportIO({ hostname: async () => 'lock-owner' }), helium, paths)).toBe(false);
  });
});

describe('chromiumSingletonLockIsHeld', () => {
  it('ignores a positively dead PID on the current host', async () => {
    const checked: number[] = [];
    expect(await chromiumSingletonLockIsHeld('current-host-4321', 'current-host', async pid => { checked.push(pid); return false; })).toBe(false);
    expect(checked).toEqual([4321]);
  });
  it('keeps a live PID on the current host', async () => {
    expect(await chromiumSingletonLockIsHeld('current-host-4321', 'current-host', async () => true)).toBe(true);
  });
  it('keeps foreign-host and malformed targets without probing a PID', async () => {
    let probes = 0;
    const probe = async () => { probes += 1; return false; };
    expect(await chromiumSingletonLockIsHeld('another-host-4321', 'current-host', probe)).toBe(true);
    expect(await chromiumSingletonLockIsHeld('current-host-no-pid', 'current-host', probe)).toBe(true);
    expect(await chromiumSingletonLockIsHeld('current-host-0', 'current-host', probe)).toBe(true);
    expect(probes).toBe(0);
  });
});

describe('chromiumProcessIsAlive', () => {
  it('returns false only when signal 0 reports a missing process', async () => {
    const missing = Object.assign(new Error('missing'), { code: 'ESRCH' }), denied = Object.assign(new Error('denied'), { code: 'EPERM' });
    expect(await chromiumProcessIsAlive(4321, () => { throw missing; })).toBe(false);
    expect(await chromiumProcessIsAlive(4321, () => { throw denied; })).toBe(true);
    expect(await chromiumProcessIsAlive(4321, () => { throw undefined; })).toBe(true);
    expect(await chromiumProcessIsAlive(4321, () => { throw 'unknown failure'; })).toBe(true);
    expect(await chromiumProcessIsAlive(4321, () => { throw null; })).toBe(true);
    expect(await chromiumProcessIsAlive(4321, () => true)).toBe(true);
  });
});

describe('isSourceInstalled', () => {
  it('ignores a user-data directory that holds no cookie database', async () => {
    const context = withSourceHome(), root = userDataDirectory(context);
    mkdir(`${root}/NativeMessagingHosts`);
    expect(await isSourceInstalled(io, helium, context)).toBe(false);
    mkdir(`${root}/Default`); write(`${root}/Default/Cookies`, 'db');
    expect(await isSourceInstalled(io, helium, context)).toBe(true);
    NodeFs.rmSync(`${root}/Default`, { recursive: true });
    mkdir(`${root}/Profile 1`); write(`${root}/Profile 1/Cookies`, 'db');
    expect(await isSourceInstalled(io, helium, context)).toBe(true);
    NodeFs.rmSync(root, { recursive: true });
    expect(await isSourceInstalled(io, helium, context)).toBe(false);
  });
  it('detects a Chromium 127+ install with cookies under Network/', async () => {
    const context = withSourceHome(), root = userDataDirectory(context);
    mkdir(`${root}/Default/Network`); write(`${root}/Default/Network/Cookies`, 'db');
    expect(await isSourceInstalled(io, helium, context)).toBe(true);
  });
  it('follows cookie database symlinks when detecting profiles', async () => {
    const context = withSourceHome(), root = userDataDirectory(context);
    mkdir(`${root}/Default`);
    NodeFs.symlinkSync('missing-cookies', `${root}/Default/Cookies`);
    expect(await listSourceProfiles(io, helium, context)).toEqual([]);
    expect(await isSourceInstalled(io, helium, context)).toBe(false);
    write(`${root}/Default/missing-cookies`, 'db');
    expect(await listSourceProfiles(io, helium, context)).toEqual([{ directory: 'Default', name: 'Default' }]);
    expect(await isSourceInstalled(io, helium, context)).toBe(true);
  });
});

describe('listSourceProfiles', () => {
  it('ignores a profile whose Cookies entry is not a file', async () => {
    const paths = withSourceHome(), root = helium.userDataDirectory(paths)!;
    mkdir(`${root}/Broken/Cookies`); mkdir(`${root}/Real`); write(`${root}/Real/Cookies`, 'db');
    expect(await listSourceProfiles(io, helium, paths)).toEqual([{ directory: 'Real', name: 'Real' }]);
    expect(await isSourceInstalled(io, helium, paths)).toBe(true);
  });
  it('discovers profiles by their cookie database when Local State is absent', async () => {
    const context = withSourceHome(), root = userDataDirectory(context);
    mkdir(`${root}/Profile 1`); write(`${root}/Profile 1/Cookies`, 'db'); mkdir(`${root}/NativeMessagingHosts`);
    expect(await listSourceProfiles(io, helium, context)).toEqual([{ directory: 'Profile 1', name: 'Profile 1' }]);
  });
  it('reads the profile names the browser shows', async () => {
    const context = withSourceHome();
    write(`${userDataDirectory(context)}/Local State`, '{"profile":{"info_cache":{"Default":{"name":"You"},"Profile 2":{"name":"  "}}}}');
    expect(await listSourceProfiles(io, helium, context)).toEqual([{ directory: 'Default', name: 'You' }, { directory: 'Profile 2', name: 'Profile 2' }]);
  });
  it('scans for profiles when Local State is malformed', async () => {
    const context = withSourceHome(), root = userDataDirectory(context);
    write(`${root}/Local State`, '{not-json'); mkdir(`${root}/Default`); write(`${root}/Default/Cookies`, 'db');
    expect(await listSourceProfiles(io, helium, context)).toEqual([{ directory: 'Default', name: 'Default' }]);
  });
  it('reports nothing when no directory holds a cookie database', async () => {
    expect(await listSourceProfiles(io, helium, withSourceHome())).toEqual([]);
  });
  it('drops Firefox profiles that hold no cookie database', async () => {
    const context = withSourceHome(), root = firefox.userDataDirectory(context)!;
    mkdir(root);
    write(`${root}/profiles.ini`, '[Profile0]\nName=original\nIsRelative=1\nPath=Profiles/abcd.default-release\nDefault=1\n\n[Profile1]\nName=empty\nIsRelative=1\nPath=Profiles/wxyz.empty\n');
    mkdir(`${root}/Profiles/abcd.default-release`); write(`${root}/Profiles/abcd.default-release/cookies.sqlite`, 'db'); mkdir(`${root}/Profiles/wxyz.empty`);
    expect(await listSourceProfiles(io, firefox, context)).toEqual([{ directory: posixPath.join('Profiles', 'abcd.default-release'), name: 'original' }]);
  });
  it('drops empty profiles when falling back to the Profiles/ scan', async () => {
    const context = withSourceHome(), root = firefox.userDataDirectory(context)!;
    mkdir(`${root}/Profiles/filled.default`); write(`${root}/Profiles/filled.default/cookies.sqlite`, 'db'); mkdir(`${root}/Profiles/empty.default`);
    expect(await listSourceProfiles(io, firefox, context)).toEqual([{ directory: posixPath.join('Profiles', 'filled.default'), name: 'filled.default' }]);
  });
  it('discovers profiles with cookies under Network/ (Chromium 127+)', async () => {
    const context = withSourceHome(), root = userDataDirectory(context);
    mkdir(`${root}/Default/Network`); write(`${root}/Default/Network/Cookies`, 'db');
    expect(await listSourceProfiles(io, helium, context)).toEqual([{ directory: 'Default', name: 'Default' }]);
  });
  it("counts a profile's cookies without decrypting them", async () => {
    const paths = withSourceHome(), root = helium.userDataDirectory(paths)!;
    mkdir(`${root}/Default`); writeCookieDatabase(`${root}/Default/Cookies`, 3);
    expect((await listSourceProfiles(io, helium, paths))[0]?.cookieCount).toBe(3);
  });
  it('falls through to the legacy database when Network/Cookies is a directory', async () => {
    const paths = withSourceHome(), root = helium.userDataDirectory(paths)!;
    mkdir(`${root}/Default/Network/Cookies`); writeCookieDatabase(`${root}/Default/Cookies`, 2);
    const [profile] = await listSourceProfiles(io, helium, paths);
    expect(profile?.directory).toBe('Default');
    expect(profile?.cookieCount).toBe(2);
  });
});

describe('cookieDatabaseCandidatePaths', () => {
  it('prefers Network/Cookies and falls back to the legacy Cookies', () => {
    const context = withSourceHome(), profile = posixPath.join(context.home, 'Library', 'Application Support', 'net.imput.helium', 'Profile 1');
    expect(cookieDatabaseCandidatePaths(helium, context, 'Profile 1')).toEqual([posixPath.join(profile, 'Network', 'Cookies'), posixPath.join(profile, 'Cookies')]);
  });
  it('resolves the live Network/ jar over a leftover root Cookies', async () => {
    const context = withSourceHome(), root = userDataDirectory(context);
    mkdir(`${root}/Default/Network`); write(`${root}/Default/Network/Cookies`, 'live'); write(`${root}/Default/Cookies`, 'stale');
    expect(await resolveCookieDatabase(io, helium, context, 'Default')).toBe(posixPath.join(root, 'Default', 'Network', 'Cookies'));
    NodeFs.rmSync(`${root}/Default/Cookies`);
    expect(await isSourceInstalled(io, helium, context)).toBe(true);
  });
  it('returns only cookies.sqlite for Firefox', () => {
    const context = sourcePathContext('/tmp/test', 'darwin');
    expect(cookieDatabaseCandidatePaths(firefox, context, 'Profiles/abc.default')).toEqual([posixPath.join('/tmp/test', 'Library/Application Support/Firefox/Profiles/abc.default/cookies.sqlite')]);
  });
});

describe('Firefox Snap profiles', () => {
  it('finds Snap profiles with or without profiles.ini and checks their locks', async () => {
    const home = temp('t3code-firefox-snap-'), context = sourcePathContext(home, 'linux');
    const root = posixPath.join(home, 'snap', 'firefox', 'common', '.mozilla', 'firefox'), directory = posixPath.join(root, 'abcd.default');
    mkdir(directory); writeFirefoxCookieDatabase(`${directory}/cookies.sqlite`, 2, 1);
    write(`${root}/profiles.ini`, '[Profile0]\nName=Personal\nIsRelative=1\nPath=abcd.default\n');
    expect(await isSourceInstalled(io, firefox, context)).toBe(true);
    expect(await listSourceProfiles(io, firefox, context)).toEqual([{ directory, name: 'Personal', cookieCount: 2 }]);
    expect(await resolveCookieDatabase(io, firefox, context, directory)).toBe(posixPath.join(directory, 'cookies.sqlite'));
    expect(await isSourceRunning(io, firefox, context)).toBe(false);
    NodeFs.symlinkSync('foreign-host:+4242', `${directory}/lock`);
    expect(await isSourceRunning(io, firefox, context)).toBe(true);
    NodeFs.rmSync(`${directory}/lock`);
    expect(await isSourceRunning(io, firefox, context)).toBe(false);
    NodeFs.rmSync(`${root}/profiles.ini`);
    expect(await listSourceProfiles(io, firefox, context)).toEqual([{ directory, name: 'abcd.default', cookieCount: 2 }]);
  });
  it('keeps matching profile names in native and Snap installs distinct', async () => {
    const home = temp('t3code-firefox-snap-'), context = sourcePathContext(home, 'linux');
    const native = posixPath.join(home, '.mozilla', 'firefox'), snap = posixPath.join(home, 'snap', 'firefox', 'common', '.mozilla', 'firefox');
    for (const root of [native, snap]) {
      mkdir(`${root}/abcd.default`); writeFirefoxCookieDatabase(`${root}/abcd.default/cookies.sqlite`, 1, 0);
      write(`${root}/profiles.ini`, '[Profile0]\nName=Personal\nIsRelative=1\nPath=abcd.default\n' + `[Profile1]\nName=Shared\nIsRelative=0\nPath=${snap}/abcd.default\n`);
    }
    const profiles = await listSourceProfiles(io, firefox, context);
    expect(profiles.map(profile => profile.directory)).toEqual(['abcd.default', posixPath.join(snap, 'abcd.default')]);
    const databases = [];
    for (const profile of profiles) databases.push(await resolveCookieDatabase(io, firefox, context, profile.directory));
    expect(databases).toEqual([posixPath.join(native, 'abcd.default', 'cookies.sqlite'), posixPath.join(snap, 'abcd.default', 'cookies.sqlite')]);
  });
});

describe('listSourceProfiles Firefox fallback', () => {
  it.each([
    { platform: 'linux' as const, profileDirectory: 'linux.default' },
    { platform: 'darwin' as const, profileDirectory: NodePath.join('Profiles', 'macos.default') },
  ])('scans the $platform profile location and excludes stale entries', async ({ platform, profileDirectory }) => {
    const context = sourcePathContext(temp(`t3code-firefox-${platform}-`), platform), root = firefox.userDataDirectory(context)!;
    const scanRoot = platform === 'linux' ? root : posixPath.join(root, 'Profiles');
    mkdir(posixPath.join(root, profileDirectory)); write(posixPath.join(root, profileDirectory, 'cookies.sqlite'), 'db');
    mkdir(posixPath.join(scanRoot, 'stale.default')); write(posixPath.join(scanRoot, 'stale-file.default'), 'not-dir');
    expect(await listSourceProfiles(io, firefox, context)).toEqual([{ directory: profileDirectory, name: posixPath.basename(profileDirectory) }]);
  });
  it('scans for profiles when profiles.ini declares only ones without cookies', async () => {
    const context = sourcePathContext(temp('t3code-firefox-stale-ini-'), 'darwin'), root = firefox.userDataDirectory(context)!;
    mkdir(posixPath.join(root, 'Profiles', 'stale.default'));
    const realDirectory = posixPath.join(root, 'Profiles', 'real.default');
    mkdir(realDirectory); writeFirefoxCookieDatabase(posixPath.join(realDirectory, 'cookies.sqlite'), 3, 0);
    write(posixPath.join(root, 'profiles.ini'), ['[Profile0]', 'Name=Stale', 'IsRelative=1', 'Path=Profiles/stale.default'].join('\n'));
    expect(await listSourceProfiles(io, firefox, context)).toEqual([{ directory: posixPath.join('Profiles', 'real.default'), name: 'real.default', cookieCount: 3 }]);
    expect(await isSourceInstalled(io, firefox, context)).toBe(true);
  });
  it('counts only importable cookies for declared and fallback profiles', async () => {
    const context = sourcePathContext(temp('t3code-firefox-counts-'), 'darwin'), root = firefox.userDataDirectory(context)!;
    const declaredDirectory = posixPath.join(root, 'Profiles', 'declared.default');
    mkdir(declaredDirectory); writeFirefoxCookieDatabase(posixPath.join(declaredDirectory, 'cookies.sqlite'), 2, 3);
    write(posixPath.join(root, 'profiles.ini'), ['[Profile0]', 'Name=Declared', 'IsRelative=1', 'Path=Profiles/declared.default'].join('\n'));
    expect(await listSourceProfiles(io, firefox, context)).toEqual([{ directory: posixPath.join('Profiles', 'declared.default'), name: 'Declared', cookieCount: 2 }]);
    NodeFs.rmSync(posixPath.join(root, 'profiles.ini'));
    const fallbackDirectory = posixPath.join(root, 'Profiles', 'fallback.default');
    mkdir(fallbackDirectory); writeFirefoxCookieDatabase(posixPath.join(fallbackDirectory, 'cookies.sqlite'), 1, 4);
    expect(await listSourceProfiles(io, firefox, context)).toEqual([
      { directory: posixPath.join('Profiles', 'declared.default'), name: 'declared.default', cookieCount: 2 },
      { directory: posixPath.join('Profiles', 'fallback.default'), name: 'fallback.default', cookieCount: 1 },
    ]);
  });
});

describe('isSourceRunning for Firefox', () => {
  it('finds the lock inside the profile, not at the root', async () => {
    const context = sourcePathContext(temp('t3code-firefox-'), 'darwin'), root = firefox.userDataDirectory(context)!, profile = `${root}/Profiles/abcd.default-release`;
    mkdir(profile); write(`${profile}/cookies.sqlite`, 'db');
    expect(await isSourceRunning(io, firefox, context)).toBe(false);
    write(`${root}/lock`, '');
    expect(await isSourceRunning(io, firefox, context)).toBe(false);
    write(`${profile}/.parentlock`, '');
    expect(await isSourceRunning(io, firefox, context)).toBe(false);
    NodeFs.symlinkSync(`127.0.0.1:+${process.pid}`, `${profile}/lock`);
    expect(await isSourceRunning(io, firefox, context)).toBe(true);
  });
  it('detects a live fcntl lock on .parentlock, as macOS Firefox leaves it', async () => {
    const context = sourcePathContext(temp('t3code-firefox-'), 'darwin'), root = firefox.userDataDirectory(context)!, profile = `${root}/Profiles/abcd.default-release`;
    mkdir(profile); write(`${profile}/cookies.sqlite`, 'db');
    const parentLock = `${profile}/.parentlock`;
    write(parentLock, '');
    const holder = spawn('python3', ['-c', "import fcntl,os,sys,time\nfd=os.open(sys.argv[1],os.O_WRONLY)\nfcntl.lockf(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)\nprint('locked',flush=True)\ntime.sleep(30)", parentLock], { stdio: ['ignore', 'pipe', 'ignore'] });
    try {
      await new Promise<void>(resolve => holder.stdout!.on('data', chunk => { if (String(chunk).includes('locked')) resolve(); }));
      expect(await isSourceRunning(io, firefox, context)).toBe(true);
    } finally { holder.kill(); }
  });
  it("reads a Firefox lock symlink's pid to tell live from crashed", async () => {
    const alive = async (pid: number) => pid === 4242, local = new Set(['127.0.0.1', '127.0.1.1', '192.168.1.20']);
    expect(await firefoxSymlinkLockIsHeld('127.0.0.1:4242', local, alive)).toBe(true);
    expect(await firefoxSymlinkLockIsHeld('127.0.1.1:+4242', local, alive)).toBe(true);
    expect(await firefoxSymlinkLockIsHeld('192.168.1.20:+4242', local, alive)).toBe(true);
    expect(await firefoxSymlinkLockIsHeld('127.0.0.1:+9999', local, alive)).toBe(false);
    expect(await firefoxSymlinkLockIsHeld('192.168.1.20:+9999', local, alive)).toBe(false);
    expect(await firefoxSymlinkLockIsHeld('garbage', local, alive)).toBe(true);
    expect(await firefoxSymlinkLockIsHeld('10.0.0.7:+9999', local, alive)).toBe(true);
  });
});

describe('Windows user-data directories', () => {
  it('keeps app-bound Chromium forks unsupported on win32', () => {
    for (const source of BROWSER_IMPORT_SOURCES) if (source.engine === 'chromium' && source.id !== 'helium') expect(source.platforms).not.toContain('win32');
  });
});

describe('listSourceProfiles hardening', () => {
  it('drops profile directories that are not a single plain segment', async () => {
    const context = withSourceHome();
    write(`${userDataDirectory(context)}/Local State`, '{"profile":{"info_cache":{"Default":{"name":"You"},"../../../../secrets":{"name":"Escape"},"a/b":{"name":"Nested"},"..":{"name":"Parent"}}}}');
    expect((await listSourceProfiles(io, helium, context)).map(profile => profile.directory)).toEqual(['Default']);
  });
});

describe('Safari profiles', () => {
  const workUuid = 'C561D071-67AD-4537-866F-54F65FB8E8DD', otherUuid = '2875EB19-B938-4E38-BE92-5AE97C256BDD';
  function fixture() {
    const context = withSourceHome(), root = safari.userDataDirectory(context)!, library = posixPath.dirname(root);
    mkdir(root); write(posixPath.join(root, 'Cookies.binarycookies'), 'default');
    const store = (uuid: string) => posixPath.join(library, 'WebKit', 'WebsiteDataStore', uuid.toLowerCase(), 'Cookies');
    for (const uuid of [workUuid, otherUuid]) { mkdir(store(uuid)); write(posixPath.join(store(uuid), 'Cookies.binarycookies'), uuid); }
    mkdir(posixPath.join(library, 'Safari'));
    return { context, root, store, metadata: posixPath.join(library, 'Safari', 'SafariTabs.db') };
  }
  it("discovers named profiles and resolves only the selected profile's cookies", async () => {
    const { context, root, store, metadata } = fixture();
    const db = new Database(metadata);
    db.run('CREATE TABLE bookmarks (title TEXT, external_uuid TEXT, parent INTEGER DEFAULT 0, type INTEGER DEFAULT 1, subtype INTEGER DEFAULT 2, deleted INTEGER DEFAULT 0, order_index INTEGER DEFAULT 0)');
    for (const [title, uuid, deleted] of [['', 'DefaultProfile', 0], ['Ping', workUuid, 0], ['Deleted', otherUuid, 1], ['Unsafe', '../../outside', 0]] as const) {
      db.run('INSERT INTO bookmarks (title, external_uuid, deleted) VALUES (?, ?, ?)', [title, uuid, deleted]);
    }
    db.run("INSERT INTO bookmarks (title, external_uuid, subtype) VALUES ('Tab group', 'group', 1)");
    db.close();
    const profiles = await listSourceProfiles(io, safari, context);
    expect(profiles).toEqual([{ directory: '.', name: 'Personal' }, { directory: store(workUuid), name: 'Ping' }]);
    expect(await resolveCookieDatabase(io, safari, context, '.')).toBe(posixPath.join(root, 'Cookies.binarycookies'));
    const selected = await resolveCookieDatabase(io, safari, context, profiles[1]!.directory);
    expect(selected).toBe(posixPath.join(store(workUuid), 'Cookies.binarycookies'));
    expect(NodeFs.readFileSync(selected!, 'utf8')).toBe(workUuid);
    NodeFs.rmSync(selected!);
    expect(await resolveCookieDatabase(io, safari, context, profiles[1]!.directory)).toBeUndefined();
    expect(await listSourceProfiles(io, safari, context)).toEqual(profiles);
  });
  it.each(['missing', 'corrupt'] as const)('recovers separate cookie stores when metadata is %s', async metadataState => {
    const { context, store, metadata } = fixture();
    if (metadataState === 'corrupt') write(metadata, 'invalid');
    NodeFs.rmSync(posixPath.join(store(otherUuid), 'Cookies.binarycookies'));
    expect(await listSourceProfiles(io, safari, context)).toEqual([{ directory: '.', name: 'Safari' }, { directory: store(workUuid), name: workUuid.toLowerCase() }]);
    expect(await isSourceInstalled(io, safari, context)).toBe(true);
  });
  it('keeps Safari without profiles available', async () => {
    const context = withSourceHome();
    expect(await listSourceProfiles(io, safari, context)).toEqual([{ directory: '.', name: 'Safari' }]);
    expect(await isSourceInstalled(io, safari, context)).toBe(false);
  });
});

// ── BrowserImport.test.ts ─────────────────────────────────────────────────────────────────────────
const cookie = { url: 'https://rejected.example/path', name: 'session', value: 'value', domain: undefined, path: '/', secure: true, httpOnly: true, expirationDate: undefined, sameSite: 'lax' as const };
/** Fails the test if the import reaches the target store: every case here must be refused before a cookie is read. */
const rejectedBeforeSession = async (): Promise<CookieWriter> => { throw new Error('the target store must not be reached'); };
function withImporter() {
  const context = sourcePathContext(temp('t3code-import-'), 'darwin'), root = helium.userDataDirectory(context)!;
  mkdir(`${root}/Default`); write(`${root}/Default/Cookies`, 'db');
  return { context, home: context.home, root };
}

describe('BrowserImport.importCookies', () => {
  it('rejects a source profile the browser never reported', async () => {
    const { context, home } = withImporter();
    mkdir(`${home}/secrets`); write(`${home}/secrets/Cookies`, 'not-a-db');
    const error = await importBrowserCookies(io, context, { sourceId: 'helium', sourceProfileDirectory: '../../../../secrets', targetProfileId: 'default' }, rejectedBeforeSession).catch(cause => cause);
    expect(error).toBeInstanceOf(BrowserImportFailedError);
    expect(error.reason).toBe('unknownSourceProfile');
  });
  it('refuses to import while the source browser holds its profile', async () => {
    const { context, root } = withImporter();
    NodeFs.symlinkSync('host-that-does-not-exist-1234', `${root}/SingletonLock`);
    const error = await importBrowserCookies(io, context, { sourceId: 'helium', sourceProfileDirectory: 'Default', targetProfileId: 'default' }, rejectedBeforeSession).catch(cause => cause);
    expect(error.reason).toBe('browserRunning');
  });
});

describe('BrowserImport.writeCookies', () => {
  it('counts a rejected cookie and its domain as skipped', async () => {
    let flushes = 0;
    const result = await writeCookies({ set: async batch => batch.map(() => false), flushStore: async () => { flushes += 1; } }, { cookies: [cookie], undecryptable: 0, undecryptableHosts: [] });
    expect(result).toEqual({ imported: 0, skipped: 1, skippedDomains: ['rejected.example'] });
    expect(flushes).toBe(0);
  });
  it('flushes the store after writing, and reports success if the flush fails', async () => {
    const events: string[] = [];
    const result = await writeCookies({ set: async batch => { events.push(`set ${batch.length}`); return batch.map(() => true); }, flushStore: async () => { events.push('flush'); throw new Error('fixture flush failure'); } },
      { cookies: [cookie, cookie], undecryptable: 0, undecryptableHosts: [] });
    // One flush after every write; the clone writes a batch per call (one `set 2`), not one per cookie.
    expect(events).toEqual(['set 2', 'flush']);
    expect(result).toEqual({ imported: 2, skipped: 0, skippedDomains: [] });
  });
});
