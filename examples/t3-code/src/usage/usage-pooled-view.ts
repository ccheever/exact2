// The pooled Limits view as data for usage-pooled.contract, T3 Code 74541bab17 (MIT, see
// LICENSE-T3): apps/web/src/components/usage/UsageLimitsPooled.tsx (UsageLimitsPooled, PoolSection,
// PoolWindowCard, PoolBar, PoolSegment, SegmentTriggers, LegendRow, SegmentPopover, RedeemableSegment,
// ActiveSegmentPopover's check that its segment is still drawn,
// AccountAvatar, AccountName, AccountChip, LimitNotices), UsageLimits.tsx (UsageLimitsSection's
// provider filter) and UsagePage.tsx (CursorEnableLimits, CursorEnableRow). Changes: Contract has no container query, so the bar's `@2xl/pool` width
// (42rem) is worked out here from the page's layout (`poolBarWidth`); the popover's side is the one the reference lands on after its
// collision flip on an unscrolled page (`popoverSides`: below a bar too near the page's top for the
// popover, above every other; exact2 #112); every figure and word is otherwise the reference's.
import { arr, str, type Obj } from '../client/domain';
import { driverMeta } from '../providers/providers-meta';
import { resolveOfficialAcpRegistryIconUrl } from '../providers/acp-icons';
import { PACE_LABEL, barColor, formatResetsIn, paceRemainingPercent, remainingPercent } from './usage-limits';
import { REDEEM_IDLE, resetCreditsSummary } from './reset-credits';
import { redactedValue } from '../shared/redacted-text';
import { formatUpcomingTimestamp } from '../shared/timestamp-format';
import {
  CHATGPT_USAGE_URL, accountHue, accountInitials, collectExternalUsageLinks, collectLimitAccounts, collectLimitNotices, collectLimitPools,
  cursorUsageWindowDetails, displayLimitWindows, type LimitAccount, type LimitPresentation,
} from './usage-limits-pools';
import { cursorKeychainAccessEnvironments } from './usage-refresh';
import { selectedConnected, type EnvironmentUsageStatus, type UsageState } from './usage-environments';

const driverLabel = (driver: string) => driverMeta(driver)?.label ?? driver;
/** providerInstanceInitials (client-runtime state/providerInstanceDisplay.ts). */
export function instanceInitials(label: string): string {
  const words = label.replace(/[_-]+/g, ' ').split(/\s+/u).filter(Boolean);
  if (words.length === 0) return '';
  if (words.length === 1) return Array.from(words[0]!).slice(0, 2).join('').toUpperCase();
  return words.slice(0, 2).map(word => Array.from(word)[0]?.toUpperCase() ?? '').join('');
}
const accent = (value: string | undefined) => (value && /^#[0-9a-fA-F]{6}$/u.test(value.trim()) ? value.trim() : '');

/** The pool bar's width: the page column (max 64rem, 1.5rem gutters), the card's 1rem padding and border, and at md the 11rem figure column and its 1.5rem gap. */
export function poolBarWidth(viewport: number, sidebar: number): number {
  const content = Math.min(1024, Math.max(0, viewport - sidebar)) - 48, inner = content - 34;
  return viewport >= 768 ? inner - 176 - 24 : inner;
}
/** `@2xl/pool`: 42rem. */
export const POOL_WIDE = 672;

/** A segment's redeem is its account's in that window (the reference's useResetCredit lives in PoolSegment keyed by the account). */
const redeemKeyOf = (account: LimitAccount, window: import('../client/domain').Obj) => `${account.key}\n${str(window.kind)}:${str(window.id)}`;

/** AccountName: the instance name, else the email's chip, else the driver. */
function accountName(account: LimitAccount) {
  const chip = !account.displayName && account.email ? accountInitials(account.email) : '';
  const hue = account.email ? accountHue(account.email) : 0;
  return { name: account.displayName ?? (chip ? '' : driverLabel(account.driver)), chip,
    chipBg: `oklch(0.85 0.08 ${hue})`, chipFg: `oklch(0.35 0.1 ${hue})` };
}

/**
 * What ActiveSegmentPopover checks to keep a popover open: the same account's same window, redeemable or
 * not. Hashed, since an account key can hold an address.
 */
function segmentIdentity(account: LimitAccount, window: import('../client/domain').Obj): string {
  const text = `${account.key}\n${str(window.kind)}:${str(window.id)}\n${account.redeem ? 1 : 0}`;
  let hash = 0;
  for (let index = 0; index < text.length; index += 1) hash = (hash * 31 + text.charCodeAt(index)) | 0;
  return (hash >>> 0).toString(36);
}

export type PooledSegment = ReturnType<typeof segmentView>;
function segmentView(state: UsageState, id: string, account: LimitAccount, window: import('../client/domain').Obj, reset: { restoresPercent: number } | undefined, refillPercent: number, column: number, count: number, now: number, format: string, side: string) {
  const remaining = remainingPercent(window), resetsIn = formatResetsIn(window, now), timeLeft = paceRemainingPercent(window, now);
  const credits = Number(account.limits.resetCredits && (account.limits.resetCredits as { availableCount?: number }).availableCount) || 0;
  const label = account.displayName ?? (account.email ? accountInitials(account.email) : account.driver);
  const named = accountName(account), email = redactedValue(account.email ?? '');
  const where = account.environments.length > 0 ? account.environments.map(environment => environment.label).join(', ') : account.sourceLabel ?? '';
  const redeem = state.redeems.get(redeemKeyOf(account, window)) ?? REDEEM_IDLE;
  const resetCredits = account.limits.resetCredits as import('../client/domain').Obj | undefined;
  const avatar = account.redeem ? 'instance' : account.email ? 'chip' : '';
  return {
    id, identity: segmentIdentity(account, window), column, side, remaining, credits, ...named, showName: count > 1,
    // The even-pace tick's place along the segment; -1 without a clock.
    timeLeft: timeLeft ?? -1,
    // PopoverPopup align center, shifted inside the page at the bar's ends (Base UI's collision shift).
    align: count > 1 && column === 1 ? 'start' : count > 1 && column === count ? 'end' : 'center',
    label: `${label}: ${remaining}% left${resetsIn ? `, ${resetsIn}` : ''}${timeLeft === null ? '' : `, ${timeLeft}% of the window left`}${credits ? `, ${credits} reset ${credits === 1 ? 'credit' : 'credits'} banked` : ''}`,
    resets: resetsIn ? resetsIn.replace('resets in ', '↻ ') : '',
    creditsAria: credits ? `${credits} reset ${credits === 1 ? 'credit' : 'credits'} banked` : '',
    // The spent share a reset makes usable again, hatched from the right.
    hatch: remaining < 100 && !!reset && refillPercent > 0 ? Math.min(100 - remaining, refillPercent) : 0,
    // SegmentPopover
    title: account.displayName ?? driverLabel(account.driver), avatar, driver: account.driver,
    badge: account.redeem && account.displayName ? instanceInitials(account.displayName) : '', accent: accent(account.accentColor),
    email: email.value, emailMask: email.placeholder, plan: account.plan ?? '',
    whereLabel: where ? (account.environments.length > 0 ? 'Signed in' : 'Via') : '', where,
    left: `${remaining}%`,
    resetsAt: typeof window.resetsAt === 'string' && window.resetsAt
      ? `${formatUpcomingTimestamp(window.resetsAt, format, now)}${resetsIn ? ` · ${resetsIn.replace('resets in ', 'in ')}` : ''}` : '',
    evenPace: timeLeft === null ? '' : `${timeLeft}% left by now`,
    restores: reset && reset.restoresPercent > 0 ? `+${reset.restoresPercent}% of pool` : '',
    // RedeemableSegmentPopup: the credits row only with a redeem target and credits on show.
    redeemable: !!account.redeem && credits > 0, creditsText: account.redeem && credits > 0 && resetCredits ? resetCreditsSummary(resetCredits, now, true) : '',
    busy: redeem.busy, status: redeem.status ?? '',
  };
}

/**
 * UsageLimitsSection: hidden drivers leave both native accounts and hub accounts, so their bars, notices and
 * external links all drop out together. A hub left with no visible accounts is dropped, not reported as empty.
 */
export function withoutDrivers(config: Obj | null, hidden: ReadonlySet<string>): Obj | null {
  if (hidden.size === 0 || config === null) return config;
  const visible = (entry: Obj) => !hidden.has(str(entry.driver));
  return { ...config, providers: arr(config.providers).filter(visible),
    ...(Array.isArray(config.usageLimitSources) ? { usageLimitSources: arr(config.usageLimitSources).flatMap(source => {
      const accounts = arr(source.accounts).filter(visible);
      return accounts.length === 0 && arr(source.accounts).length > 0 ? [] : [{ ...source, accounts }];
    }) } : {}) };
}

/** Everything UsageLimitsPooled draws for the selected environments at the page's limits clock, less the hidden drivers. */
export function pooledView(state: UsageState, statuses: EnvironmentUsageStatus[], layout: { barWidth: number; format: string; md: boolean; hiddenDrivers?: ReadonlySet<string> }) {
  const now = state.limitsNow, hidden = layout.hiddenDrivers ?? new Set<string>();
  const presentations = new Map<string, LimitPresentation>(selectedConnected(state).map(env => [env.id, { entry: { target: { label: env.label } }, serverConfig: withoutDrivers(env.config, hidden) }]));
  const pools = collectLimitPools(collectLimitAccounts(presentations), now);
  const notices = collectLimitNotices(presentations);
  const links = collectExternalUsageLinks(presentations);
  const selectedStatuses = statuses.filter(status => status.connected && presentations.has(status.environmentId));
  const cursorEnvironments = hidden.has('cursor') ? [] : cursorKeychainAccessEnvironments(selectedStatuses);
  const cursorPromptAt = Math.max(pools.findIndex(pool => pool.driver === 'codex'), pools.findIndex(pool => pool.driver === 'claudeAgent')) + 1;
  const wide = layout.barWidth >= POOL_WIDE;
  state.segments = new Map();
  const views = pools.map((pool, poolIndex) => {
    const [light, dark] = barColor(pool.driver);
    const windows = displayLimitWindows(pool).map((window, windowIndex) => {
      const details = pool.driver === 'cursor' ? cursorUsageWindowDetails(window.id) : undefined;
      const restores = new Map(window.resets.map(reset => [reset.member.account.key, reset]));
      // The soonest reset that hands anything back; an untouched account resets to no effect.
      const nextRefill = window.resets.find(reset => reset.restoresPercent > 0);
      const segments = window.columns.flatMap((member, position) => {
        if (!member.window) return [];
        const id = `${poolIndex}-${windowIndex}-${position}`;
        state.segments.set(id, { account: member.account, redeemKey: redeemKeyOf(member.account, member.window), name: accountName(member.account).name });
        return [segmentView(state, id, member.account, member.window, restores.get(member.account.key), member.refillPercent, position + 1, window.columns.length, now, layout.format, 'top')];
      });
      return {
        key: `${window.kind}:${window.id}`, prefix: `${poolIndex}-${windowIndex}-`, label: details?.label ?? window.label, description: details?.description ?? '',
        remaining: window.remainingPercent, pace: window.pace ?? '', paceLabel: window.pace ? PACE_LABEL[window.pace] : '',
        refill: nextRefill && window.columns.length > 1 ? `↻ +${nextRefill.restoresPercent}%` : '',
        count: window.columns.length, segments,
        // The popover closed before the confirm, so each outcome needs a home outside it: under the bar.
        // Explicit rows after the strip (and, narrow, after its legend), as the grid places them.
        statuses: segments.filter(segment => segment.busy || segment.status !== '').map((segment, index) => ({ key: segment.id, row: (wide ? 2 : window.columns.length + 2) + index,
          name: segment.name, chip: segment.chip, chipBg: segment.chipBg, chipFg: segment.chipFg, text: segment.busy ? 'Using reset credit…' : segment.status })),
      };
    });
    // An ACP agent's pool draws its registry icon (a live provider's iconUrl).
    const acp = pool.driver === 'acpRegistry' ? [...presentations.values()].flatMap(entry => arr(entry.serverConfig?.providers)).find(provider => provider.driver === 'acpRegistry' && str(provider.iconUrl)) : undefined;
    return { key: pool.driver, prefix: `${poolIndex}-`, driver: pool.driver, icon: acp ? resolveOfficialAcpRegistryIconUrl(str(acp.iconUrl)) ?? '' : '', label: driverLabel(pool.driver), light, dark, windows, cursorBefore: cursorEnvironments.length > 0 && poolIndex === cursorPromptAt };
  });
  state.links = links.map(link => link.url);
  popoverSides(views, { wide, md: layout.md, cursorRows: cursorEnvironments.length });
  return {
    wide,
    empty: pools.length === 0 && notices.length === 0 && cursorEnvironments.length === 0 && links.length === 0,
    pools: views,
    drawn: drawnEntries(state, views.flatMap(pool => pool.windows.flatMap(window => window.segments))),
    cursorAfter: cursorEnvironments.length > 0 && cursorPromptAt === pools.length,
    cursor: cursorEnvironments.map(environment => ({ key: environment.environmentId, label: environment.label,
      button: cursorEnvironments.length > 1 ? `Enable on ${environment.label}` : 'Enable', busy: state.cursorPending.has(environment.environmentId) })),
    links: links.map(link => ({ key: link.url, label: link.label, chatgpt: link.url === CHATGPT_USAGE_URL,
      message: link.url === CHATGPT_USAGE_URL ? 'View usage in ChatGPT with your connected account.' : link.message ?? '', url: link.url })),
    notices: notices.map(text => ({ key: text, text })),
    confirm: state.confirm,
  };
}
export type PooledView = ReturnType<typeof pooledView>;

/**
 * Each drawn segment as `id=identity.appearance`: the page closes a popover whose entry is no longer
 * drawn (ActiveSegmentPopover). An identity drawn again after a draw without it is a new appearance,
 * so a popover opened on its earlier one stays closed when the segment comes back, as Base UI's
 * close on unmount leaves it.
 */
function drawnEntries(state: UsageState, segments: readonly { id: string; identity: string }[]): string[] {
  const now = new Set(segments.map(segment => segment.identity));
  for (const identity of now) if (!state.drawnIdentities.has(identity)) state.appearances.set(identity, (state.appearances.get(identity) ?? 0) + 1);
  state.drawnIdentities = now;
  return segments.map(segment => `${segment.id}=${segment.identity}.${state.appearances.get(segment.identity) ?? 1}`);
}

type SideWindow = { count: number; refill: string; description: string; statuses: unknown[];
  segments: { side: string; email: string; plan: string; whereLabel: string; resetsAt: string; evenPace: string; restores: string; redeemable: boolean }[] };
/**
 * The side each popover takes, as Base UI's collision flip would on an unscrolled page: a bar whose top
 * lies nearer the scroll area's top than the popover is tall opens below it, every other above. The
 * heights are the layout's (UsageLimitsPooled's gaps, PoolWindowCard's padding, SegmentPopover's rows),
 * in points; Contract has no position-try fallback to measure it (exact2 #112).
 */
export function popoverSides(pools: { cursorBefore: boolean; windows: SideWindow[] }[], layout: { wide: boolean; md: boolean; cursorRows: number }): void {
  const cursorHeight = 20 + 12 + 34 + 16 + 12 + 28;
  let y = 24; // the page's top padding
  pools.forEach((pool, poolIndex) => {
    if (poolIndex > 0) y += 32;
    if (pool.cursorBefore && layout.cursorRows) y += cursorHeight + 32;
    y += 20 + 12; // the section heading and its gap
    pool.windows.forEach((window, windowIndex) => {
      if (windowIndex > 0) y += 12;
      const figures = 20 + 4 + 36 + (window.refill ? 4 + 16 : 0);
      const bar = (layout.wide ? 32 : 20) + (layout.wide ? 0 : window.count * 32) + window.statuses.length * 20;
      const row = layout.md ? Math.max(figures, bar) : figures + 12 + bar;
      const barTop = y + 17 + (layout.md ? (row - bar) / 2 : figures + 12);
      for (const segment of window.segments) {
        const people = (segment.plan ? 1 : 0) + (segment.whereLabel ? 1 : 0), times = 1 + (segment.resetsAt ? 1 : 0) + (segment.evenPace ? 1 : 0) + (segment.restores ? 1 : 0);
        const popover = 32 + 20 + (segment.email ? 18 : 0) + (people ? 10 + 11 + people * 20 - 4 : 0) + 10 + 11 + times * 20 - 4 + (segment.redeemable ? 10 + 11 + 24 : 0) + 6;
        segment.side = barTop < popover ? 'bottom' : 'top';
      }
      y += 34 + row + (window.description ? 12 + 16 : 0);
    });
  });
}
export function emptyPooled(): PooledView {
  return { wide: false, empty: false, pools: [], drawn: [], cursorAfter: false, cursor: [], links: [], notices: [], confirm: '' };
}

/** CursorEnableRow in the cost view's provider list: after Codex and Claude, one per environment that could enable it. */
export function cursorRows(statuses: EnvironmentUsageStatus[], state: UsageState) {
  const selected = statuses.filter(status => status.connected && (state.selected === null || state.selected.has(status.environmentId)));
  const many = selected.length > 1;
  return cursorKeychainAccessEnvironments(selected).map(environment => ({ key: environment.environmentId, label: `Cursor${many ? ` · ${environment.label}` : ''}`,
    environment: environment.label, busy: state.cursorPending.has(environment.environmentId) }));
}
export const CURSOR_KEYCHAIN_COPY = 'Requires access to your Cursor login in macOS Keychain.';
