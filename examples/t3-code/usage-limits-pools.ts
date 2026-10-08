// The pooled subscription limits across environments, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// packages/shared/src/usageLimits.ts (CHATGPT_USAGE_URL, CURSOR_USAGE_WINDOWS,
// cursorUsageWindowDetails, collectExternalUsageLinks, LimitAccount, collectLimitAccounts,
// collectLimitNotices, LimitPoolMember, LimitPoolWindow, LimitPool, displayLimitWindows,
// collectLimitPools) and apps/web/src/components/usage/UsageLimitsPooled.tsx (accountInitials,
// accountHue). Changes: snapshots are the server's JSON as the clone keeps it (`Obj`), read with
// `str`/`num`/`arr`; `presentations` keeps the reference's shape (an ordered map of environment id
// to `{ entry: { target: { label } }, serverConfig }`) so its tests port unchanged; accountKey,
// resetMillis and paceOfShares repeat usage-limits.ts's private helpers (that file is the
// composer ticket's and is read here only). The rules are otherwise the reference's.
import { arr, num, obj, str, type Obj } from './domain';
import { elapsedShare, limitsNotice, providersWithLimits, type LimitPace, type ResetCreditInput } from './usage-limits';

export const CHATGPT_USAGE_URL = 'https://chatgpt.com/#settings/Usage';

export const CURSOR_USAGE_WINDOWS = [
  { id: 'totalPercentUsed', label: 'Overall', description: 'Combined usage across both allowances, not a third quota.' },
  { id: 'autoPercentUsed', label: 'Cursor Models', description: 'Grok and Composer use this first. Auto can use either pool.' },
  { id: 'apiPercentUsed', label: 'Other Models', description: 'Claude, GPT, and Gemini use this pool. Grok and Composer fall back here.' },
] as const;

export function cursorUsageWindowDetails(id: string) {
  return CURSOR_USAGE_WINDOWS.find(window => window.id === id);
}
function cursorUsageWindowRank(id: string): number {
  const rank = CURSOR_USAGE_WINDOWS.findIndex(window => window.id === id);
  return rank < 0 ? CURSOR_USAGE_WINDOWS.length : rank;
}

/** One environment as the pooled views read it: its label and its live server config. */
export type LimitPresentation = { readonly entry: { readonly target: { readonly label: string } }; readonly serverConfig: Obj | null };
export type LimitPresentations = ReadonlyMap<string, LimitPresentation>;

/** One destination per service, even when several accounts or environments use it. */
export function collectExternalUsageLinks(presentations: LimitPresentations) {
  const links = new Map<string, { label: string; url: string; message: string | undefined; accounts: string[] }>();
  for (const presentation of presentations.values()) {
    for (const provider of providersWithLimits(arr(presentation.serverConfig?.providers))) {
      const limits = obj(provider.usageLimits), external = obj(limits.externalUsage);
      if (str(external.url) && obj(provider.auth).status === 'authenticated') {
        const account = `${str(provider.displayName) || str(provider.instanceId)} on ${presentation.entry.target.label}`;
        links.set(str(external.url), {
          label: str(external.label), url: str(external.url),
          message: str(obj(limits.unavailable).message) || undefined,
          accounts: [...new Set([...(links.get(str(external.url))?.accounts ?? []), account])],
        });
      }
    }
  }
  return [...links.values()];
}

/** Prefer the reported email; use an identical credential when no email is available. */
function accountKey(driver: unknown, email: unknown, limits?: Obj): string | null {
  const normalizedEmail = str(email).trim().toLowerCase();
  if (normalizedEmail) return `${str(driver)}:${normalizedEmail}`;
  return str(limits?.credentialFingerprint) ? `${str(driver)}:credential:${str(limits?.credentialFingerprint)}` : null;
}

/**
 * One subscription account as the pooled views see it, whichever way it was reported. Matching
 * emails or credentials across environments name one account. Its quota is one bucket, so
 * counting it twice would misstate what is left.
 */
export interface LimitAccount {
  readonly key: string;
  readonly driver: string;
  /** The instance's configured name, which is not sensitive; null for hub accounts. */
  readonly displayName: string | null;
  readonly email: string | undefined;
  readonly plan: string | undefined;
  readonly accentColor: string | undefined;
  /** Environments the account is signed in on; empty when only a hub reports it. */
  readonly environments: ReadonlyArray<{ readonly environmentId: string; readonly label: string }>;
  /** The hub that reported it, when no environment has it natively. */
  readonly sourceLabel: string | null;
  /** Where the displayed reset credit can be redeemed. */
  readonly redeem: { readonly environmentId: string; readonly input: ResetCreditInput } | null;
  readonly limits: Obj;
}

const checked = (limits: Obj) => Date.parse(str(limits.checkedAt));

/**
 * Every account with usable windows across the connected environments, one entry per distinct
 * account. The freshest reads supply windows and credits; native instances supply names and
 * environment labels.
 */
export function collectLimitAccounts(presentations: LimitPresentations): readonly LimitAccount[] {
  const accounts = new Map<string, LimitAccount>();
  const creditSources = new Map<string, LimitAccount>();
  const hubRedeems = new Map<string, LimitAccount>();
  const merge = (key: string, next: LimitAccount) => {
    // Redeeming through a hub also clears the routing cooldown that hub holds for the account.
    // Redeeming natively against the same subscription resets it upstream but leaves the hub
    // refusing to route to the account until its own cooldown expires, so a hub target wins the
    // redemption outright while the displayed balance still follows the freshest read.
    const previousHub = hubRedeems.get(key);
    if (next.redeem && 'sourceId' in next.redeem.input && (!previousHub || checked(next.limits) > checked(previousHub.limits))) hubRedeems.set(key, next);
    const previousCredit = creditSources.get(key);
    if (next.limits.resetCredits && (!previousCredit || checked(next.limits) > checked(previousCredit.limits))) creditSources.set(key, next);
    const previous = accounts.get(key);
    if (!previous) { accounts.set(key, next); return; }
    const fresher = checked(next.limits) > checked(previous.limits);
    // Two instances on one machine sharing an account still name it once.
    const environments = [...previous.environments,
      ...next.environments.filter(candidate => !previous.environments.some(seen => seen.environmentId === candidate.environmentId))];
    const winner = fresher ? next : previous;
    // Credits and their redemption target travel together. A failed credit probe must not erase
    // a successful read from another environment.
    const creditSource = creditSources.get(key);
    const limits: Obj = { ...winner.limits };
    if (creditSource?.limits.resetCredits) limits.resetCredits = creditSource.limits.resetCredits; else delete limits.resetCredits;
    accounts.set(key, {
      ...previous,
      displayName: previous.displayName ?? next.displayName,
      plan: previous.plan ?? next.plan,
      accentColor: previous.accentColor ?? next.accentColor,
      environments,
      // A hub only names the account when no environment has it natively.
      sourceLabel: environments.length > 0 ? null : (previous.sourceLabel ?? next.sourceLabel),
      redeem: hubRedeems.get(key)?.redeem ?? (creditSource ? creditSource.redeem : (winner.redeem ?? previous.redeem ?? next.redeem)),
      limits,
    });
  };
  for (const [environmentId, presentation] of presentations) {
    const label = presentation.entry.target.label;
    for (const provider of providersWithLimits(arr(presentation.serverConfig?.providers))) {
      const limits = obj(provider.usageLimits);
      if (!provider.usageLimits || limitsNotice(limits) !== null) continue;
      const auth = obj(provider.auth);
      merge(accountKey(provider.driver, auth.email, limits) ?? `${environmentId}:${str(provider.instanceId)}`, {
        key: `${environmentId}:${str(provider.instanceId)}`, driver: str(provider.driver),
        displayName: str(provider.displayName).trim() || null,
        email: str(auth.email) || undefined, plan: str(auth.label) || undefined, accentColor: str(provider.accentColor) || undefined,
        environments: [{ environmentId, label }], sourceLabel: null,
        redeem: { environmentId, input: { instanceId: str(provider.instanceId) } },
        limits,
      });
    }
  }
  // Every hub account, including those a native instance also knows: the hub may hold a fresher
  // read of the same subscription, and the merge above keeps the redeem target consistent with
  // whichever snapshot wins.
  const labelEnvironment = presentations.size > 1;
  for (const [environmentId, presentation] of presentations) {
    for (const source of arr(presentation.serverConfig?.usageLimitSources)) {
      const sourceLabel = labelEnvironment ? `${presentation.entry.target.label} · ${str(source.label)}` : str(source.label);
      for (const account of arr(source.accounts)) {
        const limits = obj(account.usageLimits);
        if (limitsNotice(limits) !== null) continue;
        const creditId = str(obj(limits.resetCredits).nextCreditId);
        merge(accountKey(account.driver, account.email, limits) ?? `${str(source.id)}:${str(account.id)}`, {
          key: `${str(source.id)}:${str(account.id)}`, driver: str(account.driver),
          displayName: str(account.email) ? null : str(account.id).replace(/\.json$/i, ''),
          email: str(account.email) || undefined, plan: str(account.plan) || undefined, accentColor: undefined,
          environments: [], sourceLabel,
          redeem: creditId ? { environmentId, input: { sourceId: str(source.id), accountId: str(account.id), creditId } } : null,
          limits,
        });
      }
    }
  }
  return [...accounts.values()];
}

/**
 * What the pooled views cannot draw as a bar: a hub that failed to read, a provider whose probe
 * failed. Accounts that can never report (API keys) are left out; there is nothing for the user
 * to act on. The environment is named only when more than one is connected.
 */
export function collectLimitNotices(presentations: LimitPresentations): readonly string[] {
  const label = (environmentLabel: string, subject: string) => presentations.size > 1 ? `${environmentLabel} · ${subject}` : subject;
  const notices: string[] = [];
  for (const presentation of presentations.values()) {
    const environmentLabel = presentation.entry.target.label;
    for (const provider of providersWithLimits(arr(presentation.serverConfig?.providers))) {
      // An account that can never report (API key) is left out; one that failed, or reported
      // nothing at all, is worth a line.
      const limits = obj(provider.usageLimits);
      if (obj(limits.unavailable).reason === 'unsupported') continue;
      const notice = provider.usageLimits ? limitsNotice(limits) : null;
      const name = str(provider.displayName).trim() || str(provider.driver);
      if (notice) notices.push(`${label(environmentLabel, name)}: ${notice}`);
    }
    for (const source of arr(presentation.serverConfig?.usageLimitSources)) {
      const error = typeof source.error === 'string' ? source.error : str(obj(source.error).message);
      if (error) notices.push(`${label(environmentLabel, str(source.label))}: ${error}`);
      else if (arr(source.accounts).length === 0) notices.push(`${label(environmentLabel, str(source.label))}: No accounts reported.`);
    }
  }
  return notices;
}

export interface LimitPoolMember { readonly account: LimitAccount; readonly window: Obj }

/**
 * One window id across every account that reports it: the pooled share left, pace against the
 * clock, and the resets in the order they will land, each with the share of the pool it hands back.
 */
export interface LimitPoolWindow {
  readonly id: string;
  readonly kind: string;
  readonly label: string;
  readonly members: readonly LimitPoolMember[];
  /** Fixed account positions across rows; a null window leaves a gap. */
  readonly columns: ReadonlyArray<{ readonly account: LimitAccount; readonly window: Obj | null }>;
  readonly remainingPercent: number;
  readonly usedPercent: number;
  readonly pace: LimitPace | null;
  /** `restoresPercent`: points of the pool the reset restores, the member's used share over the member count. */
  readonly resets: ReadonlyArray<{ readonly member: LimitPoolMember; readonly at: number; readonly restoresPercent: number }>;
}

export interface LimitPool {
  readonly driver: string;
  readonly accounts: readonly LimitAccount[];
  readonly windows: readonly LimitPoolWindow[];
}

/** Show Cursor's two usable pools instead of a combined percentage when both are available. */
export function displayLimitWindows(pool: LimitPool): readonly LimitPoolWindow[] {
  if (pool.driver !== 'cursor') return pool.windows;
  const hasBothPools = pool.windows.some(window => window.id === 'autoPercentUsed') && pool.windows.some(window => window.id === 'apiPercentUsed');
  return pool.windows.filter(window => !hasBothPools || window.id !== 'totalPercentUsed')
    .sort((left, right) => cursorUsageWindowRank(left.id) - cursorUsageWindowRank(right.id));
}

const WINDOW_KIND_ORDER: Record<string, number> = { session: 0, weekly: 1, monthly: 2, other: 3 };
const kindRank = (kind: unknown) => WINDOW_KIND_ORDER[str(kind)] ?? 3;
function resetMillis(window: Obj): number | null {
  if (typeof window.resetsAt !== 'string') return null;
  const at = Date.parse(window.resetsAt);
  return Number.isFinite(at) ? at : null;
}
function paceOfShares(usedPercent: number, elapsed: number): LimitPace {
  const gap = usedPercent - elapsed * 100;
  return gap > 5 ? 'ahead' : gap < -5 ? 'under' : 'on';
}

/**
 * Accounts grouped by driver, each with its windows pooled by kind and id. Window ids are stable
 * per provider, so a hub row and a native row for the same window land in the same pool; the
 * kind is part of the key because Codex's `primary` is a position, not a duration (five hours on
 * paid plans, a month on Free/Go), and a monthly allowance must not average into a five-hour
 * pool. Pools order by kind, then first appearance.
 *
 * Accounts and columns share the session reset order, soonest first. When no account reports a
 * session window, use the first window by kind instead. Missing reset times sort last, with
 * account names and keys breaking ties. Each window's reset list still follows its own clock.
 */
export function collectLimitPools(accounts: readonly LimitAccount[], now: number): readonly LimitPool[] {
  const byDriver = new Map<string, LimitAccount[]>();
  for (const account of accounts) {
    const list = byDriver.get(account.driver);
    if (list) list.push(account); else byDriver.set(account.driver, [account]);
  }
  return [...byDriver].map(([driver, members]) => {
    const orderWindow = members.flatMap(account => arr(account.limits.windows)).sort((left, right) => kindRank(left.kind) - kindRank(right.kind))[0];
    const orderReset = (account: LimitAccount) => {
      const window = arr(account.limits.windows).find(candidate => candidate.kind === orderWindow?.kind && candidate.id === orderWindow?.id);
      return (window ? resetMillis(window) : null) ?? Number.POSITIVE_INFINITY;
    };
    const sorted = [...members].sort((left, right) => orderReset(left) - orderReset(right)
      || accountSortName(left).localeCompare(accountSortName(right)) || left.key.localeCompare(right.key));
    return { driver, accounts: sorted, windows: poolWindows(sorted, now) };
  });
}

function accountSortName(account: LimitAccount): string {
  return (account.displayName ?? account.email ?? account.key).toLowerCase();
}

function poolWindows(accounts: readonly LimitAccount[], now: number): readonly LimitPoolWindow[] {
  const byKey = new Map<string, LimitPoolMember[]>();
  for (const account of accounts) {
    for (const window of arr(account.limits.windows)) {
      const key = `${str(window.kind)}:${str(window.id)}`;
      const list = byKey.get(key);
      if (list) list.push({ account, window }); else byKey.set(key, [{ account, window }]);
    }
  }
  const pools = [...byKey.values()].map((members): LimitPoolWindow => {
    const memberByAccount = new Map(members.map(member => [member.account.key, member]));
    const first = members[0]!.window;
    const usedPercent = members.reduce((sum, member) => sum + num(member.window.usedPercent), 0) / members.length;
    // Pace compares spend against the clock, so it is judged only over the members that have a
    // clock; a window with no reset would otherwise count as spend with no time elapsed and skew
    // the verdict.
    const timed = members.flatMap(member => {
      const share = elapsedShare(member.window, now);
      return share === null ? [] : [{ used: num(member.window.usedPercent), elapsed: share }];
    });
    const timedUsed = timed.reduce((sum, entry) => sum + entry.used, 0) / timed.length;
    const meanElapsed = timed.length > 0 ? timed.reduce((sum, entry) => sum + entry.elapsed, 0) / timed.length : null;
    const resets = members.flatMap(member => {
      const at = resetMillis(member.window);
      return at === null ? [] : [{ member, at, restoresPercent: Math.round(num(member.window.usedPercent) / members.length) }];
    }).sort((left, right) => left.at - right.at);
    return {
      id: str(first.id), kind: str(first.kind), label: str(first.label), members,
      columns: accounts.map(account => memberByAccount.get(account.key) ?? { account, window: null }),
      usedPercent: Math.round(usedPercent), remainingPercent: Math.round(100 - usedPercent),
      pace: meanElapsed === null ? null : paceOfShares(timedUsed, meanElapsed), resets,
    };
  });
  return pools.sort((left, right) => kindRank(left.kind) - kindRank(right.kind));
}

// ── UsageLimitsPooled.tsx: account chips ───────────────────────────────────

/** `someone@example.com` → `SE`: enough to tell accounts apart, too little to identify one. */
export function accountInitials(email: string): string {
  const [local = '', domain = ''] = email.split('@');
  return `${local[0] ?? ''}${domain[0] ?? ''}`.toUpperCase() || '?';
}

/** A stable hue per email, so the same account gets the same chip on every visit. */
export function accountHue(email: string): number {
  let hash = 0;
  for (let index = 0; index < email.length; index += 1) hash = (hash * 31 + email.charCodeAt(index)) | 0;
  return Math.abs(hash) % 360;
}
