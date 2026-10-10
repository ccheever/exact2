// browser-surface part 4 (profiles): Settings › Integrations › Browser › Browser profiles and the cookie import wizard
// (MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975: apps/web/src/components/settings/IntegrationsSettings.tsx
// `BrowserProfilesSetting`, BrowserImportWizard.tsx; apps/web/src/components/permissions/PermissionChecklist.tsx).
//
// The row lists Default and the user's profiles (Incognito holds nothing to manage), with rename, Set as default, Clear
// cookies and cache, and Remove profile and data behind "Remove “<name>”?"; its "Add profile" menu creates a blank
// profile or opens the import wizard for a browser found on this machine. The wizard's state lives here (its steps
// follow async reads and writes), and the page draws it from `wizard` (browser-profiles.contract). Every action is a
// `restlocal:browser-…` op (device state only, no server write), as the reference's settings writes are client-local.
import type { T3Client } from './client';
import { bridgeReply, ClientError, type Files, type Native } from './protocol';
import { arr, obj, str, type Obj } from './domain';
import { primaryEntry } from './local-primary';
import { letGo } from './let-go';
import { pushToast } from './toast';
import {
  BROWSER_PROFILE_MAX_COUNT, BROWSER_PROFILE_NAME_MAX_LENGTH, DEFAULT_BROWSER_PROFILE_ID, browserProfilePrefs, browserProfileRemovalAvailable, clearBrowserProfileData,
  findBrowserProfile, isBuiltInBrowserProfileId, nativeProfileBridge, resolveBrowserProfiles, type BrowserProfile,
} from './browser-profiles';
import {
  BROWSER_IMPORT_FAILURE_COPY, canCloseWizard, cookieCountLabel, doneCopy, formatSkippedDomains, fullDiskAccessRecheckStep, importFailureReason, initialTargetSelection,
  initialWizardStep, isRetryableReason, outcomeToStep, refreshedSourceProfileDirectory, refreshedSourceStep, resolveWizardTarget,
  type BrowserImportSource, type BrowserImportSourceId, type ImportOutcome, type WizardStep, type WizardTarget, type WizardTargetSelection,
} from './browser-import';
import { nativeImportIO, type ImportIO } from './browser-import-io';
import { importBrowserCookies, listBrowserImportSources, sourcePathContext, type CookieWriter } from './browser-import-service';
import { safariPermissionCheck } from './browser-import-safari';
import type { BrowserImportPathContext } from './browser-import-sources';
import { applyBrowserDefault, browserDefaultsView, type BrowserDefaultsView } from './browser-defaults';

type Wizard = {
  source: BrowserImportSource; environmentId: string; environmentName: string; step: WizardStep; sourceProfileDirectory: string;
  target: WizardTargetSelection; targetError: string; newProfileId: string; opening: boolean; openingError: string; fdaGranted: boolean;
};
type Ui = { sources: BrowserImportSource[] | null; wizard: Wizard | null; removal: { profile: BrowserProfile; error: string; inFlight: boolean } | null; importInFlight: boolean; serial: number };
const uis = new WeakMap<T3Client, Ui>();
function ui(client: T3Client): Ui {
  let state = uis.get(client);
  if (!state) { state = { sources: null, wizard: null, removal: null, importInFlight: false, serial: 0 }; uis.set(client, state); }
  return state;
}
const hydrated = (client: T3Client) => (client as { preferencesLoaded?: boolean }).preferencesLoaded !== false;
const raw = (client: T3Client, native: Native) => async (request: Obj) => client.raw(native, request);
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
const GENERIC_LOCAL_ENVIRONMENT_LABELS = new Set(['local', 'local environment']);
/** resolveEnvironmentOptionLabel for the primary: its own label unless it is a generic one, else "This device". */
export function primaryEnvironmentLabel(label: string | undefined): string {
  const trimmed = label?.trim();
  return trimmed && !GENERIC_LOCAL_ENVIRONMENT_LABELS.has(trimmed.toLowerCase()) ? trimmed : 'This device';
}

// ── The cookie import through the module ─────────────────────────────────────────────────────────
async function importContext(client: T3Client, native: Native): Promise<{ io: ImportIO & { letGoSeen(): boolean }; context: BrowserImportPathContext } | null> {
  const reply = await client.raw(native, { op: 'browserImportContext' });
  const value = obj(reply.value);
  if (!reply.ok || value.allowed !== true || !str(value.home)) return null;
  return { io: nativeImportIO(raw(client, native)), context: sourcePathContext(str(value.home), 'darwin') };
}
/** `browserImportCookies`: the environment's WebKit store for the profile (T3BrowserSessions+Profiles.swift). */
function nativeCookieWriter(client: T3Client, native: Native, environmentId: string, profileId: string): CookieWriter {
  return {
    async set(cookies) {
      const reply = await client.raw(native, { op: 'browserImportCookies', environment: environmentId, profile: profileId, cookies: cookies.map(cookie => ({ ...cookie, domain: cookie.domain ?? null, expirationDate: cookie.expirationDate ?? null })) });
      if (!reply.ok) throw new Error(str(obj(reply.error).message, 'The target profile could not be opened.'));
      const results = obj(reply.value).results;
      return Array.isArray(results) ? results.map(result => result === true) : cookies.map(() => false);
    },
    // WebKit's cookie store persists what it holds on its own; there is nothing to flush.
    async flushStore() { /* WKHTTPCookieStore has no flush */ },
  };
}

/** A redraw while the wizard's command runs: the revision moves and the module announces `t3.status`, so the page
 *  re-reads now (`browserImportProgress`, T3BrowserSessions+Profiles.swift). */
async function progress(client: T3Client, native: Native): Promise<void> {
  client.revision++;
  await client.raw(native, { op: 'browserImportProgress' }).catch(() => undefined);
}

async function loadSources(client: T3Client, native: Native): Promise<BrowserImportSource[]> {
  const state = ui(client);
  try {
    const found = await importContext(client, native);
    const listed = found ? await listBrowserImportSources(found.io, found.context) : [];
    // A listing whose reads Exact let go (a newer command replaced this one) is no answer: the cached list stays.
    if (found?.io.letGoSeen()) throw new ClientError('The browser listing was replaced.', 'superseded');
    state.sources = listed;
    const summary = listed.map(source => `${source.id}=${source.unavailable ?? `ready(${source.profiles.length})`}`).join(' ');
    await client.raw(native, { op: 'browserImportLog', line: `sources ${found ? summary : 'none (no fixture home in this build)'}` }).catch(() => undefined);
  } catch (error) { if (letGo(error)) throw error; state.sources = state.sources ?? []; }
  return state.sources;
}
/** A browser not on this machine, or one this platform cannot read, is left out of the menu; every other reason stays. */
const importable = (sources: readonly BrowserImportSource[]) => sources.filter(source => source.unavailable !== 'notInstalled' && source.unavailable !== 'unsupportedPlatform');

// ── The settings writes (createProfile, renameProfile, Set as default, clearProfileData, removeProfile) ──
export function createBrowserProfile(client: T3Client, baseName: string): BrowserProfile | undefined {
  if (!hydrated(client) || ui(client).importInFlight) return undefined;
  const prefs = browserProfilePrefs(client);
  if (prefs.browserProfiles.length >= BROWSER_PROFILE_MAX_COUNT) return undefined;
  const taken = new Set(resolveBrowserProfiles(prefs.browserProfiles).map(profile => profile.name));
  let name = baseName;
  for (let index = 2; taken.has(name); index += 1) name = `${baseName} ${index}`;
  const profile: BrowserProfile = { id: `profile-${crypto.randomUUID()}`, name, kind: 'persistent' };
  prefs.browserProfiles = [...prefs.browserProfiles, profile];
  return profile;
}
export function renameBrowserProfile(client: T3Client, id: string, next: string): void {
  if (!hydrated(client) || ui(client).importInFlight) return;
  const name = next.trim().slice(0, BROWSER_PROFILE_NAME_MAX_LENGTH);
  if (name === '') return;
  const prefs = browserProfilePrefs(client);
  prefs.browserProfiles = prefs.browserProfiles.map(profile => profile.id === id ? { ...profile, name } : profile);
}

async function clearProfileData(client: T3Client, native: Native, id: string, name: string): Promise<void> {
  if (!hydrated(client) || ui(client).importInFlight) return;
  const environments = await knownEnvironmentIds(client, native);
  if (!environments.ready || environments.ids.length === 0) {
    pushToast(client, { kind: 'error', title: `Could not clear ${name}'s data`, description: "You're not connected to a server yet." });
    return;
  }
  try {
    await clearBrowserProfileData(nativeProfileBridge(raw(client, native)), environments.ids, id);
    pushToast(client, { kind: 'success', title: `Cleared ${name}'s cookies and cache` });
  } catch { pushToast(client, { kind: 'error', title: `Could not clear ${name}'s data` }); }
}

async function removeProfile(client: T3Client, native: Native): Promise<void> {
  const state = ui(client), removal = state.removal;
  if (!removal || !hydrated(client) || state.importInFlight) return;
  const environments = await knownEnvironmentIds(client, native);
  if (!browserProfileRemovalAvailable(native.available, environments.ready, environments.ids.length)) { removal.error = 'Connect to an environment before removing this profile.'; return; }
  removal.error = ''; removal.inFlight = true;
  // The store's data goes too, so a removed profile's cookies are not left on disk with nothing pointing at them.
  try { await clearBrowserProfileData(nativeProfileBridge(raw(client, native)), environments.ids, removal.profile.id); }
  catch { removal.error = 'Profile data could not be deleted. Try again.'; removal.inFlight = false; return; }
  const prefs = browserProfilePrefs(client);
  prefs.browserProfiles = prefs.browserProfiles.filter(profile => profile.id !== removal.profile.id);
  if (prefs.browserDefaultProfileId === removal.profile.id) prefs.browserDefaultProfileId = DEFAULT_BROWSER_PROFILE_ID;
  state.removal = null;
}

// ── The wizard (BrowserImportWizard + runWizardImport + refreshImportSource) ──────────────────────
const listedProfiles = (client: T3Client) => resolveBrowserProfiles(browserProfilePrefs(client).browserProfiles).filter(profile => profile.kind !== 'incognito');
const targetProfiles = (client: T3Client) => listedProfiles(client).map(profile => ({ id: profile.id, name: profile.name }));
const canCreateProfile = (client: T3Client) => hydrated(client) && browserProfilePrefs(client).browserProfiles.length < BROWSER_PROFILE_MAX_COUNT;

// import-wizard-initial-focus: when the wizard opened, when its screen last changed, and when each of its focusable
// elements was last mounted, on one clock that every opening, change of screen (a step of another kind) and grant flip
// ticks. React mounts a step's buttons and tiles with the step (a step of another kind swaps the step component), Allow
// with the step or with a grant taken back (a grant replaces it with "Allowed"), and the close X whenever a step that can
// close follows one that cannot (DialogPopup renders it after the step, so a change between two closable steps keeps it).
// Base UI moves the focus to the popup only when the element that held it was removed (`restoreFocus`), so the root
// (app.contract importWizardStepFocus) keeps the focus where it is when that element is mounted since the change it last
// saw, also when the steps between came and went before the page was read again (Quit → Checking → Quit).
let focusClock = 0;
type Mounts = { opened: number; at: number; step: number; allow: number; x: number };
const mounts = new WeakMap<object, Mounts>();
export function wizardMounts(wizard: object): Mounts {
  let marks = mounts.get(wizard);
  if (!marks) { const now = ++focusClock; marks = { opened: now, at: now, step: now, allow: now, x: now }; mounts.set(wizard, marks); }
  return marks;
}
/** Every place the wizard's step is set: a step of another kind is a new screen; the same step set again (Configure after
 *  a vanished target) keeps its screen. */
export function goTo(wizard: Pick<Wizard, 'step'>, step: WizardStep): void {
  if (step.step !== wizard.step.step) {
    const marks = wizardMounts(wizard), now = ++focusClock;
    marks.at = marks.step = marks.allow = now;
    if (canCloseWizard(step) && !canCloseWizard(wizard.step)) marks.x = now;
  }
  wizard.step = step;
}
/** usePermissionStatus's answer on Full Disk Access: a grant replaces Allow, a grant taken back mounts it again. */
export function setGranted(wizard: Pick<Wizard, 'fdaGranted'>, granted: boolean): void {
  if (granted !== wizard.fdaGranted) {
    const marks = wizardMounts(wizard), now = ++focusClock;
    marks.at = now;
    if (!granted) marks.allow = now;
  }
  wizard.fdaGranted = granted;
}

function openWizard(client: T3Client, sourceId: string): void {
  const state = ui(client), source = (state.sources ?? []).find(entry => entry.id === sourceId), primary = primaryEntry();
  if (!source || !hydrated(client) || !primary) return;
  state.wizard = {
    source, environmentId: str(primary.environmentId), environmentName: primaryEnvironmentLabel(str(primary.label)), step: initialWizardStep(source),
    sourceProfileDirectory: source.profiles[0]?.directory ?? '', target: initialTargetSelection(canCreateProfile(client), targetProfiles(client)), targetError: '',
    // Stable across retries, so a keychain re-approval lands in one profile, not a new one each time.
    newProfileId: `profile-${crypto.randomUUID()}`, opening: false, openingError: '', fdaGranted: false,
  };
  wizardMounts(state.wizard);
}

/** runWizardImport: a new profile is registered only once something came over, so a blocked attempt leaves none behind. */
export async function runWizardImport(client: T3Client, wizard: Wizard, target: WizardTarget, run: (targetProfileId: string) => Promise<{ imported: number; skipped: number; skippedDomains: string[] }>,
  clear: (environmentId: string, profileId: string) => Promise<void>): Promise<ImportOutcome> {
  const state = ui(client), stillListed = () => resolveBrowserProfiles(browserProfilePrefs(client).browserProfiles).some(profile => profile.id === target.profileId);
  if (!hydrated(client)) return { kind: 'blocked', reason: 'sessionUnavailable' };
  if (target.kind === 'existing' && !stillListed()) return { kind: 'blocked', reason: 'readFailed' };
  if (state.importInFlight) return { kind: 'blocked', reason: 'readFailed' };
  state.importInFlight = true;
  try {
    const result = await run(target.profileId);
    if (target.kind === 'existing' && !stillListed()) return { kind: 'blocked', reason: 'readFailed' };
    let targetName: string;
    if (target.kind === 'new') {
      if (result.imported > 0) {
        const prefs = browserProfilePrefs(client), existing = prefs.browserProfiles.find(profile => profile.id === target.profileId);
        if (existing) targetName = existing.name;
        else if (prefs.browserProfiles.length >= BROWSER_PROFILE_MAX_COUNT) {
          // The cap can be reached while the import runs: the cookies came over and are cleared again with the profile.
          await clear(wizard.environmentId, target.profileId).catch(() => undefined);
          throw new Error(`Importing cookies from ${wizard.source.id} failed: profileLimitReached.`);
        } else {
          const taken = new Set(resolveBrowserProfiles(prefs.browserProfiles).map(profile => profile.name));
          let name = wizard.source.name;
          for (let index = 2; taken.has(name); index += 1) name = `${wizard.source.name} ${index}`;
          prefs.browserProfiles = [...prefs.browserProfiles, { id: target.profileId, name, kind: 'persistent' }];
          targetName = name;
        }
      } else targetName = wizard.source.name;
    } else targetName = target.name;
    return { kind: 'imported', imported: result.imported, skipped: result.skipped, skippedDomains: result.skippedDomains, targetName };
  } catch (cause) { return { kind: 'blocked', reason: importFailureReason(cause) }; }
  finally { state.importInFlight = false; }
}

async function startImport(client: T3Client, native: Native, storage: Files): Promise<void> {
  const state = ui(client), wizard = state.wizard;
  if (!wizard || state.importInFlight) return;
  const chosen = resolveWizardTarget(wizard.target, wizard.newProfileId, targetProfiles(client));
  if (chosen === undefined) { wizard.targetError = 'That profile is no longer available. Choose where to import these cookies.'; goTo(wizard, { step: 'configure' }); return; }
  wizard.targetError = ''; goTo(wizard, { step: 'importing' });
  await progress(client, native); // the page shows "Importing cookies" while the read and the writes run
  const found = await importContext(client, native).catch(() => null);
  const outcome = await runWizardImport(client, wizard, chosen, async targetProfileId => {
    if (!found) throw new Error(`Importing cookies from ${wizard.source.id} failed: sessionUnavailable.`);
    return importBrowserCookies(found.io, found.context, { sourceId: wizard.source.id as BrowserImportSourceId, sourceProfileDirectory: wizard.sourceProfileDirectory, targetProfileId },
      async () => nativeCookieWriter(client, native, wizard.environmentId, targetProfileId));
  }, (environmentId, profileId) => clearBrowserProfileData(nativeProfileBridge(raw(client, native)), [environmentId], profileId)).catch((): ImportOutcome => ({ kind: 'blocked', reason: 'readFailed' }));
  // An import whose reads Exact let go made no answer: back to the choice (the reference's wizard cannot be left mid-import).
  if (found?.io.letGoSeen()) { if (state.wizard === wizard) goTo(wizard, { step: 'configure' }); throw new ClientError('The import was replaced.', 'superseded'); }
  if (state.wizard === wizard) goTo(wizard, outcomeToStep(outcome));
  await client.savePreferences(storage);
}

/** Re-lists the source after the user did something outside the app and routes to where the refreshed source says. */
async function recheck(client: T3Client, native: Native, check: 'browser' | 'fullDiskAccess'): Promise<void> {
  const wizard = ui(client).wizard;
  if (!wizard) return;
  goTo(wizard, { step: 'checking', check });
  await progress(client, native);
  let refreshed: BrowserImportSource | undefined;
  try { refreshed = (await loadSources(client, native)).find(source => source.id === wizard.source.id); }
  catch (error) { if (letGo(error)) throw error; goTo(wizard, { step: 'blocked', reason: 'readFailed' }); return; }
  if (refreshed) { wizard.source = refreshed; wizard.sourceProfileDirectory = refreshedSourceProfileDirectory(wizard.sourceProfileDirectory, refreshed); }
  goTo(wizard, check === 'browser' ? refreshedSourceStep(refreshed) : fullDiskAccessRecheckStep(refreshed));
}

/** PermissionChecklist's Allow: System Settings › Privacy & Security › Full Disk Access (a development build records it
 *  instead of opening anything, T3BrowserImportIO.swift; the clone never grants the permission itself). */
async function allowFullDiskAccess(client: T3Client, native: Native): Promise<void> {
  const wizard = ui(client).wizard;
  if (!wizard || wizard.opening) return;
  wizard.opening = true; wizard.openingError = '';
  try {
    const reply = await client.raw(native, { op: 'browserImportOpenSettings' });
    if (!reply.ok || obj(reply.value).opened !== true) {
      wizard.openingError = 'Could not open System Settings. Try Allow again.';
      pushToast(client, { kind: 'error', title: 'Could not open System Settings', description: 'Open Privacy & Security → Full Disk Access manually.' });
    }
  } finally { wizard.opening = false; }
}

/** The `restlocal:browser-…` ops of the profiles row and the wizard. */
export async function browserProfilesLocal(client: T3Client, native: Native, storage: Files, op: string, value: string): Promise<string> {
  const input = params(value), state = ui(client), prefs = browserProfilePrefs(client), wizard = state.wizard;
  const save = () => client.savePreferences(storage);
  switch (op) {
    case 'browser-profiles-noop': return ''; // a disabled menu row's press
    case 'browser-defaults': if (hydrated(client) && applyBrowserDefault(client, input.key ?? '', input.value ?? '')) await save(); return ''; // the default rows (browser-defaults.ts)
    case 'browser-profiles-sources': await loadSources(client, native); return '';
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
    case 'browser-import-open': openWizard(client, input.source ?? ''); return '';
    case 'browser-import-close': if (wizard && canCloseWizard(wizard.step)) state.wizard = null; return '';
    case 'browser-import-from': if (wizard && wizard.source.profiles.some(profile => profile.directory === input.directory)) wizard.sourceProfileDirectory = input.directory!; return '';
    case 'browser-import-into':
      if (wizard) { wizard.target = input.target === 'new' ? { kind: 'new' } : { kind: 'existing', profileId: input.target ?? '' }; wizard.targetError = ''; }
      return '';
    case 'browser-import-run': case 'browser-import-retry': await startImport(client, native, storage); return '';
    case 'browser-import-quit': await recheck(client, native, 'browser'); return '';
    case 'browser-import-fda-allow': await allowFullDiskAccess(client, native); return '';
    case 'browser-import-fda-continue':
      if (wizard?.step.step === 'fullDiskAccess') { if (wizard.step.resume === 'import') await startImport(client, native, storage); else await recheck(client, native, 'fullDiskAccess'); }
      return '';
  }
  throw new ClientError(`Unknown browser profile action: ${op}`);
}

// ── The page's projection ──────────────────────────────────────────────────────────────────────────
export type BrowserProfileRowView = { id: string; name: string; builtIn: boolean; isDefault: boolean; first: boolean };
export type WizardTileView = { key: string; title: string; subtitle: string; selected: boolean };
export type BrowserImportWizardView = {
  open: boolean; step: string; sourceName: string; environmentName: string; canClose: boolean; check: string;
  from: WizardTileView[]; into: WizardTileView[]; feedback: string; importDisabled: boolean;
  fdaGranted: boolean; fdaStillRequired: boolean; fdaBusy: boolean; fdaNote: string; fdaResume: string;
  doneTitle: string; doneDescription: string; skipped: string; blockedText: string; retry: boolean;
  /** Where the focus goes as the wizard opens and the first and last Tab stops (Tab and Shift+Tab from the popup itself);
   *  when it opened and last changed screen, and since when each focusable element is mounted (app.contract
   *  importWizardStepFocus; -1 and none while closed). */
  openFocus: string; tabFirst: string; tabLast: string; openedAt: number; focusAt: number; mounted: WizardMount[];
};
export type WizardMount = { id: string; since: number };
export type BrowserProfilesView = {
  hydrated: boolean; writesDisabled: boolean; importInFlight: boolean; atLimit: boolean; rows: BrowserProfileRowView[];
  sourcesState: string; sources: { id: string; name: string }[]; canImport: boolean; removalAvailable: boolean; removalNote: string;
  removalOpen: boolean; removalId: string; removalName: string; removalError: string; removalBusy: boolean; wizard: BrowserImportWizardView;
  /** The group's other rows: viewport, zoom, appearance, recording, auto-show (browser-defaults.ts). */
  defaults: BrowserDefaultsView;
};
const closedWizard = (): BrowserImportWizardView => ({ open: false, step: '', sourceName: '', environmentName: '', canClose: true, check: '', from: [], into: [], feedback: '', importDisabled: true,
  fdaGranted: false, fdaStillRequired: false, fdaBusy: false, fdaNote: '', fdaResume: '', doneTitle: '', doneDescription: '', skipped: '', blockedText: '', retry: false,
  openFocus: '', tabFirst: '', tabLast: '', openedAt: -1, focusAt: -1, mounted: [] });
export const emptyBrowserProfilesView = (): BrowserProfilesView => ({ hydrated: false, writesDisabled: true, importInFlight: false, atLimit: false, rows: [], sourcesState: 'loading', sources: [],
  canImport: false, removalAvailable: false, removalNote: '', removalOpen: false, removalId: '', removalName: '', removalError: '', removalBusy: false, wizard: closedWizard(),
  defaults: browserDefaultsView({ local: {} }) });

/** import-wizard-initial-focus: the wizard's focusable elements in tree order, as Base UI's focus trap walks them: the
 *  screen's tiles and buttons, then the close X (ui/dialog DialogPopup renders it after the step), each with whether it
 *  is enabled (a Tab stop) and since when it is mounted (`wizardMounts`). The first stop takes the focus as the wizard
 *  opens (initialFocus); from the popup itself, which a step change that removed the focused element leaves focused
 *  (restoreFocus "popup"), Tab goes to the first and Shift+Tab (the trap's guard) to the last. The ids are
 *  browser-profiles.contract's. */
type TabStopFacts = Pick<BrowserImportWizardView, 'step' | 'canClose' | 'importDisabled' | 'fdaGranted' | 'fdaBusy' | 'retry'> & { from: unknown[]; into: unknown[] };
type WizardElement = WizardMount & { enabled: boolean };
export function wizardElements(view: TabStopFacts, marks: Pick<Mounts, 'step' | 'allow' | 'x'> = { step: 0, allow: 0, x: 0 }): WizardElement[] {
  const of = (id: string, enabled = true): WizardElement => ({ id, since: marks.step, enabled });
  const close = view.canClose ? [{ id: 'browser-import-x', since: marks.x, enabled: true }] : [];
  switch (view.step) {
    case 'configure': return [...view.from.map((_, i) => of(`browser-import-from-${i}`)), ...view.into.map((_, i) => of(`browser-import-into-${i}`)), of('browser-import-cancel'),
      of('browser-import-run', !view.importDisabled), ...close];
    case 'quit': return [of('browser-import-cancel'), of('browser-import-quit'), ...close];
    case 'fullDiskAccess': return [...(view.fdaGranted ? [] : [{ id: 'browser-import-fda-allow', since: marks.allow, enabled: !view.fdaBusy }]), of('browser-import-cancel'),
      of('browser-import-fda-continue', view.fdaGranted && !view.fdaBusy), ...close];
    case 'done': return [of('browser-import-done'), ...close];
    case 'blocked': return [of('browser-import-close'), ...(view.retry ? [of('browser-import-retry')] : []), ...close];
    default: return close; // importing (no X) and checking: only the X
  }
}
export const wizardTabStops = (view: TabStopFacts): string[] => wizardElements(view).filter(element => element.enabled).map(element => element.id);
/** As the wizard opens: its first Tab stop. On Full Disk Access already granted, the reference first focuses Allow, which
 *  the grant then replaces with "Allowed", so the popup itself ends with it. */
export function wizardOpenFocus(view: TabStopFacts): string {
  return view.step === 'fullDiskAccess' && view.fdaGranted ? 'browser-import-popup' : wizardTabStops(view)[0] ?? 'browser-import-popup';
}
async function wizardView(client: T3Client, native: Native, wizard: Wizard | null): Promise<BrowserImportWizardView> {
  if (!wizard) return closedWizard();
  const step = wizard.step, profiles = targetProfiles(client), creatable = canCreateProfile(client);
  if (step.step === 'fullDiskAccess') {
    // usePermissionStatus: the grant is read again whenever the page is drawn, which app.contract's fdaPolling asks for every
    // 1.5 s and when the window takes the focus.
    const found = await importContext(client, native).catch(() => null);
    const granted = found ? await safariPermissionCheck(found.io, found.context)().catch(() => false) : false;
    if (!found?.io.letGoSeen()) setGranted(wizard, granted);
  }
  const targetMissing = wizard.target.kind === 'existing' && !profiles.some(profile => profile.id === (wizard.target as { profileId: string }).profileId);
  const targetUncreatable = wizard.target.kind === 'new' && !creatable;
  const feedback = wizard.targetError || (targetMissing ? 'That profile is no longer available. Choose where to import these cookies.'
    : targetUncreatable ? "You've reached the profile limit. Choose an existing profile to import into." : '');
  const done = step.step === 'done' ? doneCopy(step, wizard.environmentName) : { title: '', description: '' };
  const view: BrowserImportWizardView = {
    open: true, step: step.step, sourceName: wizard.source.name, environmentName: wizard.environmentName, canClose: canCloseWizard(step), check: step.step === 'checking' ? step.check : '',
    from: wizard.source.profiles.map(profile => ({ key: profile.directory, title: profile.name, subtitle: cookieCountLabel(profile.cookieCount), selected: wizard.sourceProfileDirectory === profile.directory })),
    into: [...(creatable ? [{ key: 'new', title: 'New profile', subtitle: 'Created for these cookies', selected: wizard.target.kind === 'new' }] : []),
      ...profiles.map(profile => ({ key: `existing:${profile.id}`, title: profile.name, subtitle: 'Existing profile', selected: wizard.target.kind === 'existing' && wizard.target.profileId === profile.id }))],
    feedback, importDisabled: wizard.sourceProfileDirectory === '' || targetMissing || targetUncreatable,
    fdaGranted: wizard.fdaGranted, fdaStillRequired: step.step === 'fullDiskAccess' && step.checked === true, fdaBusy: wizard.opening,
    fdaNote: wizard.openingError || (wizard.fdaGranted ? '' : step.step === 'fullDiskAccess' && step.checked === true
      ? 'Access is still required. Quit and reopen T3 Code if you just allowed it, then retry the import.'
      : "If access doesn't update after you allow it, quit and reopen T3 Code, then retry the import."),
    fdaResume: step.step === 'fullDiskAccess' ? step.resume : '',
    doneTitle: done.title, doneDescription: done.description, skipped: step.step === 'done' ? formatSkippedDomains(step.skippedDomains) : '',
    blockedText: step.step === 'blocked' ? BROWSER_IMPORT_FAILURE_COPY[step.reason] : '', retry: step.step === 'blocked' && isRetryableReason(step.reason),
    openFocus: '', tabFirst: '', tabLast: '', openedAt: -1, focusAt: -1, mounted: [],
  };
  const marks = wizardMounts(wizard), elements = wizardElements(view, marks), stops = elements.filter(element => element.enabled);
  return { ...view, openFocus: wizardOpenFocus(view), tabFirst: stops[0]?.id ?? '', tabLast: stops[stops.length - 1]?.id ?? '', openedAt: marks.opened, focusAt: marks.at,
    mounted: elements.map(({ id, since }) => ({ id, since })) };
}

/** The Browser profiles row, its menus and dialogs, for the Integrations page (source-control-view.ts integrationsPage). */
export async function browserProfilesView(client: T3Client, native: Native | null | undefined): Promise<BrowserProfilesView> {
  if (!native?.available) return emptyBrowserProfilesView();
  const state = ui(client), prefs = browserProfilePrefs(client), ready = hydrated(client);
  const listed = listedProfiles(client), resolvedDefaultId = findBrowserProfile(listed, prefs.browserDefaultProfileId)?.id ?? DEFAULT_BROWSER_PROFILE_ID;
  const environments = await knownEnvironmentIds(client, native);
  const removalAvailable = browserProfileRemovalAvailable(native.available, environments.ready, environments.ids.length);
  const sources = importable(state.sources ?? []);
  return {
    hydrated: ready, writesDisabled: !ready || state.importInFlight, importInFlight: state.importInFlight, atLimit: prefs.browserProfiles.length >= BROWSER_PROFILE_MAX_COUNT,
    rows: listed.map((profile, index) => ({ id: profile.id, name: profile.name, builtIn: isBuiltInBrowserProfileId(profile.id), isDefault: profile.id === resolvedDefaultId, first: index === 0 })),
    sourcesState: state.sources === null ? 'loading' : sources.length === 0 ? 'empty' : 'ready', sources: sources.map(source => ({ id: source.id, name: source.name })),
    canImport: ready && primaryEntry() !== null, removalAvailable,
    removalNote: removalAvailable ? '' : environments.ready ? 'Connect to an environment to clear profile data' : 'Checking environments…',
    removalOpen: state.removal !== null, removalId: state.removal?.profile.id ?? '', removalName: state.removal?.profile.name ?? '', removalError: state.removal?.error ?? '', removalBusy: state.removal?.inFlight ?? false,
    wizard: await wizardView(client, native, state.wizard), defaults: browserDefaultsView(client),
  };
}
