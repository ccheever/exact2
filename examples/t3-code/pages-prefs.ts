// Lane "pages" preferences, kept inside the one versioned preference file the
// client persists (app:/data/t3-code.json, the only path the native adapter
// accepts) under `pages`: the Usage page's metric and period
// (usagePagePreferences), the Pull Requests list controls
// (pullRequestListPreferences) and the first-run flag (ClientSettings
// onboardingCompletedAt). The client writes the file after every command.
import { obj, str, num, type Obj } from './domain';
import { storedCompletion } from './r9-connect-onboarding';

export type UsagePrefs = { metric: string; windowDays: number };
export type PagesPrefs = { usage: UsagePrefs | null; pullRequests: Obj | null; onboardingCompletedAt: string };
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
  if (typeof usage.metric === 'string' && typeof usage.windowDays === 'number') prefs.usage = { metric: str(usage.metric), windowDays: num(usage.windowDays) };
  if (value.pullRequests && typeof value.pullRequests === 'object' && !Array.isArray(value.pullRequests)) prefs.pullRequests = obj(value.pullRequests);
  prefs.onboardingCompletedAt = storedCompletion(value.onboardingCompletedAt); // r9-connect: a 1970 value is unset
  (next as { pages?: PagesPrefs }).pages = prefs;
}
