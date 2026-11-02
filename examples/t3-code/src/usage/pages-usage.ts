// The Usage page: server.getUsageSummary merged and presented
// as the reference UsagePage does. Sources: T3 Code v0.1.6 (MIT, see LICENSE-T3)
// apps/web/src/components/usage/{UsagePage,UsageProviderChart,usageProviders,
// usageShortcuts,usagePagePreferences,UsageLimits,UsageLimitsPooled}.tsx,
// apps/web/src/state/usage.ts (withoutProviders, the last answered usage useUsage keeps on
// screen) and packages/shared/src/{usageFormat,usageMerge,usageLimits}.ts.
import { arr, num, obj, str, type Obj } from '../client/domain';
import { ClientError, type Files, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { pagesPrefs, type UsagePrefs } from '../pages/pages-prefs';
import { emptyPrices, presentPrices } from './pages-usage-prices';
import { checkMenu, uniqueProbes, type Probe } from '../composer/composer-menus';
import { emptyDetail, modelDetail, modelKey, modelRows, openModel, pageShares, setOpenModel, type ModelRowView, type ShareBarView } from './pages-usage-detail';
import { contractVersionOf, environmentStatuses, forgetSummaries, isSelected, phaseText, prepareUsage, usageState, windowKeyOf, type EnvironmentUsageStatus } from './usage-environments';
import { updatingProvidersLabel, usageEnvironmentProgress, usageLoadingState } from './usage-progress';
import { cursorRows, emptyPooled, poolBarWidth, pooledView } from './usage-pooled-view';
import { usesChatGptSharing } from '../providers/chatgpt-plan';
import { PROVIDER_ORDER, PROVIDERS, type ProviderKind } from './usage-providers';
import { resolveHour12 } from '../shared/timestamp-format';

export const USAGE_CONTRACT_VERSION = 6;
const MERGE_COMPATIBLE_SINCE = 4;
const HOUR = 3_600_000;

// ── Formatting (usageFormat.ts) ─────────────────────────────────────────────

const group = (digits: string) => digits.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
export function formatMalformedUsageWarning(count: number): string {
  return `${count} malformed or inconsistent usage record(s) were skipped; usage may be incomplete.`;
}
/**
 * usageFormat.ts usageSourceMessages: the notices under the totals, with the skipped-record warnings of every
 * source combined into one count (a source's highest count, once per transcript directory).
 */
export function usageSourceMessages(sources: readonly Obj[]): string[] {
  const messages = new Set<string>();
  const malformedBySource = new Map<string, number>();
  for (const source of sources) {
    const fingerprint = obj(source.fingerprint), malformed = num(source.malformedRecords);
    if (source.action || (source.status !== 'partial' && source.status !== 'failed' && fingerprint.provider !== 'cursor' && !(source.status === 'missing' && malformed > 0))) continue;
    let message = str(source.message);
    if (malformed > 0) {
      const key = JSON.stringify([fingerprint.hostId, fingerprint.provider, fingerprint.resolvedHomePath, fingerprint.volumeId]);
      malformedBySource.set(key, Math.max(malformedBySource.get(key) ?? 0, malformed));
      // Servers also append this warning to message; keep any read failure before it.
      const warning = formatMalformedUsageWarning(malformed);
      if (message.endsWith(warning)) message = message.slice(0, -warning.length).trim();
    }
    if (message) messages.add(message);
  }
  const malformedRecords = [...malformedBySource.values()].reduce((total, count) => total + count, 0);
  if (malformedRecords > 0) messages.add(formatMalformedUsageWarning(malformedRecords));
  return [...messages];
}

/**
 * usageFormat.ts CURRENCY (`Intl.NumberFormat` en-US USD, two fraction digits). ICU rounds the number's shortest
 * decimal digits half away from zero (`halfExpand`), so 0.825 is $0.83 where `toFixed` rounds the binary value
 * (0.8249…) down. Hermes's Apple `Intl` rounded half to even ($0.82) until main #447 gave the Apple prelude
 * ICU's rounding; the clone formatted USD itself until then.
 */
let currency: Intl.NumberFormat | undefined; // made at the first amount, not as the module loads
export function formatUsd(value: number): string {
  currency ??= new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return currency.format(value);
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
/**
 * An hourly bucket start in this Mac's zone: `3 PM`, or with a 24-hour clock `15:00`. Rolling buckets
 * can start between whole hours, but their labels always name the hour.
 */
export function formatHourShort(hourStart: string, hour12 = true): string {
  const at = new Date(hourStart);
  if (Number.isNaN(at.getTime())) return hourStart;
  const hour = at.getHours();
  return hour12 ? `${hour % 12 === 0 ? 12 : hour % 12} ${hour < 12 ? 'AM' : 'PM'}` : `${String(hour).padStart(2, '0')}:00`;
}
export function formatDateTimeShort(instant: string, hour12 = true): string {
  const at = new Date(instant);
  if (Number.isNaN(at.getTime())) return instant;
  return `${MONTHS[at.getMonth()]} ${at.getDate()}, ${formatHourShort(instant, hour12)}`;
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

export { PROVIDER_ORDER, PROVIDERS, type ProviderKind } from './usage-providers';
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
export type Chart = {
  ticks: { key: string; label: string; top: number }[]; series: { key: string; area: string; line: string; light: string; dark: string; loading: boolean }[];
  start: string; middle: string; end: string;
  /** False while placeholder gridlines stand in for a scale that has nothing to show yet: the axis has no labels. */
  labeled: boolean;
};
export const emptyChart = (): Chart => ({ ticks: [], series: [], start: '', middle: '', end: '', labeled: true });
/**
 * The layered provider areas (heaviest painted first), gridline ticks and the three axis dates. The
 * scale tops out at the largest single provider-period; with nothing to show yet while providers
 * load, unlabeled placeholder gridlines hold their usual spacing so nothing shifts on arrival.
 * `loading` mutes those providers' series.
 */
export function buildChart(merged: Merged, periods: string[], resolution: 'day' | 'hour', metric: string, width: number,
  providers: ProviderKind[] = providersWithUsage(merged), loading: ReadonlySet<string> = new Set(), hour12 = true): Chart {
  const byPeriod = new Map((resolution === 'hour' ? merged.hourly : merged.daily).map(period => [period.key, period]));
  const active = providers;
  const value = (period: string, provider: string) => { const band = byPeriod.get(period)?.byProvider.get(provider); return band ? (metric === 'tokens' ? band.totalTokens : band.costUsd) : 0; };
  const peak = periods.reduce((max, period) => PROVIDER_ORDER.reduce((inner, provider) => Math.max(inner, value(period, provider)), max), 0);
  const labeled = !(peak === 0 && loading.size > 0);
  const { max, ticks } = labeled ? niceScale(peak, 4) : { max: 4, ticks: [0, 1, 2, 3, 4] };
  const step = periods.length <= 1 ? 0 : width / (periods.length - 1);
  const toY = (amount: number) => max === 0 ? PLOT_HEIGHT : PLOT_HEIGHT - (amount / max) * (PLOT_HEIGHT - PLOT_TOP * PLOT_HEIGHT / VIEW_HEIGHT);
  const format = metric === 'tokens' ? formatTokens : formatUsd;
  const series = active.map(provider => {
    const line = curvePath(periods.map((period, index) => ({ x: index * step, y: toY(value(period, provider)) })));
    const total = periods.reduce((sum, period) => sum + value(period, provider), 0);
    const [light, dark] = PROVIDERS[provider].color;
    return { key: provider, total, line, area: line ? `${line} L${width.toFixed(2)},${PLOT_HEIGHT} L0,${PLOT_HEIGHT} Z` : '', light, dark, loading: loading.has(provider) };
  }).sort((a, b) => b.total - a.total).map(({ total: _total, ...rest }) => rest);
  const label = (period: string | undefined) => period === undefined ? '' : resolution === 'hour' ? formatHourShort(period, hour12) : formatDayShort(period);
  return { ticks: ticks.map(tick => ({ key: String(tick), label: !labeled ? '' : tick === 0 ? '0' : format(tick), top: toY(tick) })), series,
    start: label(periods[0]), middle: label(periods[Math.floor(periods.length / 2)]), end: label(periods[periods.length - 1]), labeled };
}
export function providersWithUsage(merged: Merged): ProviderKind[] {
  const active = new Set(merged.providers.filter(entry => entry.totalTokens > 0 || entry.costUsd > 0).map(entry => entry.provider));
  return PROVIDER_ORDER.filter(provider => active.has(provider));
}

// ── Limits: the pooled view (usage-limits-pools.ts, usage-pooled-view.ts) ─────
// The page's environments, refreshes and requests are usage-environments.ts's.

// ── Page state and the resource ─────────────────────────────────────────────

type Prefs = UsagePrefs;
export const METRICS = ['cost', 'tokens', 'limits'];
export const WINDOWS = [{ days: 1, label: 'Past 24h' }, { days: 7, label: '7 days' }, { days: 30, label: '30 days' }, { days: 90, label: '90 days' }];
const validPrefs = (prefs: Prefs) => METRICS.includes(prefs.metric) && WINDOWS.some(window => window.days === prefs.windowDays)
  && (prefs.hiddenProviders ?? []).every(provider => (PROVIDER_ORDER as string[]).includes(provider));
/** usagePagePreferences: Limits, 30 days and every provider on first visit; the last pick sticks after that. */
export function usagePrefs(owner: { local: object }): Prefs {
  const saved = pagesPrefs(owner).usage;
  return saved && validPrefs(saved) ? { ...saved } : { metric: 'limits', windowDays: 30 };
}
export function saveUsagePrefs(owner: { local: object }, change: Partial<Prefs>): void {
  const next = { ...usagePrefs(owner), ...change };
  if (!validPrefs(next)) throw new ClientError('Unknown usage view.');
  pagesPrefs(owner).usage = next;
}

/** UsageProviderFilter's change: "All providers" shows every one or hides every one; a provider toggles alone. */
export function toggleProvider(hidden: readonly string[], provider: string): string[] {
  if (!provider) return hidden.length === 0 ? [...PROVIDER_ORDER] : [];
  return PROVIDER_ORDER.filter(entry => (entry === provider ? !hidden.includes(entry) : hidden.includes(entry)));
}
/** UsageProviderFilter: its trigger's label and its menu's rows. */
export function providerFilter(hidden: ReadonlySet<string>) {
  const visible = PROVIDER_ORDER.filter(provider => !hidden.has(provider));
  const label = visible.length === PROVIDER_ORDER.length ? 'All providers' : visible.length === 0 ? 'No providers'
    : visible.length === 1 ? PROVIDERS[visible[0]!].label : `${visible.length} providers`;
  return { label, all: hidden.size === 0,
    rows: PROVIDER_ORDER.map(provider => ({ key: provider, label: PROVIDERS[provider].label, driver: PROVIDERS[provider].driver, checked: !hidden.has(provider) })) };
}

/** withoutProviders: hidden providers' buckets and sources leave before merging, so totals, shares and session counts describe only the visible providers. */
export function withoutProviders(summary: Obj, hidden: ReadonlySet<string>): Obj {
  if (hidden.size === 0) return summary;
  return { ...summary, buckets: arr(summary.buckets).filter(bucket => !hidden.has(str(bucket.provider))),
    sources: arr(summary.sources).filter(source => !hidden.has(str(obj(source.fingerprint).provider))) };
}

/** Which figures on screen are still coming in (usageLoadingState, less hidden providers). */
export type UsageLoading = { partial: boolean; everyProvider: boolean; providers: ReadonlySet<string> };
const NOTHING_LOADING: UsageLoading = { partial: false, everyProvider: false, providers: new Set() };

/**
 * The usage the page last drew for an answered window (useUsage's lastAnswered): it stays on screen,
 * muted, while a new window has no answer yet, for the same environments and provider filter only.
 */
type Shown = { window: UsageWindow; environments: { id: string; label: string; summary: Obj }[]; merged: Merged };
type Kept = Shown & { selection: string; hidden: string };
const kept = new WeakMap<object, Kept>();

/** Prices or mappings changed: the next read re-aggregates (the server folds and prices on read). */
export const forgetUsage = (client: object) => { forgetSummaries(usageState(client)); };

/** The plot's width: the page container (max 1024, 24pt gutters), less the summary column at lg and the 64pt axis. */
export function plotWidth(viewportWidth: number, sidebarWidth: number): number {
  const content = Math.min(1024, Math.max(0, viewportWidth - sidebarWidth)) - 48;
  const chartColumn = viewportWidth >= 1024 ? content - 288 - 24 : content;
  return Math.max(40, Math.round(chartColumn - 64));
}

const providerLabel = (provider: string) => PROVIDERS[provider as ProviderKind]?.label ?? provider;

type UsageInput = { open: boolean; metric: string; windowDays: number; breakdown: string; refresh: number; width: number; now: number; viewport?: number; sidebar?: number };
export async function usagePage(client: T3Client, native: Native | null | undefined, storage: Files | null, input: UsageInput) {
  const page = await usageView(client, native, storage, input);
  // The environment menu sizes to its widest row (Model prices included), as MenuPopup does.
  // Both menus size from measured texts (composer-menus.ts checkMenu; probes drawn by the page).
  const environments = checkMenu(client.presentation, [{ label: 'All environments', status: '' }, ...page.environments.map(row => ({ label: row.label, status: row.status }))], ['Model prices']);
  const prices = checkMenu(client.presentation, [{ label: 'All environments', status: '' }, ...page.prices.targets.map(target => ({ label: target.label, status: target.unavailable }))]);
  page.menuWidth = environments.width; page.prices.menuWidth = prices.width;
  page.menuProbes = uniqueProbes([...environments.probes, ...prices.probes]);
  return page;
}
/**
 * UsageEnvironmentFilter's menu status for an environment (UsagePage.tsx), or its connection phase when it cannot
 * report. While a manual refresh runs, every summary counts as stale.
 */
function environmentStatusText(status: EnvironmentUsageStatus, refreshing: boolean): string {
  if (!status.connected) return phaseText(status.phase);
  if (status.error !== null) return 'Unavailable';
  const version = contractVersionOf(status.summary);
  if (status.summary !== null && (version < MERGE_COMPATIBLE_SINCE || version > USAGE_CONTRACT_VERSION)) return 'Update required';
  const progress = usageEnvironmentProgress(status, refreshing);
  return progress.phase === 'loading' ? 'Scanning…' : progress.phase === 'stale' ? 'Refreshing…'
    : progress.phase === 'partway' ? updatingProvidersLabel(progress.providers, providerLabel) : 'Ready';
}
async function usageView(client: T3Client, native: Native | null | undefined, _storage: Files | null, input: UsageInput) {
  const prefs = usagePrefs(client);
  const metric = METRICS.includes(input.metric) ? input.metric : prefs.metric;
  const windowDays = WINDOWS.some(window => window.days === input.windowDays) ? input.windowDays : prefs.windowDays;
  const page = emptyUsage(metric, windowDays, input.breakdown === 'time' ? 'time' : 'model', input.width);
  const limits = metric === 'limits', noun = limits ? 'limits' : 'usage';
  const hidden = new Set(prefs.hiddenProviders ?? []);
  page.providerFilter = providerFilter(hidden);
  // UsagePage renders its price dialog whatever the metric: Model prices opens on Limits too (closed, this is one lookup).
  if (input.open) page.prices = presentPrices(client);
  const state = await prepareUsage(client, native, { open: input.open && !!native?.available, metric, windowDays, refresh: input.refresh, now: input.now },
    (days, now) => makeWindow(days, now));
  if (!input.open) kept.delete(client);
  const statuses = environmentStatuses(state);
  const selected = statuses.filter(status => status.connected && isSelected(state, status.environmentId));
  page.refreshing = state.refreshing;
  const refreshingUsage = state.refreshing && !limits;
  page.environments = statuses.map(status => ({ key: status.environmentId, label: status.label, checked: status.connected && isSelected(state, status.environmentId),
    disabled: !status.connected, status: !status.connected || !limits ? environmentStatusText(status, refreshingUsage) : '' }));
  const connected = statuses.filter(status => status.connected);
  page.environmentLabel = state.selected === null ? 'All environments' : selected.length === 1 ? selected[0]!.label : `${selected.length} environments`;
  page.allSelected = state.selected === null;
  if (connected.length === 0) { page.empty = `Connect an environment to see ${noun}.`; return page; }
  if (selected.length === 0) { page.empty = `Select an environment to see ${noun}.`; return page; }
  // The header's alert (UsageEnvironmentFilter): a failed or incompatible environment.
  page.environmentIcon = !limits && selected.some(status => status.error !== null || environmentStatusText(status, false) === 'Update required') ? 'issue' : '';
  if (!input.open || !native?.available || !state.window) return page;
  if (limits) {
    page.pooled = pooledView(state, statuses, { barWidth: poolBarWidth(input.viewport ?? 1280, input.sidebar ?? 0), format: client.local.deviceSettings.timestampFormat, md: (input.viewport ?? 1280) >= 768,
      hiddenDrivers: new Set([...hidden].map(provider => PROVIDERS[provider as ProviderKind]?.driver ?? provider)) });
    return page;
  }
  // useUsage: merge every selected environment that has answered; the rest are still scanning or failed.
  const answered = selected.filter(status => status.summary !== null);
  const stillReporting = selected.filter(status => status.summary === null && status.error === null).length;
  page.partial = answered.length > 0 && stillReporting > 0 ? 'Totals are partial while selected environments scan.' : '';
  const environments = answered.map(status => ({ id: status.environmentId, label: status.label, summary: withoutProviders(status.summary!, hidden) }));
  const current: Shown = { window: state.window, environments, merged: mergeUsage(environments) };
  // Until a new window's first answer, the last one answered for the same environments and filter stays on screen,
  // muted; with no answers, even failed ones keep it. Null while nothing has answered and something still could.
  const selection = state.selected === null ? '*' : [...state.selected].sort().join('\n'), hiddenKey = [...hidden].sort().join(',');
  if (answered.length > 0) kept.set(client, { ...current, selection, hidden: hiddenKey });
  const last = kept.get(client), keptHere = last?.selection === selection && last.hidden === hiddenKey ? last : null;
  const shown = answered.length > 0 ? current : keptHere ?? (stillReporting > 0 ? null : current);
  if (!shown) return page; // the skeleton
  // Usage kept from another window is all old, so every figure stays muted.
  const showingKept = windowKeyOf(shown.window) !== windowKeyOf(state.window);
  const loadingState = usageLoadingState(selected, refreshingUsage);
  // A hidden provider still refreshing must not add its row or mute the totals.
  const loadingProviders = new Set([...loadingState.providers].filter(provider => !hidden.has(provider)));
  const loading: UsageLoading = showingKept ? { partial: true, everyProvider: true, providers: new Set() }
    : { partial: loadingState.everyProvider || loadingProviders.size > 0, everyProvider: loadingState.everyProvider, providers: loadingProviders };
  // A selected environment shares a ChatGPT plan (UsagePage.tsx showChatGptSharing), unless Codex is filtered out.
  page.chatgptShared = !hidden.has('codex') && selected.some(status => arr(state.envs.find(env => env.id === status.environmentId)?.config?.providers).some(usesChatGptSharing));
  const merged = shown.merged;
  // The hourly labels follow the Time format setting (resolveHour12).
  const hour12 = resolveHour12(client.local.deviceSettings.timestampFormat);
  presentUsage(page, merged, shown.window, { sources: environments.flatMap(environment => arr(environment.summary.sources)) }, loading, hour12);
  page.coverage = [...selected.filter(status => status.error !== null).map(status => ({ key: `f:${status.environmentId}`, text: `${status.label} could not report usage.` })), ...page.coverage];
  // CursorEnableRow: after Codex and Claude Code in the provider list, unless Cursor is filtered out.
  const cursor = hidden.has('cursor') ? [] : cursorRows(statuses, state);
  if (cursor.length) {
    const at = Math.max(page.providers.findIndex(row => row.key === 'codex'), page.providers.findIndex(row => row.key === 'claude')) + 1;
    page.providers.splice(at, 0, ...cursor.map(row => ({ key: `enable:${row.key}`, driver: 'cursor', label: row.label, light: PROVIDERS.cursor.color[0], dark: PROVIDERS.cursor.color[1],
      sessions: '', value: '', detail: '', loading: false, awaiting: false, enable: row.key, enableLabel: `Enable Cursor usage from ${row.environment}`, busy: row.busy })));
  }
  // UsageModelDialog for the row the person opened, while it is still in the window.
  const open = merged.models.find(model => modelKey(model) === openModel(client));
  if (open) page.detail = modelDetail(open, shown.environments, windowPeriods(shown.window), shown.window.resolution, metric, input.viewport ?? 1280, hour12);
  else if (openModel(client)) setOpenModel(client, '');
  return page;
}
export function windowPeriods(window: UsageWindow): string[] {
  return window.resolution === 'hour' && window.sinceTime && window.untilTime ? enumerateHourStarts(window.sinceTime, window.untilTime) : enumerateDays(window.sinceDay, window.untilDay);
}

/** `loading`: its figures are still coming in; `awaiting`: it is still refreshing and has no usage yet, so its figures hold their place. */
type ProviderRowView = { key: string; driver: string; label: string; light: string; dark: string; sessions: string; value: string; detail: string; loading: boolean; awaiting: boolean; enable: string; enableLabel: string; busy: boolean };
export function emptyUsage(metric: string, windowDays: number, breakdown: string, width: number) {
  return {
    chatgptShared: false, metric, windowDays, breakdown, plotWidth: width, menuWidth: 224, menuProbes: [] as Probe[], empty: '', error: '', environmentLabel: 'All environments', allSelected: true,
    environments: [] as { key: string; label: string; checked: boolean; disabled: boolean; status: string }[], environmentIcon: '', partial: '', refreshing: false,
    providerFilter: providerFilter(new Set()),
    // Figures still coming in are muted: `loading` the totals, a row's or cell's own flag its provider's.
    loading: false,
    windowLabel: '', total: '', sessions: '', unpriced: '', notices: [] as { key: string; text: string }[],
    providers: [] as ProviderRowView[],
    chartTitle: '', chart: emptyChart(),
    totals: [] as { key: string; label: string; value: string }[],
    models: [] as (ModelRowView & { loading: boolean })[], shares: [] as ShareBarView[], detail: emptyDetail(), prices: emptyPrices(),
    periodLabel: 'Day', columns: [] as { key: string; label: string }[], periodColumns: '',
    periods: [] as { key: string; label: string; cells: { key: string; text: string; loading: boolean }[]; total: string; tokens: string }[],
    pooled: emptyPooled(), coverage: [] as { key: string; text: string }[],
  };
}
export type UsageView = ReturnType<typeof emptyUsage>;

/**
 * Everything UsagePage renders for a loaded summary. A provider still refreshing keeps its row, its line
 * and its column before its usage lands; `loading` names the figures to mute.
 */
export function presentUsage(page: UsageView, merged: Merged, window: UsageWindow, summary: Obj, loading: UsageLoading = NOTHING_LOADING, hour12 = true): UsageView {
  const hourly = window.resolution === 'hour';
  const periods = windowPeriods(window);
  const tokens = page.metric === 'tokens';
  page.windowLabel = hourly && window.sinceTime && window.untilTime ? `${formatDateTimeShort(window.sinceTime, hour12)} to ${formatDateTimeShort(window.untilTime, hour12)}` : `${formatDayShort(window.sinceDay)} to ${formatDayShort(window.untilDay)}`;
  page.total = tokens ? formatTokens(merged.totalTokens) : formatUsd(merged.costUsd);
  page.sessions = `${formatCount(merged.sessions)} sessions`;
  page.unpriced = !tokens && merged.unpricedShare > 0 ? `API estimate excludes ${formatPercent(merged.unpricedShare)} unpriced records.` : '';
  page.notices = usageSourceMessages(arr(summary.sources)).map(text => ({ key: text, text }));
  page.coverage = [
    ...merged.mismatches.map(mismatch => ({ key: `m:${mismatch.environment}`, text: mismatch.direction === 'serverBehind'
      ? `${mismatch.environment} runs an older server version and is excluded from totals.` : `This client is older than the server on ${mismatch.environment}; its usage is excluded from totals.` })),
    ...(merged.duplicates.length ? [{ key: 'duplicates', text: `Counted once across environments sharing a transcript directory: ${merged.duplicates.join(', ')}` }] : []),
  ];
  const withData = providersWithUsage(merged);
  const active = PROVIDER_ORDER.filter(provider => withData.includes(provider) || loading.providers.has(provider));
  const isLoading = (provider: string) => loading.everyProvider || loading.providers.has(provider);
  page.loading = loading.partial;
  page.providers = active.map(provider => {
    const totals = merged.providers.find(entry => entry.provider === provider);
    const count = totals?.sessions ?? 0, share = tokens ? totals?.tokenShare ?? 0 : totals?.costShare ?? 0;
    const meta = PROVIDERS[provider], awaiting = isLoading(provider) && !withData.includes(provider);
    return { key: provider, driver: meta.driver, label: meta.label, light: meta.color[0], dark: meta.color[1], sessions: `${formatCount(count)} ${count === 1 ? 'session' : 'sessions'}`,
      value: awaiting ? '-' : tokens ? formatTokens(totals?.totalTokens ?? 0) : formatUsd(totals?.costUsd ?? 0),
      detail: tokens ? `${formatPercent(share)} of tokens · ${formatUsd(totals?.costUsd ?? 0)}` : `${formatPercent(share)} of cost · ${formatTokens(totals?.totalTokens ?? 0)} tokens`,
      loading: isLoading(provider), awaiting, enable: '', enableLabel: '', busy: false };
  });
  page.chartTitle = `${hourly ? 'Hourly' : 'Daily'} ${tokens ? 'processed tokens' : 'cost'}`;
  page.chart = buildChart(merged, periods, hourly ? 'hour' : 'day', page.metric, page.plotWidth, active, new Set(active.filter(isLoading)), hour12);
  page.totals = [
    { key: 'processed', label: 'Processed tokens', value: formatTokens(merged.totalTokens) },
    { key: 'cached', label: 'Cached input', value: formatTokens(merged.cachedInputTokens) },
    { key: 'uncached', label: 'Uncached input', value: formatTokens(merged.uncachedInputTokens) },
    { key: 'output', label: 'Output', value: formatTokens(merged.outputTokens) },
    { key: 'savings', label: 'Cache savings', value: formatUsd(merged.cacheSavingsUsd) },
  ];
  const models = page.breakdown === 'model' && tokens ? [...merged.models].sort((a, b) => b.totalTokens - a.totalTokens || b.costUsd - a.costUsd) : merged.models;
  page.models = modelRows(models, page.metric).map((row, index) => ({ ...row, loading: isLoading(models[index]!.provider) }));
  page.shares = pageShares(merged, page.metric);
  page.periodLabel = hourly ? 'Hour' : 'Day';
  page.columns = active.map(provider => ({ key: provider, label: PROVIDERS[provider].label }));
  page.periods = [...(hourly ? merged.hourly : merged.daily)].reverse().map(period => ({ key: period.key, label: hourly ? formatHourShort(period.key, hour12) : formatDayShort(period.key),
    cells: active.map(provider => ({ key: provider, text: formatUsd(period.byProvider.get(provider)?.costUsd ?? 0), loading: isLoading(provider) })),
    total: formatUsd(period.costUsd), tokens: formatTokens(period.totalTokens) }));
  page.periodColumns = periodColumns(page);
  return page;
}

/**
 * The Day/Hour table's grid columns, as HTML's auto table layout sizes UsagePage's: none narrower than its
 * longest figure (whitespace-nowrap), the period column 40% when there is room (w-2/5), and the room left
 * shared by the other columns in proportion to their longest string.
 */
export function periodColumns(page: Pick<UsageView, 'columns' | 'periods'>): string {
  const longest = (header: string, cells: string[]) => Math.max(header.length, ...cells.map(cell => cell.length));
  const figures = [
    ...page.columns.map((column, index) => longest(column.label, page.periods.map(period => period.cells[index]?.text ?? ''))),
    longest('Total', page.periods.map(period => period.total)), longest('Tokens', page.periods.map(period => period.tokens)),
  ];
  return ['minmax(max-content, 40%)', ...figures.map(length => `minmax(max-content, ${length}fr)`)].join(' ');
}

/** pages:usage-* local commands: remember the metric, the period and the hidden providers. */
export async function usageLocal(client: { local: object }, op: string, value: string): Promise<string> {
  if (op === 'metric') saveUsagePrefs(client, { metric: value });
  else if (op === 'window') saveUsagePrefs(client, { windowDays: Number(value) });
  else if (op === 'provider') saveUsagePrefs(client, { hiddenProviders: toggleProvider(usagePrefs(client).hiddenProviders ?? [], value) });
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
/** keybindings.ts formatShortcutLabel on macOS (formatShortcutKeyLabel's key names): ⌃⌥⇧⌘, then the key. */
export function shortcutGlyphs(shortcut: Obj): string {
  const key = str(shortcut.key);
  const names: Record<string, string> = { ' ': 'Space', escape: 'Esc', arrowup: 'Up', arrowdown: 'Down', arrowleft: 'Left', arrowright: 'Right' };
  const label = names[key] ?? (key.length === 1 ? key.toUpperCase() : key.slice(0, 1).toUpperCase() + key.slice(1));
  return `${shortcut.ctrlKey ? '⌃' : ''}${shortcut.altKey ? '⌥' : ''}${shortcut.shiftKey ? '⇧' : ''}${shortcut.metaKey || shortcut.modKey ? '⌘' : ''}${label}`;
}
/**
 * The usage page's chords from the server's resolved keybindings, evaluated with usagePageOpen; each carries
 * UsagePage's shortcutTitle: the label and the chord, "Past 24h (⇧⌘1)". A command bound
 * twice comes first with its effective (latest winning) binding, as findEffectiveShortcutForCommand picks it.
 */
export function usageKeys(config: Obj, chord: (shortcut: Obj) => string): { id: string; chord: string; kind: string; value: string; days: number; label: string; title: string }[] {
  const bindings = arr(config.keybindings);
  const context = { usagePageOpen: true, isDesktop: true, true: true };
  const winners = new Map<string, Obj>();
  for (const binding of [...bindings].reverse()) {
    const key = chord(obj(binding.shortcut));
    if (!key || winners.has(key) || !holds(binding.whenAst, context)) continue;
    winners.set(key, binding);
  }
  return [...winners].filter(([, binding]) => USAGE_COMMANDS[str(binding.command)]).map(([key, binding]) => {
    const option = USAGE_COMMANDS[str(binding.command)]!;
    return { id: str(binding.command), chord: key, ...option, title: `${option.label} (${shortcutGlyphs(obj(binding.shortcut))})` };
  });
}
