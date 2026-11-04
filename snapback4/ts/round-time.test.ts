import { expect, test } from 'bun:test';
import { staleRound } from './round-time.ts';

test('stale rounds require explicit time strictly beyond the budget', () => {
  for (const since of [0, 1_000, 1_800_000_000_000]) {
    expect(staleRound(since, since + 30_000, 30_000)).toBe(false);
    expect(staleRound(since, since + 30_001, 30_000)).toBe(true);
    expect(staleRound(since, since - 1, 30_000)).toBe(false);
    expect(staleRound(since, since, 0)).toBe(false);
    expect(staleRound(since, since + 1, 0)).toBe(true);
    expect(staleRound(since, since + 31_000, 60_000)).toBe(false);
    expect(staleRound(since, undefined, 30_000)).toBe(false);
    expect(staleRound(undefined, since, 30_000)).toBe(false);
    expect(staleRound(since, NaN, 30_000)).toBe(false);
    expect(staleRound(since, Infinity, 30_000)).toBe(false);
  }
});
