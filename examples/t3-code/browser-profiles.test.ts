// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names: packages/contracts/src/
// browserProfile.test.ts (9 of 9; BrowserProfileId's schema is the clone's `isBrowserProfileId`) and
// apps/web/src/components/settings/IntegrationsSettings.logic.test.ts's clearBrowserProfileData (4) and
// browserProfileRemovalAvailable (1) (its importFailureReason rows are in browser-import.test.ts). Then the clone's own
// rows, each named after the reference code it follows: browserDefaults' profile resolution, the saved settings,
// PreviewView's badge, RightPanelTabs' profile lists, the settings writes of BrowserProfilesSetting and
// runWizardImport's registration of a new profile, the More menu's Clear cookies / Clear cache, and the page's projection.
import { beforeEach, describe, expect, it, mock } from 'bun:test';
import {
  BUILT_IN_BROWSER_PROFILES, DEFAULT_BROWSER_PROFILE_ID, INCOGNITO_BROWSER_PROFILE_ID, adoptBrowserProfilePrefs, browserDefaults, browserProfileChoices, browserProfileName,
  browserProfileRemovalAvailable, clearBrowserProfileData, findBrowserProfile, isBrowserProfileId, isBuiltInBrowserProfileId, launcherOffersProfiles, resolveBrowserProfiles,
  tabProfile, type BrowserProfile,
} from './browser-profiles';
import { browserProfilesLocal, browserProfilesView, createBrowserProfile, primaryEnvironmentLabel, renameBrowserProfile, runWizardImport } from './browser-profiles-settings';
import { browserHost, browserLocal, browserView, openBrowserIn } from './browser-surface';
import type { T3Client } from './client';
import { ClientError, type Files, type Native } from './protocol';
import type { Obj } from './domain';
import type { PanelState } from './r4-surfaces-panel';
import { resetPrimary } from './local-primary-fixture';

const work: BrowserProfile = { id: 'profile-work', name: 'Work', kind: 'persistent' };
// The app's one primary (local-primary.ts) is module state another test file may leave set: these rows name their own
// environments, so each starts with none (the clear-in-every-environment row counts them).
beforeEach(() => resetPrimary());

describe('resolveBrowserProfiles', () => {
  it("lists built-ins ahead of the user's own profiles", () => {
    expect(resolveBrowserProfiles([work]).map(profile => profile.id)).toEqual([DEFAULT_BROWSER_PROFILE_ID, INCOGNITO_BROWSER_PROFILE_ID, work.id]);
  });
  it('drops stored entries that collide with a built-in id', () => {
    expect(resolveBrowserProfiles([{ id: DEFAULT_BROWSER_PROFILE_ID, name: 'Hijacked', kind: 'persistent' }, { id: INCOGNITO_BROWSER_PROFILE_ID, name: 'Not incognito', kind: 'persistent' }, work]))
      .toEqual([...BUILT_IN_BROWSER_PROFILES, work]);
  });
  it('keeps incognito ephemeral', () => {
    expect(findBrowserProfile(resolveBrowserProfiles([]), INCOGNITO_BROWSER_PROFILE_ID)?.kind).toBe('incognito');
  });
});

describe('findBrowserProfile', () => {
  it('returns nothing for an id that no longer exists', () => {
    expect(findBrowserProfile(resolveBrowserProfiles([]), work.id)).toBeUndefined();
    expect(findBrowserProfile(resolveBrowserProfiles([work]), undefined)).toBeUndefined();
  });
});

describe('isBuiltInBrowserProfileId', () => {
  it('separates built-ins from user profiles', () => {
    expect(isBuiltInBrowserProfileId(DEFAULT_BROWSER_PROFILE_ID)).toBe(true);
    expect(isBuiltInBrowserProfileId(INCOGNITO_BROWSER_PROFILE_ID)).toBe(true);
    expect(isBuiltInBrowserProfileId(work.id)).toBe(false);
  });
});

describe('resolveBrowserProfiles normalization', () => {
  it('keeps only the first entry for a repeated id', () => {
    expect(resolveBrowserProfiles([{ id: 'work', name: 'Work', kind: 'persistent' }, { id: 'work', name: 'Work (old)', kind: 'persistent' }]).filter(profile => profile.id === 'work'))
      .toEqual([{ id: 'work', name: 'Work', kind: 'persistent' }]);
  });
  it('reports a custom incognito profile as persistent', () => {
    expect(resolveBrowserProfiles([{ id: 'throwaway', name: 'Throwaway', kind: 'incognito' }]).find(profile => profile.id === 'throwaway'))
      .toEqual({ id: 'throwaway', name: 'Throwaway', kind: 'persistent' });
  });
  it('still lets the built-in incognito profile stay ephemeral', () => {
    expect(resolveBrowserProfiles([]).find(profile => profile.id === INCOGNITO_BROWSER_PROFILE_ID)?.kind).toBe('incognito');
  });
});

describe('BrowserProfileId', () => {
  it('rejects control characters', () => {
    expect(isBrowserProfileId('profile-a\u0000b')).toBe(false);
    expect(isBrowserProfileId('profile-a')).toBe(true);
  });
});

const environmentId = 'environment-a', secondEnvironmentId = 'environment-b';
describe('clearBrowserProfileData', () => {
  it('waits for cookie and cache cleanup', async () => {
    const clearCookies = mock(async () => undefined), clearCache = mock(async () => undefined);
    await clearBrowserProfileData({ clearCookies, clearCache }, [environmentId], 'profile-a');
    expect(clearCookies).toHaveBeenCalledWith(environmentId, 'profile-a');
    expect(clearCache).toHaveBeenCalledWith(environmentId, 'profile-a');
  });
  it('clears every known environment before succeeding', async () => {
    const clearCookies = mock(async (_environment: string, _profile: string) => undefined), clearCache = mock(async (_environment: string, _profile: string) => undefined);
    await clearBrowserProfileData({ clearCookies, clearCache }, [environmentId, secondEnvironmentId], 'profile-a');
    expect(clearCookies.mock.calls).toEqual([[environmentId, 'profile-a'], [secondEnvironmentId, 'profile-a']]);
    expect(clearCache.mock.calls).toEqual([[environmentId, 'profile-a'], [secondEnvironmentId, 'profile-a']]);
  });
  it('propagates cleanup failures', async () => {
    const failure = new Error('clear failed');
    await expect(clearBrowserProfileData({ clearCookies: async () => { throw failure; }, clearCache: async () => undefined }, [environmentId], 'profile-a')).rejects.toBe(failure);
  });
  it('does not report success without an environment or bridge', async () => {
    const bridge = { clearCookies: mock(async () => undefined), clearCache: mock(async () => undefined) };
    await expect(clearBrowserProfileData(bridge, [], 'profile-a')).rejects.toThrow();
    await expect(clearBrowserProfileData(null, [environmentId], 'profile-a')).rejects.toThrow();
    expect(bridge.clearCookies).not.toHaveBeenCalled();
    expect(bridge.clearCache).not.toHaveBeenCalled();
  });
});

describe('browserProfileRemovalAvailable', () => {
  it('requires a ready non-empty catalog and desktop bridge', () => {
    expect(browserProfileRemovalAvailable(true, true, 1)).toBe(true);
    expect(browserProfileRemovalAvailable(true, true, 0)).toBe(false);
    expect(browserProfileRemovalAvailable(true, false, 1)).toBe(false);
    expect(browserProfileRemovalAvailable(false, true, 1)).toBe(false);
  });
});

// ── Clone rows ──────────────────────────────────────────────────────────────────────────────────
type Op = { op: string } & Obj;
/** A client with the pieces the profiles read: its preference record, `raw` to a fake module, the thread for a tab. */
function fakeClient(local: Obj = {}, answers: (request: Op) => Obj = () => ({})) {
  const native: Op[] = [];
  const client = {
    environmentId: 'local', threadId: 'thread-1', projectId: 'p1', generation: 1, connection: 'connected', ready: true, revision: 0, presentation: {} as Obj, config: { keybindings: [] } as Obj, shell: { projects: [], threads: [], sequence: 0 },
    local: { clientSettings: {}, deviceSettings: {}, ...local }, preferencesLoaded: true, savePreferences: async () => undefined,
    async raw(_native: Native, request: Op) { native.push(request); return { ok: true, generation: 0, value: answers(request) }; },
  } as unknown as T3Client;
  return { client, native };
}
const module = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: { saved: [{ environmentId: 'env-remote', origin: 'http://remote' }] } }) } as unknown as Native;
const storage = {} as Files;

describe('browserDefaults (the profile half of browserDefaults.ts)', () => {
  it('falls back to Default for a deleted profile and never makes Incognito the default', () => {
    expect(browserDefaults({ local: { browserProfiles: [work], browserDefaultProfileId: work.id } }).profileId).toBe(work.id);
    expect(browserDefaults({ local: { browserProfiles: [], browserDefaultProfileId: work.id } }).profileId).toBe(DEFAULT_BROWSER_PROFILE_ID);
    expect(browserDefaults({ local: { browserProfiles: [], browserDefaultProfileId: INCOGNITO_BROWSER_PROFILE_ID } }).profileId).toBe(DEFAULT_BROWSER_PROFILE_ID);
  });
  it('reads the saved settings, dropping malformed entries (ClientSettingsSchema)', () => {
    const next: Obj = {};
    adoptBrowserProfilePrefs(next, { browserProfiles: [work, { id: 'bad\u0001id', name: 'Bad', kind: 'persistent' }, { id: 'long', name: 'x'.repeat(49), kind: 'persistent' }, { id: 'odd', name: 'Odd', kind: 'other' }], browserDefaultProfileId: work.id });
    expect(next).toEqual({ browserProfiles: [work], browserDefaultProfileId: work.id });
    const empty: Obj = {};
    adoptBrowserProfilePrefs(empty, {});
    expect(empty).toEqual({ browserProfiles: [], browserDefaultProfileId: DEFAULT_BROWSER_PROFILE_ID });
  });
});

describe("PreviewView's profile badge and RightPanelTabs' lists", () => {
  it('badges a tab only when its profile differs from the configured default, and names a removed one', () => {
    const owner = { local: { browserProfiles: [work], browserDefaultProfileId: DEFAULT_BROWSER_PROFILE_ID } };
    expect(tabProfile(owner, undefined)).toEqual({ profileId: DEFAULT_BROWSER_PROFILE_ID, profileName: 'Default', showProfile: false });
    expect(tabProfile(owner, INCOGNITO_BROWSER_PROFILE_ID)).toEqual({ profileId: 'incognito', profileName: 'Incognito', showProfile: true });
    expect(tabProfile({ local: { browserProfiles: [work], browserDefaultProfileId: work.id } }, DEFAULT_BROWSER_PROFILE_ID)).toEqual({ profileId: 'default', profileName: 'Default', showProfile: true });
    expect(tabProfile(owner, 'profile-gone')).toEqual({ profileId: 'profile-gone', profileName: 'Removed profile', showProfile: true });
    expect(browserProfileName(resolveBrowserProfiles([work]), work.id)).toBe('Work');
  });
  it('lists Default, Incognito and the named profiles, so the launcher always offers its chevron', () => {
    const choices = browserProfileChoices({ local: { browserProfiles: [work], browserDefaultProfileId: 'default' } });
    expect(choices).toEqual([{ id: 'default', name: 'Default' }, { id: 'incognito', name: 'Incognito' }, { id: work.id, name: 'Work' }]);
    expect(launcherOffersProfiles(browserProfileChoices(null))).toBe(true);
  });
});

describe('BrowserProfilesSetting writes', () => {
  it('creates numbered blank profiles up to the cap and renames within 48 characters', () => {
    const { client } = fakeClient();
    expect(createBrowserProfile(client, 'New profile')?.name).toBe('New profile');
    expect(createBrowserProfile(client, 'New profile')?.name).toBe('New profile 2');
    const [first] = (client.local as unknown as { browserProfiles: BrowserProfile[] }).browserProfiles;
    renameBrowserProfile(client, first!.id, `  ${'n'.repeat(60)}  `);
    renameBrowserProfile(client, first!.id, '   ');
    expect((client.local as unknown as { browserProfiles: BrowserProfile[] }).browserProfiles[0]!.name).toBe('n'.repeat(48));
    for (let index = 0; index < 30; index += 1) createBrowserProfile(client, 'New profile');
    expect((client.local as unknown as { browserProfiles: BrowserProfile[] }).browserProfiles).toHaveLength(24);
    expect(createBrowserProfile(client, 'New profile')).toBeUndefined();
  });
  it('writes nothing before the settings were read', () => {
    const { client } = fakeClient();
    (client as unknown as { preferencesLoaded: boolean }).preferencesLoaded = false;
    expect(createBrowserProfile(client, 'New profile')).toBeUndefined();
  });
  it('sets the default, clears a profile in every known environment, and removes it with its data', async () => {
    const { client, native } = fakeClient({ browserProfiles: [work], browserDefaultProfileId: 'default' }, request => request.op === 'browserClearData' ? { cleared: true } : {});
    await browserProfilesLocal(client, module, storage, 'browser-profiles-default', `id=${work.id}`);
    expect((client.local as unknown as { browserDefaultProfileId: string }).browserDefaultProfileId).toBe(work.id);
    await browserProfilesLocal(client, module, storage, 'browser-profiles-clear', `id=${work.id}`);
    const cleared = native.filter(request => request.op === 'browserClearData').map(request => `${request.environment}/${request.profile}/${request.what}`);
    expect(cleared.sort()).toEqual(['env-remote/profile-work/cache', 'env-remote/profile-work/cookies', 'local/profile-work/cache', 'local/profile-work/cookies']);
    native.length = 0;
    await browserProfilesLocal(client, module, storage, 'browser-profiles-remove-ask', `id=${work.id}`);
    expect((await browserProfilesView(client, module))).toMatchObject({ removalOpen: true, removalName: 'Work', removalAvailable: true });
    await browserProfilesLocal(client, module, storage, 'browser-profiles-remove', '');
    expect(native.filter(request => request.op === 'browserClearData')).toHaveLength(4);
    expect(client.local).toMatchObject({ browserProfiles: [], browserDefaultProfileId: 'default' });
    expect((await browserProfilesView(client, module)).removalOpen).toBe(false);
  });
  it('keeps a profile whose data could not be deleted, and says so', async () => {
    const { client } = fakeClient({ browserProfiles: [work], browserDefaultProfileId: 'default' }, () => ({ cleared: false }));
    await browserProfilesLocal(client, module, storage, 'browser-profiles-remove-ask', `id=${work.id}`);
    await browserProfilesLocal(client, module, storage, 'browser-profiles-remove', '');
    expect(client.local).toMatchObject({ browserProfiles: [work] });
    expect(await browserProfilesView(client, module)).toMatchObject({ removalOpen: true, removalError: 'Profile data could not be deleted. Try again.', removalBusy: false });
  });
  it('lists Default and the named profiles (not Incognito) with the Default badge', async () => {
    const { client } = fakeClient({ browserProfiles: [work], browserDefaultProfileId: INCOGNITO_BROWSER_PROFILE_ID });
    expect((await browserProfilesView(client, module)).rows).toEqual([
      { id: 'default', name: 'Default', builtIn: true, isDefault: true, first: true }, { id: work.id, name: 'Work', builtIn: false, isDefault: false, first: false },
    ]);
  });
});

describe('runWizardImport', () => {
  const wizard = (sourceName = 'Helium') => ({ source: { id: 'helium' as const, name: sourceName, profiles: [] }, environmentId: 'local', environmentName: 'This device', step: { step: 'importing' as const },
    sourceProfileDirectory: 'Default', target: { kind: 'new' as const }, targetError: '', newProfileId: 'profile-new', opening: false, openingError: '', fdaGranted: false });
  const result = (imported: number) => async () => ({ imported, skipped: 1, skippedDomains: ['a.test'] });
  it('registers a new profile, named after the browser, only once cookies came over', async () => {
    const { client } = fakeClient({ browserProfiles: [{ id: 'profile-x', name: 'Helium', kind: 'persistent' }], browserDefaultProfileId: 'default' });
    expect(await runWizardImport(client, wizard(), { kind: 'new', profileId: 'profile-none' }, result(0), async () => undefined))
      .toEqual({ kind: 'imported', imported: 0, skipped: 1, skippedDomains: ['a.test'], targetName: 'Helium' });
    expect((client.local as unknown as { browserProfiles: BrowserProfile[] }).browserProfiles.map(profile => profile.id)).toEqual(['profile-x']);
    expect(await runWizardImport(client, wizard(), { kind: 'new', profileId: 'profile-new' }, result(3), async () => undefined)).toMatchObject({ kind: 'imported', imported: 3, targetName: 'Helium 2' });
    expect((client.local as unknown as { browserProfiles: BrowserProfile[] }).browserProfiles.map(profile => profile.name)).toEqual(['Helium', 'Helium 2']);
  });
  it('clears the cookies again when the profile cap was reached during the import', async () => {
    const full = Array.from({ length: 24 }, (_, index) => ({ id: `profile-${index}`, name: `P${index}`, kind: 'persistent' as const }));
    const { client } = fakeClient({ browserProfiles: full, browserDefaultProfileId: 'default' });
    const cleared: string[] = [];
    expect(await runWizardImport(client, wizard(), { kind: 'new', profileId: 'profile-new' }, result(2), async (environment, profile) => { cleared.push(`${environment}/${profile}`); }))
      .toEqual({ kind: 'blocked', reason: 'profileLimitReached' });
    expect(cleared).toEqual(['local/profile-new']);
  });
  it('refuses a vanished existing target and maps a failed read to its reason', async () => {
    const { client } = fakeClient({ browserProfiles: [], browserDefaultProfileId: 'default' });
    expect(await runWizardImport(client, wizard(), { kind: 'existing', profileId: 'profile-gone', name: 'Gone' }, result(1), async () => undefined)).toEqual({ kind: 'blocked', reason: 'readFailed' });
    expect(await runWizardImport(client, wizard(), { kind: 'existing', profileId: 'default', name: 'Default' }, async () => { throw new Error('Importing cookies from helium failed: browserRunning.'); }, async () => undefined))
      .toEqual({ kind: 'blocked', reason: 'browserRunning' });
  });
  it('names the destination as resolveEnvironmentOptionLabel does for the primary', () => {
    expect(primaryEnvironmentLabel('Local')).toBe('This device');
    expect(primaryEnvironmentLabel('')).toBe('This device');
    expect(primaryEnvironmentLabel('Studio Mac')).toBe('Studio Mac');
  });
});

describe("PreviewMoreMenu's Profile group", () => {
  it("clears the tab's own environment and profile only", async () => {
    const { client, native } = fakeClient({ browserProfiles: [work], browserDefaultProfileId: 'default' }, request => request.op === 'browserClearData' ? { cleared: true } : {});
    const state: PanelState = { surfaces: [], active: '', visible: false, userRevision: 0 };
    browserHost(client).store.reconcileServerSessions({ environmentId: 'local', threadId: 'thread-1' }, { sessions: [{ threadId: 'thread-1', tabId: 'tab-1', navStatus: { _tag: 'Idle' }, canGoBack: false, canGoForward: false, updatedAt: '2026-10-09T00:00:00.000Z', profileId: work.id }], serverEpoch: 'epoch-1', revision: 1 });
    const surface = openBrowserIn(state, 'tab-1', 'local:thread-1');
    expect(browserView(client, surface)).toMatchObject({ profileId: work.id, profileName: 'Work', showProfile: true });
    await browserLocal(client, module, state, 'clear-cookies', 'tab-1', '');
    await browserLocal(client, module, state, 'clear-cache', 'tab-1', '');
    expect(native.filter(request => request.op === 'browserClearData').map(request => [request.environment, request.profile, request.what])).toEqual([['local', work.id, 'cookies'], ['local', work.id, 'cache']]);
  });
});

const localState = btoa(JSON.stringify({ profile: { info_cache: { Default: { name: 'You' } } } }));
/** A fixture module: every candidate is a file, every user-data folder names one profile, nothing is locked. */
const ioAnswer = (request: Op): Obj => request.call === 'stat' ? { kind: 'File' } : request.call === 'readDirectory' ? { entries: [] }
  : request.call === 'readFile' ? { id: 'staged', size: atob(localState).length } : request.call === 'readChunk' ? { data: localState }
    : request.call === 'release' ? {} : request.call === 'query' ? { rows: [] } : { code: 'ENOENT', message: 'missing' };
describe('the browser listing under a replaced command (clone: docs/agent-pitfalls.md, a superseded send)', () => {
  it("keeps the cached list when the listing's reads were let go, instead of reading every browser as running", async () => {
    let lose = false;
    const { client } = fakeClient({}, request => request.op === 'browserImportContext' ? { allowed: true, home: '/fixture-home', platform: 'darwin' }
      : request.op === 'browserImportIO' ? ioAnswer(request) : {});
    const raw = client.raw.bind(client);
    (client as unknown as { raw: typeof raw }).raw = async (native, request) => {
      if (lose && (request as Obj).op === 'browserImportIO') throw new ClientError('replaced', 'superseded');
      return raw(native, request);
    };
    await browserProfilesLocal(client, module, storage, 'browser-profiles-sources', '');
    const listed = (await browserProfilesView(client, module)).sources.map(source => source.id);
    expect(listed).toContain('chrome');
    lose = true;
    await expect(browserProfilesLocal(client, module, storage, 'browser-profiles-sources', '')).rejects.toBeInstanceOf(ClientError);
    expect((await browserProfilesView(client, module)).sources.map(source => source.id)).toEqual(listed);
  });
});
