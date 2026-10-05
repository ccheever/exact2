// The Usage page's cost/token mixes, the reshaped Breakdown model table and the
// model detail dialog (lane "pages", upstream e8545b293b). Sources: T3 Code
// (MIT, see LICENSE-T3) apps/web/src/components/usage/{UsageShareBar,
// UsageModelDialog,usageBreakdown}.ts(x) and UsagePage.tsx.
import { arr, type Obj } from './domain';
import {
  buildChart, formatPercent, formatTokens, formatUsd, isModelCostUnknown, mergeUsage, PROVIDERS,
  type CategoryCost, type Chart, type Merged, type ModelTotals, type ProviderKind, type SpeedCost, type TokenTotals,
} from './pages-usage';

/** color-mix(in oklab, var(--foreground) P%, var(--background)), read from the served reference (light, dark). */
const INK: Record<number, [string, string]> = {
  100: ['#27272a', '#f5f5f5'], 72: ['#5c5c5f', '#aaaaaa'], 66: ['#68686b', '#9b9b9b'], 60: ['#747477', '#8c8c8c'],
  44: ['#979799', '#656565'], 34: ['#adadae', '#4e4e4e'], 30: ['#b6b6b7', '#464646'],
};
type Segment = { label: string; value: number; ink: number };
export type ShareSegmentView = { key: string; label: string; value: string; grow: number; light: string; dark: string; tip: string; first: boolean; last: boolean };
export type ShareBarView = { key: string; label: string; aside: string; ariaLabel: string; segments: ShareSegmentView[] };

/** Adjacent segments stay above the 15 ΔE separation floor in both themes. */
export function costTypeSegments(cost: CategoryCost): Segment[] {
  return [
    { label: 'Input', value: cost.input, ink: 60 }, { label: 'Cache read', value: cost.cacheRead, ink: 30 },
    { label: 'Cache write', value: cost.cacheWrite, ink: 72 }, { label: 'Output', value: cost.output, ink: 100 },
    // Reported cost with no rates to split it, or from older servers. Below a cent it is rounding, not usage.
    { label: 'Other', value: cost.unsplit >= 0.005 ? cost.unsplit : 0, ink: 44 },
  ];
}
export function tokenTypeSegments(tokens: TokenTotals): Segment[] {
  return [
    { label: 'Input', value: tokens.uncachedInputTokens, ink: 60 }, { label: 'Cache read', value: tokens.cachedInputTokens, ink: 30 },
    { label: 'Cache write', value: tokens.cacheCreationTokens, ink: 72 }, { label: 'Output', value: tokens.outputTokens, ink: 100 },
  ];
}
/** Speeds are ordered by price, so they brighten from standard to ultrafast. */
export function speedCostSegments(cost: SpeedCost): Segment[] {
  return [{ label: 'Standard', value: cost.standard, ink: 34 }, { label: 'Fast', value: cost.fast, ink: 66 }, { label: 'Ultrafast', value: cost.ultrafast, ink: 100 }];
}

/** UsageShareBar: empty segments are left out, and nothing renders without a total. */
export function shareBar(label: string, segments: Segment[], format: (value: number) => string, aside = ''): ShareBarView | null {
  const visible = segments.filter(segment => segment.value > 0);
  const total = visible.reduce((sum, segment) => sum + segment.value, 0);
  if (total <= 0) return null;
  return {
    key: label, label, aside,
    ariaLabel: `${label}: ${visible.map(segment => `${segment.label} ${format(segment.value)}`).join(', ')}`,
    segments: visible.map((segment, index) => ({
      key: segment.label, label: segment.label, value: format(segment.value), grow: Math.round((segment.value / total) * 1e6) / 1e4,
      light: INK[segment.ink]![0], dark: INK[segment.ink]![1], tip: `${segment.label} · ${format(segment.value)} · ${formatPercent(segment.value / total)}`,
      first: index === 0, last: index === visible.length - 1,
    })),
  };
}
const present = (bars: (ShareBarView | null)[]) => bars.filter((bar): bar is ShareBarView => bar !== null);

/** The section between Totals and Breakdown: tokens by type, or cost by type and (with faster speeds) by speed. */
export function pageShares(merged: Merged, metric: string): ShareBarView[] {
  if (merged.totalTokens <= 0) return [];
  if (metric === 'tokens') return present([shareBar('Tokens by type', tokenTypeSegments(merged), formatTokens)]);
  return present([
    shareBar('Cost by type', costTypeSegments(merged.categoryCost), formatUsd),
    merged.speedCost.fast + merged.speedCost.ultrafast > 0 ? shareBar('Cost by speed', speedCostSegments(merged.speedCost), formatUsd, formatUsd(merged.speedCost.premium)) : null,
  ]);
}

/** Share of a model's input read from cache; cache writes count as misses. */
export function cacheHitRate({ tokens }: ModelTotals): number | null {
  const input = tokens.uncachedInputTokens + tokens.cachedInputTokens + tokens.cacheCreationTokens;
  return input === 0 ? null : tokens.cachedInputTokens / input;
}
/** Effective USD per million priced tokens, or null when none were priced. */
export function costPerMillionTokens(model: ModelTotals): number | null {
  const priced = model.totalTokens - model.unpricedTokens;
  return priced <= 0 || isModelCostUnknown(model) ? null : (model.costUsd / priced) * 1_000_000;
}

/** usageBreakdown modelShare (upstream 0c81120137): the share column follows the selected metric; an unknown cost has no cost share. */
export function modelShare(model: ModelTotals, metric: 'cost' | 'tokens'): number | null {
  if (metric === 'tokens') return model.tokenShare;
  return isModelCostUnknown(model) ? null : model.costShare;
}
const shareText = (model: ModelTotals, metric: string) => { const share = modelShare(model, metric === 'tokens' ? 'tokens' : 'cost'); return share === null ? '' : formatPercent(share); };
export const modelKey = (model: { provider: string; model: string }) => `${model.provider}:${model.model}`;
export type ModelRowView = { key: string; rank: string; row: number; driver: string; model: string; cost: string; share: string; tokens: string; unpriced: boolean; bar: number; light: string; dark: string };
/** Breakdown → Model: rank, name over a provider-coloured bar scaled to the peak, cost, share and tokens. */
export function modelRows(models: ModelTotals[], metric: string): ModelRowView[] {
  const valueOf = (model: ModelTotals) => metric === 'tokens' ? model.totalTokens : model.costUsd;
  const peak = models.reduce((max, model) => Math.max(max, valueOf(model)), 0);
  return models.map((model, index) => {
    const meta = PROVIDERS[model.provider as ProviderKind], unknown = isModelCostUnknown(model), value = valueOf(model);
    return {
      key: modelKey(model), rank: String(index + 1), row: index + 2, driver: meta?.driver ?? model.provider, model: model.model,
      cost: unknown ? 'Unpriced' : formatUsd(model.costUsd), share: shareText(model, metric), tokens: formatTokens(model.totalTokens), unpriced: unknown,
      // A short minimum (0.5rem, drawn as the bar's min-width) keeps tiny shares a dash, not a dot.
      bar: value > 0 && peak > 0 ? Math.round((value / peak) * 10000) / 100 : 0,
      light: meta?.color[0] ?? '#8b8b8b', dark: meta?.color[1] ?? '#8b8b8b',
    };
  });
}

export type ModelDetailView = {
  open: boolean; key: string; model: string; driver: string; description: string; stats: { key: string; label: string; value: string }[];
  chartTitle: string; chart: Chart; plotWidth: number; shares: ShareBarView[]; unpricedNote: string;
};
export function emptyDetail(): ModelDetailView {
  return { open: false, key: '', model: '', driver: '', description: '', stats: [], chartTitle: '', chart: { ticks: [], series: [], start: '', middle: '', end: '' }, plotWidth: 0, shares: [], unpricedNote: '' };
}
/** DialogPopup max-w-3xl inside the 16pt viewport inset, less its border, DialogPanel p-6 and the 64pt axis. */
export function dialogPlotWidth(viewportWidth: number): number {
  return Math.max(40, Math.round(Math.min(768, viewportWidth - 32) - 2 - 48 - 64));
}

/**
 * UsageModelDialog: one model's usage in the window, its trend and mixes from
 * the same merge narrowed to this model's buckets. Unpriced cost is unknown,
 * not zero, so its trend shows tokens.
 */
export function modelDetail(model: ModelTotals, environment: { id: string; label: string; summary: Obj }, periods: string[], resolution: 'day' | 'hour', metric: string, viewportWidth: number): ModelDetailView {
  const summary = { ...environment.summary, buckets: arr(environment.summary.buckets).filter(bucket => bucket.provider === model.provider && bucket.model === model.model) };
  const usage = mergeUsage([{ ...environment, summary }]);
  const meta = PROVIDERS[model.provider as ProviderKind];
  const unknown = isModelCostUnknown(model), hit = cacheHitRate(model), perMillion = costPerMillionTokens(model);
  const chartMetric = unknown ? 'tokens' : metric === 'tokens' ? 'tokens' : 'cost';
  const width = dialogPlotWidth(viewportWidth);
  const stats = [
    { key: 'cost', label: 'Cost', value: unknown ? 'Unpriced' : formatUsd(model.costUsd) },
    { key: 'tokens', label: 'Tokens', value: formatTokens(model.totalTokens) },
    ...(perMillion === null ? [] : [{ key: 'per-million', label: 'Per 1M tokens', value: formatUsd(perMillion) }]),
    ...(hit === null ? [] : [{ key: 'cache-hit', label: 'Cache hit', value: formatPercent(hit) }]),
  ];
  return {
    open: true, key: modelKey(model), model: model.model, driver: meta?.driver ?? model.provider,
    description: `${meta?.label ?? model.provider}${unknown ? '' : ` · ${formatPercent(model.costShare)} of cost`}`,
    stats, chartTitle: `${resolution === 'hour' ? 'Hourly' : 'Daily'} ${chartMetric === 'tokens' ? 'processed tokens' : 'cost'}`,
    chart: buildChart(usage, periods, resolution, chartMetric, width), plotWidth: width,
    shares: present([
      unknown ? null : shareBar('Cost by type', costTypeSegments(usage.categoryCost), formatUsd),
      shareBar('Tokens by type', tokenTypeSegments(model.tokens), formatTokens),
      usage.speedCost.fast + usage.speedCost.ultrafast > 0 ? shareBar('Cost by speed', speedCostSegments(usage.speedCost), formatUsd, formatUsd(usage.speedCost.premium)) : null,
    ]),
    unpricedNote: model.unpricedTokens > 0 ? `${formatTokens(model.unpricedTokens)} tokens have no known price` : '',
  };
}

/** The open dialog per client (pages:usage-model-open / -close); Set price hands over to Model prices. */
const openModels = new WeakMap<object, string>();
export const openModel = (client: object) => openModels.get(client) ?? '';
export function setOpenModel(client: object, key: string): void {
  if (key) openModels.set(client, key); else openModels.delete(client);
}
