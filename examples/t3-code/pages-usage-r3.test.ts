// Lane "pages", round 3: Usage mixes, the reshaped model table, the model
// dialog (e8545b293b) and Model prices with Map to (b05adb70b0).
import { expect, test } from 'bun:test';
import { mergeUsage, presentUsage, emptyUsage, makeWindow } from './pages-usage';
import { costTypeSegments, shareBar, pageShares, modelRows, modelDetail, cacheHitRate, costPerMillionTokens, dialogPlotWidth } from './pages-usage-detail';
import { tableChanges, settingsPatch, aliasCell, priceCell, usagePricesLocal, presentPrices, applyPatch, parseUsagePriceForm, usagePriceForm, type Draft, type Target } from './pages-usage-prices';
import { formatUsd } from './pages-usage';
import type { Obj } from './domain';
import type { T3Client } from './client';

const totals = (input: number, cached: number, write: number, output: number) => ({ uncachedInputTokens: input, cachedInputTokens: cached, cacheCreationTokens: write, outputTokens: output, reasoningTokens: 0 });
const bucket = (over: Obj): Obj => ({ day: '2026-10-01', provider: 'codex', model: 'gpt-6-astra', totals: totals(100, 900, 0, 50), costUsd: 2, cacheSavingsUsd: 1, costSource: 'modelPriced', records: 4, unpricedRecords: 0, sessions: 1, ...over });
const source = (provider: string, home: string) => ({ fingerprint: { hostId: 'mac', provider, resolvedHomePath: home, volumeId: '1' }, status: 'ok', distinctSessions: 2, message: null });
const summary = (buckets: Obj[]) => ({ contractVersion: 6, readAt: '2026-10-04T00:00:00Z', sources: [source('codex', '/c'), source('claude', '/a')], buckets });

test('the merge splits cost by token type and speed, clamping the unsplit and standard remainders', () => {
  const merged = mergeUsage([{ id: 'env', label: 'Mac', summary: summary([
    bucket({ costUsd: 10, categoryCostUsd: { input: 2, cacheRead: 3, cacheWrite: 1, output: 3 }, fastCostUsd: 6, speedPremiumUsd: 3 }),
    bucket({ day: '2026-10-02', costUsd: 5, ultrafastCostUsd: 1, speedPremiumUsd: 0.5 }), // an older server: nothing split
    bucket({ provider: 'claude', model: 'mystery', costUsd: 0, records: 2, unpricedRecords: 2, totals: totals(40, 0, 0, 10) }),
  ]) }]);
  expect(merged.categoryCost).toEqual({ input: 2, cacheRead: 3, cacheWrite: 1, output: 3, unsplit: 6 });
  expect(merged.speedCost).toEqual({ fast: 6, ultrafast: 1, premium: 3.5, standard: 8 });
  const astra = merged.models.find(model => model.model === 'gpt-6-astra')!;
  expect(astra.tokens).toEqual({ uncachedInputTokens: 200, cachedInputTokens: 1800, cacheCreationTokens: 0, outputTokens: 100 });
  expect(astra.unpricedTokens).toBe(0);
  expect(merged.models.find(model => model.model === 'mystery')!.unpricedTokens).toBe(50);
  // A mixed cell counts unpriced tokens by record share.
  const mixed = mergeUsage([{ id: 'env', label: 'Mac', summary: summary([bucket({ records: 4, unpricedRecords: 1, totals: totals(100, 0, 0, 100) })]) }]);
  expect(mixed.models[0]!.unpricedTokens).toBe(50);
  expect(mergeUsage([{ id: 'env', label: 'Mac', summary: summary([bucket({ costUsd: 1, categoryCostUsd: { input: 0.6, cacheRead: 0.2, cacheWrite: 0.1, output: 0.10000000001 } })]) }]).categoryCost.unsplit).toBe(0);
});

test('share bars leave out empty segments, round below a cent away and caption each segment', () => {
  expect(costTypeSegments({ input: 1, cacheRead: 0, cacheWrite: 0, output: 1, unsplit: 0.004 }).find(segment => segment.label === 'Other')!.value).toBe(0);
  const bar = shareBar('Cost by type', costTypeSegments({ input: 2926.36, cacheRead: 15367.19, cacheWrite: 1263.49, output: 2253.61, unsplit: 0 }), formatUsd)!;
  expect(bar.ariaLabel).toBe('Cost by type: Input $2,926.36, Cache read $15,367.19, Cache write $1,263.49, Output $2,253.61');
  expect(bar.segments.map(segment => [segment.label, segment.light, segment.dark, segment.first, segment.last])).toEqual([
    ['Input', '#747477', '#8c8c8c', true, false], ['Cache read', '#b6b6b7', '#464646', false, false], ['Cache write', '#5c5c5f', '#aaaaaa', false, false], ['Output', '#27272a', '#f5f5f5', false, true]]);
  expect(bar.segments[1]!.tip).toBe('Cache read · $15,367.19 · 70.5%');
  expect(shareBar('Empty', costTypeSegments({ input: 0, cacheRead: 0, cacheWrite: 0, output: 0, unsplit: 0 }), formatUsd)).toBeNull();
  const merged = mergeUsage([{ id: 'env', label: 'Mac', summary: summary([bucket({ costUsd: 10, categoryCostUsd: { input: 2, cacheRead: 3, cacheWrite: 1, output: 4 }, fastCostUsd: 4, speedPremiumUsd: 2 })]) }]);
  expect(pageShares(merged, 'cost').map(share => [share.label, share.aside])).toEqual([['Cost by type', ''], ['Cost by speed', '$2.00']]);
  expect(pageShares(merged, 'tokens').map(share => share.label)).toEqual(['Tokens by type']);
  const standardOnly = mergeUsage([{ id: 'env', label: 'Mac', summary: summary([bucket({})]) }]);
  expect(pageShares(standardOnly, 'cost').map(share => share.label)).toEqual(['Cost by type']);
});

test('the model table ranks rows, scales the bar to the peak and leaves an unpriced share blank', () => {
  const merged = mergeUsage([{ id: 'env', label: 'Mac', summary: summary([
    bucket({ costUsd: 8 }), bucket({ model: 'gpt-5.6-sol', costUsd: 2 }),
    bucket({ provider: 'claude', model: '<synthetic>', costUsd: 0, records: 1, unpricedRecords: 1, totals: totals(0, 0, 0, 0) }),
  ]) }]);
  const rows = modelRows(merged.models, 'cost');
  expect(rows.map(row => [row.rank, row.row, row.model, row.cost, row.share, row.bar])).toEqual([
    ['1', 2, 'gpt-6-astra', '$8.00', '80.0%', 100], ['2', 3, 'gpt-5.6-sol', '$2.00', '20.0%', 25], ['3', 4, '<synthetic>', 'Unpriced', '', 0]]);
  expect(rows[2]!.light).toBe('#d97757');
  const page = presentUsage(emptyUsage('tokens', 30, 'model', 600), merged, makeWindow(30, Date.parse('2026-10-04T12:00:00Z'), 'UTC'), {});
  expect(page.models.map(row => row.bar)).toEqual([100, 100, 0]);
});

test('the model dialog shows cost, tokens, the per-million rate and cache hits, and trends tokens when unpriced', () => {
  const environment = { id: 'env', label: 'Mac', summary: summary([
    bucket({ costUsd: 10, categoryCostUsd: { input: 2, cacheRead: 3, cacheWrite: 1, output: 4 }, fastCostUsd: 4, speedPremiumUsd: 2 }),
    bucket({ model: 'gpt-5.6-sol', costUsd: 10 }),
    bucket({ provider: 'claude', model: 'mystery', costUsd: 0, records: 2, unpricedRecords: 2, totals: totals(40, 60, 0, 10) }),
  ]) };
  const merged = mergeUsage([environment]);
  const astra = merged.models.find(model => model.model === 'gpt-6-astra')!;
  expect([cacheHitRate(astra), costPerMillionTokens(astra)]).toEqual([0.9, 10 / 1050 * 1e6]);
  const periods = ['2026-10-01', '2026-10-02'];
  const detail = modelDetail(astra, environment, periods, 'day', 'cost', 1280);
  expect(detail.description).toBe('Codex · 50.0% of cost');
  expect(detail.stats.map(stat => [stat.label, stat.value])).toEqual([['Cost', '$10.00'], ['Tokens', '1.05K'], ['Per 1M tokens', '$9,523.81'], ['Cache hit', '90.0%']]);
  expect(detail.shares.map(share => share.label)).toEqual(['Cost by type', 'Tokens by type', 'Cost by speed']);
  expect(detail.chart.series.map(series => series.key)).toEqual(['codex']);
  expect([detail.plotWidth, dialogPlotWidth(840), detail.unpricedNote]).toEqual([654, 654, '']);
  const mystery = modelDetail(merged.models.find(model => model.model === 'mystery')!, environment, periods, 'day', 'cost', 1280);
  expect(mystery.description).toBe('Claude Code');
  expect(mystery.stats.map(stat => stat.label)).toEqual(['Cost', 'Tokens', 'Cache hit']);
  expect(mystery.stats[0]!.value).toBe('Unpriced');
  expect(mystery.chartTitle).toBe('Daily processed tokens');
  expect(mystery.shares.map(share => share.label)).toEqual(['Tokens by type']);
  expect(mystery.unpricedNote).toBe('110 tokens have no known price');
});

const target = (over: Partial<Target> = {}): Target => ({ environmentId: 'env', label: 'Mac', prices: {}, aliases: {}, unavailable: null, ...over });
const draft = (over: Partial<Draft>): Draft => ({ id: 'model:x', model: 'x', isNew: false, values: {}, epoch: 0, seed: { model: 'x', alias: '', values: { inputCostPerMillionTokens: '', outputCostPerMillionTokens: '', cacheReadCostPerMillionTokens: '', cacheWriteCostPerMillionTokens: '' } }, ...over });
const price = { inputCostPerMillionTokens: 1, outputCostPerMillionTokens: 4 };

test('price rows: mapping drops the own price, a self mapping is refused, clearing a mapping returns to automatic', () => {
  expect(parseUsagePriceForm({ ...usagePriceForm('m'), inputCostPerMillionTokens: '1.5', outputCostPerMillionTokens: '3', cacheReadCostPerMillionTokens: '0' }))
    .toEqual({ model: 'm', price: { inputCostPerMillionTokens: 1.5, outputCostPerMillionTokens: 3, cacheReadCostPerMillionTokens: 0 } });
  expect(parseUsagePriceForm({ ...usagePriceForm('m'), inputCostPerMillionTokens: '-1', outputCostPerMillionTokens: '3' })).toBeNull();
  const priced = target({ prices: { x: price } });
  expect(tableChanges(priced, [draft({ alias: 'gpt-6' })]).changes).toEqual([{ model: 'x', alias: 'gpt-6' }, { model: 'x', price: null }]);
  expect(tableChanges(priced, [draft({ alias: 'x' })]).errors.get('model:x')).toBe('Map to a different model.');
  expect(tableChanges(target(), [draft({ id: 'new:1', model: '', isNew: true, alias: 'gpt-6' })]).errors.get('new:1')).toBe('Enter a model ID.');
  const mapped = target({ aliases: { x: 'gpt-6' } });
  expect(tableChanges(mapped, [draft({ alias: '' })]).changes).toEqual([{ model: 'x', alias: null }]);
  expect(tableChanges(mapped, [draft({ removed: true })]).changes).toEqual([{ model: 'x', alias: null }]);
  expect(tableChanges(target({ prices: { x: price }, aliases: { x: 'y' } }), [draft({ removed: true })]).changes).toEqual([{ model: 'x', price: null }, { model: 'x', alias: null }]);
  expect(tableChanges(target(), [draft({ id: 'new:2', model: 'z', isNew: true, values: { inputCostPerMillionTokens: '2' } })]).errors.get('new:2')).toBe('Output is required on Mac.');
  expect(settingsPatch(target({ aliases: null }), [{ model: 'x', alias: 'y' }])).toEqual({ error: 'Update server to map models' });
  expect(settingsPatch(target(), [{ model: 'x', alias: 'y' }, { model: 'x', price: null }])).toEqual({ patch: { usagePriceOverrides: { x: null }, usageModelAliases: { x: 'y' } } });
  expect(aliasCell([target({ aliases: null })], 'x')).toEqual({ value: '', placeholder: 'Unavailable' });
  expect(aliasCell([target()], 'x')).toEqual({ value: '', placeholder: 'None' });
  expect(priceCell([target({ prices: { x: price } })], 'x', 'cacheReadCostPerMillionTokens')).toEqual({ value: '', placeholder: 'Input rate' });
  expect(applyPatch({ usagePriceOverrides: { a: price, b: price } }, { usagePriceOverrides: { a: null }, usageModelAliases: { c: 'd' } })).toEqual({ usagePriceOverrides: { b: price }, usageModelAliases: { c: 'd' } });
});

function client(settings: Obj): T3Client {
  return { environmentId: 'env', connection: 'connected', ready: true, scopes: ['orchestration:read', 'orchestration:operate'],
    config: { environment: { label: 'Mac', capabilities: { usagePriceOverrides: true, usageModelAliases: true } }, settings } } as unknown as T3Client;
}

test('the dialog keeps typed text: seeds hold while an input is on screen, and Set price opens a prefilled row', () => {
  const c = client({ usagePriceOverrides: { 'gpt-6-astra': price }, usageModelAliases: {} });
  usagePricesLocal(c, 'open', '', 'mystery');
  let view = presentPrices(c);
  expect(view.open).toBe(true);
  expect(view.rows.map(row => [row.id, row.modelValue, row.isNew])).toEqual([['model:gpt-6-astra', 'gpt-6-astra', false], ['new:initial', 'mystery', true]]);
  expect(view.rows[0]!.cells.map(cell => [cell.value, cell.placeholder])).toEqual([['1', '0.00'], ['4', '0.00'], ['', 'Input rate'], ['', 'Input rate']]);
  expect([view.saveDisabled, view.hasChanges, view.changesLabel]).toEqual([true, true, 'Changes apply to Mac']);
  expect(view.rows[1]!.error).toBe('Input is required on Mac.');
  usagePricesLocal(c, 'edit', 'new:initial:inputCostPerMillionTokens', '2');
  usagePricesLocal(c, 'edit', 'new:initial:outputCostPerMillionTokens', '8');
  view = presentPrices(c);
  expect([view.rows[1]!.error, view.saveDisabled]).toEqual(['', false]);
  // Typing changed nothing an input shows, so nothing on screen is rewritten under the caret.
  expect(view.rows[1]!.cells.map(cell => cell.value)).toEqual(['', '', '', '']);
  // Mapping hides the rates; clearing it remounts them showing the typed draft.
  usagePricesLocal(c, 'edit', 'new:initial:alias', 'gpt-6-astra');
  view = presentPrices(c);
  expect(view.rows[1]!.countedAs).toBe('gpt-6-astra');
  usagePricesLocal(c, 'edit', 'new:initial:alias', '');
  expect(presentPrices(c).rows[1]!.cells.map(cell => cell.value)).toEqual(['2', '8', '', '']);
  // Editing an existing cell back to its saved value drops the draft.
  usagePricesLocal(c, 'edit', 'model:gpt-6-astra:inputCostPerMillionTokens', '3');
  expect(presentPrices(c).rows[0]!.key).toBe('model:gpt-6-astra:1:0');
  usagePricesLocal(c, 'edit', 'model:gpt-6-astra:inputCostPerMillionTokens', '1');
  usagePricesLocal(c, 'reset', 'model:gpt-6-astra', '');
  view = presentPrices(c);
  expect([view.rows[0]!.removed, view.rows[0]!.resetLabel, view.rows[0]!.resetTip]).toEqual([true, 'Undo reset for gpt-6-astra', 'Undo reset']);
  usagePricesLocal(c, 'reset', 'model:gpt-6-astra', '');
  expect(presentPrices(c).rows[0]!.key).toBe('model:gpt-6-astra:1:0');
  // Undoing a reset over edits remounts the row showing them.
  usagePricesLocal(c, 'edit', 'model:gpt-6-astra:outputCostPerMillionTokens', '5');
  usagePricesLocal(c, 'reset', 'model:gpt-6-astra', '');
  usagePricesLocal(c, 'reset', 'model:gpt-6-astra', '');
  expect([presentPrices(c).rows[0]!.key, presentPrices(c).rows[0]!.cells[1]!.value]).toEqual(['model:gpt-6-astra:1:1', '5']);
  usagePricesLocal(c, 'discard', '', '');
  view = presentPrices(c);
  expect([view.rows.length, view.hasChanges, view.generation]).toEqual([1, false, 2]);
  usagePricesLocal(c, 'add', '', '');
  expect(presentPrices(c).rows[1]!.aliasPlaceholder).toBe('Optional');
  usagePricesLocal(c, 'close', '', '');
  expect(presentPrices(c).open).toBe(false);
  // A model that already has a row is edited there, not duplicated.
  usagePricesLocal(c, 'open', '', 'gpt-6-astra');
  expect(presentPrices(c).rows.map(row => row.id)).toEqual(['model:gpt-6-astra']);
  usagePricesLocal(c, 'close', '', '');
  const empty = client({ usagePriceOverrides: {}, usageModelAliases: {} });
  usagePricesLocal(empty, 'open', '', '');
  expect(presentPrices(empty).emptyRows).toBe('No custom prices or mappings. Add a row to set one.');
  usagePricesLocal(empty, 'env', '', '');
  expect([presentPrices(empty).selectionLabel, presentPrices(empty).selectionNotice]).toEqual(['0 environments', 'Select an environment to see and change its model prices.']);
});

test('menus that size to their content measure their rows as the reference lays them out', async () => {
  const { textWidth, checkMenuWidth } = await import('./pages-text-width');
  expect(textWidth('Daehyeon’s MacBook Pro', 14)).toBeCloseTo(164.2, 0);
  // The reference popup is 269.3pt wide for this environment and its "Ready".
  expect(checkMenuWidth([{ label: 'All environments', status: '' }, { label: 'Daehyeon’s MacBook Pro', status: 'Ready' }])).toBe(270);
  expect(checkMenuWidth([{ label: 'A', status: '' }])).toBe(160);
});

test('Set price closes the model dialog and opens Model prices with a row for that model', async () => {
  const { pagesLocal } = await import('./pages-commands');
  const { openModel, setOpenModel } = await import('./pages-usage-detail');
  const c = client({ usagePriceOverrides: {}, usageModelAliases: {} });
  setOpenModel(c, 'claude:mystery');
  await pagesLocal(c, { available: true } as never, {} as never, 'usage-prices-open', '', 'mystery');
  expect(openModel(c)).toBe('');
  expect(presentPrices(c).rows.map(row => [row.id, row.modelValue, row.isNew])).toEqual([['new:initial', 'mystery', true]]);
});
