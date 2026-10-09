// browser-surface part 4 (profiles): the Browser's profiles (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// packages/contracts/src/browserProfile.ts; apps/web/src/browser/browserDefaults.ts; the profile parts of
// apps/web/src/components/preview/{PreviewView,PreviewMoreMenu,openPreviewSession}.tsx and RightPanelTabs.tsx).
//
// A tab opens under a profile and keeps it: the built-in Default (persistent) and Incognito (in memory, gone when the
// app quits), and up to 24 named profiles. Each profile is its own WebKit data store per environment
// (T3BrowserSessions.swift `store(environment:profile:)`: a persistent `WKWebsiteDataStore(forIdentifier:)`, Incognito a
// non-persistent one). The profiles and the default one are client settings, as in the reference (its Chromium guest is
// desktop-local): `browserProfiles` and `browserDefaultProfileId` in the preference file (t3-code.json).
import { arr, obj, str, type Obj } from './domain';
import { named } from './browser-import-io';

export const BROWSER_PROFILE_NAME_MAX_LENGTH = 48;
export const BROWSER_PROFILE_MAX_COUNT = 24;
export type BrowserProfileKind = 'persistent' | 'incognito';
export type BrowserProfile = { id: string; name: string; kind: BrowserProfileKind };
export const DEFAULT_BROWSER_PROFILE_ID = 'default';
export const INCOGNITO_BROWSER_PROFILE_ID = 'incognito';

/** Built-ins are synthesized rather than stored, so they cannot be renamed out of existence or deleted by hand. */
export const BUILT_IN_BROWSER_PROFILES: readonly BrowserProfile[] = [
  { id: DEFAULT_BROWSER_PROFILE_ID, name: 'Default', kind: 'persistent' },
  { id: INCOGNITO_BROWSER_PROFILE_ID, name: 'Incognito', kind: 'incognito' },
];

const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;
/** BrowserProfileId: trimmed, non-empty, at most 64 characters, no control characters (ids join delimiter-keyed caches). */
export function isBrowserProfileId(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value === value.trim() && value.length <= 64 && !CONTROL.test(value);
}
/** BrowserProfileName: trimmed, non-empty, at most 48 characters. */
export function isBrowserProfileName(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value === value.trim() && value.length <= BROWSER_PROFILE_NAME_MAX_LENGTH;
}

export function isBuiltInBrowserProfileId(id: string): boolean {
  return BUILT_IN_BROWSER_PROFILES.some(profile => profile.id === id);
}

/**
 * The full picker list: built-ins first, then the user's own. Entries colliding with a built-in id, repeated ids
 * (first wins) and `kind: "incognito"` on anything but the built-in are normalized away.
 */
export function resolveBrowserProfiles(userProfiles: readonly BrowserProfile[]): readonly BrowserProfile[] {
  const seen = new Set(BUILT_IN_BROWSER_PROFILES.map(profile => profile.id));
  const resolved: BrowserProfile[] = [...BUILT_IN_BROWSER_PROFILES];
  for (const profile of userProfiles) {
    if (seen.has(profile.id)) continue;
    seen.add(profile.id);
    resolved.push(profile.kind === 'persistent' ? profile : { ...profile, kind: 'persistent' });
  }
  return resolved;
}

export function findBrowserProfile(profiles: readonly BrowserProfile[], id: string | undefined): BrowserProfile | undefined {
  return id === undefined ? undefined : profiles.find(profile => profile.id === id);
}

// ── The client settings (ClientSettingsSchema browserProfiles, browserDefaultProfileId) ─────────────
export type BrowserProfilePrefs = { browserProfiles: BrowserProfile[]; browserDefaultProfileId: string };
type Holder = { local: object };

function decodeProfiles(value: unknown): BrowserProfile[] {
  return arr(value).flatMap(entry => isBrowserProfileId(entry.id) && isBrowserProfileName(entry.name) && (entry.kind === 'persistent' || entry.kind === 'incognito')
    ? [{ id: entry.id, name: entry.name, kind: entry.kind }] : []);
}
/** load(): the saved profiles and default profile; a malformed entry is dropped, a missing key keeps its default. */
export function adoptBrowserProfilePrefs(next: object, saved: Obj): void {
  const target = next as Partial<BrowserProfilePrefs>;
  target.browserProfiles = decodeProfiles(saved.browserProfiles);
  target.browserDefaultProfileId = isBrowserProfileId(saved.browserDefaultProfileId) ? saved.browserDefaultProfileId : DEFAULT_BROWSER_PROFILE_ID;
}
/** The live settings on the client's preference record (created on first use). */
export function browserProfilePrefs(owner: Holder): BrowserProfilePrefs {
  const local = owner.local as Partial<BrowserProfilePrefs>;
  if (!Array.isArray(local.browserProfiles)) local.browserProfiles = [];
  if (typeof local.browserDefaultProfileId !== 'string') local.browserDefaultProfileId = DEFAULT_BROWSER_PROFILE_ID;
  return local as BrowserProfilePrefs;
}

// ── browserDefaults.ts (the profile half; the viewport is part 2's) ───────────────────────────────
export type BrowserDefaults = { profiles: readonly BrowserProfile[]; profileId: string };
/**
 * toBrowserDefaults: a default pointing at a deleted profile falls back to Default, and Incognito is a per-tab choice,
 * not a default (a profile that discards everything on close would leave every new tab signed out).
 */
export function toBrowserDefaults(prefs: BrowserProfilePrefs): BrowserDefaults {
  const profiles = resolveBrowserProfiles(prefs.browserProfiles);
  return {
    profiles,
    profileId: profiles.find(profile => profile.id === prefs.browserDefaultProfileId && profile.kind !== 'incognito')?.id ?? DEFAULT_BROWSER_PROFILE_ID,
  };
}
export const browserDefaults = (owner: Holder): BrowserDefaults => toBrowserDefaults(browserProfilePrefs(owner));

/** openFileInPreview BrowserSettingsReadError: a tab is never opened from settings that were not read. */
export class BrowserSettingsReadError extends Error {
  constructor(readonly cause?: unknown) { super('Browser settings could not be read.'); named(this, 'BrowserSettingsReadError'); }
}
/** resolveBrowserDefaults: the defaults once the client's settings were actually read (`preferencesLoaded`); before that,
 *  a tab would be born under the schema defaults and never corrected, so the open is refused. */
export function resolveBrowserDefaults(owner: Holder & { preferencesLoaded?: boolean }): BrowserDefaults {
  if (owner.preferencesLoaded === false) throw new BrowserSettingsReadError();
  return browserDefaults(owner);
}

/** PreviewView previewProfileName: a tab's profile by name, "Removed profile" once it is gone. */
export const browserProfileName = (profiles: readonly { id: string; name: string }[], profileId: string): string =>
  profiles.find(profile => profile.id === profileId)?.name ?? 'Removed profile';
/** RightPanelEmptyState: the launcher's Browser row shows its profile chevron only with a choice to make. */
export const launcherOffersProfiles = (profiles: readonly unknown[]): boolean => profiles.length > 1;
/** The "+" menu's and the launcher's profile list (RightPanelTabs `useBrowserDefaults().profiles`). */
export const browserProfileChoices = (owner: Holder | null | undefined): { id: string; name: string }[] =>
  (owner ? browserDefaults(owner).profiles : BUILT_IN_BROWSER_PROFILES).map(profile => ({ id: profile.id, name: profile.name }));

/**
 * The tab's profile (PreviewView): a tab created before profiles existed runs in the built-in Default, which is what its
 * label names and its clear actions target. The badge shows only when it differs from the configured default.
 */
export function tabProfile(owner: Holder | null | undefined, snapshotProfileId: string | undefined): { profileId: string; profileName: string; showProfile: boolean } {
  const defaults = owner ? browserDefaults(owner) : { profiles: BUILT_IN_BROWSER_PROFILES, profileId: DEFAULT_BROWSER_PROFILE_ID };
  const profileId = snapshotProfileId ?? DEFAULT_BROWSER_PROFILE_ID;
  return { profileId, profileName: browserProfileName(defaults.profiles, profileId), showProfile: profileId !== defaults.profileId };
}

// ── Clearing (BrowserSession.ts clearCookies / clearCache; IntegrationsSettings clearBrowserProfileData) ──
export type BrowserProfileDataBridge = {
  clearCookies(environmentId: string, profileId: string): Promise<void>;
  clearCache(environmentId: string, profileId: string): Promise<void>;
};
/** Clears a profile's cookies and cache in every known environment; refuses without an environment or a bridge. */
export async function clearBrowserProfileData(bridge: BrowserProfileDataBridge | null, environmentIds: readonly string[], profileId: string): Promise<void> {
  if (bridge === null || environmentIds.length === 0) throw new Error('Browser profile data is not available to clear.');
  await Promise.all(environmentIds.flatMap(environmentId => [bridge.clearCookies(environmentId, profileId), bridge.clearCache(environmentId, profileId)]));
}
export function browserProfileRemovalAvailable(bridgeAvailable: boolean, environmentsReady: boolean, environmentCount: number): boolean {
  return bridgeAvailable && environmentsReady && environmentCount > 0;
}

type Raw = (request: Obj) => Promise<{ ok: boolean; value?: unknown; error?: unknown }>;
/**
 * The module's half of the bridge (T3BrowserSessions+Profiles.swift `browserClearData`): cookies are cookies, local
 * storage, IndexedDB and service workers (the reference's `clearStorageData` storages); the cache is the HTTP cache.
 * Only that environment's store for that profile is touched.
 */
export function nativeProfileBridge(raw: Raw): BrowserProfileDataBridge {
  const clear = async (environmentId: string, profileId: string, what: 'cookies' | 'cache') => {
    const reply = await raw({ op: 'browserClearData', environment: environmentId, profile: profileId, what });
    if (!reply.ok || obj(reply.value).cleared !== true) throw new Error(str(obj(reply.error).message, `Could not clear the ${what}.`));
  };
  return { clearCookies: (environmentId, profileId) => clear(environmentId, profileId, 'cookies'), clearCache: (environmentId, profileId) => clear(environmentId, profileId, 'cache') };
}
