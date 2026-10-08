// The pooled Limits view as data for usage-pooled.contract, T3 Code 1e2ecbd975 (MIT, see
// LICENSE-T3): apps/web/src/components/usage/UsageLimitsPooled.tsx (UsageLimitsPooled, PoolSection,
// PoolWindowCard, PoolBar, PoolSegment, LegendRow, SegmentPopover, RedeemableSegmentPopup,
// AccountAvatar, AccountName, AccountChip, LimitNotices) and UsagePage.tsx (CursorEnableLimits,
// CursorEnableRow). Changes: Contract has no container query, so the bar's `@2xl/pool` width
// (42rem) is worked out here from the page's layout (`poolBarWidth`); the hatching of the spent share
// (a repeating-linear-gradient, which the kernel does not paint) is one SVG path of the same 1px
// stripes 5px apart at 135°; the popover's side is the one the reference lands on after its
// collision flip on an unscrolled page (`popoverSides`: below a bar too near the page's top for the
// popover, above every other; EXACT2-GAPS X17); every figure and word is otherwise the reference's.
import { arr, str } from './domain';
import { driverMeta } from './providers-meta';
import { resolveOfficialAcpRegistryIconUrl } from './acp-icons';
import { PACE_LABEL, barColor, formatResetsIn, remainingPercent } from './usage-limits';
import { REDEEM_IDLE, resetCreditsSummary } from './reset-credits';
import { redactedValue } from './redacted-text';
import { formatUpcomingTimestamp } from './timestamp-format';
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

/** repeating-linear-gradient(135deg, c 0 1px, transparent 1px 5px) as stripes on a box `height` tall and up to 1,400 wide, anchored at its right edge. */
export function hatchPath(height: number, width = 1400): string {
  const step = 5 * Math.SQRT2, parts: string[] = [];
  for (let x = width + height; x >= -height; x -= step) parts.push(`M${(x - height).toFixed(2)} ${height}L${x.toFixed(2)} 0`);
  return parts.join('');
}

/** AccountName: the instance name, else the email's chip, else the driver. */
function accountName(account: LimitAccount) {
  const chip = !account.displayName && account.email ? accountInitials(account.email) : '';
  const hue = account.email ? accountHue(account.email) : 0;
  return { name: account.displayName ?? (chip ? '' : driverLabel(account.driver)), chip,
    chipBg: `oklch(0.85 0.08 ${hue})`, chipFg: `oklch(0.35 0.1 ${hue})` };
}

export type PooledSegment = ReturnType<typeof segmentView>;
function segmentView(state: UsageState, id: string, account: LimitAccount, window: import('./domain').Obj, reset: { restoresPercent: number } | undefined, column: number, count: number, now: number, format: string, side: string) {
  const remaining = remainingPercent(window), resetsIn = formatResetsIn(window, now);
  const credits = Number(account.limits.resetCredits && (account.limits.resetCredits as { availableCount?: number }).availableCount) || 0;
  const label = account.displayName ?? (account.email ? accountInitials(account.email) : account.driver);
  const named = accountName(account), email = redactedValue(account.email ?? '');
  const where = account.environments.length > 0 ? account.environments.map(environment => environment.label).join(', ') : account.sourceLabel ?? '';
  const redeem = state.redeems.get(id) ?? REDEEM_IDLE;
  const resetCredits = account.limits.resetCredits as import('./domain').Obj | undefined;
  const avatar = account.redeem ? 'instance' : account.email ? 'chip' : '';
  return {
    id, column, side, remaining, credits, ...named, showName: count > 1,
    // PopoverPopup align center, shifted inside the page at the bar's ends (Base UI's collision shift).
    align: count > 1 && column === 1 ? 'start' : count > 1 && column === count ? 'end' : 'center',
    label: `${label}: ${remaining}% left${resetsIn ? `, ${resetsIn}` : ''}${credits ? `, ${credits} reset ${credits === 1 ? 'credit' : 'credits'} banked` : ''}`,
    resets: resetsIn ? resetsIn.replace('resets in ', '↻ ') : '',
    creditsAria: credits ? `${credits} reset ${credits === 1 ? 'credit' : 'credits'} banked` : '',
    hatch: remaining < 100 && !!reset,
    // SegmentPopover
    title: account.displayName ?? driverLabel(account.driver), avatar, driver: account.driver,
    badge: account.redeem && account.displayName ? instanceInitials(account.displayName) : '', accent: accent(account.accentColor),
    email: email.value, emailMask: email.placeholder, plan: account.plan ?? '',
    whereLabel: where ? (account.environments.length > 0 ? 'Signed in' : 'Via') : '', where,
    left: `${remaining}%`,
    resetsAt: typeof window.resetsAt === 'string' && window.resetsAt
      ? `${formatUpcomingTimestamp(window.resetsAt, format, now)}${resetsIn ? ` · ${resetsIn.replace('resets in ', 'in ')}` : ''}` : '',
    restores: reset && reset.restoresPercent > 0 ? `+${reset.restoresPercent}% of pool` : '',
    // RedeemableSegmentPopup: the credits row only with a redeem target and credits on show.
    redeemable: !!account.redeem && credits > 0, creditsText: account.redeem && credits > 0 && resetCredits ? resetCreditsSummary(resetCredits, now, true) : '',
    busy: redeem.busy, status: redeem.status ?? '',
  };
}

/** Everything UsageLimitsPooled draws for the selected environments at the page's limits clock. */
export function pooledView(state: UsageState, statuses: EnvironmentUsageStatus[], layout: { barWidth: number; format: string; md: boolean }) {
  const now = state.limitsNow;
  const presentations = new Map<string, LimitPresentation>(selectedConnected(state).map(env => [env.id, { entry: { target: { label: env.label } }, serverConfig: env.config }]));
  const pools = collectLimitPools(collectLimitAccounts(presentations), now);
  const notices = collectLimitNotices(presentations);
  const links = collectExternalUsageLinks(presentations);
  const selectedStatuses = statuses.filter(status => status.connected && presentations.has(status.environmentId));
  const cursorEnvironments = cursorKeychainAccessEnvironments(selectedStatuses);
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
        state.segments.set(id, { account: member.account, name: accountName(member.account).name });
        return [segmentView(state, id, member.account, member.window, restores.get(member.account.key), position + 1, window.columns.length, now, layout.format, 'top')];
      });
      return {
        key: `${window.kind}:${window.id}`, prefix: `${poolIndex}-${windowIndex}-`, label: details?.label ?? window.label, description: details?.description ?? '',
        remaining: window.remainingPercent, pace: window.pace ?? '', paceLabel: window.pace ? PACE_LABEL[window.pace] : '',
        refill: nextRefill && window.columns.length > 1 ? `↻ +${nextRefill.restoresPercent}%` : '',
        count: window.columns.length, segments,
        // The popover closed before the confirm, so each outcome needs a home outside it: under the bar.
        // Explicit rows after the strip (and, narrow, after its legend), as the grid places them.
        statuses: segments.filter(segment => segment.status !== '').map((segment, index) => ({ key: segment.id, row: (wide ? 2 : window.columns.length + 2) + index,
          name: segment.name, chip: segment.chip, chipBg: segment.chipBg, chipFg: segment.chipFg, text: segment.status })),
      };
    });
    // provider-settings-upkeep: an ACP agent's pool draws its registry icon (a live provider's iconUrl).
    const acp = pool.driver === 'acpRegistry' ? [...presentations.values()].flatMap(entry => arr(entry.serverConfig?.providers)).find(provider => provider.driver === 'acpRegistry' && str(provider.iconUrl)) : undefined;
    return { key: pool.driver, prefix: `${poolIndex}-`, driver: pool.driver, icon: acp ? resolveOfficialAcpRegistryIconUrl(str(acp.iconUrl)) ?? '' : '', label: driverLabel(pool.driver), light, dark, windows, cursorBefore: cursorEnvironments.length > 0 && poolIndex === cursorPromptAt };
  });
  state.links = links.map(link => link.url);
  popoverSides(views, { wide, md: layout.md, cursorRows: cursorEnvironments.length });
  return {
    wide, hatchWide: hatchPath(32), hatchNarrow: hatchPath(20),
    empty: pools.length === 0 && notices.length === 0 && cursorEnvironments.length === 0 && links.length === 0,
    pools: views, cursorAfter: cursorEnvironments.length > 0 && cursorPromptAt === pools.length,
    cursor: cursorEnvironments.map(environment => ({ key: environment.environmentId, label: environment.label,
      button: cursorEnvironments.length > 1 ? `Enable on ${environment.label}` : 'Enable', busy: state.cursorPending.has(environment.environmentId) })),
    links: links.map(link => ({ key: link.url, label: link.label, chatgpt: link.url === CHATGPT_USAGE_URL,
      message: link.url === CHATGPT_USAGE_URL ? 'View usage in ChatGPT with your connected account.' : link.message ?? '', url: link.url })),
    notices: notices.map(text => ({ key: text, text })),
    confirm: state.confirm,
  };
}
export type PooledView = ReturnType<typeof pooledView>;

type SideWindow = { count: number; refill: string; description: string; statuses: unknown[];
  segments: { side: string; email: string; plan: string; whereLabel: string; resetsAt: string; restores: string; redeemable: boolean }[] };
/**
 * The side each popover takes, as Base UI's collision flip would on an unscrolled page: a bar whose top
 * lies nearer the scroll area's top than the popover is tall opens below it, every other above. The
 * heights are the layout's (UsageLimitsPooled's gaps, PoolWindowCard's padding, SegmentPopover's rows),
 * in points; Contract has no position-try fallback to measure it (X17, exact2 #112).
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
        const people = (segment.plan ? 1 : 0) + (segment.whereLabel ? 1 : 0), times = 1 + (segment.resetsAt ? 1 : 0) + (segment.restores ? 1 : 0);
        const popover = 32 + 20 + (segment.email ? 18 : 0) + (people ? 10 + 11 + people * 20 - 4 : 0) + 10 + 11 + times * 20 - 4 + (segment.redeemable ? 10 + 11 + 24 : 0) + 6;
        segment.side = barTop < popover ? 'bottom' : 'top';
      }
      y += 34 + row + (window.description ? 12 + 16 : 0);
    });
  });
}
export function emptyPooled(): PooledView {
  return { wide: false, hatchWide: '', hatchNarrow: '', empty: false, pools: [], cursorAfter: false, cursor: [], links: [], notices: [], confirm: '' };
}

/** CursorEnableRow in the cost view's provider list: after Codex and Claude, one per environment that could enable it. */
export function cursorRows(statuses: EnvironmentUsageStatus[], state: UsageState) {
  const selected = statuses.filter(status => status.connected && (state.selected === null || state.selected.has(status.environmentId)));
  const many = selected.length > 1;
  return cursorKeychainAccessEnvironments(selected).map(environment => ({ key: environment.environmentId, label: `Cursor${many ? ` · ${environment.label}` : ''}`,
    environment: environment.label, busy: state.cursorPending.has(environment.environmentId) }));
}
export const CURSOR_KEYCHAIN_COPY = 'Requires access to your Cursor login in macOS Keychain.';
