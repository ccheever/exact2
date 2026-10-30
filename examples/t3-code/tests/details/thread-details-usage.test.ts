// The thread details card's Usage item (ThreadDetailsUsage.tsx at 6d450a4284): the selected provider's limit windows and
// the thread's context window, each as room left, with the error and warning tones and the tooltip's lines.
import { describe, expect, test } from 'bun:test';
import { usageView } from '../../src/details/thread-details-usage';

const NOW = Date.parse('2026-10-31T12:00:30Z');
const window = (id: string, label: string, usedPercent: number, extra: object = {}) => ({ id, label, usedPercent, ...extra });
const provider = (windows: object[], extra: object = {}) => ({ instanceId: 'codex', driver: 'codex', enabled: true, installed: true, usageLimits: { windows, ...extra } });
const projection = (usedTokens: number, maxTokens?: number) => ({ thread: { id: 't1' }, providerTurns: [{ tokenUsage: { usedTokens, ...(maxTokens ? { maxTokens } : {}), updatedAt: '2026-10-31T12:00:00Z' } }] });

describe('ThreadDetailsUsage', () => {
  test('nothing to show without limits or a context window', () => {
    expect(usageView({ instanceId: '', providers: [], sources: [], projection: null, now: NOW, timestampFormat: '24-hour' }).show).toBe(false);
  });
  test('each window as room left, toned at 20% and 10%, with when it resets', () => {
    const view = usageView({ instanceId: 'codex', providers: [provider([window('5h', '5h', 85, { resetsAt: '2026-10-31T14:30:00Z' }), window('weekly', 'Weekly', 30), window('opus', 'Opus', 95)])], sources: [],
      projection: null, now: NOW, timestampFormat: '24-hour' });
    expect(view.rows.map(row => [row.label, row.left, row.value, row.tone, row.aria])).toEqual([
      ['5h', 15, '15%', 'warning', '5h: 15% left'], ['Weekly', 70, '70%', '', 'Weekly: 70% left'], ['Opus', 5, '5%', 'error', 'Opus: 5% left']]);
    expect([view.rows[0]!.tip, view.rows[0]!.tipLines[0]]).toEqual(['5h: 15% left', expect.stringMatching(/^Resets in 2h 30m · /)]);
  });
  test('the context row: its share left with the tokens in the tooltip, or the tokens alone without a window size', () => {
    const sized = usageView({ instanceId: 'codex', providers: [provider([window('5h', '5h', 10)])], sources: [], projection: projection(50_000, 200_000), now: NOW, timestampFormat: '24-hour' });
    expect(sized.rows.at(-1)).toMatchObject({ label: 'Context', left: 75, value: '75%', muted: true, spaced: true, tip: 'Context: 75% left', tipLines: ['50k of 200k tokens used'] });
    const unsized = usageView({ instanceId: '', providers: [], sources: [], projection: projection(1_500), now: NOW, timestampFormat: '24-hour' });
    expect(unsized.rows).toEqual([expect.objectContaining({ label: 'Context', left: -1, value: '1.5k', spaced: false, tip: '1.5k tokens used', aria: 'Context: 1.5k' })]);
  });
  test('a provider whose limits cannot be read says so on one row', () => {
    const view = usageView({ instanceId: 'codex', providers: [provider([], { unavailable: { reason: 'probeFailed', message: 'Sign in again to see limits.' } })], sources: [], projection: null, now: NOW, timestampFormat: '24-hour' });
    expect(view.rows.map(row => [row.label, row.value, row.left, row.tip])).toEqual([['Limits', 'Unavailable', -1, 'Sign in again to see limits.']]);
  });
});
