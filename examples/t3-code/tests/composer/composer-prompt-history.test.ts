import { describe, expect, test } from 'bun:test';
import { composerHistoryAvailability, composerHistoryComplete, composerHistoryRecall, promptHistoryStep } from '../../src/composer/composer-prompt-history';
import { T3Client } from '../../src/client/client';

const entries = [{ id: 'one', prompt: 'first' }, { id: 'two', prompt: 'two\nlines' }];
describe('sent prompt recall', () => {
  test('starts from an empty draft, reaches earlier entries and returns to the empty draft', () => {
    const newest = promptHistoryStep(entries, null, '', true);
    expect(newest?.prompt).toBe('two\nlines');
    const older = promptHistoryStep(entries, newest!.position, newest!.prompt, true);
    expect(older?.prompt).toBe('first');
    expect(promptHistoryStep(entries, older!.position, older!.prompt, true)).toBeNull();
    const forward = promptHistoryStep(entries, older!.position, older!.prompt, false);
    expect(forward?.prompt).toBe('two\nlines');
    expect(promptHistoryStep(entries, forward!.position, forward!.prompt, false)).toEqual({ position: null, prompt: '' });
  });
  test('typed or refused source cannot continue a speculative recall position', () => {
    const position = { entryId: 'two', recalled: 'two\nlines' };
    expect(promptHistoryStep(entries, position, 'typed draft', true)).toBeNull();
    expect(promptHistoryStep(entries, position, '', false)).toBeNull();
    expect(promptHistoryStep([], null, '', true)).toBeNull();
  });
  test('a replaced message id falls back to the last matching sent prompt', () => {
    expect(promptHistoryStep(entries, { entryId: 'replaced', recalled: 'two\nlines' }, 'two\nlines', true)?.prompt).toBe('first');
  });
  function client() {
    const value = new T3Client();
    value.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null,
      projection: { visibleTurnItems: entries.map(entry => ({ sourceThreadId: 'thread', sourceItemId: entry.id, item: { type: 'user_message', text: entry.prompt } })) } };
    return value;
  }
  test('a refused sibling preserves the prior accepted position and receipts commit independently', () => {
    const value = client();
    const newest = composerHistoryRecall(value, 'draft', '', true)!;
    expect(composerHistoryAvailability(value, 'draft', newest.prompt).historyNext).toBe(false);
    composerHistoryComplete(value, newest.receipt, true);
    const older = composerHistoryRecall(value, 'draft', newest.prompt, true)!;
    const empty = composerHistoryRecall(value, 'draft', newest.prompt, false)!;
    composerHistoryComplete(value, older.receipt, false);
    expect(composerHistoryAvailability(value, 'draft', newest.prompt)).toEqual({ historyPrevious: true, historyNext: true });
    composerHistoryComplete(value, empty.receipt, false);
    expect(composerHistoryAvailability(value, 'draft', newest.prompt)).toEqual({ historyPrevious: true, historyNext: true });
    const accepted = composerHistoryRecall(value, 'draft', newest.prompt, true)!;
    composerHistoryComplete(value, accepted.receipt, true);
    expect(composerHistoryAvailability(value, 'draft', accepted.prompt)).toEqual({ historyPrevious: false, historyNext: true });
  });
  test('generation and owner changes discard receipts, and pending recall metadata is bounded', () => {
    const value = client(), pending = Array.from({ length: 16 }, () => composerHistoryRecall(value, 'draft', '', true)!);
    expect(composerHistoryRecall(value, 'draft', '', true)).toBeNull();
    value.generation++;
    composerHistoryComplete(value, pending[0]!.receipt, true);
    expect(composerHistoryAvailability(value, 'draft', pending[0]!.prompt).historyNext).toBe(false);
    const current = composerHistoryRecall(value, 'draft', '', true)!;
    composerHistoryAvailability(value, 'other', '');
    composerHistoryComplete(value, current.receipt, true);
    expect(composerHistoryAvailability(value, 'other', current.prompt).historyNext).toBe(false);
    expect(composerHistoryRecall(value, 'other', '', true)).not.toBeNull();
    const returned = composerHistoryRecall(value, 'draft', '', true)!;
    expect(returned.receipt.history).not.toBe(current.receipt.history);
    composerHistoryComplete(value, current.receipt, false);
    composerHistoryComplete(value, returned.receipt, true);
    expect(composerHistoryAvailability(value, 'draft', returned.prompt).historyNext).toBe(true);
  });
});
