// Selection and pace maths for the provider limits, T3 Code 74541bab17 (MIT, see LICENSE-T3):
// packages/shared/src/usageLimits.ts (providersWithLimits, accountKey, accountKeyResolver, limitsNotice,
// remainingPercent, elapsedShare, paceRemainingPercent, paceOf, formatDuration, formatResetsIn, USAGE_LIMITS_COMMAND,
// isUsageLimitsCommand, hasProviderUsageLimits, withUsageLimitsCommands,
// collectProviderUsageLimits) and apps/web/src/components/usage/UsageLimits.tsx (barColor, PACE).
// Changes: the snapshots are the server's JSON as the clone keeps it (`Obj`), so fields are read
// with `str`/`num`/`arr`; `createdAt` is `toISOString()` of `now` (DateTime.formatIso); barColor
// returns the light and dark colours the Usage page's chart uses (usage-providers.ts PROVIDERS),
// since Contract has no CSS variables. The functions and their rules are otherwise the reference's.
import { arr, num, obj, str, type Obj } from '../client/domain';
import { PROVIDERS } from './usage-providers';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export type LimitPace = 'ahead' | 'on' | 'under';
export type ResetCreditInput = { instanceId: string } | { sourceId: string; accountId: string; creditId: string };
export type UsageLimitsAccount = {
  id: string; driver: string; label: string; plan?: string; email?: string; sourceLabel?: string; instanceId?: string;
  resetCreditInput?: ResetCreditInput; displayName?: string; accentColor?: string; limits: Obj;
};
export type UsageLimitsReport = { createdAt: string; accounts: UsageLimitsAccount[]; notices: string[] };

/**
 * Providers that belong on the Limits view: enabled, installed, and one whose driver reports
 * subscription usage at all. A driver with no notion of usage never sets `usageLimits`, so it
 * has no row rather than an empty one.
 */
export function providersWithLimits(providers: readonly Obj[]): Obj[] {
  return providers.filter(provider => provider.enabled === true && provider.installed === true
    && provider.availability !== 'unavailable' && provider.usageLimits !== undefined && provider.usageLimits !== null);
}

/** Prefer the reported email; use an identical credential when no email is available. */
function accountKey(driver: unknown, email: unknown, limits?: Obj): string | null {
  const normalizedEmail = str(email).trim().toLowerCase();
  if (normalizedEmail) return `${str(driver)}:${normalizedEmail}`;
  return str(limits?.credentialFingerprint) ? `${str(driver)}:credential:${str(limits?.credentialFingerprint)}` : null;
}

/** A login or hub account as accountKeyResolver reads it. */
export type AccountReport = { driver: unknown; email?: unknown; workspaceId?: unknown; usageLimits?: unknown };
/** The account fields read from a server's JSON report. */
export const accountReport = (report: Obj): AccountReport => ({ driver: report.driver, email: report.email, workspaceId: report.workspaceId, usageLimits: report.usageLimits });
/** A native instance's report: its auth names the email and the workspace. */
export const nativeReport = (provider: Obj): AccountReport => ({
  driver: provider.driver, email: obj(provider.auth).email, workspaceId: obj(provider.auth).workspaceId, usageLimits: provider.usageLimits,
});

/**
 * Keys each report by the account it draws on. One email can belong to several workspaces, each
 * with its own quota, so a report that names its workspace is keyed by it too. One that does not
 * (a hub, an older server) joins the only workspace known for its email, and gets no key when there
 * are several, since it cannot say which one it read. Pass every enabled login, read or not, so an
 * unreadable workspace still counts as one.
 */
export function accountKeyResolver(reports: Iterable<AccountReport>): (report: AccountReport) => string | null {
  const workspaces = new Map<string, Set<string>>();
  for (const report of reports) {
    const key = accountKey(report.driver, report.email, obj(report.usageLimits)), workspaceId = str(report.workspaceId);
    if (key && workspaceId) workspaces.set(key, (workspaces.get(key) ?? new Set()).add(workspaceId));
  }
  return report => {
    const key = accountKey(report.driver, report.email, obj(report.usageLimits));
    if (!key) return null;
    const workspaceId = str(report.workspaceId);
    if (workspaceId) return `${key}:${workspaceId}`;
    const known = workspaces.get(key);
    if (!known) return key;
    return known.size === 1 ? `${key}:${[...known][0]}` : null;
  };
}

/** The one-line status under a provider heading when there are no bars to draw. */
export function limitsNotice(limits: Obj): string | null {
  const unavailable = obj(limits.unavailable);
  if (unavailable.reason === 'unsupported') return str(unavailable.message) || 'This account has no subscription limits.';
  if (unavailable.reason === 'probeFailed') return str(unavailable.message) || 'Could not read limits.';
  return arr(limits.windows).length === 0 ? 'No limits reported.' : null;
}

/** Quota left in the window, 0..100. Bars and labels show what remains, as Codex does. */
export function remainingPercent(window: Obj): number {
  return Math.round(100 - Math.max(0, Math.min(100, num(window.usedPercent))));
}

function resetMillis(window: Obj): number | null {
  if (typeof window.resetsAt !== 'string') return null;
  const at = Date.parse(window.resetsAt);
  return Number.isFinite(at) ? at : null;
}

/** Elapsed share of the window, 0..1, or null when its length or reset is unknown. */
export function elapsedShare(window: Obj, now: number): number | null {
  const resetsAt = resetMillis(window);
  if (resetsAt === null || typeof window.windowDurationMins !== 'number') return null;
  const length = window.windowDurationMins * MINUTE;
  if (length <= 0) return null;
  return Math.max(0, Math.min(1, (length - (resetsAt - now)) / length));
}

/** Bars show quota remaining, so even pace tracks time remaining: 0..100, or null without a clock. */
export function paceRemainingPercent(window: Obj, now: number): number | null {
  const elapsed = elapsedShare(window, now);
  return elapsed === null ? null : Math.round((1 - elapsed) * 100);
}

/**
 * Usage against the clock. Spending evenly leaves the same share of quota as there is time left
 * in the window; within five points of that counts as on pace, further ahead means the window
 * may run dry first.
 */
export function paceOf(window: Obj, now: number): LimitPace | null {
  const elapsed = elapsedShare(window, now);
  return elapsed === null ? null : paceOfShares(num(window.usedPercent), elapsed);
}
function paceOfShares(usedPercent: number, elapsed: number): LimitPace {
  const gap = usedPercent - elapsed * 100;
  if (gap > 5) return 'ahead';
  if (gap < -5) return 'under';
  return 'on';
}
/** UsageLimits.tsx PACE: the glyph's words (its icon is chosen in usage-bars.contract). */
export const PACE_LABEL: Record<LimitPace, string> = {
  ahead: 'Ahead of pace: spending faster than the window elapses',
  on: 'On pace with the window',
  under: 'Under pace: headroom left for the rest of the window',
};

/** `2h 13m`, `3d 4h`, `12m`. */
export function formatDuration(ms: number): string {
  const remaining = Math.max(0, ms);
  const days = Math.floor(remaining / DAY);
  const hours = Math.floor((remaining % DAY) / HOUR);
  const minutes = Math.floor((remaining % HOUR) / MINUTE);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

/** `resets in 2h 13m`, or null when the window has no reset. */
export function formatResetsIn(window: Obj, now: number): string | null {
  const resetsAt = resetMillis(window);
  if (resetsAt === null) return null;
  if (resetsAt <= now) return 'resets now';
  if (resetsAt - now < MINUTE) return 'Resets momentarily';
  return `resets in ${formatDuration(resetsAt - now)}`;
}

/** UsageLimits.tsx barColor: the series colour the cost chart uses for this driver, as [light, dark]; the foreground for any other. */
export function barColor(driver: string): [string, string] {
  return Object.values(PROVIDERS).find(entry => entry.driver === driver)?.color ?? ['#27272a', '#f5f5f5'];
}

/** Limit commands are served by T3 from the same snapshots as Usage → Limits. */
export const USAGE_LIMITS_COMMAND = { name: 'usage-limits', description: "Show this provider's usage limits" };

/** Handled by the client without sending a turn; anything with arguments stays an ordinary prompt. */
export function isUsageLimitsCommand(prompt: string): boolean {
  return prompt.trim().toLowerCase() === '/usage-limits';
}

/**
 * Whether Limits has anything to say about this driver. A source that failed to read keeps no
 * accounts, so its error counts for every driver rather than disappearing until the next
 * successful refresh.
 */
export function hasProviderUsageLimits(driver: unknown, providers: readonly Obj[], sources: readonly Obj[]): boolean {
  return providersWithLimits(providers).some(provider => provider.driver === driver)
    || sources.some(source => arr(source.accounts).some(account => account.driver === driver)
      || (source.error !== undefined && source.error !== null && arr(source.accounts).length === 0));
}

/** Advertise on workspace catalogs too, which replace the global command list. */
export function withUsageLimitsCommands(providers: readonly Obj[], sources: readonly Obj[]): Obj[] {
  return providers.map(provider => {
    if (!hasProviderUsageLimits(provider.driver, providers, sources)) return provider;
    const commands = (items: unknown) => [...arr(items).filter(command => command.name !== USAGE_LIMITS_COMMAND.name), USAGE_LIMITS_COMMAND];
    return {
      ...provider,
      slashCommands: commands(provider.slashCommands),
      ...(Array.isArray(provider.workspaceSnapshots)
        ? { workspaceSnapshots: arr(provider.workspaceSnapshots).map(snapshot => ({ ...snapshot, slashCommands: commands(snapshot.slashCommands) })) }
        : {}),
    };
  });
}

/** A point-in-time report; never refreshes or guesses which pooled account serves a turn. */
export function collectProviderUsageLimits(instanceId: string, providers: readonly Obj[], sources: readonly Obj[], now: number): UsageLimitsReport | null {
  const selected = providers.find(provider => provider.instanceId === instanceId);
  if (!selected || !hasProviderUsageLimits(selected.driver, providers, sources)) return null;
  const resolveKey = accountKeyResolver([...providers.filter(provider => provider.enabled === true).map(nativeReport),
    ...sources.flatMap(source => arr(source.accounts)).map(accountReport)]);
  // One row per native account: a usable read beats a failed one, then the freshest wins; the
  // freshest credits come along whichever instance read them.
  type NativeSnapshot = { provider: Obj; limits: Obj };
  const nativeByAccount = new Map<string, { usage: NativeSnapshot; credits: NativeSnapshot | null }>();
  for (const provider of providersWithLimits(providers)) {
    if (provider.driver !== selected.driver) continue;
    const snapshot = { provider, limits: obj(provider.usageLimits) };
    const key = resolveKey(nativeReport(provider)) ?? `instance:${str(provider.instanceId)}`;
    const previous = nativeByAccount.get(key);
    const previousUsable = previous ? limitsNotice(previous.usage.limits) === null : false;
    const usable = limitsNotice(snapshot.limits) === null;
    const providerWins = !previous || (usable && !previousUsable)
      || (usable === previousUsable && Date.parse(str(snapshot.limits.checkedAt)) > Date.parse(str(previous.usage.limits.checkedAt)));
    const previousCredit = previous?.credits;
    const credits = snapshot.limits.resetCredits
      && (!previousCredit || Date.parse(str(snapshot.limits.checkedAt)) > Date.parse(str(previousCredit.limits.checkedAt)))
      ? snapshot : previousCredit ?? null;
    nativeByAccount.set(key, { usage: providerWins ? snapshot : previous.usage, credits });
  }
  const native = [...nativeByAccount.values()];
  const nativeAccountKeys = new Set(native.flatMap(({ usage: { provider, limits } }) => {
    const key = resolveKey(nativeReport(provider));
    return key && arr(limits.windows).length && !limits.unavailable ? [key] : [];
  }));
  const accounts: UsageLimitsAccount[] = [];
  const notices: string[] = [];
  for (const { usage: { provider, limits }, credits } of native) {
    const auth = obj(provider.auth);
    const key = resolveKey(nativeReport(provider));
    const hubCredits = sources
      .flatMap(source => arr(source.accounts).map(account => ({ source, account })))
      .filter(({ account }) => key !== null && resolveKey(accountReport(account)) === key
        && !!obj(account.usageLimits).resetCredits && !limitsNotice(obj(account.usageLimits)))
      .sort((a, b) => Date.parse(str(obj(b.account.usageLimits).checkedAt)) - Date.parse(str(obj(a.account.usageLimits).checkedAt)))[0];
    // Two independent decisions. Which balance to *display* follows whichever snapshot is
    // fresher. Which path to *redeem through* always prefers the hub, because only the hub path
    // clears the routing cooldown it holds for that account; redeeming natively against the same
    // account resets the subscription upstream but leaves the hub refusing to route to it until
    // its own cooldown expires. A hub credit id that a fresher native redeem already spent comes
    // back as `alreadyRedeemed`, which still clears the cooldown, so preferring it is safe even
    // when the hub snapshot is stale.
    const hubLimits = obj(hubCredits?.account.usageLimits);
    const hubCreditId = str(obj(hubLimits.resetCredits).nextCreditId);
    const showHubCredits = hubCredits && (!credits || Date.parse(str(hubLimits.checkedAt)) > Date.parse(str(credits.limits.checkedAt)));
    const shownCredits = showHubCredits ? hubLimits.resetCredits : credits?.limits.resetCredits;
    const displayName = typeof provider.displayName === 'string' ? provider.displayName : undefined;
    accounts.push({
      id: str(provider.instanceId), driver: str(provider.driver),
      label: `${displayName?.trim() || str(provider.driver)} [${str(provider.instanceId)}]`,
      ...(str(auth.label) ? { plan: str(auth.label) } : {}),
      instanceId: str(provider.instanceId),
      resetCreditInput: hubCreditId && hubCredits
        ? { sourceId: str(hubCredits.source.id), accountId: str(hubCredits.account.id), creditId: hubCreditId }
        : { instanceId: str(credits?.provider.instanceId ?? provider.instanceId) },
      ...(displayName ? { displayName } : {}),
      ...(str(provider.accentColor) ? { accentColor: str(provider.accentColor) } : {}),
      ...(str(auth.email) ? { email: str(auth.email) } : {}),
      limits: shownCredits ? { ...limits, resetCredits: shownCredits } : limits,
    });
  }
  for (const source of sources) {
    const matching = arr(source.accounts).filter(account => account.driver === selected.driver);
    for (const account of matching) {
      const limits = obj(account.usageLimits);
      const key = resolveKey(accountReport(account));
      if (key && nativeAccountKeys.has(key)) continue;
      const creditId = str(obj(limits.resetCredits).nextCreditId);
      accounts.push({
        id: `${str(source.id)}:${str(account.id)}`, driver: str(account.driver),
        label: `${str(source.label)} · ${str(account.id)}`, sourceLabel: 'CLI Proxy',
        ...(creditId ? { resetCreditInput: { sourceId: str(source.id), accountId: str(account.id), creditId } } : {}),
        ...(str(account.plan) ? { plan: str(account.plan) } : {}),
        ...(str(account.email) ? { email: str(account.email) } : {}),
        limits,
      });
    }
    // A source that failed to read has no accounts left to match on, so its error is reported
    // to every provider rather than silently dropped.
    if (sourceError(source) && (matching.length > 0 || arr(source.accounts).length === 0)) notices.push(`${str(source.label)}: ${sourceError(source)}`);
  }
  return { createdAt: new Date(now).toISOString(), accounts, notices };
}
/** A source's `error` is a string in the contract; an object with a message is read the same way. */
function sourceError(source: Obj): string {
  return typeof source.error === 'string' ? source.error : str(obj(source.error).message);
}
