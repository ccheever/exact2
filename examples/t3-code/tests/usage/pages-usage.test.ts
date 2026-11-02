import { describe, expect, test } from 'bun:test';
import { formatUsd, formatTokens, formatPercent, formatCount, formatDayShort, formatHourShort, formatDateTimeShort, makeWindow, enumerateDays, enumerateHourStarts, mergeUsage, niceScale,
  curvePath, buildChart, periodColumns, presentUsage, emptyUsage, usageKeys, shortcutGlyphs, plotWidth, usagePrefs, saveUsagePrefs, isModelCostUnknown, usageSourceMessages } from '../../src/usage/pages-usage';
import { keybindingDefaults } from '../../src/settings/keybinding-settings';
import { collectLimitAccounts, collectLimitPools } from '../../src/usage/usage-limits-pools';
import { formatResetsIn } from '../../src/usage/usage-limits';
import { adoptPagesPrefs, pagesPrefs } from '../../src/pages/pages-prefs';
import type { Obj } from '../../src/client/domain';
import { appPath } from '../support/app-files';
import { contractComponent } from '../support/contract-action-fixture';
import { resolveHour12 } from '../../src/shared/timestamp-format';

test('usage figures format as usageFormat.ts does', () => {
  expect([formatUsd(13644.97), formatUsd(0.2), formatUsd(1234567.891)]).toEqual(['$13,644.97', '$0.20', '$1,234,567.89']);
  expect([formatTokens(19_850_000_000), formatTokens(228_400_000), formatTokens(7_204_000), formatTokens(804_000), formatTokens(999), formatTokens(20_000_000_000)])
    .toEqual(['19.9B', '228M', '7.20M', '804K', '999', '20B']);
  expect([formatPercent(0.589), formatPercent(0.0004), formatPercent(0)]).toEqual(['58.9%', '<0.1%', '0.0%']);
  expect([formatCount(1022), formatDayShort('2026-09-05')]).toEqual(['1,022', 'Sep 5']);
});

// usageFormat.ts's CURRENCY is Intl.NumberFormat("en-US", USD, 2 digits), which rounds
// half away from zero on the shortest digits (0.825 → $0.83); Bun's Intl is ICU, as Chrome's is, so it is the oracle.
// Since main #447 (issues/closed/20261010-apple-intl-number-format-half-even.md) the Apple hosts' Intl rounds the same, so `formatUsd` is that
// formatter, as the reference's; the fixed list is ICU's answers, and the macOS app's own Hermes was checked live
// (the Usage page's Day table, USD 0.825 on Oct 9 is $0.83).
test('USD amounts round as the reference currency format does (half-expand)', async () => {
  const source = await Bun.file(appPath('pages-usage.ts')).text();
  expect(source).toContain("currency ??= new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2, maximumFractionDigits: 2 });");
  expect(source).not.toContain('function plainDigits');
  const currency = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2, maximumFractionDigits: 2 });
  expect([formatUsd(0.825), formatUsd(0.46 + 0.365), formatUsd(1.275), formatUsd(0.005), formatUsd(0.125), formatUsd(999.995), formatUsd(-0.825), formatUsd(-0.004), formatUsd(0)])
    .toEqual(['$0.83', '$0.83', '$1.28', '$0.01', '$0.13', '$1,000.00', '-$0.83', '-$0.00', '$0.00']);
  const values = [0, -0, 0.004, 0.0049999, 0.005, 0.015, 0.045, 0.825, 1.005, 1.275, 2.675, 8.345, 13644.97, 999.995, 1234567.891,
    123456789.995, 1e-7, 5e-324, 1e21, 1.5e22, 1.7976931348623157e308, -0.004, -0.825, -1234.565, NaN, Infinity, -Infinity];
  for (let step = 0; step < 4000; step += 1) values.push(step / 1000, step / 1000 + 0.0005, (step * 7919) % 100003 / 200, -(step / 400));
  for (const value of values) expect(formatUsd(value)).toBe(currency.format(value));
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

// usageFormat.ts formatHourShort / formatDateTimeShort at 74541bab17 (6cfd82fe1f): hourly labels follow the Time format setting.
test('hourly labels follow the Time format setting', () => {
  const at = (hour: number, minute = 0) => new Date(2026, 9, 4, hour, minute).toISOString();
  expect([formatHourShort(at(15)), formatHourShort(at(0)), formatHourShort(at(12))]).toEqual(['3 PM', '12 AM', '12 PM']);
  expect([formatHourShort(at(15), false), formatHourShort(at(3, 30), false), formatHourShort(at(0), false)]).toEqual(['15:00', '03:00', '00:00']);
  expect(formatDateTimeShort(at(15, 31), false)).toBe('Oct 4, 15:00');
  expect([resolveHour12('12-hour'), resolveHour12('24-hour')]).toEqual([true, false]);
  const hourly = makeWindow(1, new Date(2026, 9, 4, 15, 30).getTime());
  const page = presentUsage(emptyUsage('cost', 1, 'time', 600), mergeUsage([]), hourly, {}, undefined, false);
  expect(page.windowLabel).toBe('Oct 3, 15:00 to Oct 4, 15:00');
  expect([page.chart.start, page.chart.end]).toEqual(['15:00', '14:00']);
});

// UsagePage.tsx 805ff51e58: auto table layout, the period column w-2/5, every column at least its figures.
test('the Day/Hour table shares its room by each column\'s longest string and never shrinks a column below its figures', () => {
  const cells = (text: string) => [{ key: 'claude', text, loading: false }];
  const columns = periodColumns({ columns: [{ key: 'claude', label: 'Claude Code' }],
    periods: [{ key: 'a', label: 'Oct 1', cells: cells('$29,401.92'), total: '$29,401.92', tokens: '57.5M' }, { key: 'b', label: 'Oct 2', cells: cells('$1.00'), total: '$1.00', tokens: '9K' }] });
  expect(columns).toBe('minmax(max-content, 40%) minmax(max-content, 11fr) minmax(max-content, 10fr) minmax(max-content, 6fr)');
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

test('Muse Code sessions draw a Muse Code series in #0866ff, last in the provider order', () => {
  const summary = { contractVersion: 6, readAt: 'x', sources: [source('pi', '/p', 1), source('muse', '/m', 2)],
    buckets: [bucket({ provider: 'pi', model: 'pi-model', costUsd: 5 }), bucket({ provider: 'muse', model: 'muse-model', costUsd: 7 })] };
  const merged = mergeUsage([{ id: 'env', label: 'Mac', summary }]);
  const chart = buildChart(merged, ['2026-10-01'], 'day', 'cost', 600);
  expect(chart.series.map(series => [series.key, series.light, series.dark])).toEqual([['muse', '#0866ff', '#0866ff'], ['pi', '#4d9f8a', '#4d9f8a']]);
  const page = presentUsage(emptyUsage('cost', 30, 'model', 600), merged, { sinceDay: '2026-09-05', untilDay: '2026-10-04', timeZone: 'UTC', resolution: 'day' }, summary);
  expect(page.providers.map(row => [row.label, row.sessions, row.value])).toEqual([['Pi', '1 session', '$5.00'], ['Muse Code', '2 sessions', '$7.00']]);
  expect(page.periods[0]!.cells.map(cell => [cell.key, cell.text])).toEqual([['pi', '$5.00'], ['muse', '$7.00']]);
});

// UsagePage.tsx at 7c3b3b3605ef: the modal's trap wraps Shift+Tab from its first stop to the page's last tabbable, in
// Cost › Breakdown › Day the "Usage breakdown by time" region (tabIndex 0). The runner's querySelectorAll lists only nodes
// with their own id (runner/src/geometry.rs data_matches), in tree order; RouteFocusGuard focuses the last of them.
test("Shift+Tab from the Usage modal's first stop wraps to the page's last control, the breakdown by time in Cost › Day", async () => {
  const lines = (await Promise.all(['pages-usage.contract', 'usage-pooled.contract', 'usage-bars.contract'].map(file => Bun.file(appPath(file)).text()))).join('\n').split('\n');
  const stops = lines.filter(line => /data-focus-stop=(\([^)]*"enabled page"\)|"enabled page")/.test(line));
  expect(stops.length).toBeGreaterThan(15);
  // Every one is a node the runner can list: an id of its own.
  for (const line of stops) expect(line).toMatch(/ id=[`"\w]/);
  // The page's stops in tree order, as the runner lists them in Cost with Breakdown by Day: the header's, then the body's,
  // the breakdown region last (the Limits view's stops live in usage-pooled and usage-bars).
  const usage = (await Bun.file(appPath('pages-usage.contract')).text()).split('\n');
  const last = usage.filter(line => /data-focus-stop=(\([^)]*"enabled page"\)|"enabled page")/.test(line)).at(-1)!;
  expect(last).toContain('scroll id="usage-period-scroll"');
  expect(last).toContain('aria-label="Usage breakdown by time" tabindex=0');
  const guard = contractComponent('dialog-trap.contract', 'RouteFocusGuard'), focused: string[] = [], scrolled: string[] = [];
  const listed = ['usage-environment', 'usage-provider', 'usage-period-1', 'usage-period-7', 'usage-period-30', 'usage-period-90', 'usage-metric-cost', 'usage-metric-tokens',
    'usage-metric-limits', 'usage-refresh', 'usage-breakdown-model', 'usage-breakdown-time', 'usage-period-scroll'];
  guard.call('hold', {}, { targets: ['usage-environment', 'close-usage'], popup: 'usage-modal', preventDefault() {}, stopPropagation() {},
    querySelectorAll: (selector: string) => (selector.includes('"page"') ? listed : ['usage-environment', 'close-usage', ...listed]), focus: (id: string) => focused.push(id),
    scrollIntoView: (id: string) => scrolled.push(id) },
    ['Tab', { metaKey: false, ctrlKey: false, altKey: false, shiftKey: true }]);
  expect(focused).toEqual(['usage-period-scroll']);
  expect(scrolled).toEqual(['usage-period-scroll']); // the Mac host's focus does not scroll, so the guard does
  // A disabled control is no stop; a popover's rows and the dialogs' buttons are not the page's.
  const stop = (marker: string) => lines.find(line => line.includes(marker))?.match(/data-focus-stop=(\([^)]*\)|"[^"]*")/)?.[1];
  expect(stop('id="usage-refresh"')).toBe('(busy ? "" : "enabled page")');
  expect(stop('testId="reset-credit-confirm"') ?? '').not.toContain('page');
  expect(stop('testId=`usage-seg-reset-${seg.id}`') ?? '').not.toContain('page');
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

test('limits list providers reporting windows, with pace and reset (pooled: usage-limits-pools.ts)', () => {
  const now = Date.parse('2026-10-04T12:00:00Z');
  const providers = [
    { driver: 'codex', instanceId: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, usageLimits: { checkedAt: 'x', windows: [
      { id: 'primary', kind: 'session', label: '5 hour', usedPercent: 80, resetsAt: '2026-10-04T14:00:00Z', windowDurationMins: 300 }] } },
    { driver: 'claudeAgent', instanceId: 'claude', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' } },
  ];
  const pools = collectLimitPools(collectLimitAccounts(new Map([['env', { entry: { target: { label: 'Mac' } }, serverConfig: { providers } }]])), now);
  expect(pools).toHaveLength(1);
  expect(pools[0]!.windows[0]).toMatchObject({ label: '5 hour', remainingPercent: 20, pace: 'ahead' });
  expect(formatResetsIn(pools[0]!.windows[0]!.members[0]!.window, now)).toBe('resets in 2h 0m');
  expect(collectLimitPools(collectLimitAccounts(new Map()), now)).toEqual([]);
});

test('usage shortcuts resolve with usagePageOpen and the last binding wins', () => {
  const chord = (shortcut: Obj) => [shortcut.modKey ? 'Meta' : '', shortcut.shiftKey ? 'Shift' : '', String(shortcut.key)].filter(Boolean).join('+');
  const keybindings = [
    { command: 'usage.cost', shortcut: { key: 'c' }, whenAst: { type: 'identifier', name: 'usagePageOpen' } },
    { command: 'usage.period.day', shortcut: { key: '1', modKey: true, shiftKey: true }, whenAst: { type: 'identifier', name: 'usagePageOpen' } },
    { command: 'chat.new', shortcut: { key: 'c' }, whenAst: { type: 'identifier', name: 'terminalFocus' } },
  ];
  expect(usageKeys({ keybindings }, chord)).toEqual([
    { id: 'usage.period.day', chord: 'Meta+Shift+1', kind: 'window', value: '1', days: 1, label: 'Past 24h', title: 'Past 24h (⇧⌘1)' },
    { id: 'usage.cost', chord: 'c', kind: 'metric', value: 'cost', days: 0, label: 'Cost', title: 'Cost (C)' },
  ]);
});

// Each toggle's and select option's title is shortcutTitle(option), as the reference reads
// it from the served keybindings (the audit's reference: Cost (C) … 90 days (⇧⌘4)).
test('the metric and period options carry their label and shortcut as their title', () => {
  const chord = (shortcut: Obj) => [shortcut.modKey ? 'Meta' : '', shortcut.shiftKey ? 'Shift' : '', String(shortcut.key)].filter(Boolean).join('+');
  const usage = keybindingDefaults.filter(entry => entry.command.startsWith('usage.'));
  const keybindings = [{ command: 'commandPalette.toggle', shortcut: { key: 'k', modKey: true }, whenAst: null }, ...usage.map(entry => {
    const parts = entry.key.split('+'), key = parts.pop()!;
    return { command: entry.command, shortcut: { key, modKey: parts.includes('mod'), shiftKey: parts.includes('shift') }, whenAst: entry.when ? { type: 'identifier', name: entry.when } : null };
  })];
  expect(usageKeys({ keybindings }, chord).map(entry => entry.title).sort()).toEqual(
    ['Cost (C)', 'Tokens (T)', 'Limits (L)', 'Past 24h (⇧⌘1)', '7 days (⇧⌘2)', '30 days (⇧⌘3)', '90 days (⇧⌘4)'].sort());
  // A command bound twice is titled by its effective binding, which usageKeys lists first.
  const twice = usageKeys({ keybindings: [...keybindings, { command: 'usage.cost', shortcut: { key: 'x', modKey: true, altKey: true }, whenAst: null }] }, chord);
  expect(twice.filter(entry => entry.id === 'usage.cost').map(entry => entry.title)).toEqual(['Cost (⌥⌘X)', 'Cost (C)']);
  expect(shortcutGlyphs({ key: 'arrowup', ctrlKey: true })).toBe('⌃Up');
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

// usageFormat.test.ts "usage source messages" (T3 Code v0.1.6, MIT, see LICENSE-T3), with its original names.
describe('usage source messages', () => {
  const warning = (count: number) => `${count} malformed or inconsistent usage record(s) were skipped; usage may be incomplete.`;
  const malformedSource = (count: number, overrides: Obj = {}): Obj => ({
    fingerprint: { hostId: 'demo-host', provider: 'codex', resolvedHomePath: '/demo/codex', volumeId: 'demo-volume' },
    status: 'partial', scannedFiles: 1, skippedFiles: 0, malformedRecords: count, distinctSessions: 1, message: warning(count), ...overrides,
  });
  const fp = (source: Obj, over: Obj) => ({ fingerprint: { ...(source.fingerprint as Obj), ...over } });

  test('combines different skipped-record counts into one warning', () => {
    const codex = malformedSource(1);
    const claude = malformedSource(11, fp(codex, { provider: 'claude', resolvedHomePath: '/demo/claude' }));
    expect(usageSourceMessages([codex, claude])).toEqual([warning(12)]);
  });
  test.each([{ first: 1, second: 1, total: 2 }, { first: 1, second: 11, total: 12 }])('combines hosts reporting $first and $second skipped records once per source', ({ first, second, total }) => {
    const source = malformedSource(first);
    const otherHost = malformedSource(second, fp(source, { hostId: 'other-demo-host' }));
    expect(usageSourceMessages([source, source, otherHost, otherHost])).toEqual([warning(total)]);
    expect(usageSourceMessages([source, source])).toEqual([warning(first)]);
    expect(usageSourceMessages([otherHost, otherHost])).toEqual([warning(second)]);
  });
  test('keeps different machines with the same hostname and home path distinct', () => {
    const source = malformedSource(1);
    expect(usageSourceMessages([source, malformedSource(11, fp(source, { volumeId: 'other-demo-volume' }))])).toEqual([warning(12)]);
  });
  test('uses the largest count when connected environments report the same source', () => {
    const source = malformedSource(1);
    expect(usageSourceMessages([source, malformedSource(11), source])).toEqual([warning(11)]);
  });
  test('retains other diagnostics beside the combined warning', () => {
    const source = malformedSource(1);
    const readFailure = '2 transcript path(s) could not be read; usage may be incomplete.';
    const cursor = malformedSource(0, { ...fp(source, { provider: 'cursor' }), status: 'ok', message: 'Cursor history is limited to the last 30 days.' });
    expect(usageSourceMessages([{ ...source, message: `${readFailure} ${source.message}` }, { ...source, malformedRecords: 0, message: readFailure }, cursor]))
      .toEqual([readFailure, cursor.message, warning(1)]);
  });
  test('includes cached diagnostics when a transcript directory is missing', () => {
    const source = malformedSource(1);
    const missing = malformedSource(2, { ...fp(source, { hostId: 'other-demo-host' }), status: 'missing', scannedFiles: 0, skippedFiles: 1, distinctSessions: 0,
      message: `No transcript directory on this environment. ${warning(2)}` });
    expect(usageSourceMessages([source, missing, missing])).toEqual(['No transcript directory on this environment.', warning(3)]);
  });
  test('omits empty missing sources, healthy sources, and diagnostics handled by an action', () => {
    expect(usageSourceMessages([malformedSource(0, { status: 'missing' }), malformedSource(1, { status: 'ok' }), malformedSource(1, { action: 'enableCursorKeychain' })])).toEqual([]);
    expect(usageSourceMessages([])).toEqual([]);
  });
  test('the page shows one combined line under the totals', () => {
    const page = presentUsage(emptyUsage(), mergeUsage([]), makeWindow(7, Date.parse('2026-10-04T12:00:00Z')),
      { sources: [malformedSource(1), malformedSource(11, fp(malformedSource(1), { provider: 'claude', resolvedHomePath: '/demo/claude' }))] });
    expect(page.notices).toEqual([{ key: warning(12), text: warning(12) }]);
  });
});
