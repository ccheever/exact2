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
import { buildDiffReviewComment, findDiffReviewLineIndex, reviewCommentContextRecord, type SelectedLineRange, type SelectionSide } from './diff-comments';
import { dragTo, gutterClick, parseLineCellId, pressGutter, pressLine, releaseDrag, type RowIndex } from './diff-line-drag';
import { contentsKey, currentSelection, diffFiles, noteDraftFocus, reviewLinesOf, reviewSection } from './diff';
import { addReviewCommentChip, insertContext, localId, removeReviewCommentChip } from './composer-editor';
import { ASSISTANT_CITATION_MAX_TEXT_LENGTH, createAssistantTextSelector, formatAssistantCitationHref, parseAssistantCitationHref } from './diff-citations';
import { pushToast } from './toast';
import { letGo } from './let-go';

/** After a preview answers: read the per-file patches it asked for (the first four, then whatever was requested since). */
export async function loadDiffFiles(client: T3Client, native: Native): Promise<void> {
  const state = client.diffState, lazy = state.lazy, source = state.source;
  if (!lazy || !source) return;
  await loadFilePatches(lazy, source, state.ignoreWhitespace, (method, payload) => client.rpc(native, method, payload));
}

/**
 * `diffreview` ops; answers the toast text ('' for none). Line ops name the side and modifier in
 * the op (`drag:additions:shift`, `gutter:deletions`, `comment:deletions`), the file in `value` and the line in `n`;
 * a drag's `to` takes the cell id under the pointer in `value`, its `end` nothing; `expand` takes the hidden range's index in `n`.
 */
export async function diffReview(client: T3Client, native: Native, op: string, value: string, n: number): Promise<string> {
  const state = client.diffState, scope = state.scopeKey;
  const [action = '', sideName = '', modifier = ''] = op.split(':');
  const at = { path: value, side: (sideName === 'deletions' ? 'deletions' : 'additions') as SelectionSide, line: n };
  const file = (path: string) => diffFiles(client).find(entry => entry.path === path);
  // The open draft's textarea took or let go of the focus: while it holds it, ⌘↩ is the draft's (composer-presentation.ts sendChords).
  if (action === 'focus' || action === 'blur') { noteDraftFocus(client, action === 'focus'); return ''; }
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
  if (action === 'to' || action === 'end') return lineDrag(client, state, scope, action, value);
  if (state.draft && ['comment', 'drag', 'gutter'].includes(action)) return ''; // an open draft holds the gutter (enableLineSelection: false)
  if (action === 'drag' || action === 'gutter') {
    // A press on a line number or the "+" starts the gutter's drag (realinput-1010f RF-3, diff-line-drag.ts).
    const target = file(at.path);
    if (!target || !(at.line > 0)) return '';
    const lines = reviewLinesOf(state, target);
    const index: RowIndex = point => { const found = findDiffReviewLineIndex(lines, point.line, point.side); return found < 0 ? null : found; };
    const selection = state.selection?.scope === scope ? { path: state.selection.path, range: state.selection.range } : null;
    const step = action === 'gutter' ? pressGutter(selection, at.path, { line: at.line, side: at.side }, index) : pressLine(selection, at.path, { line: at.line, side: at.side }, modifier === 'shift', index);
    state.drag = step.drag;
    state.selection = step.selection ? { scope, ...step.selection } : null;
    return '';
  }
  if (action === 'comment') {
    // The "+"'s press. Its pointer gesture (`gutter`, then `end`, on the same queued send) owns the click: a press that
    // comes while that gesture is in flight is its release, one after it finds the draft open. A press no gesture carried
    // (a keyboard or accessibility press) comments on what the gesture would: the selection's top to bottom, else its line.
    if (state.drag?.mode === 'gutter') return lineDrag(client, state, scope, 'end', '');
    const target = file(at.path);
    if (!target || !(at.line > 0)) return '';
    const lines = reviewLinesOf(state, target);
    const index: RowIndex = point => { const found = findDiffReviewLineIndex(lines, point.line, point.side); return found < 0 ? null : found; };
    const selection = state.selection?.scope === scope ? { path: state.selection.path, range: state.selection.range } : null;
    return openDraft(client, state, scope, at.path, gutterClick(selection, at.path, { line: at.line, side: at.side }, index));
  }
  if (action === 'cite') return citeSelection(client, native, op.slice('cite:'.length), value);
  if (action === 'partial') return ''; // DiffFileStatus: the partial mark only explains itself
  if (action === 'cancel') { state.draft = null; state.selection = null; noteDraftFocus(client, false); return ''; }
  if (action === 'save') {
    const draft = state.draft, target = draft ? file(draft.path) : undefined;
    if (!draft || !target || !value.trim()) return '';
    const section = reviewSection(client);
    const comment = buildDiffReviewComment({ id: draft.id, sectionId: section.id, sectionTitle: section.title, filePath: draft.path, lines: reviewLinesOf(state, target), range: draft.range, text: value });
    if (!comment) throw new ClientError('Those lines are no longer in the diff.');
    const record = reviewCommentContextRecord(comment);
    await addReviewCommentChip(client, native, record);
    state.saved.push({ contextId: record.contextId, scope: draft.scope, path: draft.path, range: draft.range, rangeLabel: comment.rangeLabel, text: comment.text });
    state.draft = null; state.selection = null; noteDraftFocus(client, false);
    return '';
  }
  if (action === 'delete') {
    await removeReviewCommentChip(client, native, value);
    state.saved = state.saved.filter(entry => entry.contextId !== value);
    return '';
  }
  throw new ClientError('Unsupported diff review action.');
}

/** beginComment: the draft on a range, its textarea mounting with the focus (autofocus). */
function openDraft(client: T3Client, state: T3Client['diffState'], scope: string, path: string, range: SelectedLineRange): string {
  const target = diffFiles(client).find(entry => entry.path === path);
  if (!target || state.draft) return '';
  const id = `file-comment-${localId()}`; // nextFileCommentId's grammar; a data source has no wall clock it may trust
  const comment = buildDiffReviewComment({ id, sectionId: '', sectionTitle: '', filePath: path, lines: reviewLinesOf(state, target), range, text: '' });
  if (!comment) return '';
  state.selection = { scope, path, range };
  state.draft = { scope, path, id, range, rangeLabel: comment.rangeLabel };
  noteDraftFocus(client, true);
  return '';
}
/**
 * The gutter's drag after its press (diff-line-drag.ts): `to` names the cell under the pointer, `end` is the release. The
 * Diff panel's viewer has no onLineSelectionEnd (AnnotatableCodeView), so only the "+"'s range opens a draft; a drag on
 * the numbers leaves its lines selected.
 */
function lineDrag(client: T3Client, state: T3Client['diffState'], scope: string, action: string, value: string): string {
  const selection = state.selection?.scope === scope ? { path: state.selection.path, range: state.selection.range } : null;
  if (action === 'to') {
    const hit = parseLineCellId(value);
    if (!hit || !state.drag) return '';
    const step = dragTo(state.drag, selection, hit.path, hit.at);
    state.drag = step.drag;
    state.selection = step.selection ? { scope, ...step.selection } : null;
    return '';
  }
  const path = state.drag?.path ?? '';
  const { selection: kept, gutter } = releaseDrag(state.drag, selection);
  state.drag = null;
  state.selection = kept ? { scope, ...kept } : null;
  return gutter ? openDraft(client, state, scope, path, gutter) : '';
}

/**
 * The message id a citation names: the served client's `message:…` id for the row's turn item
 * (r5-composer-citation.ts reads both spellings back).
 */
export function citedMessageId(rowId: string): string {
  let item = rowId;
  try { const key = JSON.parse(rowId) as unknown; if (Array.isArray(key)) item = String(key[1] ?? ''); } catch { /* a plain id */ }
  return item.replace(/^turn-item:/, 'message:');
}

/**
 * Cite (AssistantSelectionToolbar → ChatView insertAssistantCitation): the selected parts of an
 * answer's text stream ("start-end,…|stream") become a t3-citation:// chip at the composer caret.
 */
async function citeSelection(client: T3Client, native: Native, rowId: string, value: string): Promise<string> {
  const bar = value.indexOf('|');
  if (bar < 0 || !client.threadId) return '';
  const stream = value.slice(bar + 1);
  const ranges = value.slice(0, bar).split(',').map(part => part.split('-').map(Number)).filter(([start, end]) => Number.isFinite(start) && Number.isFinite(end) && end! > start!);
  if (!ranges.length) return '';
  const selector = createAssistantTextSelector(stream, Math.min(...ranges.map(range => range[0]!)), Math.max(...ranges.map(range => range[1]!)));
  if (!selector) return '';
  if (selector.text.length > ASSISTANT_CITATION_MAX_TEXT_LENGTH) throw new ClientError('Selection is too long to cite');
  const href = formatAssistantCitationHref({ version: 1, environmentId: client.environmentId, threadId: client.threadId, messageId: citedMessageId(rowId), ...selector });
  if (!parseAssistantCitationHref(href)) throw new ClientError('Unable to cite this selection.');
  try { await insertContext(client, native, 'citation', href); }
  catch (error) { if (letGo(error)) throw error; pushToast(client, { kind: 'warning', title: 'The composer is not ready', description: 'Try citing the selection after the connection or pending input is resolved.', hideCopy: true }); }
  return '';
}
