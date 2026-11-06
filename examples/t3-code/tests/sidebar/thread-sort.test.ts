// Ports of T3 Code 6d450a4284's threadSort.test.ts and the Sidebar.logic.test.ts cases for orderItemsByPreferredIds
// and resolveAdjacentThreadId (MIT, see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import { orderItemsByPreferredIds, resolveAdjacentThreadId, sortThreads } from '../../src/sidebar/thread-sort';

const at = (minute: number) => `2026-03-09T10:${String(minute).padStart(2, '0')}:00.000Z`;
const thread = (id: string, extra: Obj = {}): Obj => ({ id, projectId: 'p1', title: `Thread ${id}`, status: 'idle', latestRunId: null, activeProviderThreadId: null,
  pendingRuntimeRequest: null, createdAt: at(0), updatedAt: at(0), archivedAt: null, settledOverride: null, lineage: { relationshipToParent: null }, ...extra });

describe('sortThreads (threadSort.test.ts)', () => {
  test('sorts threads by the latest user message in recency mode', () => {
    const sorted = sortThreads([
      thread('thread-1', { updatedAt: at(10), messages: [{ role: 'user', createdAt: at(1) }] }),
      thread('thread-2', { createdAt: at(5), updatedAt: at(5), messages: [{ role: 'user', createdAt: at(6) }] }),
    ], 'updated_at');
    expect(sorted.map(entry => entry.id)).toEqual(['thread-2', 'thread-1']);
  });
  test('falls back to thread timestamps when there is no user message', () => {
    const sorted = sortThreads([
      thread('thread-1', { updatedAt: at(1), messages: [{ role: 'assistant', createdAt: at(2) }] }),
      thread('thread-2', { createdAt: at(5), updatedAt: at(5), messages: [] }),
    ], 'updated_at');
    expect(sorted.map(entry => entry.id)).toEqual(['thread-2', 'thread-1']);
  });
  test('can sort threads by createdAt when configured', () => {
    const sorted = sortThreads([thread('thread-1', { createdAt: at(5), updatedAt: at(5) }), thread('thread-2', { createdAt: at(0), updatedAt: at(10) })], 'created_at');
    expect(sorted.map(entry => entry.id)).toEqual(['thread-1', 'thread-2']);
  });
  test('uses updatedAt as a fallback for created_at sorting when createdAt is invalid', () => {
    const sorted = sortThreads([thread('thread-1', { createdAt: 'invalid-date', updatedAt: at(5) }), thread('thread-2', { createdAt: at(0), updatedAt: at(10) })], 'created_at');
    expect(sorted.map(entry => entry.id)).toEqual(['thread-1', 'thread-2']);
  });
  test('the shell stamp latestUserMessageAt wins over messages; ties order by id descending', () => {
    expect(sortThreads([thread('a', { latestUserMessageAt: at(9) }), thread('b', { latestUserMessageAt: at(3), messages: [{ role: 'user', createdAt: at(30) }] })], 'updated_at').map(entry => entry.id)).toEqual(['a', 'b']);
    expect(sortThreads([thread('a'), thread('c'), thread('b')], 'updated_at').map(entry => entry.id)).toEqual(['c', 'b', 'a']);
  });
});

describe('Sidebar.logic.test.ts cases', () => {
  test('orderItemsByPreferredIds keeps preferred ids first, skips stale ids and keeps the rest in order', () => {
    const items = [{ id: 'project-1' }, { id: 'project-2' }, { id: 'project-3' }];
    expect(orderItemsByPreferredIds({ items, preferredIds: ['project-3', 'project-missing', 'project-1'], getId: item => item.id }).map(item => item.id)).toEqual(['project-3', 'project-1', 'project-2']);
  });
  test('orderItemsByPreferredIds does not duplicate items when preferred ids repeat', () => {
    expect(orderItemsByPreferredIds({ items: [{ id: 'project-1' }, { id: 'project-2' }], preferredIds: ['project-2', 'project-1', 'project-2'], getId: item => item.id }).map(item => item.id)).toEqual(['project-2', 'project-1']);
  });
  test('orderItemsByPreferredIds resolves preference aliases', () => {
    const items = [{ id: 'physical-a', cwd: '/work/a' }, { id: 'physical-b', cwd: '/work/b' }, { id: 'physical-c', cwd: '/work/c' }];
    expect(orderItemsByPreferredIds({ items, preferredIds: ['legacy:/work/c', 'legacy:/work/a'], getId: item => item.id, getPreferenceIds: item => [item.id, `legacy:${item.cwd}`] }).map(item => item.id))
      .toEqual(['physical-c', 'physical-a', 'physical-b']);
  });
  test('resolveAdjacentThreadId resolves adjacent ids without wrapping', () => {
    const ids = ['thread-1', 'thread-2', 'thread-3'];
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: 'thread-2', direction: 'previous' })).toBe('thread-1');
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: 'thread-2', direction: 'next' })).toBe('thread-3');
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: null, direction: 'next' })).toBe('thread-1');
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: null, direction: 'previous' })).toBe('thread-3');
    expect(resolveAdjacentThreadId({ threadIds: ids, currentThreadId: 'thread-1', direction: 'previous' })).toBeNull();
  });
});
