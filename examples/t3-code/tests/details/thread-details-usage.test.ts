// The thread details card's Usage limits and Context window items (ThreadDetailsUsage.tsx, 2359643cff): the selected
// provider's limit windows as room left, toned at 20% and 10%; the thread's context window as a bar that fills as context
// is used, toned at 80% and 90%, read as tokens against the limit, with a tick where it compacts automatically.
import { describe, expect, test } from 'bun:test';
import { contextView, usageView } from '../../src/details/thread-details-usage';

const NOW = Date.parse('2026-10-31T12:00:30Z');
const window = (id: string, label: string, usedPercent: number, extra: object = {}) => ({ id, label, usedPercent, ...extra });
const provider = (windows: object[], extra: object = {}) => ({ instanceId: 'codex', driver: 'codex', enabled: true, installed: true, usageLimits: { windows, ...extra } });
const projection = (usedTokens: number, maxTokens?: number, autoCompactThreshold?: number) => ({ thread: { id: 't1' },
  providerTurns: [{ tokenUsage: { usedTokens, ...(maxTokens ? { maxTokens } : {}), ...(autoCompactThreshold ? { autoCompactThreshold } : {}), updatedAt: '2026-10-31T12:00:00Z' } }] });

describe('ThreadDetailsUsage', () => {
  test('nothing to show without limits', () => {
    expect(usageView({ instanceId: '', providers: [], sources: [], now: NOW, timestampFormat: '24-hour' }).show).toBe(false);
  });
  test('each window as room left, toned at 20% and 10%, with when it resets', () => {
    const view = usageView({ instanceId: 'codex', providers: [provider([window('5h', '5h', 85, { resetsAt: '2026-10-31T14:30:00Z' }), window('weekly', 'Weekly', 30), window('opus', 'Opus', 95)])], sources: [],
      now: NOW, timestampFormat: '24-hour' });
    expect(view.rows.map(row => [row.label, row.fill, row.value, row.tone, row.aria])).toEqual([
      ['5h', 15, '15%', 'warning', '5h: 15% left'], ['Weekly', 70, '70%', '', 'Weekly: 70% left'], ['Opus', 5, '5%', 'error', 'Opus: 5% left']]);
    expect([view.rows[0]!.tip, view.rows[0]!.tipLines[0]]).toEqual(['5h: 15% left', expect.stringMatching(/^Resets in 2h 30m · /)]);
  });
  test('a provider whose limits cannot be read says so on one row', () => {
    const view = usageView({ instanceId: 'codex', providers: [provider([], { unavailable: { reason: 'probeFailed', message: 'Sign in again to see limits.' } })], sources: [], now: NOW, timestampFormat: '24-hour' });
    expect(view.rows.map(row => [row.label, row.value, row.fill, row.tip])).toEqual([['Limits', 'Unavailable', -1, 'Sign in again to see limits.']]);
  });
});

describe('ThreadDetailsContext', () => {
  test('nothing to show without a context window', () => {
    expect(contextView(null).show).toBe(false);
  });
  test('the bar fills as context is used and reads the tokens against the limit', () => {
    const [row] = contextView(projection(120_000, 200_000)).rows;
    expect(row).toMatchObject({ label: 'Context', fill: 60, tick: -1, value: '120k / 200k', tone: '', muted: true, aria: 'Context: 60% used', tip: 'Context: 60% used', tipLines: ['120,000 of 200,000 tokens'] });
  });
  test('warns at 80% and errors at 90%', () => {
    expect(contextView(projection(160_000, 200_000)).rows[0]!.tone).toBe('warning');
    expect(contextView(projection(158_000, 200_000)).rows[0]!.tone).toBe('');
    expect(contextView(projection(180_000, 200_000)).rows[0]!.tone).toBe('error');
  });
  test('a tick marks where it compacts automatically', () => {
    const [row] = contextView(projection(50_000, 200_000, 150_000)).rows;
    expect([row!.tick, row!.tipLines]).toEqual([75, ['50,000 of 200,000 tokens', 'Compacts automatically at about 150,000 tokens.']]);
  });
  test('without a window size, the tokens alone and no bar', () => {
    expect(contextView(projection(1_500)).rows).toEqual([expect.objectContaining({ fill: -1, value: '1.5k', tip: 'Context: 1.5k tokens used', aria: 'Context: 1.5k tokens used', tipLines: [] })]);
  });
});
