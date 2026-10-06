// Lane "shell" preferences, kept inside the one versioned preference file the
// client persists (app:/data/t3-code.json) under `shell`: the Nightly beta-app
// notice's dismissal (NightlyMobileBeta.tsx NOTICE_DISMISSED_STORAGE_KEY) and
// the threads whose inline workspace card was closed (rightPanelStore
// threadPanelVisibilityByThreadKey, which persists only `inlineOpen: false`)
// and the dismissed provider update prompts (ProviderUpdateLaunchNotification
// `t3code:provider-update-dismissals:v1`), and the dismissed version-skew notices
// (versionSkew.ts `t3code:version-mismatch-dismissals:v1`, keyed environment:client:server).
// The client writes the file after every command.
import { obj, type Obj } from './domain';

export type ShellPrefs = { nightlyNoticeDismissed: boolean; inlineClosed: string[]; providerUpdateDismissals: string[]; versionMismatchDismissals: string[] };
type Holder = { local: object };

const MAX_CLOSED = 500;
const emptyPrefs = (): ShellPrefs => ({ nightlyNoticeDismissed: false, inlineClosed: [], providerUpdateDismissals: [], versionMismatchDismissals: [] });

/** The live prefs object on the client's preference record (created on first use). */
export function shellPrefs(owner: Holder): ShellPrefs {
  const local = owner.local as { shell?: ShellPrefs };
  if (!local.shell || typeof local.shell !== 'object') local.shell = emptyPrefs();
  if (!Array.isArray(local.shell.versionMismatchDismissals)) local.shell.versionMismatchDismissals = [];
  return local.shell;
}

/** load(): carry the saved `shell` record into a fresh preference record, shape-checked. */
export function adoptShellPrefs(next: object, saved: Obj): void {
  const value = obj(saved.shell), prefs = emptyPrefs();
  prefs.nightlyNoticeDismissed = value.nightlyNoticeDismissed === true;
  if (Array.isArray(value.inlineClosed)) prefs.inlineClosed = value.inlineClosed.filter((key): key is string => typeof key === 'string' && key.length > 0 && key.length <= 512).slice(-MAX_CLOSED);
  if (Array.isArray(value.providerUpdateDismissals)) prefs.providerUpdateDismissals = value.providerUpdateDismissals.filter((key): key is string => typeof key === 'string').slice(-100);
  if (Array.isArray(value.versionMismatchDismissals)) prefs.versionMismatchDismissals = value.versionMismatchDismissals.filter((key): key is string => typeof key === 'string' && key.length <= 512).slice(-200);
  (next as { shell?: ShellPrefs }).shell = prefs;
}

/** selectThreadPanelOpen(…, "inline"): open unless this thread closed it. */
export function inlineOpen(owner: Holder, key: string): boolean {
  return !shellPrefs(owner).inlineClosed.includes(key);
}

/** toggleThreadPanel for the inline presentation: flip and remember only closed threads. */
export function toggleInline(owner: Holder, key: string): boolean {
  const prefs = shellPrefs(owner);
  const open = prefs.inlineClosed.includes(key);
  prefs.inlineClosed = open ? prefs.inlineClosed.filter(entry => entry !== key) : [...prefs.inlineClosed, key].slice(-MAX_CLOSED);
  return open;
}
