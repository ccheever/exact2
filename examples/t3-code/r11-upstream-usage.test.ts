// Lane r11-upstream: upstream 0c81120137 (usage model shares and order follow the
// selected metric), the reference's usageBreakdown.test.ts and usageMerge.test.ts cases.
import { expect, test } from 'bun:test';
import './client'; // loads the module graph in its app order (pages-usage alone meets an import cycle)
import { mergeUsage, presentUsage, emptyUsage, makeWindow, type ModelTotals } from './pages-usage';
import { modelRows, modelShare } from './pages-usage-detail';
import type { Obj } from './domain';

const totals = (input: number, cached: number, write: number, output: number) => ({ uncachedInputTokens: input, cachedInputTokens: cached, cacheCreationTokens: write, outputTokens: output, reasoningTokens: 0 });
const bucket = (over: Obj): Obj => ({ day: '2026-10-01', provider: 'claude', model: 'claude-fable-5', totals: totals(1160, 0, 0, 0), costUsd: 2, cacheSavingsUsd: 0, costSource: 'modelPriced', records: 4, unpricedRecords: 0, sessions: 1, ...over });
const source = (provider: string, home: string) => ({ fingerprint: { hostId: 'mac', provider, resolvedHomePath: home, volumeId: '1' }, status: 'ok', distinctSessions: 2, message: null });
const summary = (buckets: Obj[]) => ({ contractVersion: 6, readAt: '2026-10-04T00:00:00Z', sources: [source('claude', '/a/.claude'), source('codex', '/a/.codex')], buckets });
const model = (name: string, totalTokens: number, costUsd: number, over: Partial<ModelTotals> = {}): ModelTotals => ({
  model: name, provider: 'codex', costUsd, totalTokens, tokens: { uncachedInputTokens: totalTokens, cachedInputTokens: 0, cacheCreationTokens: 0, outputTokens: 0 },
  records: 1, unpricedRecords: 0, unpricedTokens: 0, costShare: 0, tokenShare: 0, ...over });

test('mergeUsage derives model token shares independently of their cost shares', () => {
  const merged = mergeUsage([{ id: 'env-a', label: 'Mac', summary: summary([
    bucket({ costUsd: 90 }),
    bucket({ provider: 'codex', model: 'gpt-5.6-sol', costUsd: 10, totals: totals(3 * 1160, 0, 0, 0) }),
  ]) }]);
  const byModel = Object.fromEntries(merged.models.map(entry => [entry.model, entry]));
  expect(byModel['claude-fable-5']!.costShare).toBeCloseTo(0.9, 5);
  expect(byModel['claude-fable-5']!.tokenShare).toBeCloseTo(0.25, 5);
  expect(byModel['gpt-5.6-sol']!.costShare).toBeCloseTo(0.1, 5);
  expect(byModel['gpt-5.6-sol']!.tokenShare).toBeCloseTo(0.75, 5);
});

test('modelShare follows the selected metric; an unknown cost keeps its token share', () => {
  const priced = model('priced', 100, 9, { costShare: 0.9, tokenShare: 0.25 });
  expect(modelShare(priced, 'cost')).toBe(0.9);
  expect(modelShare(priced, 'tokens')).toBe(0.25);
  const unpriced = model('unpriced', 300, 0, { unpricedRecords: 1, unpricedTokens: 300, tokenShare: 0.75 });
  expect(modelShare(unpriced, 'cost')).toBeNull();
  expect(modelShare(unpriced, 'tokens')).toBe(0.75);
});

test('the Breakdown table orders and shares rows by the selected metric', () => {
  const merged = mergeUsage([{ id: 'env-a', label: 'Mac', summary: summary([
    bucket({ costUsd: 90 }),
    bucket({ provider: 'codex', model: 'gpt-5.6-sol', costUsd: 10, totals: totals(3 * 1160, 0, 0, 0) }),
    bucket({ provider: 'claude', model: '<synthetic>', costUsd: 0, records: 1, unpricedRecords: 1, totals: totals(1160, 0, 0, 0) }),
  ]) }]);
  expect(modelRows(merged.models, 'cost').map(row => [row.model, row.share])).toEqual([
    ['claude-fable-5', '90.0%'], ['gpt-5.6-sol', '10.0%'], ['<synthetic>', '']]);
  const page = presentUsage(emptyUsage('tokens', 30, 'model', 600), merged, makeWindow(30, Date.parse('2026-10-04T12:00:00Z'), 'UTC'), {});
  expect(page.models.map(row => [row.model, row.share])).toEqual([
    ['gpt-5.6-sol', '60.0%'], ['claude-fable-5', '20.0%'], ['<synthetic>', '20.0%']]);
});
