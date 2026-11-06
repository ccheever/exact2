// Settings › Integrations › Browser › Browser profiles (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
// apps/web/src/components/settings/IntegrationsSettings.tsx `BrowserProfilesSetting`).
//
// The row lists Default and the user's profiles (Incognito holds nothing to manage), with rename, Set as default, Clear
// cookies and cache, and Remove profile and data behind "Remove “<name>”?"; its "Add profile" menu creates a blank
// profile. The reference's "Import from" rows are a declared difference (LLP 1020.002 D4): a person signs in again in
// the clone. Every action is a `restlocal:browser-…` op (device state only, no server write), as the reference's
// settings writes are client-local.
import type { T3Client } from '../client/client';
import { bridgeReply, ClientError, type Files, type Native } from '../client/protocol';
import { arr, obj, str, type Obj } from '../client/domain';
import { EnvironmentFleet, fleet } from '../connections/settings-b-fleet';
import { primaryEntry } from '../connections/local-primary';
import { pushToast } from '../shell/toast';
import { frameProfileBridge } from './browser-frames';
import {
  BROWSER_PROFILE_MAX_COUNT, BROWSER_PROFILE_NAME_MAX_LENGTH, DEFAULT_BROWSER_PROFILE_ID, browserProfilePrefs, browserProfileRemovalAvailable, clearBrowserProfileData,
  findBrowserProfile, isBuiltInBrowserProfileId, resolveBrowserProfiles, type BrowserProfile,
} from './browser-profiles';
import { applyBrowserDefault, browserDefaultsView, type BrowserDefaultsView } from './browser-defaults';
import { uuid } from '../transport/transport-wire'; // drawn ahead: code resumed by a server reply may not draw

type Ui = { removal: { profile: BrowserProfile; error: string; inFlight: boolean } | null };
const uis = new WeakMap<T3Client, Ui>();
function ui(client: T3Client): Ui {
  let state = uis.get(client);
  if (!state) { state = { removal: null }; uis.set(client, state); }
  return state;
}
const hydrated = (client: T3Client) => (client as { preferencesLoaded?: boolean }).preferencesLoaded !== false;
/** Contract sends `a=encodeURIComponent(x)&b=…` (settings-rest-commands.ts `params`). */
function params(value: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const part of value.split('&')) {
    if (!part) continue;
    const at = part.indexOf('=');
    out[decodeURIComponent(at < 0 ? part : part.slice(0, at))] = at < 0 ? '' : decodeURIComponent(part.slice(at + 1));
  }
  return out;
}

// ── The environments a profile's data lives in (useEnvironments, usePrimaryEnvironment) ────────────
/** Every environment this client knows: the primary, the saved ones and the focused one. */
export async function knownEnvironmentIds(client: T3Client, native: Native): Promise<{ ready: boolean; ids: string[] }> {
  const listed = await bridgeReply(native, { op: 'environments' }).catch(() => null);
  const ids = new Set<string>();
  const primary = primaryEntry();
  if (primary) ids.add(str(primary.environmentId));
  for (const entry of listed?.ok ? arr(obj(listed.value).saved) : []) if (str(entry.environmentId)) ids.add(str(entry.environmentId));
  if (client.environmentId) ids.add(client.environmentId);
  return { ready: !!listed?.ok, ids: [...ids].filter(Boolean) };
}

// ── The settings writes (createProfile, renameProfile, Set as default, clearProfileData, removeProfile) ──
export function createBrowserProfile(client: T3Client, baseName: string): BrowserProfile | undefined {
  if (!hydrated(client)) return undefined;
  const prefs = browserProfilePrefs(client);
  if (prefs.browserProfiles.length >= BROWSER_PROFILE_MAX_COUNT) return undefined;
  const taken = new Set(resolveBrowserProfiles(prefs.browserProfiles).map(profile => profile.name));
  let name = baseName;
  for (let index = 2; taken.has(name); index += 1) name = `${baseName} ${index}`;
  const profile: BrowserProfile = { id: `profile-${uuid()}`, name, kind: 'persistent' };
  prefs.browserProfiles = [...prefs.browserProfiles, profile];
  return profile;
}
export function renameBrowserProfile(client: T3Client, id: string, next: string): void {
  if (!hydrated(client)) return;
  const name = next.trim().slice(0, BROWSER_PROFILE_NAME_MAX_LENGTH);
  if (name === '') return;
  const prefs = browserProfilePrefs(client);
  prefs.browserProfiles = prefs.browserProfiles.map(profile => profile.id === id ? { ...profile, name } : profile);
}

/** A server-hosted profile lives on its environment. The legacy local store
 * is cleared as well, since the same profile can have desktop tabs. */
async function clearProfileEverywhere(client: T3Client, native: Native, ids: string[], profileId: string): Promise<void> {
  for (const environmentId of ids) {
    if (environmentId === client.environmentId) {
      if (obj(obj(client.config.environment).capabilities).serverBrowser === true) await client.rpc(native, 'preview.clearProfile', { profileId });
    } else {
      const entry = [...fleet.entries.values()].find(value => value.environmentId === environmentId);
      if (!entry || obj(obj(entry.config.environment).capabilities).serverBrowser !== true) continue;
      const reply = await bridgeReply(EnvironmentFleet.native(native, entry.key), { op: 'request', method: 'preview.clearProfile', payload: { profileId }, generation: entry.generation });
      if (!reply.ok) throw new ClientError(reply.error?.message ?? 'The profile could not be cleared.');
    }
  }
  await clearBrowserProfileData(frameProfileBridge(client), ids, profileId);
}

async function clearProfileData(client: T3Client, native: Native, id: string, name: string): Promise<void> {
  if (!hydrated(client)) return;
  const environments = await knownEnvironmentIds(client, native);
  if (!environments.ready || environments.ids.length === 0) {
    pushToast(client, { kind: 'error', title: `Could not clear ${name}'s data`, description: "You're not connected to a server yet." });
    return;
  }
  try {
    await clearProfileEverywhere(client, native, environments.ids, id);
    pushToast(client, { kind: 'success', title: `Cleared ${name}'s cookies and cache` });
  } catch { pushToast(client, { kind: 'error', title: `Could not clear ${name}'s data` }); }
}

async function removeProfile(client: T3Client, native: Native): Promise<void> {
  const state = ui(client), removal = state.removal;
  if (!removal || !hydrated(client)) return;
  const environments = await knownEnvironmentIds(client, native);
  if (!browserProfileRemovalAvailable(native.available, environments.ready, environments.ids.length)) { removal.error = 'Connect to an environment before removing this profile.'; return; }
  removal.error = ''; removal.inFlight = true;
  // The store's data goes too, so a removed profile's cookies are not left on disk with nothing pointing at them.
  try { await clearProfileEverywhere(client, native, environments.ids, removal.profile.id); }
  catch { removal.error = 'Profile data could not be deleted. Try again.'; removal.inFlight = false; return; }
  const prefs = browserProfilePrefs(client);
  prefs.browserProfiles = prefs.browserProfiles.filter(profile => profile.id !== removal.profile.id);
  if (prefs.browserDefaultProfileId === removal.profile.id) prefs.browserDefaultProfileId = DEFAULT_BROWSER_PROFILE_ID;
  state.removal = null;
}

const listedProfiles = (client: T3Client) => resolveBrowserProfiles(browserProfilePrefs(client).browserProfiles).filter(profile => profile.kind !== 'incognito');

/** The `restlocal:browser-…` ops of the profiles row and its dialog. */
export async function browserProfilesLocal(client: T3Client, native: Native, storage: Files, op: string, value: string): Promise<string> {
  const input = params(value), state = ui(client), prefs = browserProfilePrefs(client);
  const save = () => client.savePreferences(storage);
  switch (op) {
    case 'browser-profiles-noop': return ''; // a disabled menu row's press
    case 'browser-defaults': if (hydrated(client) && applyBrowserDefault(client, input.key ?? '', input.value ?? '')) await save(); return ''; // the default rows (browser-defaults.ts)
    case 'browser-profiles-create': if (createBrowserProfile(client, 'New profile')) await save(); return '';
    case 'browser-profiles-rename': renameBrowserProfile(client, input.id ?? '', input.name ?? ''); await save(); return '';
    case 'browser-profiles-default':
      if (hydrated(client) && findBrowserProfile(listedProfiles(client), input.id)) { prefs.browserDefaultProfileId = input.id!; await save(); }
      return '';
    case 'browser-profiles-clear': { const profile = findBrowserProfile(listedProfiles(client), input.id); if (profile) await clearProfileData(client, native, profile.id, profile.name); return ''; }
    case 'browser-profiles-remove-ask': {
      const profile = findBrowserProfile(listedProfiles(client), input.id);
      if (profile && !isBuiltInBrowserProfileId(profile.id) && hydrated(client)) state.removal = { profile, error: '', inFlight: false };
      return '';
    }
    case 'browser-profiles-remove-cancel': if (!state.removal?.inFlight) state.removal = null; return '';
    case 'browser-profiles-remove': await removeProfile(client, native); await save(); return '';
  }
  throw new ClientError(`Unknown browser profile action: ${op}`);
}

// ── The page's projection ──────────────────────────────────────────────────────────────────────────
export type BrowserProfileRowView = { id: string; name: string; builtIn: boolean; isDefault: boolean; first: boolean };
export type BrowserProfilesView = {
  hydrated: boolean; writesDisabled: boolean; atLimit: boolean; rows: BrowserProfileRowView[]; removalAvailable: boolean; removalNote: string;
  removalOpen: boolean; removalId: string; removalName: string; removalError: string; removalBusy: boolean;
  /** The group's other rows: viewport, zoom, appearance, recording, auto-show (browser-defaults.ts). */
  defaults: BrowserDefaultsView;
};
export const emptyBrowserProfilesView = (): BrowserProfilesView => ({ hydrated: false, writesDisabled: true, atLimit: false, rows: [],
  removalAvailable: false, removalNote: '', removalOpen: false, removalId: '', removalName: '', removalError: '', removalBusy: false,
  defaults: browserDefaultsView({ local: {} }) });

/** The Browser profiles row, its menus and dialog, for the Integrations page (app.ts `integrationsPage`). */
export async function browserProfilesView(client: T3Client, native: Native | null | undefined): Promise<BrowserProfilesView> {
  if (!native?.available) return emptyBrowserProfilesView();
  const state = ui(client), prefs = browserProfilePrefs(client), ready = hydrated(client);
  const listed = listedProfiles(client), resolvedDefaultId = findBrowserProfile(listed, prefs.browserDefaultProfileId)?.id ?? DEFAULT_BROWSER_PROFILE_ID;
  const environments = await knownEnvironmentIds(client, native);
  const removalAvailable = browserProfileRemovalAvailable(native.available, environments.ready, environments.ids.length);
  return {
    hydrated: ready, writesDisabled: !ready, atLimit: prefs.browserProfiles.length >= BROWSER_PROFILE_MAX_COUNT,
    rows: listed.map((profile, index) => ({ id: profile.id, name: profile.name, builtIn: isBuiltInBrowserProfileId(profile.id), isDefault: profile.id === resolvedDefaultId, first: index === 0 })),
    removalAvailable, removalNote: removalAvailable ? '' : environments.ready ? 'Connect to an environment to clear profile data' : 'Checking environments…',
    removalOpen: state.removal !== null, removalId: state.removal?.profile.id ?? '', removalName: state.removal?.profile.name ?? '', removalError: state.removal?.error ?? '', removalBusy: state.removal?.inFlight ?? false,
    defaults: browserDefaultsView(client),
  };
}
