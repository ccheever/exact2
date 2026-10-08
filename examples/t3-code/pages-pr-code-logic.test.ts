// 20261005-pr-code-tab: the Code tab's pure logic, ported with the reference's own test names and
// cases (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: components/pullRequest/{pullRequestDiff.logic,
// pullRequestFilesViewed.logic,pullRequestFileOrder.logic,pullRequestDetail.logic}.test.ts), plus
// the clone's own: the whitespace filter, the slice adoption and the review positions.
import { describe, expect, test } from 'bun:test';
import {
  adoptDiffSlice, countViewedFiles, editPullRequestThreadComment, hideWhitespaceChanges, isFileDiffCollapsed, isFileViewed, isLineInFileDiff, isStaleViewedState,
  mergePullRequestThreadComments, orderDiffFiles, renderablePatch, resolveDiffReviewPosition, reviewPositionAnchor, revertFileViewedOverlay, settleFileViewedOverlay,
  toFileViewedBatch, toFileViewedStates, toggleFileDiffFoldForViewed, type DiffSlice, type FileViewedOverlay, type PrDiffFile,
} from './pages-pr-code-logic';

const fileWithHunks = (hunks: { deletionStart: number; deletionCount: number; additionStart: number; additionCount: number }[]) => ({ hunks });

describe('isLineInFileDiff', () => {
  const file = fileWithHunks([{ deletionStart: 10, deletionCount: 3, additionStart: 10, additionCount: 5 }, { deletionStart: 40, deletionCount: 0, additionStart: 42, additionCount: 2 }]);
  test('places a line inside a hunk, on the side that hunk counts', () => {
    expect(isLineInFileDiff(file, 'right', 12)).toBe(true);
    expect(isLineInFileDiff(file, 'left', 11)).toBe(true);
  });
  test('includes the first line of a hunk and excludes the one past its last', () => {
    expect(isLineInFileDiff(file, 'right', 10)).toBe(true);
    expect(isLineInFileDiff(file, 'right', 14)).toBe(true);
    expect(isLineInFileDiff(file, 'right', 15)).toBe(false);
    expect(isLineInFileDiff(file, 'left', 9)).toBe(false);
    expect(isLineInFileDiff(file, 'left', 12)).toBe(true);
    expect(isLineInFileDiff(file, 'left', 13)).toBe(false);
  });
  test('keeps the two sides apart, since one line number means two lines', () => {
    expect(isLineInFileDiff(file, 'right', 43)).toBe(true);
    expect(isLineInFileDiff(file, 'left', 40)).toBe(false);
  });
  test('places nothing in a file whose hunks the host withheld', () => {
    expect(isLineInFileDiff(fileWithHunks([]), 'right', 1)).toBe(false);
  });
  test('reads the ranges the clone parses from a patch', () => {
    const parsed = renderablePatch('diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -10,3 +10,5 @@\n a\n-b\n+c\n+d\n+e\n f\n@@ -40,0 +42,2 @@\n+x\n+y\n', false);
    expect(parsed?.kind).toBe('files');
    const [one] = parsed?.kind === 'files' ? parsed.files : [];
    expect(one!.hunks.map(hunk => [hunk.deletionStart, hunk.deletionCount, hunk.additionStart, hunk.additionCount])).toEqual([[10, 3, 10, 5], [40, 0, 42, 2]]);
  });
});

describe('isFileDiffCollapsed', () => {
  const NO_TOGGLES: ReadonlySet<string> = new Set();
  test('opens every file before the reader has touched anything', () => {
    expect(isFileDiffCollapsed('a.ts', null, NO_TOGGLES)).toBe(false);
    expect(isFileDiffCollapsed('b.ts', null, NO_TOGGLES)).toBe(false);
  });
  test('opens every file once the toolbar has asked for it', () => {
    expect(isFileDiffCollapsed('a.ts', 'expanded', NO_TOGGLES)).toBe(false);
    expect(isFileDiffCollapsed('b.ts', 'expanded', NO_TOGGLES)).toBe(false);
  });
  test('folds every file again on the second press', () => {
    expect(isFileDiffCollapsed('a.ts', 'folded', NO_TOGGLES)).toBe(true);
    expect(isFileDiffCollapsed('b.ts', 'folded', NO_TOGGLES)).toBe(true);
  });
  test('keeps a file the reader folded closed as the next slice arrives', () => {
    const toggled = new Set(['b.ts']);
    expect(isFileDiffCollapsed('b.ts', null, toggled)).toBe(true);
    expect(isFileDiffCollapsed('c.ts', null, toggled)).toBe(false);
  });
  test('still answers to a toggle after either toolbar press', () => {
    expect(isFileDiffCollapsed('a.ts', 'expanded', new Set(['a.ts']))).toBe(true);
    expect(isFileDiffCollapsed('a.ts', 'folded', new Set(['a.ts']))).toBe(false);
  });
});

describe('toggleFileDiffFoldForViewed', () => {
  test('puts a file away when it is ticked off', () => { expect([...toggleFileDiffFoldForViewed('a.ts', true, null, new Set())]).toEqual(['a.ts']); });
  test('brings a file back when the tick is taken off', () => { expect([...toggleFileDiffFoldForViewed('a.ts', false, null, new Set(['a.ts']))]).toEqual([]); });
  test('leaves the fold alone when it already says what the tick does', () => { const folded = new Set(['a.ts']); expect(toggleFileDiffFoldForViewed('a.ts', true, null, folded)).toBe(folded); });
  test('moves against whatever the toolbar last asked for', () => {
    expect([...toggleFileDiffFoldForViewed('a.ts', true, 'expanded', new Set())]).toEqual(['a.ts']);
    expect(toggleFileDiffFoldForViewed('a.ts', false, 'expanded', new Set()).size).toBe(0);
  });
  test('touches only the file that was ticked', () => { expect([...toggleFileDiffFoldForViewed('a.ts', false, null, new Set(['a.ts', 'b.ts']))]).toEqual(['b.ts']); });
});

const NO_OVERLAY: FileViewedOverlay = new Map();
const NOTHING_PENDING: ReadonlySet<string> = new Set();
const NOTHING_ANSWERED: ReadonlySet<string> = new Set();
const states = toFileViewedStates({ files: [{ path: 'a.ts', state: 'viewed' }, { path: 'b.ts', state: 'unviewed' }, { path: 'c.ts', state: 'dismissed' }] });

describe('isFileViewed', () => {
  test('follows the host for a file the reader has not pressed', () => {
    expect(isFileViewed('a.ts', states, NO_OVERLAY)).toBe(true);
    expect(isFileViewed('b.ts', states, NO_OVERLAY)).toBe(false);
  });
  test('reads a file pushed to since it was cleared as unread', () => {
    expect(isFileViewed('c.ts', states, NO_OVERLAY)).toBe(false);
    expect(isStaleViewedState(states?.get('c.ts'))).toBe(true);
    expect(isStaleViewedState(states?.get('a.ts'))).toBe(false);
  });
  test("shows the press ahead of the host's answer", () => {
    expect(isFileViewed('b.ts', states, new Map([['b.ts', true]]))).toBe(true);
    expect(isFileViewed('a.ts', states, new Map([['a.ts', false]]))).toBe(false);
  });
  test('answers a file the host has said nothing about, before its answer arrives', () => {
    expect(isFileViewed('z.ts', null, NO_OVERLAY)).toBe(false);
    expect(isFileViewed('z.ts', null, new Map([['z.ts', true]]))).toBe(true);
  });
});

describe('countViewedFiles', () => {
  test('counts only the files on screen, presses included', () => {
    expect(countViewedFiles(['a.ts', 'b.ts', 'c.ts'], states, NO_OVERLAY)).toBe(1);
    expect(countViewedFiles(['a.ts', 'b.ts', 'c.ts'], states, new Map([['b.ts', true]]))).toBe(2);
    expect(countViewedFiles(['b.ts'], states, NO_OVERLAY)).toBe(0);
  });
});

describe('settleFileViewedOverlay', () => {
  test('drops a press the host has caught up on', () => { expect(settleFileViewedOverlay(new Map([['a.ts', true]]), states, NOTHING_PENDING, NOTHING_ANSWERED).size).toBe(0); });
  test('keeps a press the host still disagrees with', () => { const overlay = new Map([['b.ts', true]]); expect(settleFileViewedOverlay(overlay, states, NOTHING_PENDING, NOTHING_ANSWERED)).toBe(overlay); });
  test('keeps a press the host cannot have heard yet', () => {
    expect(settleFileViewedOverlay(new Map([['a.ts', false]]), states, new Set(['a.ts']), NOTHING_ANSWERED).get('a.ts')).toBe(false);
  });
  test('settles a file pushed to since it was cleared against un-ticking it', () => {
    expect(settleFileViewedOverlay(new Map([['c.ts', false]]), states, NOTHING_PENDING, NOTHING_ANSWERED).size).toBe(0);
  });
  test('drops a tick once a read has answered for it, against what the reader pressed', () => {
    const settled = settleFileViewedOverlay(new Map([['c.ts', true]]), states, NOTHING_PENDING, new Set(['c.ts']));
    expect(settled.size).toBe(0);
    expect(isFileViewed('c.ts', states, settled)).toBe(false);
    expect(isStaleViewedState(states?.get('c.ts'))).toBe(true);
  });
  test('holds a press made since the read that would otherwise answer for it', () => {
    expect(settleFileViewedOverlay(new Map([['c.ts', true]]), states, new Set(['c.ts']), new Set(['c.ts'])).get('c.ts')).toBe(true);
  });
  test('holds everything until the host has answered at all', () => { const overlay = new Map([['a.ts', true]]); expect(settleFileViewedOverlay(overlay, null, NOTHING_PENDING, NOTHING_ANSWERED)).toBe(overlay); });
});

describe('toFileViewedBatch', () => {
  test('carries both directions in one batch', () => {
    expect(toFileViewedBatch(new Map([['a.ts', false], ['b.ts', true]]))).toEqual([{ path: 'a.ts', viewed: false }, { path: 'b.ts', viewed: true }]);
  });
});

describe('revertFileViewedOverlay', () => {
  const batch = [{ path: 'a.ts', viewed: true }, { path: 'b.ts', viewed: false }];
  const both = new Set(['a.ts', 'b.ts']);
  test("puts the checkbox back to the host's answer for everything the request answers for", () => {
    expect(revertFileViewedOverlay(new Map([['a.ts', true], ['b.ts', false]]), batch, both).size).toBe(0);
  });
  test('leaves a press the reader made after the request went out', () => {
    expect([...revertFileViewedOverlay(new Map([['a.ts', false], ['b.ts', false]]), batch, new Set(['b.ts']))]).toEqual([['a.ts', false]]);
  });
  test('leaves a path a later request took over, even pressed the same way', () => { const overlay = new Map([['a.ts', true]]); expect(revertFileViewedOverlay(overlay, batch, new Set(['b.ts']))).toBe(overlay); });
  test('leaves a path the request never carried', () => { const overlay = new Map([['c.ts', true]]); expect(revertFileViewedOverlay(overlay, batch, both)).toBe(overlay); });
});

const file = (name: string, additionLines: string[] = []) => ({ name, additionLines, deletionLines: [] as string[] });
const order = (files: ReturnType<typeof file>[]) => orderDiffFiles(files).map(entry => entry.name);
describe('orderDiffFiles', () => {
  test('places source before tests and generated files across path conventions', () => {
    const source = ['src/app.ts', 'src/dist.ts', 'src/testing.ts'];
    const tests = ['src/__tests__/app.ts', 'src/app.spec.tsx', 'src/app.test.ts', 'test/app.ts', 'tests/helpers/app.ts'];
    const generated = ['apps/web/package-lock.json', 'dist/app.js', 'packages/core/vendor/lib.js', 'pnpm-lock.yaml', 'public/app.min.js', 'src/__snapshots__/app.ts', 'src/api.generated.ts', 'src/app.test.ts.snap'];
    expect(order([...generated, ...tests, ...source].reverse().map(path => file(path)))).toEqual([...source, ...tests, ...generated]);
  });
  test('answers an empty diff with an empty order', () => { expect(order([])).toEqual([]); });
  test('puts a dependency before what imports it', () => { expect(order([file('src/a.ts', ['import { b } from "./b";']), file('src/b.ts')])).toEqual(['src/b.ts', 'src/a.ts']); });
  test('follows a chain the whole way down', () => {
    expect(order([file('src/a.ts', ['import { b } from "./b";']), file('src/b.ts', ['import { c } from "./c";']), file('src/c.ts')])).toEqual(['src/c.ts', 'src/b.ts', 'src/a.ts']);
  });
  test("resolves a specifier that climbs out of the importer's directory", () => { expect(order([file('src/ui/a.ts', ['import { b } from "../lib/b";']), file('src/lib/b.ts')])).toEqual(['src/lib/b.ts', 'src/ui/a.ts']); });
  test("resolves a directory specifier to that directory's index file", () => { expect(order([file('src/a.ts', ['import { b } from "./lib";']), file('src/lib/index.ts')])).toEqual(['src/lib/index.ts', 'src/a.ts']); });
  test("falls back to the specifier's last segment when the path does not line up", () => { expect(order([file('src/a.ts', ['import { b } from "~/lib/b";']), file('lib/b.ts')])).toEqual(['lib/b.ts', 'src/a.ts']); });
  test('ignores an ambiguous last segment rather than guessing', () => {
    expect(order([file('src/a.ts', ['import { b } from "~/somewhere/b";']), file('one/b.ts'), file('two/b.ts')])).toEqual(['one/b.ts', 'src/a.ts', 'two/b.ts']);
  });
  test('resolves a specifier naming an extension, including the .js beside a .ts', () => { expect(order([file('src/a.ts', ['import { b } from "./b.js";']), file('src/b.ts')])).toEqual(['src/b.ts', 'src/a.ts']); });
  test('reads require and bare imports too', () => {
    expect(order([file('src/a.ts', ['const b = require("./b");', 'import "./c";']), file('src/b.ts'), file('src/c.ts')])).toEqual(['src/b.ts', 'src/c.ts', 'src/a.ts']);
  });
  test('falls back to path order inside a cycle', () => { expect(order([file('src/b.ts', ['import { a } from "./a";']), file('src/a.ts', ['import { b } from "./b";'])])).toEqual(['src/a.ts', 'src/b.ts']); });
  test('clusters unrelated files by path', () => { expect(order([file('src/ui/b.ts'), file('src/lib/z.ts'), file('src/lib/a.ts')])).toEqual(['src/lib/a.ts', 'src/lib/z.ts', 'src/ui/b.ts']); });
  test('keeps the tiers apart and orders tests by the source they cover', () => {
    expect(order([file('pnpm-lock.yaml'), file('src/a.test.ts'), file('src/__tests__/b.ts'), file('src/a.ts', ['import { b } from "./b";']), file('src/b.ts'), file('dist/a.js')]))
      .toEqual(['src/b.ts', 'src/a.ts', 'src/__tests__/b.ts', 'src/a.test.ts', 'dist/a.js', 'pnpm-lock.yaml']);
  });
  test('puts a test with no source in the diff after the ones that have theirs', () => {
    expect(order([file('src/orphan.spec.ts'), file('src/a.test.ts'), file('src/a.ts')])).toEqual(['src/a.ts', 'src/a.test.ts', 'src/orphan.spec.ts']);
  });
  test('orders the same diff the same way whatever order it arrives in', () => {
    const files = [file('src/a.ts', ['import { b } from "./b";']), file('src/b.ts'), file('src/c.ts'), file('src/a.test.ts'), file('yarn.lock')];
    expect(order(files)).toEqual(order([...files].reverse()));
  });
});

describe('review thread comment pages', () => {
  test('appends new comments once and keeps refreshed base comments', () => {
    expect(mergePullRequestThreadComments([{ id: 'c1', body: 'refreshed' }, { id: 'c2', body: 'already in base' }], [{ id: 'c2', body: 'stale page copy' }, { id: 'c3', body: 'next page' }]))
      .toEqual([{ id: 'c1', body: 'refreshed' }, { id: 'c2', body: 'already in base' }, { id: 'c3', body: 'next page' }]);
  });
  test('keeps a loaded comment after its body is edited', () => {
    expect(editPullRequestThreadComment([{ id: 'c2', body: 'old body' }, { id: 'c3', body: 'another loaded comment' }], 'c2', 'saved body')).toEqual([{ id: 'c2', body: 'saved body' }, { id: 'c3', body: 'another loaded comment' }]);
  });
});

// ── The clone's own ──────────────────────────────────────────────────────────

const slice = (cursor: string | null, patch: string, nextCursor: string | null = null): DiffSlice => ({ cursor, patch, truncated: false, nextCursor, omittedFileStats: [] });
describe('adoptDiffSlice (PullRequestCodeTab sliceState)', () => {
  test('appends a new slice, keeps an identical answer, and drops the slices after one that changed', () => {
    const first = adoptDiffSlice([], slice(null, 'p1', '100'));
    const second = adoptDiffSlice(first, slice('100', 'p2', '200'));
    const third = adoptDiffSlice(second, slice('200', 'p3'));
    expect(third.map(entry => entry.cursor)).toEqual([null, '100', '200']);
    expect(adoptDiffSlice(third, slice('100', 'p2', '200'))).toBe(third);
    expect(adoptDiffSlice(third, slice('100', 'p2 moved', '200')).map(entry => [entry.cursor, entry.patch])).toEqual([[null, 'p1'], ['100', 'p2 moved']]);
  });
});

describe('hideWhitespaceChanges (diffRendering.ts)', () => {
  const patch = 'diff --git a/a.js b/a.js\n--- a/a.js\n+++ b/a.js\n@@ -1,3 +1,3 @@\n-if (a) {\n-  go();\n+if (a)  {\n+    go();\n+  stop();\n }\n';
  test('a line whose only change is its spacing reads as context, and the counts follow', () => {
    const shown = renderablePatch(patch, true), plain = renderablePatch(patch, false);
    const filtered = shown?.kind === 'files' ? shown.files[0]! : null, whole = plain?.kind === 'files' ? plain.files[0]! : null;
    expect([whole!.additions, whole!.deletions]).toEqual([3, 2]);
    expect([filtered!.additions, filtered!.deletions]).toEqual([1, 0]);
    expect(filtered!.hunks[0]!.lines.map(line => `${line.kind[0]}${line.old}:${line.next} ${line.text}`)).toEqual(['c1:1 if (a)  {', 'c2:2     go();', 'a0:3   stop();', 'c3:4 }']);
  });
  test('leaves a file without whitespace-only changes as it was', () => {
    const file = { hunks: [], additions: 2, deletions: 1 } as unknown as PrDiffFile;
    expect(hideWhitespaceChanges(file)).toMatchObject({ additions: 0, deletions: 0 });
  });
  test('a patch that is not files is shown as raw text', () => {
    expect(renderablePatch('Binary blob without headers', false)).toEqual({ kind: 'raw', text: 'Binary blob without headers', reason: 'Unsupported diff format. Showing raw patch.' });
    expect(renderablePatch('  ', false)).toBeNull();
  });
});

describe('resolveDiffReviewPosition (reviewCommentContext.ts)', () => {
  const parsed = renderablePatch('diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -4,4 +4,4 @@\n one\n-two\n+TWO\n three\n four\n', false);
  const target = parsed?.kind === 'files' ? parsed.files[0]! : null;
  test('an added, a deleted and a context line each name their coordinates', () => {
    expect(resolveDiffReviewPosition(target!, 5, 'additions')).toEqual({ kind: 'added', newLine: 5 });
    expect(resolveDiffReviewPosition(target!, 5, 'deletions')).toEqual({ kind: 'deleted', oldLine: 5 });
    expect(resolveDiffReviewPosition(target!, 6, 'deletions')).toEqual({ kind: 'context', oldLine: 6, newLine: 6, side: 'left' });
    expect(resolveDiffReviewPosition(target!, 4, 'additions')).toEqual({ kind: 'context', oldLine: 4, newLine: 4, side: 'right' });
    expect(resolveDiffReviewPosition(target!, 40, 'additions')).toBeNull();
  });
  test('each position is pinned under its own line and side', () => {
    expect(reviewPositionAnchor({ kind: 'added', newLine: 5 })).toEqual({ line: 5, side: 'right' });
    expect(reviewPositionAnchor({ kind: 'deleted', oldLine: 5 })).toEqual({ line: 5, side: 'left' });
    expect(reviewPositionAnchor({ kind: 'context', oldLine: 6, newLine: 7, side: 'left' })).toEqual({ line: 6, side: 'left' });
  });
});
