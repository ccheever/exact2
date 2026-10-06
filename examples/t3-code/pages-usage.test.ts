import { expect, test } from 'bun:test';
import { formatUsd, formatTokens, formatPercent, formatCount, formatDayShort, makeWindow, enumerateDays, enumerateHourStarts, mergeUsage, niceScale,
  curvePath, buildChart, presentUsage, emptyUsage, limitPools, usageKeys, plotWidth, usagePrefs, saveUsagePrefs, isModelCostUnknown } from './pages-usage';
import { adoptPagesPrefs, pagesPrefs } from './pages-prefs';
import type { Obj } from './domain';

test('usage figures format as usageFormat.ts does', () => {
  expect([formatUsd(13644.97), formatUsd(0.2), formatUsd(1234567.891)]).toEqual(['$13,644.97', '$0.20', '$1,234,567.89']);
  expect([formatTokens(19_850_000_000), formatTokens(228_400_000), formatTokens(7_204_000), formatTokens(804_000), formatTokens(999), formatTokens(20_000_000_000)])
    .toEqual(['19.9B', '228M', '7.20M', '804K', '999', '20B']);
  expect([formatPercent(0.589), formatPercent(0.0004), formatPercent(0)]).toEqual(['58.9%', '<0.1%', '0.0%']);
  expect([formatCount(1022), formatDayShort('2026-09-05')]).toEqual(['1,022', 'Sep 5']);
});

test('windows are calendar days ending today, or an exact rolling 24 hours', () => {
  const now = new Date(2026, 9, 4, 15, 30, 12).getTime();
  const month = makeWindow(30, now, 'Asia/Seoul');
  expect(month).toMatchObject({ untilDay: '2026-10-04', sinceDay: '2026-09-05', resolution: 'day' });
  expect(enumerateDays(month.sinceDay, month.untilDay)).toHaveLength(30);
  const day = makeWindow(1, now, 'Asia/Seoul');
  expect(day.resolution).toBe('hour');
  expect(Date.parse(day.untilTime!) - Date.parse(day.sinceTime!)).toBe(24 * 3_600_000);
  expect(enumerateHourStarts(day.sinceTime!, day.untilTime!)).toHaveLength(24);
});

const bucket = (over: Obj): Obj => ({ day: '2026-10-01', provider: 'codex', model: 'gpt-6-astra', totals: { uncachedInputTokens: 100, cachedInputTokens: 900, cacheCreationTokens: 0, outputTokens: 50, reasoningTokens: 10 },
  costUsd: 2, cacheSavingsUsd: 1, costSource: 'modelPriced', records: 4, unpricedRecords: 0, sessions: 1, ...over });
const source = (provider: string, home: string, sessions: number, status = 'ok'): Obj => ({ fingerprint: { hostId: 'mac', provider, resolvedHomePath: home, volumeId: '1:2' },
  status, scannedFiles: 1, skippedFiles: 0, malformedRecords: 0, distinctSessions: sessions, message: null });

test('merging sums owned buckets, counts sessions per source and keeps unpriced models unknown', () => {
  const summary = { contractVersion: 6, readAt: '2026-10-04T00:00:00Z', sources: [source('codex', '/c', 3), source('claude', '/a', 2)], buckets: [
    bucket({}), bucket({ day: '2026-10-02', costUsd: 3 }),
    bucket({ provider: 'claude', model: 'claude-opus-5-5', costUsd: 5, totals: { uncachedInputTokens: 10, cachedInputTokens: 0, cacheCreationTokens: 5, outputTokens: 5, reasoningTokens: 0 } }),
    bucket({ provider: 'claude', model: '<synthetic>', costUsd: 0, records: 2, unpricedRecords: 2, costSource: 'unpriced', totals: { uncachedInputTokens: 0, cachedInputTokens: 0, cacheCreationTokens: 0, outputTokens: 0, reasoningTokens: 0 } }),
  ] };
  const merged = mergeUsage([{ id: 'env', label: 'Mac', summary }]);
  expect([merged.costUsd, merged.sessions, merged.totalTokens, merged.cachedInputTokens]).toEqual([10, 5, 2120, 1800]);
  expect(merged.providers.map(entry => [entry.provider, entry.costUsd, entry.sessions])).toEqual([['codex', 5, 3], ['claude', 5, 2]]);
  expect(merged.unpricedShare).toBeCloseTo(2 / 14);
  const synthetic = merged.models.find(model => model.model === '<synthetic>')!;
  expect(isModelCostUnknown(synthetic)).toBe(true);
  expect(merged.daily.map(day => day.key)).toEqual(['2026-10-01', '2026-10-02']);
});

test('a transcript directory two environments share counts once; incompatible servers are reported', () => {
  const shared = { contractVersion: 6, readAt: '2026-10-04T00:00:00Z', sources: [source('codex', '/c', 3)], buckets: [bucket({})] };
  const merged = mergeUsage([{ id: 'a', label: 'A', summary: shared }, { id: 'b', label: 'B', summary: { ...shared, readAt: '2026-10-03T00:00:00Z' } },
    { id: 'c', label: 'Old', summary: { ...shared, contractVersion: 3 } }]);
  expect([merged.costUsd, merged.sessions]).toEqual([2, 3]);
  expect(merged.duplicates).toEqual(['B: /c']);
  expect(merged.mismatches).toEqual([{ environment: 'Old', direction: 'serverBehind' }]);
});

test('the chart rounds its scale up and draws monotone curves in plot pixels', () => {
  expect(niceScale(1386, 4)).toEqual({ max: 1500, ticks: [0, 500, 1000, 1500] });
  expect(niceScale(0, 4)).toEqual({ max: 0, ticks: [0] });
  const path = curvePath([{ x: 0, y: 224 }, { x: 300, y: 10 }, { x: 600, y: 224 }]);
  expect(path.startsWith('M0.00,224.00 C')).toBe(true);
  expect(path.split(' C')).toHaveLength(3);
  const merged = mergeUsage([{ id: 'env', label: 'Mac', summary: { contractVersion: 6, readAt: 'x', sources: [source('codex', '/c', 1)], buckets: [bucket({ costUsd: 1400 })] } }]);
  const chart = buildChart(merged, ['2026-09-30', '2026-10-01', '2026-10-02'], 'day', 'cost', 600);
  expect(chart.ticks.map(tick => tick.label)).toEqual(['0', '$500.00', '$1,000.00', '$1,500.00']);
  expect(chart.ticks[0]!.top).toBe(224);
  expect(chart.series.map(series => series.key)).toEqual(['codex']);
  expect(chart.series[0]!.area.endsWith('L600.00,224 L0,224 Z')).toBe(true);
  expect([chart.start, chart.middle, chart.end]).toEqual(['Sep 30', 'Oct 1', 'Oct 2']);
});

test('the plot spans the chart column at both reference widths', () => {
  expect(plotWidth(1280, 256)).toBe(600);
  expect(plotWidth(840, 256)).toBe(472);
});

test('the page presents the summary, provider rows, totals and both breakdowns', () => {
  const summary = { contractVersion: 6, readAt: 'x', sources: [source('codex', '/c', 913), source('claude', '/a', 109)],
    buckets: [bucket({ costUsd: 9604.72 }), bucket({ provider: 'claude', model: 'claude-opus-5-5', costUsd: 4040.25 })] };
  const merged = mergeUsage([{ id: 'env', label: 'Mac', summary }]);
  const window = { sinceDay: '2026-09-05', untilDay: '2026-10-04', timeZone: 'UTC', resolution: 'day' as const };
  const page = presentUsage(emptyUsage('cost', 30, 'model', 600), merged, window, summary);
  expect([page.total, page.sessions, page.chartTitle, page.windowLabel]).toEqual(['$13,644.97', '1,022 sessions', 'Daily cost', 'Sep 5 to Oct 4']);
  expect(page.providers.map(row => [row.label, row.sessions, row.value, row.detail])).toEqual([
    ['Codex', '913 sessions', '$9,604.72', '70.4% of cost · 1.05K tokens'], ['Claude Code', '109 sessions', '$4,040.25', '29.6% of cost · 1.05K tokens']]);
  expect(page.totals.map(metric => metric.label)).toEqual(['Processed tokens', 'Cached input', 'Uncached input', 'Output', 'Cache savings']);
  expect(page.models[0]).toMatchObject({ model: 'gpt-6-astra', cost: '$9,604.72', share: '70.4%', driver: 'codex' });
  expect(page.periods[0]).toMatchObject({ label: 'Oct 1', total: '$13,644.97' });
  const tokens = presentUsage(emptyUsage('tokens', 30, 'model', 600), merged, window, summary);
  expect([tokens.total, tokens.chartTitle, tokens.providers[0]!.detail]).toEqual(['2.10K', 'Daily processed tokens', '50.0% of tokens · $9,604.72']);
});

test('limits list providers reporting windows, with pace and reset', () => {
  const now = Date.parse('2026-10-04T12:00:00Z');
  const providers = [
    { driver: 'codex', instanceId: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, usageLimits: { checkedAt: 'x', windows: [
      { id: 'primary', kind: 'session', label: '5 hour', usedPercent: 80, resetsAt: '2026-10-04T14:00:00Z', windowDurationMins: 300 }] } },
    { driver: 'claudeAgent', instanceId: 'claude', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' } },
  ];
  const pools = limitPools(providers, now);
  expect(pools).toHaveLength(1);
  expect(pools[0]!.windows[0]).toMatchObject({ label: '5 hour', remaining: 20, pace: 'ahead', resets: 'resets in 2h 0m' });
  expect(limitPools([], now)).toEqual([]);
});

test('usage shortcuts resolve with usagePageOpen and the last binding wins', () => {
  const chord = (shortcut: Obj) => [shortcut.modKey ? 'Meta' : '', shortcut.shiftKey ? 'Shift' : '', String(shortcut.key)].filter(Boolean).join('+');
  const keybindings = [
    { command: 'usage.cost', shortcut: { key: 'c' }, whenAst: { type: 'identifier', name: 'usagePageOpen' } },
    { command: 'usage.period.day', shortcut: { key: '1', modKey: true, shiftKey: true }, whenAst: { type: 'identifier', name: 'usagePageOpen' } },
    { command: 'chat.new', shortcut: { key: 'c' }, whenAst: { type: 'identifier', name: 'terminalFocus' } },
  ];
  expect(usageKeys({ keybindings }, chord)).toEqual([
    { id: 'usage.period.day', chord: 'Meta+Shift+1', kind: 'window', value: '1', days: 1, label: 'Past 24h' },
    { id: 'usage.cost', chord: 'c', kind: 'metric', value: 'cost', days: 0, label: 'Cost' },
  ]);
});

test('the metric and period are remembered in the preference record; Limits and 30 days on a first visit', () => {
  const owner = { local: {} as Record<string, unknown> };
  expect(usagePrefs(owner)).toEqual({ metric: 'limits', windowDays: 30 });
  saveUsagePrefs(owner, { metric: 'cost' });
  saveUsagePrefs(owner, { windowDays: 7 });
  // The client persists `local` whole; a reload adopts the `pages` record back.
  const reloaded = { local: {} as Record<string, unknown> };
  adoptPagesPrefs(reloaded.local, JSON.parse(JSON.stringify({ version: 1, ...owner.local })));
  expect(usagePrefs(reloaded)).toEqual({ metric: 'cost', windowDays: 7 });
  expect(() => saveUsagePrefs(owner, { windowDays: 5 })).toThrow('Unknown usage view.');
  adoptPagesPrefs(reloaded.local, { pages: { usage: { metric: 'bogus', windowDays: 7 }, onboardingCompletedAt: 'yesterday' } });
  expect(usagePrefs(reloaded)).toEqual({ metric: 'limits', windowDays: 30 });
  expect(pagesPrefs(reloaded).onboardingCompletedAt).toBe('');
});
