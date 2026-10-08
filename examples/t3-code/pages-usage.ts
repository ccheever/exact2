// The Usage page (lane "pages"): server.getUsageSummary merged and presented
// as the reference UsagePage does. Sources: T3 Code (MIT, see LICENSE-T3)
// apps/web/src/components/usage/{UsagePage,UsageProviderChart,usageProviders,
// usageShortcuts,usagePagePreferences,UsageLimits,UsageLimitsPooled}.tsx and
// packages/shared/src/{usageFormat,usageMerge,usageLimits}.ts.
import { resolveOfficialAcpRegistryIconUrl } from './acp-icons';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, providerAvailable, type Files, type Native } from './protocol';
import type { T3Client } from './client';
import { pagesPrefs, type UsagePrefs } from './pages-prefs';
import { emptyPrices, presentPrices } from './pages-usage-prices';
import { checkMenu, uniqueProbes, type Probe } from './r5-composer-menus';
import { emptyDetail, modelDetail, modelKey, modelRows, openModel, pageShares, setOpenModel, type ModelRowView, type ShareBarView } from './pages-usage-detail';
import { letGo } from './let-go';
import { usesChatGptSharing } from './chatgpt-plan'; // managed-codex-chatgpt

export const USAGE_CONTRACT_VERSION = 6;
const MERGE_COMPATIBLE_SINCE = 4;
const HOUR = 3_600_000;

// ── Formatting (usageFormat.ts) ─────────────────────────────────────────────

const group = (digits: string) => digits.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
export function formatUsd(value: number): string {
  const fixed = Math.abs(value).toFixed(2);
  const [whole = '0', cents = '00'] = fixed.split('.');
  return `${value < 0 && Number(fixed) !== 0 ? '-' : ''}$${group(whole)}.${cents}`;
}
export function formatCount(value: number): string { return group(String(Math.round(value))); }
const trim = (value: number) => {
  const abs = Math.abs(value);
  return value.toFixed(abs >= 100 ? 0 : abs >= 10 ? 1 : 2).replace(/\.0+$/, '');
};
export function formatTokens(value: number): string {
  const abs = Math.abs(value);
  if (abs >= 1e12) return `${trim(value / 1e12)}T`;
  if (abs >= 1e9) return `${trim(value / 1e9)}B`;
  if (abs >= 1e6) return `${trim(value / 1e6)}M`;
  if (abs >= 1e3) return `${trim(value / 1e3)}K`;
  return formatCount(value);
}
export function formatPercent(share: number, digits = 1): string {
  const percent = share * 100, smallest = 10 ** -digits;
  if (percent > 0 && percent < smallest) return `<${smallest.toFixed(digits)}%`;
  return `${percent.toFixed(digits)}%`;
}
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
export function formatDayShort(day: string): string {
  const [year, month, date] = day.split('-').map(Number);
  if (!year || !month || !date) return day;
  return `${MONTHS[month - 1] ?? ''} ${date}`;
}
/** `3 PM` for an hourly bucket start, in this Mac's zone. */
export function formatHourShort(hourStart: string): string {
  const at = new Date(hourStart);
  if (Number.isNaN(at.getTime())) return hourStart;
  const hour = at.getHours();
  return `${hour % 12 === 0 ? 12 : hour % 12} ${hour < 12 ? 'AM' : 'PM'}`;
}
export function formatDateTimeShort(instant: string): string {
  const at = new Date(instant);
  if (Number.isNaN(at.getTime())) return instant;
  return `${MONTHS[at.getMonth()]} ${at.getDate()}, ${formatHourShort(instant)}`;
}
export function enumerateDays(sinceDay: string, untilDay: string): string[] {
  const days: string[] = [];
  const start = Date.parse(`${sinceDay}T00:00:00Z`), end = Date.parse(`${untilDay}T00:00:00Z`);
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return days;
  for (let cursor = start; cursor <= end; cursor += 86_400_000) days.push(new Date(cursor).toISOString().slice(0, 10));
  return days;
}
export function enumerateHourStarts(sinceTime: string, untilTime: string): string[] {
  const starts: string[] = [];
  const start = Date.parse(sinceTime), end = Date.parse(untilTime);
  if (Number.isNaN(start) || Number.isNaN(end) || end <= start) return starts;
  for (let cursor = start; cursor < end; cursor += HOUR) starts.push(new Date(cursor).toISOString());
  return starts;
}
const localDay = (at: Date) => `${at.getFullYear()}-${String(at.getMonth() + 1).padStart(2, '0')}-${String(at.getDate()).padStart(2, '0')}`;
function timeZone(): string {
  try { return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'; } catch { return 'UTC'; }
}
export type UsageWindow = { sinceDay: string; untilDay: string; timeZone: string; resolution: 'day' | 'hour'; sinceTime?: string; untilTime?: string };
/** makeWindow: calendar days in this Mac's zone, or an exact rolling 24 hours. */
export function makeWindow(days: number, now: number, zone = timeZone()): UsageWindow {
  const end = new Date(now);
  if (days === 1) {
    const untilMs = Math.floor(now / 60_000) * 60_000, sinceMs = untilMs - 24 * HOUR;
    return { sinceDay: localDay(new Date(sinceMs)), untilDay: localDay(new Date(untilMs)), timeZone: zone, resolution: 'hour',
      sinceTime: new Date(sinceMs).toISOString(), untilTime: new Date(untilMs).toISOString() };
  }
  const untilDay = localDay(end);
  const [year = 0, month = 1, date = 1] = untilDay.split('-').map(Number);
  const start = new Date(Date.UTC(year, month - 1, date - (days - 1)));
  return { sinceDay: start.toISOString().slice(0, 10), untilDay, timeZone: zone, resolution: 'day' };
}

// ── Merge (usageMerge.ts) ───────────────────────────────────────────────────

export type ProviderKind = 'codex' | 'claude' | 'grok' | 'cursor' | 'opencode' | 'antigravity';
export const PROVIDER_ORDER: ProviderKind[] = ['codex', 'claude', 'grok', 'cursor', 'opencode', 'antigravity'];
// usageProviders.ts: label, series colour (light, dark) and the driver whose mark it shows.
export const PROVIDERS: Record<ProviderKind, { label: string; color: [string, string]; driver: string }> = {
  codex: { label: 'Codex', color: ['#27272a', '#f5f5f5'], driver: 'codex' },
  claude: { label: 'Claude Code', color: ['#d97757', '#d97757'], driver: 'claudeAgent' },
  grok: { label: 'Grok Build', color: ['#636365', '#b3b3b3'], driver: 'grok' },
  cursor: { label: 'Cursor', color: ['#8b8b8b', '#8b8b8b'], driver: 'cursor' },
  opencode: { label: 'OpenCode', color: ['#5b9bbd', '#5b9bbd'], driver: 'opencode' },
  antigravity: { label: 'Antigravity', color: ['#8c7bd1', '#8c7bd1'], driver: 'antigravity' },
};
type Band = { costUsd: number; totalTokens: number };
export type Period = { key: string; day: string; costUsd: number; totalTokens: number; byProvider: Map<string, Band> };
export type TokenTotals = { uncachedInputTokens: number; cachedInputTokens: number; cacheCreationTokens: number; outputTokens: number };
/** costUsd by token category; `unsplit` is cost no rates could split (usageMerge CategoryCost). */
export type CategoryCost = { input: number; cacheRead: number; cacheWrite: number; output: number; unsplit: number };
/** costUsd by request speed; servers that predate speeds count as standard (usageMerge SpeedCost). */
export type SpeedCost = { standard: number; fast: number; ultrafast: number; premium: number };
export type ModelTotals = { model: string; provider: string; costUsd: number; totalTokens: number; tokens: TokenTotals; records: number; unpricedRecords: number; unpricedTokens: number; costShare: number; tokenShare: number };
export type Merged = {
  costUsd: number; uncachedInputTokens: number; cachedInputTokens: number; cacheCreationTokens: number; outputTokens: number; totalTokens: number;
  records: number; sessions: number; unpricedShare: number; cacheSavingsUsd: number;
  providers: { provider: string; costUsd: number; totalTokens: number; sessions: number; costShare: number; tokenShare: number }[];
  models: ModelTotals[];
  daily: Period[]; hourly: Period[]; duplicates: string[]; mismatches: { environment: string; direction: string }[];
  categoryCost: CategoryCost; speedCost: SpeedCost;
};
const fingerprintKey = (fingerprint: Obj) => [str(fingerprint.hostId), str(fingerprint.provider), str(fingerprint.resolvedHomePath), str(fingerprint.volumeId)].join(' ');
const bucketTokens = (bucket: Obj) => { const totals = obj(bucket.totals); return num(totals.uncachedInputTokens) + num(totals.cachedInputTokens) + num(totals.cacheCreationTokens) + num(totals.outputTokens); };

/** mergeUsage for the environments this client reads: duplicate transcript directories count once. */
export function mergeUsage(environments: { id: string; label: string; summary: Obj }[]): Merged {
  const current = environments.filter(environment => num(environment.summary.contractVersion) >= MERGE_COMPATIBLE_SINCE && num(environment.summary.contractVersion) <= USAGE_CONTRACT_VERSION);
  const mismatches = environments.filter(environment => !current.includes(environment))
    .map(environment => ({ environment: environment.label, direction: num(environment.summary.contractVersion) < USAGE_CONTRACT_VERSION ? 'serverBehind' : 'clientBehind' }));
  const ordered = [...current].sort((a, b) => (Date.parse(str(b.summary.readAt)) || 0) - (Date.parse(str(a.summary.readAt)) || 0) || a.id.localeCompare(b.id));
  const owner = new Map<string, string>(), sessionsByFingerprint = new Map<string, number>(), duplicates: string[] = [];
  for (const status of ['ok', 'partial', 'failed']) for (const environment of ordered) for (const source of arr(environment.summary.sources)) {
    if (source.status !== status) continue;
    const key = fingerprintKey(obj(source.fingerprint));
    if (owner.has(key)) { duplicates.push(`${environment.label}: ${str(obj(source.fingerprint).resolvedHomePath)}`); continue; }
    owner.set(key, environment.id); sessionsByFingerprint.set(key, num(source.distinctSessions));
  }
  let costUsd = 0, uncached = 0, cached = 0, creation = 0, output = 0, records = 0, sessions = 0, savings = 0, unpriced = 0;
  const category = { input: 0, cacheRead: 0, cacheWrite: 0, output: 0 }, speed = { fast: 0, ultrafast: 0, premium: 0 };
  const providers = new Map<string, { costUsd: number; totalTokens: number; sessions: number }>();
  const models = new Map<string, Omit<ModelTotals, 'model' | 'costShare' | 'tokenShare'>>();
  const daily = new Map<string, Period>(), hourly = new Map<string, Period>();
  const add = (map: Map<string, Period>, key: string, day: string, bucket: Obj, tokens: number) => {
    const period = map.get(key) ?? { key, day, costUsd: 0, totalTokens: 0, byProvider: new Map<string, Band>() };
    period.costUsd += num(bucket.costUsd); period.totalTokens += tokens;
    const band = period.byProvider.get(str(bucket.provider)) ?? { costUsd: 0, totalTokens: 0 };
    band.costUsd += num(bucket.costUsd); band.totalTokens += tokens;
    period.byProvider.set(str(bucket.provider), band); map.set(key, period);
  };
  for (const environment of current) {
    const ownedProviders = new Set<string>(), ownedSources = new Set<string>();
    for (const source of arr(environment.summary.sources)) {
      if (source.status === 'missing') continue;
      const fingerprint = obj(source.fingerprint), key = fingerprintKey(fingerprint);
      if (owner.get(key) !== environment.id) continue;
      const provider = str(fingerprint.provider), count = sessionsByFingerprint.get(key) ?? num(source.distinctSessions);
      ownedProviders.add(provider); ownedSources.add(`${provider}\u0000${str(fingerprint.resolvedHomePath)}`);
      sessions += count;
      if (count) { const entry = providers.get(provider) ?? { costUsd: 0, totalTokens: 0, sessions: 0 }; entry.sessions += count; providers.set(provider, entry); }
    }
    for (const bucket of arr(environment.summary.buckets)) {
      const provider = str(bucket.provider);
      const owned = bucket.sourcePath === undefined ? ownedProviders.has(provider) : ownedSources.has(`${provider}\u0000${str(bucket.sourcePath)}`);
      if (!owned) continue;
      const tokens = bucketTokens(bucket), totals = obj(bucket.totals);
      costUsd += num(bucket.costUsd); savings += num(bucket.cacheSavingsUsd);
      uncached += num(totals.uncachedInputTokens); cached += num(totals.cachedInputTokens); creation += num(totals.cacheCreationTokens); output += num(totals.outputTokens);
      records += num(bucket.records); unpriced += num(bucket.unpricedRecords);
      if (bucket.categoryCostUsd && typeof bucket.categoryCostUsd === 'object') {
        const split = obj(bucket.categoryCostUsd);
        category.input += num(split.input); category.cacheRead += num(split.cacheRead); category.cacheWrite += num(split.cacheWrite); category.output += num(split.output);
      }
      speed.fast += num(bucket.fastCostUsd); speed.ultrafast += num(bucket.ultrafastCostUsd); speed.premium += num(bucket.speedPremiumUsd);
      const entry = providers.get(provider) ?? { costUsd: 0, totalTokens: 0, sessions: 0 };
      entry.costUsd += num(bucket.costUsd); entry.totalTokens += tokens; providers.set(provider, entry);
      const modelKey = `${provider} ${str(bucket.model)}`;
      const model = models.get(modelKey) ?? { provider, costUsd: 0, totalTokens: 0, tokens: { uncachedInputTokens: 0, cachedInputTokens: 0, cacheCreationTokens: 0, outputTokens: 0 }, records: 0, unpricedRecords: 0, unpricedTokens: 0 };
      model.costUsd += num(bucket.costUsd); model.totalTokens += tokens; model.records += num(bucket.records); model.unpricedRecords += num(bucket.unpricedRecords);
      model.tokens = { uncachedInputTokens: model.tokens.uncachedInputTokens + num(totals.uncachedInputTokens), cachedInputTokens: model.tokens.cachedInputTokens + num(totals.cachedInputTokens),
        cacheCreationTokens: model.tokens.cacheCreationTokens + num(totals.cacheCreationTokens), outputTokens: model.tokens.outputTokens + num(totals.outputTokens) };
      // A cell that mixes unpriced records with reported costs counts its tokens by record share.
      if (num(bucket.records) > 0) model.unpricedTokens += (tokens * num(bucket.unpricedRecords)) / num(bucket.records);
      models.set(modelKey, model);
      add(daily, str(bucket.day), str(bucket.day), bucket, tokens);
      if (typeof bucket.hourStart === 'string') add(hourly, bucket.hourStart, str(bucket.day), bucket, tokens);
    }
  }
  const totalTokens = uncached + cached + creation + output;
  return {
    costUsd, uncachedInputTokens: uncached, cachedInputTokens: cached, cacheCreationTokens: creation, outputTokens: output, totalTokens,
    records, sessions, unpricedShare: records === 0 ? 0 : unpriced / records, cacheSavingsUsd: savings,
    providers: [...providers].map(([provider, totals]) => ({ provider, ...totals, costShare: costUsd === 0 ? 0 : totals.costUsd / costUsd, tokenShare: totalTokens === 0 ? 0 : totals.totalTokens / totalTokens }))
      .sort((a, b) => b.costUsd - a.costUsd),
    models: [...models].map(([key, totals]) => ({ model: key.slice(key.indexOf(' ') + 1), ...totals, costShare: costUsd === 0 ? 0 : totals.costUsd / costUsd,
      tokenShare: totalTokens === 0 ? 0 : totals.totalTokens / totalTokens })) // upstream 0c81120137
      .sort((a, b) => b.costUsd - a.costUsd || b.totalTokens - a.totalTokens),
    daily: [...daily.values()].sort((a, b) => a.key.localeCompare(b.key)),
    hourly: [...hourly.values()].sort((a, b) => a.key.localeCompare(b.key)),
    duplicates, mismatches,
    // Clamped so float error never shows as a negative remainder.
    categoryCost: { ...category, unsplit: Math.max(0, costUsd - category.input - category.cacheRead - category.cacheWrite - category.output) },
    speedCost: { ...speed, standard: Math.max(0, costUsd - speed.fast - speed.ultrafast) },
  };
}
export const isModelCostUnknown = (model: { records: number; unpricedRecords: number }) => model.records > 0 && model.unpricedRecords >= model.records;

// ── Chart (UsageProviderChart.tsx) ──────────────────────────────────────────

export const PLOT_HEIGHT = 224;
const VIEW_HEIGHT = 260, PLOT_TOP = 8;
/** A readable 1/2/5 × 10ⁿ maximum at or above the peak, and its ticks. */
export function niceScale(peak: number, count: number): { max: number; ticks: number[] } {
  if (peak <= 0) return { max: 0, ticks: [0] };
  const raw = peak / count, magnitude = 10 ** Math.floor(Math.log10(raw)), normalized = raw / magnitude;
  const step = (normalized > 5 ? 10 : normalized > 2 ? 5 : normalized > 1 ? 2 : 1) * magnitude;
  const max = Math.ceil(peak / step) * step, ticks: number[] = [];
  for (let value = 0; value <= max + step * 1e-6; value += step) ticks.push(value);
  return { max, ticks };
}
type Point = { x: number; y: number };
function tangents(points: Point[]): number[] {
  const count = points.length;
  if (count < 2) return [0];
  const slopes: number[] = [];
  for (let index = 0; index < count - 1; index++) {
    const dx = points[index + 1]!.x - points[index]!.x, dy = points[index + 1]!.y - points[index]!.y;
    slopes.push(dx === 0 ? 0 : dy / dx);
  }
  const result = Array.from({ length: count }, () => 0);
  result[0] = slopes[0] ?? 0; result[count - 1] = slopes[count - 2] ?? 0;
  for (let index = 1; index < count - 1; index++) {
    const previous = slopes[index - 1] ?? 0, next = slopes[index] ?? 0;
    result[index] = previous * next <= 0 ? 0 : (previous + next) / 2;
  }
  for (let index = 0; index < count - 1; index++) {
    const slope = slopes[index] ?? 0;
    if (slope === 0) { result[index] = 0; result[index + 1] = 0; continue; }
    const a = result[index]! / slope, b = result[index + 1]! / slope, magnitude = a * a + b * b;
    if (magnitude > 9) { const scale = 3 / Math.sqrt(magnitude); result[index] = scale * a * slope; result[index + 1] = scale * b * slope; }
  }
  return result;
}
/** The monotone cubic through the points, in the plot's own pixels. */
export function curvePath(points: Point[]): string {
  if (points.length < 2) return '';
  const t = tangents(points), f = (value: number) => value.toFixed(2);
  let path = `M${f(points[0]!.x)},${f(points[0]!.y)}`;
  for (let index = 0; index < points.length - 1; index++) {
    const from = points[index]!, to = points[index + 1]!, dx = to.x - from.x;
    path += ` C${f(from.x + dx / 3)},${f(from.y + (t[index]! * dx) / 3)} ${f(to.x - dx / 3)},${f(to.y - (t[index + 1]! * dx) / 3)} ${f(to.x)},${f(to.y)}`;
  }
  return path;
}
export type Chart = { ticks: { key: string; label: string; top: number }[]; series: { key: string; area: string; line: string; light: string; dark: string }[]; start: string; middle: string; end: string };
/** The layered provider areas (heaviest painted first), gridline ticks and the three axis dates. */
export function buildChart(merged: Merged, periods: string[], resolution: 'day' | 'hour', metric: string, width: number): Chart {
  const byPeriod = new Map((resolution === 'hour' ? merged.hourly : merged.daily).map(period => [period.key, period]));
  const active = providersWithUsage(merged);
  const value = (period: string, provider: string) => { const band = byPeriod.get(period)?.byProvider.get(provider); return band ? (metric === 'tokens' ? band.totalTokens : band.costUsd) : 0; };
  const peak = periods.reduce((max, period) => PROVIDER_ORDER.reduce((inner, provider) => Math.max(inner, value(period, provider)), max), 0);
  const { max, ticks } = niceScale(peak, 4);
  const step = periods.length <= 1 ? 0 : width / (periods.length - 1);
  const toY = (amount: number) => max === 0 ? PLOT_HEIGHT : PLOT_HEIGHT - (amount / max) * (PLOT_HEIGHT - PLOT_TOP * PLOT_HEIGHT / VIEW_HEIGHT);
  const format = metric === 'tokens' ? formatTokens : formatUsd;
  const series = active.map(provider => {
    const line = curvePath(periods.map((period, index) => ({ x: index * step, y: toY(value(period, provider)) })));
    const total = periods.reduce((sum, period) => sum + value(period, provider), 0);
    const [light, dark] = PROVIDERS[provider].color;
    return { key: provider, total, line, area: line ? `${line} L${width.toFixed(2)},${PLOT_HEIGHT} L0,${PLOT_HEIGHT} Z` : '', light, dark };
  }).sort((a, b) => b.total - a.total).map(({ total: _total, ...rest }) => rest);
  const label = (period: string | undefined) => period === undefined ? '' : resolution === 'hour' ? formatHourShort(period) : formatDayShort(period);
  return { ticks: ticks.map(tick => ({ key: String(tick), label: tick === 0 ? '0' : format(tick), top: toY(tick) })), series,
    start: label(periods[0]), middle: label(periods[Math.floor(periods.length / 2)]), end: label(periods[periods.length - 1]) };
}
export function providersWithUsage(merged: Merged): ProviderKind[] {
  const active = new Set(merged.providers.filter(entry => entry.totalTokens > 0 || entry.costUsd > 0).map(entry => entry.provider));
  return PROVIDER_ORDER.filter(provider => active.has(provider));
}

// ── Limits (UsageLimits.tsx / usageLimits.ts) ───────────────────────────────

const LIMIT_DRIVERS: Record<string, { label: string; color: [string, string] }> = {
  codex: { label: 'Codex', color: PROVIDERS.codex.color }, claudeAgent: { label: 'Claude Code', color: PROVIDERS.claude.color },
  cursor: { label: 'Cursor', color: ['#27272a', '#f5f5f5'] }, grok: { label: 'Grok Build', color: ['#27272a', '#f5f5f5'] },
};
export function formatDuration(ms: number): string {
  const remaining = Math.max(0, ms), days = Math.floor(remaining / 86_400_000), hours = Math.floor((remaining % 86_400_000) / HOUR), minutes = Math.floor((remaining % HOUR) / 60_000);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}
export type LimitPool = { key: string; driver: string; icon: string; label: string; windows: { key: string; label: string; remaining: number; fill: number; pace: string; resets: string; light: string; dark: string }[] };
/** Providers on this environment that report subscription windows (providersWithLimits), one section each. */
export function limitPools(providers: Obj[], now: number): LimitPool[] {
  const pools = new Map<string, LimitPool>();
  for (const provider of providers) {
    if (provider.enabled === false || provider.installed === false || !providerAvailable(provider)) continue;
    const limits = obj(provider.usageLimits);
    if (!provider.usageLimits) continue;
    const driver = str(provider.driver), meta = LIMIT_DRIVERS[driver] ?? { label: str(provider.displayName, driver), color: ['#27272a', '#f5f5f5'] as [string, string] };
    // provider-settings-upkeep: an ACP agent's pool draws its registry icon (the live provider's iconUrl).
    const pool = pools.get(driver) ?? { key: driver, driver, icon: driver === 'acpRegistry' ? resolveOfficialAcpRegistryIconUrl(str(provider.iconUrl)) ?? '' : '', label: meta.label, windows: [] };
    for (const window of arr(limits.windows)) {
      if (pool.windows.some(existing => existing.key === str(window.id))) continue;
      const used = Math.max(0, Math.min(100, num(window.usedPercent))), remaining = Math.round(100 - used);
      const resetsAt = Date.parse(str(window.resetsAt)), duration = num(window.windowDurationMins) * 60_000;
      const elapsed = Number.isFinite(resetsAt) && duration > 0 ? Math.max(0, Math.min(1, (duration - (resetsAt - now)) / duration)) : null;
      const gap = elapsed === null ? 0 : used - elapsed * 100;
      pool.windows.push({ key: str(window.id), label: str(window.label), remaining, fill: remaining,
        pace: elapsed === null ? '' : gap > 5 ? 'ahead' : gap < -5 ? 'under' : 'on',
        resets: Number.isFinite(resetsAt) ? (resetsAt <= now ? 'resets now' : `resets in ${formatDuration(resetsAt - now)}`) : '', light: meta.color[0], dark: meta.color[1] });
    }
    if (pool.windows.length) pools.set(driver, pool);
  }
  return [...pools.values()];
}

// ── Page state and the resource ─────────────────────────────────────────────

type Prefs = UsagePrefs;
export const METRICS = ['cost', 'tokens', 'limits'];
export const WINDOWS = [{ days: 1, label: 'Past 24h' }, { days: 7, label: '7 days' }, { days: 30, label: '30 days' }, { days: 90, label: '90 days' }];
/** usagePagePreferences: Limits and 30 days on first visit; the last pick sticks after that. */
export function usagePrefs(owner: { local: object }): Prefs {
  const saved = pagesPrefs(owner).usage;
  return saved && METRICS.includes(saved.metric) && WINDOWS.some(window => window.days === saved.windowDays) ? { ...saved } : { metric: 'limits', windowDays: 30 };
}
export function saveUsagePrefs(owner: { local: object }, change: Partial<Prefs>): void {
  const next = { ...usagePrefs(owner), ...change };
  if (!METRICS.includes(next.metric) || !WINDOWS.some(window => window.days === next.windowDays)) throw new ClientError('Unknown usage view.');
  pagesPrefs(owner).usage = next;
}

type Cached = { key: string; window: UsageWindow; summary: Obj | null; error: string };
const summaries = new WeakMap<object, Cached>();
/** Prices or mappings changed: the next read re-aggregates (the server folds and prices on read). */
export const forgetUsage = (client: object) => { summaries.delete(client); };
const limitRefreshes = new WeakMap<object, string>();

/** The plot's width: the page container (max 1024, 24pt gutters), less the summary column at lg and the 64pt axis. */
export function plotWidth(viewportWidth: number, sidebarWidth: number): number {
  const content = Math.min(1024, Math.max(0, viewportWidth - sidebarWidth)) - 48;
  const chartColumn = viewportWidth >= 1024 ? content - 288 - 24 : content;
  return Math.max(40, Math.round(chartColumn - 64));
}

type UsageInput = { open: boolean; metric: string; windowDays: number; breakdown: string; refresh: number; width: number; now: number; environmentOff: boolean; viewport?: number };
export async function usagePage(client: T3Client, native: Native | null | undefined, storage: Files | null, input: UsageInput) {
  const page = await usageView(client, native, storage, input);
  // The environment menu sizes to its widest row (Model prices included), as MenuPopup does.
  // r5-composer: both menus size from measured texts (r5-composer-menus.ts checkMenu; probes drawn by the page).
  const environments = checkMenu(client.presentation, [{ label: 'All environments', status: '' }, ...(page.environmentCount ? [{ label: page.environmentName, status: page.metric === 'limits' ? '' : page.environmentStatus }] : [])], ['Model prices']);
  const prices = checkMenu(client.presentation, [{ label: 'All environments', status: '' }, ...page.prices.targets.map(target => ({ label: target.label, status: target.unavailable }))]);
  page.menuWidth = environments.width; page.prices.menuWidth = prices.width;
  page.menuProbes = uniqueProbes([...environments.probes, ...prices.probes]);
  return page;
}
async function usageView(client: T3Client, native: Native | null | undefined, storage: Files | null, input: UsageInput) {
  const prefs = usagePrefs(client);
  const metric = METRICS.includes(input.metric) ? input.metric : prefs.metric;
  const windowDays = WINDOWS.some(window => window.days === input.windowDays) ? input.windowDays : prefs.windowDays;
  const page = emptyUsage(metric, windowDays, input.breakdown === 'time' ? 'time' : 'model', input.width);
  const connected = client.connection === 'connected' && client.ready;
  page.environmentLabel = 'All environments';
  page.environmentName = str(obj(client.config.environment).label, 'This environment');
  page.environmentSelected = !input.environmentOff;
  page.environmentCount = connected ? 1 : 0;
  if (input.open && metric !== 'limits') page.prices = presentPrices(client);
  if (!connected) { page.empty = `Connect an environment to see ${metric === 'limits' ? 'limits' : 'usage'}.`; page.environmentStatus = ''; return page; }
  if (input.environmentOff) { page.empty = `Select an environment to see ${metric === 'limits' ? 'limits' : 'usage'}.`; page.environmentLabel = '0 environments'; return page; }
  if (!input.open || !native?.available) return page;
  if (metric === 'limits') {
    // refreshLimits: the provider probe re-reads subscription windows; the config subscription delivers them.
    const key = `${client.environmentId}:${input.refresh}`;
    if (limitRefreshes.get(client) !== key) {
      limitRefreshes.set(client, key);
      try { await client.rpc(native, 'server.refreshProviders', {}); } catch { /* the last published windows still show */ }
    }
    page.limits = limitPools(arr(client.config.providers), input.now);
    page.limitsEmpty = page.limits.length === 0;
    page.environmentStatus = 'Ready';
    return page;
  }
  const key = `${client.environmentId}:${windowDays}:${input.refresh}`;
  let cached = summaries.get(client);
  if (!cached || cached.key !== key) {
    const window = makeWindow(windowDays, input.now);
    cached = { key, window, summary: null, error: '' };
    try {
      const payload: Obj = { sinceDay: window.sinceDay, untilDay: window.untilDay, timeZone: window.timeZone, resolution: window.resolution };
      if (window.sinceTime) { payload.sinceTime = window.sinceTime; payload.untilTime = window.untilTime; }
      cached.summary = await client.rpc(native, 'server.getUsageSummary', payload);
    } catch (error) { if (letGo(error)) throw error; cached.error = error instanceof Error ? error.message : 'Usage could not be read.'; }
    summaries.set(client, cached);
  }
  if (!cached.summary) { page.error = cached.error; page.environmentStatus = 'Unavailable'; page.empty = `${page.environmentName} could not report usage.`; return page; }
  const environment = { id: client.environmentId, label: page.environmentName, summary: cached.summary };
  page.chatgptShared = arr(client.config.providers).some(usesChatGptSharing); // the selected environment shares a ChatGPT plan
  const merged = mergeUsage([environment]);
  presentUsage(page, merged, cached.window, cached.summary);
  // UsageModelDialog for the row the person opened, while it is still in the window.
  const open = merged.models.find(model => modelKey(model) === openModel(client));
  if (open) page.detail = modelDetail(open, environment, windowPeriods(cached.window), cached.window.resolution, metric, input.viewport ?? 1280);
  else if (openModel(client)) setOpenModel(client, '');
  return page;
}
export function windowPeriods(window: UsageWindow): string[] {
  return window.resolution === 'hour' && window.sinceTime && window.untilTime ? enumerateHourStarts(window.sinceTime, window.untilTime) : enumerateDays(window.sinceDay, window.untilDay);
}

export function emptyUsage(metric: string, windowDays: number, breakdown: string, width: number) {
  return {
    chatgptShared: false, metric, windowDays, breakdown, plotWidth: width, menuWidth: 224, menuProbes: [] as Probe[], empty: '', error: '', environmentLabel: 'All environments', environmentName: '', environmentStatus: 'Scanning…',
    environmentSelected: true, environmentCount: 0, windowLabel: '', total: '', sessions: '', unpriced: '', notices: [] as { key: string; text: string }[],
    providers: [] as { key: string; driver: string; label: string; light: string; dark: string; sessions: string; value: string; detail: string }[],
    chartTitle: '', chart: { ticks: [], series: [], start: '', middle: '', end: '' } as Chart,
    totals: [] as { key: string; label: string; value: string }[],
    models: [] as ModelRowView[], shares: [] as ShareBarView[], detail: emptyDetail(), prices: emptyPrices(),
    periodLabel: 'Day', columns: [] as { key: string; label: string }[],
    periods: [] as { key: string; label: string; cells: { key: string; text: string }[]; total: string; tokens: string }[],
    limits: [] as LimitPool[], limitsEmpty: false, coverage: [] as { key: string; text: string }[],
  };
}
export type UsageView = ReturnType<typeof emptyUsage>;

/** Everything UsagePage renders for a loaded summary. */
export function presentUsage(page: UsageView, merged: Merged, window: UsageWindow, summary: Obj): UsageView {
  const hourly = window.resolution === 'hour';
  const periods = windowPeriods(window);
  const tokens = page.metric === 'tokens';
  page.environmentStatus = 'Ready';
  page.windowLabel = hourly && window.sinceTime && window.untilTime ? `${formatDateTimeShort(window.sinceTime)} to ${formatDateTimeShort(window.untilTime)}` : `${formatDayShort(window.sinceDay)} to ${formatDayShort(window.untilDay)}`;
  page.total = tokens ? formatTokens(merged.totalTokens) : formatUsd(merged.costUsd);
  page.sessions = `${formatCount(merged.sessions)} sessions`;
  page.unpriced = !tokens && merged.unpricedShare > 0 ? `API estimate excludes ${formatPercent(merged.unpricedShare)} unpriced records.` : '';
  page.notices = [...new Set(arr(summary.sources).flatMap(source => str(source.message) && !source.action
    && (source.status === 'partial' || source.status === 'failed' || obj(source.fingerprint).provider === 'cursor') ? [str(source.message)] : []))]
    .map(text => ({ key: text, text }));
  page.coverage = [
    ...merged.mismatches.map(mismatch => ({ key: `m:${mismatch.environment}`, text: mismatch.direction === 'serverBehind'
      ? `${mismatch.environment} runs an older server version and is excluded from totals.` : `This client is older than the server on ${mismatch.environment}; its usage is excluded from totals.` })),
    ...(merged.duplicates.length ? [{ key: 'duplicates', text: `Counted once across environments sharing a transcript directory: ${merged.duplicates.join(', ')}` }] : []),
  ];
  if (merged.mismatches.length) page.environmentStatus = 'Update required';
  const active = providersWithUsage(merged);
  page.providers = active.map(provider => {
    const totals = merged.providers.find(entry => entry.provider === provider);
    const count = totals?.sessions ?? 0, share = tokens ? totals?.tokenShare ?? 0 : totals?.costShare ?? 0;
    const meta = PROVIDERS[provider];
    return { key: provider, driver: meta.driver, label: meta.label, light: meta.color[0], dark: meta.color[1], sessions: `${formatCount(count)} ${count === 1 ? 'session' : 'sessions'}`,
      value: tokens ? formatTokens(totals?.totalTokens ?? 0) : formatUsd(totals?.costUsd ?? 0),
      detail: tokens ? `${formatPercent(share)} of tokens · ${formatUsd(totals?.costUsd ?? 0)}` : `${formatPercent(share)} of cost · ${formatTokens(totals?.totalTokens ?? 0)} tokens` };
  });
  page.chartTitle = `${hourly ? 'Hourly' : 'Daily'} ${tokens ? 'processed tokens' : 'cost'}`;
  page.chart = buildChart(merged, periods, hourly ? 'hour' : 'day', page.metric, page.plotWidth);
  page.totals = [
    { key: 'processed', label: 'Processed tokens', value: formatTokens(merged.totalTokens) },
    { key: 'cached', label: 'Cached input', value: formatTokens(merged.cachedInputTokens) },
    { key: 'uncached', label: 'Uncached input', value: formatTokens(merged.uncachedInputTokens) },
    { key: 'output', label: 'Output', value: formatTokens(merged.outputTokens) },
    { key: 'savings', label: 'Cache savings', value: formatUsd(merged.cacheSavingsUsd) },
  ];
  const models = page.breakdown === 'model' && tokens ? [...merged.models].sort((a, b) => b.totalTokens - a.totalTokens || b.costUsd - a.costUsd) : merged.models;
  page.models = modelRows(models, page.metric);
  page.shares = pageShares(merged, page.metric);
  page.periodLabel = hourly ? 'Hour' : 'Day';
  page.columns = active.map(provider => ({ key: provider, label: PROVIDERS[provider].label }));
  page.periods = [...(hourly ? merged.hourly : merged.daily)].reverse().map(period => ({ key: period.key, label: hourly ? formatHourShort(period.key) : formatDayShort(period.key),
    cells: active.map(provider => ({ key: provider, text: formatUsd(period.byProvider.get(provider)?.costUsd ?? 0) })), total: formatUsd(period.costUsd), tokens: formatTokens(period.totalTokens) }));
  return page;
}

/** pages:usage-* local commands: remember the metric and period. */
export async function usageLocal(client: { local: object }, op: string, value: string): Promise<string> {
  if (op === 'metric') saveUsagePrefs(client, { metric: value });
  else if (op === 'window') saveUsagePrefs(client, { windowDays: Number(value) });
  else if (op === 'model-open' || op === 'model-close') setOpenModel(client, op === 'model-open' ? value : '');
  else throw new ClientError(`Unknown usage action: ${op}`);
  return '';
}

// ── Keys (usageShortcuts.ts) ────────────────────────────────────────────────

const USAGE_COMMANDS: Record<string, { kind: string; value: string; days: number; label: string }> = {
  'usage.cost': { kind: 'metric', value: 'cost', days: 0, label: 'Cost' }, 'usage.tokens': { kind: 'metric', value: 'tokens', days: 0, label: 'Tokens' },
  'usage.limits': { kind: 'metric', value: 'limits', days: 0, label: 'Limits' }, 'usage.period.day': { kind: 'window', value: '1', days: 1, label: 'Past 24h' },
  'usage.period.week': { kind: 'window', value: '7', days: 7, label: '7 days' }, 'usage.period.month': { kind: 'window', value: '30', days: 30, label: '30 days' },
  'usage.period.quarter': { kind: 'window', value: '90', days: 90, label: '90 days' },
};
function holds(ast: unknown, context: Record<string, boolean>, depth = 0): boolean {
  const node = obj(ast);
  if (!node.type || depth > 64) return !node.type;
  if (node.type === 'identifier') return context[str(node.name)] === true;
  if (node.type === 'not') return !holds(node.node, context, depth + 1);
  const left = holds(node.left, context, depth + 1), right = holds(node.right, context, depth + 1);
  return node.type === 'and' ? left && right : node.type === 'or' ? left || right : false;
}
/** The usage page's chords from the server's resolved keybindings, evaluated with usagePageOpen. */
export function usageKeys(config: Obj, chord: (shortcut: Obj) => string): { id: string; chord: string; kind: string; value: string; days: number; label: string }[] {
  const bindings = arr(config.keybindings);
  const context = { usagePageOpen: true, isDesktop: true, true: true };
  const winners = new Map<string, string>();
  for (const binding of [...bindings].reverse()) {
    const key = chord(obj(binding.shortcut));
    if (!key || winners.has(key) || !holds(binding.whenAst, context)) continue;
    winners.set(key, str(binding.command));
  }
  return [...winners].filter(([, command]) => USAGE_COMMANDS[command]).map(([key, command]) => ({ id: command, chord: key, ...USAGE_COMMANDS[command]! }));
}
