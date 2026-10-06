// Ports of T3 Code 1e2ecbd975 apps/web/src/worktreeCleanup.test.ts (9 cases, original names),
// Sidebar.logic.test.ts "deleteSelectedThreadEntries" and "getFallbackThreadIdAfterDelete",
// and packages/shared/src/threadReference.test.ts (6 cases; MIT, see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { formatWorktreePathForDisplay, getOrphanedWorktreePathForThread } from './worktree-cleanup';
import { deleteSelectedThreadEntries, getFallbackThreadIdAfterDelete, type DeleteResult } from './sidebar-delete-logic';
import { resolveThreadReferenceCopyTarget } from './thread-reference';
import type { Obj } from './domain';

const thread = (overrides: Obj = {}): Obj => ({ id: 'thread-1', projectId: 'project-1', title: 'Thread', createdAt: '2026-02-13T00:00:00.000Z', updatedAt: '2026-02-13T00:00:00.000Z',
  archivedAt: null, branch: null, worktreePath: null, messages: [], ...overrides });

describe('getOrphanedWorktreePathForThread', () => {
  test('returns null when the target thread does not exist', () => {
    expect(getOrphanedWorktreePathForThread([], 'missing-thread')).toBeNull();
  });
  test('returns null when the target thread has no worktree', () => {
    expect(getOrphanedWorktreePathForThread([thread()], 'thread-1')).toBeNull();
  });
  test('returns the path when no other thread links to that worktree', () => {
    expect(getOrphanedWorktreePathForThread([thread({ worktreePath: '/tmp/repo/worktrees/feature-a' })], 'thread-1')).toBe('/tmp/repo/worktrees/feature-a');
  });
  test('returns null when another thread links to the same worktree', () => {
    const threads = [thread({ id: 'thread-1', worktreePath: '/tmp/repo/worktrees/feature-a' }), thread({ id: 'thread-2', worktreePath: '/tmp/repo/worktrees/feature-a' })];
    expect(getOrphanedWorktreePathForThread(threads, 'thread-1')).toBeNull();
  });
  test('ignores threads linked to different worktrees', () => {
    const threads = [thread({ id: 'thread-1', worktreePath: '/tmp/repo/worktrees/feature-a' }), thread({ id: 'thread-2', worktreePath: '/tmp/repo/worktrees/feature-b' })];
    expect(getOrphanedWorktreePathForThread(threads, 'thread-1')).toBe('/tmp/repo/worktrees/feature-a');
  });
});

describe('formatWorktreePathForDisplay', () => {
  test('shows only the last path segment for unix-like paths', () => {
    expect(formatWorktreePathForDisplay('/Users/julius/.t3/worktrees/t3code-mvp/t3code-4e609bb8')).toBe('t3code-4e609bb8');
  });
  test('normalizes windows separators before selecting the final segment', () => {
    expect(formatWorktreePathForDisplay('C:\\Users\\julius\\.t3\\worktrees\\t3code-mvp\\t3code-4e609bb8')).toBe('t3code-4e609bb8');
  });
  test('uses the final segment even when outside ~/.t3/worktrees', () => {
    expect(formatWorktreePathForDisplay('/tmp/custom-worktrees/my-worktree')).toBe('my-worktree');
  });
  test('ignores trailing slashes', () => {
    expect(formatWorktreePathForDisplay('/tmp/custom-worktrees/my-worktree/')).toBe('my-worktree');
  });
});

describe('deleteSelectedThreadEntries', () => {
  const entries = [{ threadKey: 'one' }, { threadKey: 'two' }, { threadKey: 'three' }] as const;
  const success: DeleteResult = { ok: true };
  const failure = { ok: false as const, error: new Error('Delete failed') };
  const interrupted: DeleteResult = { ok: false, error: new Error('interrupted'), interrupted: true };

  test('waits for each delete and excludes only earlier successes from worktree checks', async () => {
    let resolveDelete!: (result: DeleteResult) => void;
    const pendingDelete = new Promise<DeleteResult>(resolve => { resolveDelete = resolve; });
    const worktreeChecks: { threadKey: string; deletedThreadKeys: string[] }[] = [];
    const deletion = deleteSelectedThreadEntries({ entries, delete: async ({ threadKey }, deletedThreadKeys) => {
      worktreeChecks.push({ threadKey, deletedThreadKeys: [...deletedThreadKeys] });
      return threadKey === 'one' ? pendingDelete : success;
    } });
    expect(worktreeChecks).toEqual([{ threadKey: 'one', deletedThreadKeys: [] }]);
    resolveDelete(success);
    const outcome = await deletion;
    expect(worktreeChecks).toEqual([{ threadKey: 'one', deletedThreadKeys: [] }, { threadKey: 'two', deletedThreadKeys: ['one'] }, { threadKey: 'three', deletedThreadKeys: ['one', 'two'] }]);
    expect(outcome).toEqual({ deletedThreadKeys: new Set(['one', 'two', 'three']), firstFailure: null, pausedAt: -1 });
  });
  test('continues after ordinary failures and keeps the first failure', async () => {
    const laterFailure = { ok: false as const, error: new Error('Later failure') };
    const deletedKeysAtLastEntry: string[][] = [];
    const outcome = await deleteSelectedThreadEntries({ entries: [...entries, { threadKey: 'four' }], delete: async ({ threadKey }, deletedThreadKeys) => {
      if (threadKey === 'one') return failure;
      if (threadKey === 'three') return laterFailure;
      if (threadKey === 'four') deletedKeysAtLastEntry.push([...deletedThreadKeys]);
      return success;
    } });
    expect(deletedKeysAtLastEntry).toEqual([['two']]);
    expect(outcome).toEqual({ deletedThreadKeys: new Set(['two', 'four']), firstFailure: failure, pausedAt: -1 });
  });
  for (const [index, testCase] of [{ firstResult: success, deletedThreadKeys: new Set(['one']), firstFailure: null }, { firstResult: failure, deletedThreadKeys: new Set<string>(), firstFailure: failure }].entries()) {
    test(`stops on interruption and preserves earlier results ${index}`, async () => {
      const attemptedThreadKeys: string[] = [];
      const outcome = await deleteSelectedThreadEntries({ entries, delete: async ({ threadKey }) => {
        attemptedThreadKeys.push(threadKey);
        return threadKey === 'one' ? testCase.firstResult : interrupted;
      } });
      expect(attemptedThreadKeys).toEqual(['one', 'two']);
      expect(outcome).toEqual({ deletedThreadKeys: testCase.deletedThreadKeys, firstFailure: testCase.firstFailure, pausedAt: -1 });
    });
  }
  test('does not count a skipped entry as deleted', async () => {
    const visibleEntries = new Set(entries.map(({ threadKey }) => threadKey));
    const worktreeChecks: string[][] = [];
    const outcome = await deleteSelectedThreadEntries({ entries, delete: async ({ threadKey }, deletedThreadKeys) => {
      if (!visibleEntries.has(threadKey)) return null;
      worktreeChecks.push([...deletedThreadKeys]);
      visibleEntries.delete('two');
      return success;
    } });
    expect(worktreeChecks).toEqual([[], ['one']]);
    expect(outcome).toEqual({ deletedThreadKeys: new Set(['one', 'three']), firstFailure: null, pausedAt: -1 });
  });
  test('(clone) pauses at a question and resumes from the paused entry with the earlier results', async () => {
    const asked = new Set<string>();
    const run = (slice: readonly { threadKey: string }[], start?: { deletedThreadKeys: Set<string>; firstFailure: null }) => deleteSelectedThreadEntries({ entries: slice, start,
      delete: async ({ threadKey }): Promise<DeleteResult> => {
        if (threadKey === 'two' && !asked.has(threadKey)) { asked.add(threadKey); return { paused: true }; }
        return success;
      } });
    const first = await run(entries);
    expect(first).toEqual({ deletedThreadKeys: new Set(['one']), firstFailure: null, pausedAt: 1 });
    const second = await run(entries.slice(first.pausedAt), { deletedThreadKeys: first.deletedThreadKeys, firstFailure: null });
    expect(second).toEqual({ deletedThreadKeys: new Set(['one', 'two', 'three']), firstFailure: null, pausedAt: -1 });
  });
});

describe('getFallbackThreadIdAfterDelete', () => {
  test("returns the top remaining thread in the deleted thread's project sidebar order", () => {
    const fallbackThreadId = getFallbackThreadIdAfterDelete({ threads: [
      thread({ id: 'thread-oldest', projectId: 'project-1', createdAt: '2026-03-09T10:00:00.000Z' }),
      thread({ id: 'thread-active', projectId: 'project-1', createdAt: '2026-03-09T10:05:00.000Z' }),
      thread({ id: 'thread-newest', projectId: 'project-1', createdAt: '2026-03-09T10:10:00.000Z' }),
      thread({ id: 'thread-other-project', projectId: 'project-2', createdAt: '2026-03-09T10:20:00.000Z' }),
    ], deletedThreadId: 'thread-active', sortOrder: 'created_at' });
    expect(fallbackThreadId).toBe('thread-newest');
  });
  test('skips other threads being deleted in the same action', () => {
    const fallbackThreadId = getFallbackThreadIdAfterDelete({ threads: [
      thread({ id: 'thread-active', projectId: 'project-1', createdAt: '2026-03-09T10:05:00.000Z' }),
      thread({ id: 'thread-newest', projectId: 'project-1', createdAt: '2026-03-09T10:10:00.000Z' }),
      thread({ id: 'thread-next', projectId: 'project-1', createdAt: '2026-03-09T10:07:00.000Z' }),
    ], deletedThreadId: 'thread-active', deletedThreadIds: new Set(['thread-active', 'thread-newest']), sortOrder: 'created_at' });
    expect(fallbackThreadId).toBe('thread-next');
  });
});

describe('resolveThreadReferenceCopyTarget', () => {
  const crossRepositoryPullRequest: Obj = { host: 'github.com', repository: 'other/repo', number: 42, url: 'https://github.com/other/repo/pull/42', source: 'manual',
    linkedAt: '2026-01-01T00:00:00.000Z', snapshot: null, stack: null };
  test('does not copy another reference while the open panel URL is unavailable', () => {
    expect(resolveThreadReferenceCopyTarget({ threadId: 'thread-1', openPanelPullRequestUrl: null, pullRequests: [crossRepositoryPullRequest], linkedPullRequestUrl: 'https://github.com/t3/pr/12' })).toBeNull();
  });
  test('prefers the open panel pull request over the thread pull request', () => {
    expect(resolveThreadReferenceCopyTarget({ threadId: 'thread-1', openPanelPullRequestUrl: 'https://github.com/t3/pr/14', pullRequests: [crossRepositoryPullRequest], linkedPullRequestUrl: 'https://github.com/t3/pr/12' }))
      .toMatchObject({ kind: 'pull-request', value: 'https://github.com/t3/pr/14', successTitle: 'PR link copied' });
  });
  for (const linkedPullRequestUrl of [null, 'https://github.com/t3/pr/12']) {
    test(`copies a native cross-repository link before the fallback URL ${linkedPullRequestUrl}`, () => {
      expect(resolveThreadReferenceCopyTarget({ threadId: 'thread-1', pullRequests: [crossRepositoryPullRequest], linkedPullRequestUrl }))
        .toMatchObject({ kind: 'pull-request', value: crossRepositoryPullRequest.url });
    });
  }
  test('copies the highest open stack layer instead of the first link', () => {
    const stack = { kind: 'native', id: 'stack-1', number: 1, url: 'https://github.com/other/repo/stacks/1', base: 'main',
      layers: [{ number: 42, headBranch: 'first', state: 'open' }, { number: 43, headBranch: 'second', state: 'open' }] };
    const top = { ...crossRepositoryPullRequest, number: 43, url: 'https://github.com/other/repo/pull/43', stack };
    expect(resolveThreadReferenceCopyTarget({ threadId: 'thread-1', pullRequests: [{ ...crossRepositoryPullRequest, stack }, top] })).toMatchObject({ kind: 'pull-request', value: top.url });
  });
  test('uses the fallback URL when native links are dismissed', () => {
    expect(resolveThreadReferenceCopyTarget({ threadId: 'thread-1', pullRequests: [{ ...crossRepositoryPullRequest, source: 'stack-dismissed' }], linkedPullRequestUrl: 'https://github.com/t3/pr/12' }))
      .toMatchObject({ kind: 'pull-request', value: 'https://github.com/t3/pr/12' });
  });
  test('uses the thread pull request when no panel is open', () => {
    expect(resolveThreadReferenceCopyTarget({ threadId: 'thread-1', linkedPullRequestUrl: 'https://github.com/t3/pr/12' }))
      .toMatchObject({ kind: 'pull-request', value: 'https://github.com/t3/pr/12', successTitle: 'PR link copied' });
  });
  test('falls back to the thread ID', () => {
    expect(resolveThreadReferenceCopyTarget({ threadId: 'thread-1' })).toEqual({ kind: 'thread', value: 'thread-1', clipboardTarget: 'thread ID', successTitle: 'Thread ID copied', failureTitle: 'Failed to copy thread ID' });
  });
});
