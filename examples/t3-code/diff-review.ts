// The Diff panel's `diffreview` commands, adapted from T3 Code 1e2ecbd975 (MIT; see LICENSE-T3):
// DiffPanel.tsx (revealDiffFile: the file opens, a later lazy file is asked for, the viewer
// scrolls to it), useReviewFilePatches.ts (loadNextFiles, retry), the hunk expansion's
// loadDiffFiles (createGitDiffFileContentsLoader over review.getDiffFileContents, git sources
// only) and AnnotatableCodeView.tsx (line selection, beginComment, submitEntry, removeEntry: a
// comment becomes a review-comment chip in the composer, a deleted one takes its chip away).
// Contract draws the results (diff.contract, diff-tree.contract); diff.ts projects them.
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import type { Obj } from './domain';
import { ancestorDirectories } from './diff-tree';
import { createGitDiffFileContentsLoader, changeType, expandRange, loadFileContents, loadFilePatches, loadNextFiles, requestFiles, retryFile, settledFileCount } from './diff-lazy';
import { buildDiffReviewComment, reviewCommentContextRecord, type SelectedLineRange, type SelectionSide } from './diff-comments';
import { contentsKey, currentSelection, diffFiles, reviewLinesOf, reviewSection } from './diff';
import { addReviewCommentChip, localId, removeReviewCommentChip } from './composer-editor';

/** After a preview answers: read the per-file patches it asked for (the first four, then whatever was requested since). */
export async function loadDiffFiles(client: T3Client, native: Native): Promise<void> {
  const state = client.diffState, lazy = state.lazy, source = state.source;
  if (!lazy || !source) return;
  await loadFilePatches(lazy, source, state.ignoreWhitespace, (method, payload) => client.rpc(native, method, payload));
}

/**
 * `diffreview` ops; answers the toast text ('' for none). Line ops name the side and modifier in
 * the op (`line:additions:shift`, `comment:deletions`), the file in `value` and the line in `n`;
 * `expand` takes the hidden range's index in `n`.
 */
export async function diffReview(client: T3Client, native: Native, op: string, value: string, n: number): Promise<string> {
  const state = client.diffState, scope = state.scopeKey;
  const [action = '', sideName = '', modifier = ''] = op.split(':');
  const at = { path: value, side: (sideName === 'deletions' ? 'deletions' : 'additions') as SelectionSide, line: n };
  const file = (path: string) => diffFiles(client).find(entry => entry.path === path);
  if (action === 'next') { if (state.lazy && loadNextFiles(state.lazy).length) await loadDiffFiles(client, native); return ''; }
  if (action === 'retry') { if (state.lazy && retryFile(state.lazy, value)) await loadDiffFiles(client, native); return ''; }
  if (action === 'reveal') {
    // A tree click opens the file and its folders, asks for a lazy file past the settled ones, and keeps it selected.
    state.expanded[`${scope}::${value}`] = true;
    state.treeSelected = value;
    const open = new Set(ancestorDirectories(value));
    state.treeCollapsed[scope] = (state.treeCollapsed[scope] ?? []).filter(path => !open.has(path));
    const lazy = state.lazy;
    if (lazy) {
      const index = lazy.files.findIndex(entry => entry.path === value);
      if (index >= settledFileCount(lazy) && requestFiles(lazy, [index]).length) await loadDiffFiles(client, native);
    }
    return '';
  }
  if (action === 'expand') {
    // A hidden range opens from the file's contents (git sources only; a turn diff has no contents RPC).
    const path = value, gap = n;
    const target = file(path), source = state.source;
    if (!target || !source || currentSelection(client).kind === 'turn') return '';
    const key = contentsKey(state, path);
    const load = createGitDiffFileContentsLoader(input => client.rpc(native, 'review.getDiffFileContents', input) as Promise<Obj & { oldContents: string; newContents: string }>,
      { cwd: source.cwd, sourceKind: source.kind, baseRef: source.baseRef, headRef: source.headRef, cacheKey: source.diffHash });
    await loadFileContents(state.contents, key, () => load({ type: changeType(target.status, target.hunks.length), name: target.path, ...(target.previous && target.previous !== target.path ? { prevName: target.previous } : {}) }));
    const contents = state.contents[key];
    if (contents?.state !== 'loaded') return '';
    const index = gap;
    const lines = contents.newContents.split('\n'), total = lines.length - (contents.newContents.endsWith('\n') ? 1 : 0);
    let newNext = 1;
    for (const [at, hunk] of target.hunks.entries()) {
      const count = hunk.lines.filter(line => line.kind !== 'deletion').length, start = hunk.newStart + (count === 0 ? 1 : 0);
      if (at === index) break;
      newNext = start + count;
    }
    const hunk = target.hunks[index];
    const hidden = hunk ? hunk.newStart + (hunk.lines.some(line => line.kind !== 'deletion') ? 0 : 1) - newNext : total - newNext + 1;
    const gapKey = `${scope}::${path}::${index}`;
    state.expansions[gapKey] = expandRange(state.expansions[gapKey], hidden);
    return '';
  }
  if (state.draft && ['line', 'comment'].includes(action)) return ''; // an open draft holds the gutter (enableLineSelection: false)
  if (action === 'line') {
    // Pierre's line selection: a press selects the line, a Shift-press extends the selection in the same file.
    const anchor = state.selection?.scope === scope && state.selection.path === at.path ? state.selection.range : null;
    state.selection = { scope, path: at.path, range: modifier === 'shift' && anchor ? { ...anchor, end: at.line, endSide: at.side } : { start: at.line, side: at.side, end: at.line, endSide: at.side } };
    return '';
  }
  if (action === 'comment') {
    // The gutter's "+": the selection when the line is in it, else that line alone.
    const target = file(at.path);
    if (!target) return '';
    const selected = state.selection?.scope === scope && state.selection.path === at.path ? state.selection.range : null;
    const lines = reviewLinesOf(state, target);
    const inside = selected && (() => {
      const comment = buildDiffReviewComment({ id: 'probe', sectionId: '', sectionTitle: '', filePath: at.path, lines, range: selected, text: '' });
      const probe = buildDiffReviewComment({ id: 'probe', sectionId: '', sectionTitle: '', filePath: at.path, lines, range: { start: at.line, side: at.side, end: at.line, endSide: at.side }, text: '' });
      return !!comment && !!probe && probe.startIndex >= comment.startIndex && probe.startIndex <= comment.endIndex;
    })();
    const range: SelectedLineRange = inside && selected ? selected : { start: at.line, side: at.side, end: at.line, endSide: at.side };
    const id = `file-comment-${localId()}`; // nextFileCommentId's grammar; a data source has no wall clock it may trust
    const comment = buildDiffReviewComment({ id, sectionId: '', sectionTitle: '', filePath: at.path, lines, range, text: '' });
    if (!comment) return '';
    state.selection = { scope, path: at.path, range };
    state.draft = { scope, path: at.path, id, range, rangeLabel: comment.rangeLabel };
    return '';
  }
  if (action === 'partial') return ''; // DiffFileStatus: the partial mark only explains itself
  if (action === 'cancel') { state.draft = null; state.selection = null; return ''; }
  if (action === 'save') {
    const draft = state.draft, target = draft ? file(draft.path) : undefined;
    if (!draft || !target || !value.trim()) return '';
    const section = reviewSection(client);
    const comment = buildDiffReviewComment({ id: draft.id, sectionId: section.id, sectionTitle: section.title, filePath: draft.path, lines: reviewLinesOf(state, target), range: draft.range, text: value });
    if (!comment) throw new ClientError('Those lines are no longer in the diff.');
    const record = reviewCommentContextRecord(comment);
    await addReviewCommentChip(client, native, record);
    state.saved.push({ contextId: record.contextId, scope: draft.scope, path: draft.path, range: draft.range, rangeLabel: comment.rangeLabel, text: comment.text });
    state.draft = null; state.selection = null;
    return '';
  }
  if (action === 'delete') {
    await removeReviewCommentChip(client, native, value);
    state.saved = state.saved.filter(entry => entry.contextId !== value);
    return '';
  }
  throw new ClientError('Unsupported diff review action.');
}
