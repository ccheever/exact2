// The pages' preferences, kept inside the one versioned preference file the
// client persists (app:/data/t3-code.json, the only path the native adapter
// accepts) under `pages`: the Usage page's metric and period
// (usagePagePreferences), the Pull Requests list controls
// (pullRequestListPreferences), the first-run flag (ClientSettings
// onboardingCompletedAt) and the last chosen merge method (uiStateStore). The client writes the file after every command.
import { obj, str, num, type Obj } from '../client/domain';
import { storedCompletion } from './onboarding-time';

/** `hiddenProviders`: the providers filtered out of the Usage page, stored as hidden so new providers show by default. */
export type UsagePrefs = { metric: string; windowDays: number; hiddenProviders?: string[] };
export type PagesPrefs = { usage: UsagePrefs | null; pullRequests: Obj | null; onboardingCompletedAt: string; mergeMethod?: string; prFileTree?: boolean };
type Holder = { local: object };

const emptyPrefs = (): PagesPrefs => ({ usage: null, pullRequests: null, onboardingCompletedAt: '' });

/** The live prefs object on the client's preference record (created on first use). */
export function pagesPrefs(owner: Holder): PagesPrefs {
  const local = owner.local as { pages?: PagesPrefs };
  if (!local.pages || typeof local.pages !== 'object') local.pages = emptyPrefs();
  return local.pages;
}

/** load(): carry the saved `pages` record into a fresh preference record, shape-checked. */
export function adoptPagesPrefs(next: object, saved: Obj): void {
  const value = obj(saved.pages), usage = obj(value.usage);
  const prefs = emptyPrefs();
  const hidden = usage.hiddenProviders;
  // A record that does not decode is dropped whole, as usagePagePreferences' schema decode does.
  if (typeof usage.metric === 'string' && typeof usage.windowDays === 'number' && (hidden === undefined || Array.isArray(hidden) && hidden.every(entry => typeof entry === 'string'))) {
    prefs.usage = { metric: str(usage.metric), windowDays: num(usage.windowDays), ...(Array.isArray(hidden) ? { hiddenProviders: hidden as string[] } : {}) };
  }
  if (value.pullRequests && typeof value.pullRequests === 'object' && !Array.isArray(value.pullRequests)) prefs.pullRequests = obj(value.pullRequests);
  prefs.onboardingCompletedAt = storedCompletion(value.onboardingCompletedAt); // a 1970 value is unset
  // The merge method last chosen on this device (useUiStateStore pullRequestMergeMethod).
  if (value.mergeMethod === 'merge' || value.mergeMethod === 'squash' || value.mergeMethod === 'rebase') prefs.mergeMethod = value.mergeMethod;
  // The Code tab's file tree, open or not (t3code.pullRequestFileTreeOpen).
  if (value.prFileTree === true) prefs.prFileTree = true;
  (next as { pages?: PagesPrefs }).pages = prefs;
}
