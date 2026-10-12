// Line comments on the Files preview, adapted from T3 Code 1e2ecbd975 (MIT; see LICENSE-T3):
// apps/web/src/components/files/FilePreviewPanel.tsx (line selection, the gutter's "+", the draft
// card under the range's last line, buildFileReviewComment into a review-comment chip, comments
// moved with their text when the file changes: remapFileCommentAnnotations) and
// fileCommentAnnotations.ts. Changes from the reference: Pierre reports where an edited annotation
// now sits; the clone's editor is a plain text view over the lines, so a comment's line is moved
// by the edit's unchanged head and tail (`remapLine`), and the cards hide while the editor is open
// (they would shift the lines under its transparent text).
//
// The gutter (files-gutter-parity FG-2): as the diff's (diff-line-drag.ts, realinput-1010f RF-3), a press on a line
// number starts a selection that follows the pointer over the file's lines (Shift extends it) and a press on the "+"
// starts a range from the selection's top; the release ends it, and FilePreviewPanel's onLineSelectionEnd opens the draft
// on what it ended on (`beginComment`), so a click on a number opens one too. R4CodeRow (r4-surfaces-files.contract)
// names the line under the pointer by its row's id (`fileLineId`). While a selection is held the "+" sits on its bottom
// line (placeUtilityFromSelection), as the diff's.
import type { T3Client } from './client';
import type { Native } from './protocol';
import { buildFileReviewComment, formatFileCommentRange, normalizeFileCommentRange, remapFileCommentAnnotations, reviewCommentContextRecord, type SelectedLineRange } from './diff-comments';
import { dragTo, pressGutter, pressLine, releaseDrag, type FileSelection, type LineDrag, type RowIndex } from './diff-line-drag';
import { addReviewCommentChip, localId, removeReviewCommentChip } from './composer-editor';
import { closeChipFromPress } from './composer-chip-popover';

type Range = { start: number; end: number };
type Saved = { contextId: string; path: string; startLine: number; endLine: number; text: string };
/** `focus`: the right panel (its panel key) whose open draft's textarea took the focus, '' when it let go; the composer's
 * Send then leaves ⌘↩ to the draft (r4-surfaces-files.ts fileDraftHoldsCommandEnter), as diff.ts draftFocus does.
 * `drag`: the gutter's press, held until its release (diff-line-drag.ts). */
type FileComments = { selection: { path: string; range: Range } | null; draft: { path: string; id: string; startLine: number; endLine: number } | null; saved: Saved[]; texts: Map<string, string>; focus: string; drag: LineDrag | null };
const states = new WeakMap<T3Client, FileComments>();
function stateOf(client: T3Client): FileComments {
  let state = states.get(client);
  if (!state) { state = { selection: null, draft: null, saved: [], texts: new Map(), focus: '', drag: null }; states.set(client, state); }
  return state;
}

/** The id R4CodeRow gives a line's row, so `elementFromPoint` names the line under the pointer: `fl:<line>:<path>`. */
export const fileLineId = (line: number, path: string): string => `fl:${line}:${path}`;
/** The file and line a row id names, or null for any other id (a comment card, the preview). */
export function parseFileLineId(id: string): { path: string; line: number } | null {
  const match = /^fl:(\d+):(.+)$/.exec(id);
  const line = match ? Number(match[1]) : 0;
  return match && line > 0 ? { path: match[2]!, line } : null;
}
// The preview has one side: the diff's gestures over it, every line on `additions`, a line's row its number.
const ROW: RowIndex = point => point.line;
const asRange = (range: SelectedLineRange): Range => ({ start: range.start, end: range.end });
const selectionOf = (state: FileComments): FileSelection => state.selection ? { path: state.selection.path, range: { start: state.selection.range.start, side: 'additions', end: state.selection.range.end, endSide: 'additions' } } : null;
function take(state: FileComments, step: { drag: LineDrag | null; selection: FileSelection }): void {
  state.drag = step.drag;
  state.selection = step.selection ? { path: step.selection.path, range: asRange(step.selection.range) } : null;
}

/** Where a line of the old text is in the new one: the edit's unchanged head keeps it, its unchanged tail shifts it, inside the edit it clamps. */
export function remapLine(before: string, after: string, line: number): number {
  const a = before.split('\n'), b = after.split('\n');
  let head = 0;
  while (head < a.length && head < b.length && a[head] === b[head]) head++;
  let tail = 0;
  while (tail < a.length - head && tail < b.length - head && a[a.length - 1 - tail] === b[b.length - 1 - tail]) tail++;
  if (line <= head) return line;
  if (line > a.length - tail) return line + b.length - a.length;
  return Math.max(1, Math.min(line, b.length - tail));
}

/** Follows the file's text: saved comments and the draft move with their lines when it changes. */
function follow(state: FileComments, path: string, text: string): void {
  const before = state.texts.get(path);
  state.texts.set(path, text);
  if (before === undefined || before === text) return;
  const moved = remapFileCommentAnnotations(state.saved.filter(entry => entry.path === path).map(entry => ({ lineNumber: remapLine(before, text, entry.endLine),
    entries: [{ id: entry.contextId, kind: 'comment' as const, startLine: entry.startLine, endLine: entry.endLine, text: entry.text }] })));
  for (const annotation of moved) for (const entry of annotation.entries) {
    const saved = state.saved.find(candidate => candidate.contextId === entry.id);
    if (saved) { saved.startLine = entry.startLine; saved.endLine = entry.endLine; }
  }
  if (state.draft?.path === path) {
    const end = remapLine(before, text, state.draft.endLine), length = state.draft.endLine - state.draft.startLine;
    state.draft = { ...state.draft, endLine: end, startLine: Math.max(1, end - length) };
  }
  if (state.selection?.path === path) state.selection = null;
}

/** `pinned`: the file has a selection, so its "+" is drawn only on `pin`, the selection's bottom line (placeUtilityFromSelection). */
export type FileLine = { id: string; number: string; runs: { id: string; text: string; syntax: string }[]; kind: string; line: number; selected: boolean; pin: boolean; pinned: boolean; entry: string; text: string; label: string };
/** The preview's rows: its lines, each saved comment and the draft under its last line; no cards while the editor is open. */
export function fileCommentLines(client: T3Client, path: string, text: string, lines: { id: string; number: string; runs: { id: string; text: string; syntax: string }[] }[], editing: boolean): FileLine[] {
  const state = stateOf(client);
  const blank = { kind: 'line', line: 0, selected: false, pin: false, pinned: false, entry: '', text: '', label: '' };
  if (!path) return lines.map(line => ({ ...line, ...blank }));
  follow(state, path, text);
  const chips = client.draft;
  const saved = state.saved.filter(entry => entry.path === path && chips.includes(`review-comment/${entry.contextId})`));
  // While a draft is open the gutter is off (its "+" too); the selection it shows is the draft's.
  const held = !state.draft && state.selection?.path === path ? normalize(state.selection.range) : null;
  const range = state.draft?.path === path ? { start: state.draft.startLine, end: state.draft.endLine } : held;
  const out: FileLine[] = [];
  lines.forEach((line, index) => {
    const number = index + 1;
    out.push({ ...line, ...blank, line: number, selected: !!range && number >= range.start && number <= range.end, pinned: !!held, pin: !!held && number === held.end });
    if (editing) return;
    for (const entry of saved) if (entry.endLine === number) out.push({ ...line, id: `note:${entry.contextId}`, runs: [], ...blank, kind: 'note', line: number, entry: entry.contextId, text: entry.text });
    if (state.draft?.path === path && state.draft.endLine === number) out.push({ ...line, id: `draft:${state.draft.id}`, runs: [], ...blank, kind: 'draft', line: number, entry: state.draft.id, label: formatFileCommentRange(state.draft.startLine, state.draft.endLine) });
  });
  return out;
}
const normalize = (range: Range) => { const { startLine, endLine } = normalizeFileCommentRange(range); return { start: startLine, end: endLine }; };
export function fileCommentOpen(client: T3Client): boolean { return stateOf(client).draft !== null; }
/** The open draft whose textarea holds the focus, with the panel it was taken on; null when none does. */
export function focusedFileDraft(client: T3Client): { key: string; path: string; endLine: number } | null {
  const state = stateOf(client);
  return state.focus && state.draft ? { key: state.focus, path: state.draft.path, endLine: state.draft.endLine } : null;
}

/** beginComment: the draft on a range, under its last line, its textarea mounting with the focus (autofocus). */
function openDraft(state: FileComments, path: string, range: Range, panel: string): string {
  const { startLine, endLine } = normalizeFileCommentRange(range);
  state.draft = { path, id: `file-comment-${localId()}`, startLine, endLine };
  state.selection = { path, range: { start: startLine, end: endLine } };
  state.focus = panel;
  return '';
}

/**
 * `shelllocal:surface-files-comment-*`: the gutter's `drag` / `drag-shift` / `gutter` (a press on a number, with Shift, on
 * the "+"; value = the line), `to` (value = the row id under the pointer), `end` (the release) and `press` (a primary press
 * while the gutter is off); `begin` (the "+"'s or a number's press with no pointer gesture: a keyboard or accessibility
 * press; value = the line), `cancel`, `save` (value = the text), `delete` (value = the context id), and the draft textarea's
 * `focus` / `blur` (audit-wave-followups FU-3, `panel` = its panel key).
 */
export async function fileComment(client: T3Client, native: Native, op: string, path: string, value: string, contents: string, panel = ''): Promise<string> {
  const state = stateOf(client), line = Number(value) || 0;
  if (op === 'focus' || op === 'blur') { state.focus = op === 'focus' && state.draft?.path === path ? panel : ''; return ''; }
  if (op === 'drag' || op === 'drag-shift' || op === 'gutter' || op === 'press') {
    // A press on the gutter is an outside press for a skill chip's details, which the window's root never hears (FG-4).
    if (op !== 'press' && !state.draft && line > 0) take(state, op === 'gutter' ? pressGutter(selectionOf(state), path, { line, side: 'additions' }, ROW) : pressLine(selectionOf(state), path, { line, side: 'additions' }, op === 'drag-shift', ROW));
    await closeChipFromPress(native);
    return '';
  }
  if (op === 'to') {
    const hit = parseFileLineId(value);
    if (hit && state.drag) take(state, dragTo(state.drag, selectionOf(state), hit.path, { line: hit.line, side: 'additions' }));
    return '';
  }
  if (op === 'end') {
    // onGutterUtilityClick selects the "+"'s range and onLineSelectionEnd opens the draft on what the gesture ended on.
    const drag = state.drag, { selection, gutter, ended } = releaseDrag(drag, selectionOf(state));
    state.drag = null;
    state.selection = selection ? { path: selection.path, range: asRange(selection.range) } : null;
    const range = ended ?? gutter;
    return drag && range && !state.draft ? openDraft(state, drag.path, asRange(range), panel) : '';
  }
  if (op === 'begin') {
    // A press its pointer gesture carried comes after that gesture's release, which opened the draft; one that comes while
    // the gesture is in flight is its release.
    if (state.drag) return fileComment(client, native, 'end', path, '', contents, panel);
    if (state.draft || line < 1) return '';
    const selected = state.selection?.path === path ? normalize(state.selection.range) : null;
    return openDraft(state, path, selected && line >= selected.start && line <= selected.end ? selected : { start: line, end: line }, panel);
  }
  if (op === 'cancel') { state.draft = null; state.selection = null; state.focus = ''; state.drag = null; return ''; }
  if (op === 'save') {
    const draft = state.draft;
    if (!draft || draft.path !== path || !value.trim()) return '';
    const comment = buildFileReviewComment({ id: draft.id, filePath: path, startLine: draft.startLine, endLine: draft.endLine, text: value, contents });
    const record = reviewCommentContextRecord(comment);
    await addReviewCommentChip(client, native, record);
    state.saved.push({ contextId: record.contextId, path, startLine: draft.startLine, endLine: draft.endLine, text: comment.text });
    state.draft = null; state.selection = null; state.focus = '';
    return '';
  }
  if (op === 'delete') {
    await removeReviewCommentChip(client, native, value);
    state.saved = state.saved.filter(entry => entry.contextId !== value);
    return '';
  }
  return '';
}
