// 20261005-pr-code-tab: the Viewed marks' store, with usePullRequestFilesViewed.test.tsx's cases
// (T3 Code 1e2ecbd975, MIT, see LICENSE-T3) driven through the store the panel's resource and the
// flush share (the hook's timer is the root task here), plus the 500-press cap.
import { describe, expect, test } from 'bun:test';
import { FilesViewedStore, MAX_FILES_VIEWED_PRESSES } from './pages-pr-viewed';

const answer = (state: 'unviewed' | 'viewed' | 'dismissed') => ({ files: [{ path: 'a.ts', state }], truncated: false });
function store() { const marks = new FilesViewedStore(); marks.adopt(answer('unviewed')); return marks; }

describe('a mark whose file was pushed to before the read that followed it', () => {
  test('gives way to the host and shows the file as changed', () => {
    const marks = store();
    marks.setViewed('a.ts', true);
    const taken = marks.takeBatch()!;
    expect(taken.batch).toEqual([{ path: 'a.ts', viewed: true }]);
    marks.landed(taken, 'ok');
    expect(marks.due).toBe(true); // refresh()
    marks.adopt(answer('dismissed'));
    expect([marks.isViewed('a.ts'), marks.isStale('a.ts'), marks.count(['a.ts'])]).toEqual([false, true, 0]);
  });
  test('stays given way to on every later read, having nothing left to recover', () => {
    const marks = store();
    marks.setViewed('a.ts', true);
    marks.landed(marks.takeBatch()!, 'ok');
    marks.adopt(answer('dismissed'));
    marks.adopt(answer('dismissed'));
    expect([marks.isViewed('a.ts'), marks.isStale('a.ts')]).toEqual([false, true]);
  });
});

describe('a mark the host has not answered for yet', () => {
  test('holds the press while the write is still out', () => {
    const marks = store();
    marks.setViewed('a.ts', true);
    const taken = marks.takeBatch()!;
    marks.adopt(answer('unviewed')); // an answer already on its way when the box was ticked
    expect(marks.isViewed('a.ts')).toBe(true);
    marks.landed(taken, 'ok');
    expect(marks.isViewed('a.ts')).toBe(true);
  });
  test('holds a press made since the read that would otherwise answer for it', () => {
    const marks = store();
    marks.setViewed('a.ts', true);
    marks.landed(marks.takeBatch()!, 'ok');
    marks.setViewed('a.ts', true); // pressed again before the post-write read came back
    marks.adopt(answer('dismissed'));
    expect([marks.isViewed('a.ts'), marks.isStale('a.ts')]).toEqual([true, false]);
  });
});

describe('the flush', () => {
  test('a burst is one write; each press re-arms the timer key', () => {
    const marks = store(), keys = new Set<string>();
    for (const path of ['a.ts', 'b.ts', 'c.ts']) { marks.setViewed(path, true); keys.add(marks.key()); }
    expect([keys.size, marks.queuedCount()]).toEqual([3, 3]);
    expect(marks.takeBatch()!.batch.map(file => file.path)).toEqual(['a.ts', 'b.ts', 'c.ts']);
    expect([marks.queuedCount(), marks.takeBatch()]).toEqual([0, null]);
  });
  test('a write carries at most 500 presses; the rest wait for the next flush', () => {
    const marks = store();
    for (let index = 0; index < MAX_FILES_VIEWED_PRESSES + 20; index++) marks.setViewed(`f${index}`, true);
    const before = marks.key();
    expect(marks.takeBatch()!.batch).toHaveLength(MAX_FILES_VIEWED_PRESSES);
    expect([marks.queuedCount(), marks.key() !== before]).toEqual([20, true]);
  });
  test('a failed write takes its presses back and says so; a later press of the same path stays', () => {
    const marks = store();
    marks.setViewed('a.ts', true); marks.setViewed('b.ts', true);
    const taken = marks.takeBatch()!;
    marks.setViewed('b.ts', true); // pressed again: the next flush owns it
    expect(marks.landed(taken, 'failed')).toEqual({ reverted: true });
    expect([marks.isViewed('a.ts'), marks.isViewed('b.ts')]).toEqual([false, true]);
  });
  test('a write whose reply never came is answered by the next read', () => {
    const marks = store();
    marks.setViewed('a.ts', true);
    expect(marks.landed(marks.takeBatch()!, 'unknown')).toEqual({ reverted: false });
    expect(marks.due).toBe(true);
    marks.adopt(answer('viewed'));
    expect([marks.isViewed('a.ts'), marks.overlay.size]).toEqual([true, 0]);
  });
  test('a failed read keeps the last answer and says why', () => {
    const marks = store();
    marks.adopt({ files: [{ path: 'a.ts', state: 'viewed' }], truncated: true });
    marks.failed('HTTP 502');
    expect([marks.isViewed('a.ts'), marks.error, marks.truncated]).toEqual([true, 'HTTP 502', true]);
  });
});
