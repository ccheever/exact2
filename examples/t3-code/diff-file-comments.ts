// Line comments on the Files preview, adapted from T3 Code 1e2ecbd975 (MIT; see LICENSE-T3):
// apps/web/src/components/files/FilePreviewPanel.tsx (line selection, the gutter's "+", the draft
// card under the range's last line, buildFileReviewComment into a review-comment chip, comments
// moved with their text when the file changes: remapFileCommentAnnotations) and
// fileCommentAnnotations.ts. Changes from the reference: Pierre reports where an edited annotation
// now sits; the clone's editor is a plain text view over the lines, so a comment's line is moved
// by the edit's unchanged head and tail (`remapLine`), and the cards hide while the editor is open
// (they would shift the lines under its transparent text).
import type { T3Client } from './client';
import type { Native } from './protocol';
import { buildFileReviewComment, formatFileCommentRange, normalizeFileCommentRange, remapFileCommentAnnotations, reviewCommentContextRecord } from './diff-comments';
import { addReviewCommentChip, localId, removeReviewCommentChip } from './composer-editor';

type Range = { start: number; end: number };
type Saved = { contextId: string; path: string; startLine: number; endLine: number; text: string };
type FileComments = { selection: { path: string; range: Range } | null; draft: { path: string; id: string; startLine: number; endLine: number } | null; saved: Saved[]; texts: Map<string, string> };
const states = new WeakMap<T3Client, FileComments>();
function stateOf(client: T3Client): FileComments {
  let state = states.get(client);
  if (!state) { state = { selection: null, draft: null, saved: [], texts: new Map() }; states.set(client, state); }
  return state;
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

export type FileLine = { id: string; number: string; runs: { id: string; text: string; syntax: string }[]; kind: string; line: number; selected: boolean; entry: string; text: string; label: string };
/** The preview's rows: its lines, each saved comment and the draft under its last line; no cards while the editor is open. */
export function fileCommentLines(client: T3Client, path: string, text: string, lines: { id: string; number: string; runs: { id: string; text: string; syntax: string }[] }[], editing: boolean): FileLine[] {
  const state = stateOf(client);
  const blank = { kind: 'line', line: 0, selected: false, entry: '', text: '', label: '' };
  if (!path) return lines.map(line => ({ ...line, ...blank }));
  follow(state, path, text);
  const chips = client.draft;
  const saved = state.saved.filter(entry => entry.path === path && chips.includes(`review-comment/${entry.contextId})`));
  const range = state.draft?.path === path ? { start: state.draft.startLine, end: state.draft.endLine } : state.selection?.path === path ? normalize(state.selection.range) : null;
  const out: FileLine[] = [];
  lines.forEach((line, index) => {
    const number = index + 1;
    out.push({ ...line, ...blank, line: number, selected: !!range && number >= range.start && number <= range.end });
    if (editing) return;
    for (const entry of saved) if (entry.endLine === number) out.push({ ...line, id: `note:${entry.contextId}`, runs: [], ...blank, kind: 'note', line: number, entry: entry.contextId, text: entry.text });
    if (state.draft?.path === path && state.draft.endLine === number) out.push({ ...line, id: `draft:${state.draft.id}`, runs: [], ...blank, kind: 'draft', line: number, entry: state.draft.id, label: formatFileCommentRange(state.draft.startLine, state.draft.endLine) });
  });
  return out;
}
const normalize = (range: Range) => { const { startLine, endLine } = normalizeFileCommentRange(range); return { start: startLine, end: endLine }; };
export function fileCommentOpen(client: T3Client): boolean { return stateOf(client).draft !== null; }

/** `shelllocal:surface-files-comment-*`: `line` / `line-shift` (value = the line), `begin`, `cancel`, `save` (value = the text), `delete` (value = the context id). */
export async function fileComment(client: T3Client, native: Native, op: string, path: string, value: string, contents: string): Promise<string> {
  const state = stateOf(client), line = Number(value) || 0;
  if ((op === 'line' || op === 'line-shift' || op === 'begin') && (state.draft || line < 1)) return '';
  if (op === 'line' || op === 'line-shift') {
    const anchor = op === 'line-shift' && state.selection?.path === path ? state.selection.range.start : line;
    state.selection = { path, range: { start: anchor, end: line } };
    return '';
  }
  if (op === 'begin') {
    const selected = state.selection?.path === path ? normalize(state.selection.range) : null;
    const range = selected && line >= selected.start && line <= selected.end ? selected : { start: line, end: line };
    state.draft = { path, id: `file-comment-${localId()}`, startLine: range.start, endLine: range.end };
    state.selection = { path, range };
    return '';
  }
  if (op === 'cancel') { state.draft = null; state.selection = null; return ''; }
  if (op === 'save') {
    const draft = state.draft;
    if (!draft || draft.path !== path || !value.trim()) return '';
    const comment = buildFileReviewComment({ id: draft.id, filePath: path, startLine: draft.startLine, endLine: draft.endLine, text: value, contents });
    const record = reviewCommentContextRecord(comment);
    await addReviewCommentChip(client, native, record);
    state.saved.push({ contextId: record.contextId, path, startLine: draft.startLine, endLine: draft.endLine, text: comment.text });
    state.draft = null; state.selection = null;
    return '';
  }
  if (op === 'delete') {
    await removeReviewCommentChip(client, native, value);
    state.saved = state.saved.filter(entry => entry.contextId !== value);
    return '';
  }
  return '';
}
