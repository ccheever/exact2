// Line comments on the Diff panel and the Files preview, adapted from T3 Code 1e2ecbd975 (MIT; see
// LICENSE-T3): apps/web/src/reviewCommentContext.ts (formatReviewCommentFence,
// buildFileReviewComment, inferReviewCommentFenceLanguage, buildDiffReviewComment,
// restoreDiffReviewCommentRange), lib/composerContextRecords.ts (reviewCommentContextLabel,
// reviewCommentContextRecord), components/files/fileCommentAnnotations.ts and
// components/diffs/commentSubmitShortcut.ts. Changes from the reference: the review rows are built
// from the clone's parsed patch (diff.ts DiffFileModel) plus, once loaded, the file's contents
// (review.getDiffFileContents), where the reference walks Pierre's FileDiffMetadata hunks; the
// "partial" rule is the same: a file without its contents has only its patch rows.
import { contextId, contextLabel } from './composer-editor-menu';

export type SelectionSide = 'additions' | 'deletions';
export type SelectedLineRange = { start: number; side: SelectionSide; end: number; endSide: SelectionSide };
export interface ReviewCommentContext {
  readonly id: string; readonly sectionId: string; readonly sectionTitle: string; readonly filePath: string;
  readonly startIndex: number; readonly endIndex: number; readonly rangeLabel: string; readonly text: string; readonly diff: string;
  readonly fenceLanguage?: string; readonly selection?: SelectedLineRange;
  readonly pullRequest?: { number: number };
}
export type ReviewLine = { change: 'context' | 'add' | 'delete'; oldLineNumber: number | null; newLineNumber: number | null; content: string };
/** The parts of diff.ts's DiffFileModel the review rows read. */
export type ReviewFile = { hunks: { oldStart: number; newStart: number; lines: { kind: 'context' | 'addition' | 'deletion'; text: string; old: number; next: number }[] }[] };
/** Both sides of a file, as `review.getDiffFileContents` answers them. */
export type ReviewContents = { oldContents: string; newContents: string };

export function formatReviewCommentFence(language: string, contents: string): string {
  const longest = Math.max(0, ...Array.from(contents.matchAll(/`+/g), match => match[0].length));
  const fence = '`'.repeat(Math.max(3, longest + 1));
  return [`${fence}${language}`, contents.trimEnd(), fence].join('\n');
}

export function inferReviewCommentFenceLanguage(filePath: string): string {
  const normalized = filePath.split('\\').join('/');
  const name = normalized.slice(normalized.lastIndexOf('/') + 1).toLowerCase();
  const dot = name.lastIndexOf('.');
  if (dot > 0 && dot < name.length - 1) return name.slice(dot + 1);
  if (name.startsWith('.') && name.length > 1) return name.slice(1);
  return 'text';
}

export function buildFileReviewComment(input: { id: string; filePath: string; startLine: number; endLine: number; text: string; contents: string }): ReviewCommentContext {
  const startLine = Math.max(1, Math.min(input.startLine, input.endLine));
  const endLine = Math.max(startLine, Math.max(input.startLine, input.endLine));
  return {
    id: input.id, sectionId: `file:${input.filePath}`, sectionTitle: 'File comment', filePath: input.filePath,
    startIndex: startLine - 1, endIndex: endLine - 1, rangeLabel: formatFileCommentRange(startLine, endLine), text: input.text.trim(),
    diff: input.contents.split('\n').slice(startLine - 1, endLine).join('\n'), fenceLanguage: inferReviewCommentFenceLanguage(input.filePath),
  };
}

const lineCount = (text: string) => text === '' ? 0 : text.split('\n').length - (text.endsWith('\n') ? 1 : 0);
const lineAt = (text: string[], number: number) => text[number - 1] ?? '';

/**
 * buildDiffReviewLines: the file's rows in order. Without contents (a patch only, Pierre's
 * `isPartial`), only the hunks' rows; with them, the unchanged lines before, between and after the
 * hunks too, which is how the reference numbers rows once the file is hydrated.
 */
export function diffReviewLines(file: ReviewFile, contents?: ReviewContents | null): ReviewLine[] {
  const rows: ReviewLine[] = [];
  const newLines = contents ? contents.newContents.split('\n') : [];
  const gap = (oldStart: number, newStart: number, count: number) => {
    for (let offset = 0; offset < count; offset++) rows.push({ change: 'context', oldLineNumber: oldStart + offset, newLineNumber: newStart + offset, content: lineAt(newLines, newStart + offset) });
  };
  let oldNext = 1, newNext = 1;
  for (const hunk of file.hunks) {
    const oldCount = hunk.lines.filter(line => line.kind !== 'addition').length, newCount = hunk.lines.filter(line => line.kind !== 'deletion').length;
    const oldStart = hunk.oldStart + (oldCount === 0 ? 1 : 0), newStart = hunk.newStart + (newCount === 0 ? 1 : 0);
    if (contents) gap(oldNext, newNext, Math.max(0, Math.min(oldStart - oldNext, newStart - newNext)));
    for (const line of hunk.lines) {
      if (line.kind === 'context') rows.push({ change: 'context', oldLineNumber: line.old, newLineNumber: line.next, content: line.text });
      else if (line.kind === 'deletion') rows.push({ change: 'delete', oldLineNumber: line.old, newLineNumber: null, content: line.text });
      else rows.push({ change: 'add', oldLineNumber: null, newLineNumber: line.next, content: line.text });
    }
    oldNext = oldStart + oldCount; newNext = newStart + newCount;
  }
  if (contents) gap(oldNext, newNext, Math.max(0, Math.min(lineCount(contents.oldContents) - oldNext + 1, lineCount(contents.newContents) - newNext + 1)));
  return rows;
}

function selectionPoint(line: ReviewLine): { lineNumber: number; side: SelectionSide } | null {
  if (line.change === 'delete' && line.oldLineNumber !== null) return { lineNumber: line.oldLineNumber, side: 'deletions' };
  if (line.newLineNumber !== null) return { lineNumber: line.newLineNumber, side: 'additions' };
  if (line.oldLineNumber !== null) return { lineNumber: line.oldLineNumber, side: 'deletions' };
  return null;
}
/** findDiffReviewLineIndex: the row of a line number on its side, else on the other side. */
export function findDiffReviewLineIndex(lines: ReadonlyArray<ReviewLine>, lineNumber: number, side: SelectionSide | undefined): number {
  const onSide = (left: boolean) => lines.findIndex(line => left ? line.change !== 'add' && line.oldLineNumber === lineNumber : line.change !== 'delete' && line.newLineNumber === lineNumber);
  const left = side === 'deletions';
  const preferred = onSide(left);
  return preferred >= 0 ? preferred : onSide(!left);
}

export function restoreDiffReviewCommentRange(lines: ReadonlyArray<ReviewLine>, comment: ReviewCommentContext): SelectedLineRange | null {
  if (comment.selection) return comment.selection;
  const startLine = lines[comment.startIndex], endLine = lines[comment.endIndex];
  if (!startLine || !endLine) return null;
  const start = selectionPoint(startLine), end = selectionPoint(endLine);
  return start && end ? { start: start.lineNumber, side: start.side, end: end.lineNumber, endSide: end.side } : null;
}

const marker = (change: ReviewLine['change']) => change === 'add' ? '+' : change === 'delete' ? '-' : ' ';
function diffRange(lines: ReadonlyArray<ReviewLine>, key: 'oldLineNumber' | 'newLineNumber') {
  const numbered = lines.filter(line => line[key] !== null);
  return { start: numbered[0]?.[key] ?? 0, count: numbered.length };
}
function formatDiffReviewRangeLabel(lines: ReadonlyArray<ReviewLine>): string {
  const first = lines[0], last = lines[lines.length - 1];
  if (!first || !last) return 'line';
  const firstNumber = first.newLineNumber ?? first.oldLineNumber, lastNumber = last.newLineNumber ?? last.oldLineNumber;
  if (firstNumber === null || lastNumber === null) return lines.length === 1 ? 'line' : `${lines.length} lines`;
  const firstMarker = marker(first.change).trim();
  const sign = firstMarker.length > 0 && lines.every(line => line.change === first.change) ? firstMarker : '';
  return firstNumber === lastNumber ? `${sign}${firstNumber}` : `${sign}${firstNumber} to ${sign}${lastNumber}`;
}

export function buildDiffReviewComment(input: { id: string; sectionId: string; sectionTitle: string; filePath: string; lines: ReadonlyArray<ReviewLine>; range: SelectedLineRange; text: string }): ReviewCommentContext | null {
  const startIndex = findDiffReviewLineIndex(input.lines, input.range.start, input.range.side);
  const endIndex = findDiffReviewLineIndex(input.lines, input.range.end, input.range.endSide ?? input.range.side);
  if (startIndex < 0 || endIndex < 0) return null;
  const from = Math.min(startIndex, endIndex), to = Math.max(startIndex, endIndex);
  const selected = input.lines.slice(from, to + 1);
  const oldRange = diffRange(selected, 'oldLineNumber'), newRange = diffRange(selected, 'newLineNumber');
  return {
    id: input.id, sectionId: input.sectionId, sectionTitle: input.sectionTitle, filePath: input.filePath, startIndex: from, endIndex: to,
    rangeLabel: formatDiffReviewRangeLabel(selected), text: input.text.trim(),
    diff: [`@@ -${oldRange.start},${oldRange.count} +${newRange.start},${newRange.count} @@`, ...selected.map(line => `${marker(line.change)}${line.content}`)].join('\n'),
    fenceLanguage: 'diff',
    selection: { start: input.range.start, side: input.range.side ?? 'additions', end: input.range.end, endSide: input.range.endSide ?? input.range.side ?? 'additions' },
  };
}

// composerContextRecords.ts
const REVIEW_TEXT_MAX = 16_000, REVIEW_DIFF_MAX = 32_000, TRUNCATION_MARKER = '\n… truncated …';
function clamp(value: string, max: number): string {
  return value.length <= max ? value : `${value.slice(0, Math.max(0, max - TRUNCATION_MARKER.length))}${TRUNCATION_MARKER}`;
}
const basename = (filePath: string) => { const parts = filePath.split(/[\\/]/); return parts[parts.length - 1] ?? filePath; };
export function reviewCommentContextLabel(comment: ReviewCommentContext): string {
  if (comment.pullRequest !== undefined || (comment.sectionId.startsWith('pull-request:') && comment.diff.trim() === '' && /^PR #\d+$/u.test(comment.filePath))) {
    const number = comment.pullRequest?.number ?? Number(/^PR #(\d+)$/u.exec(comment.filePath)?.[1]);
    if (Number.isFinite(number)) return `#${number}`;
  }
  const range = /^([+-])(\d+)(?: to \1(\d+))?$/u.exec(comment.rangeLabel);
  const label = range ? `L${range[2]}${range[3] ? ` to L${range[3]}` : ''}${range[1] === '-' ? ' (before)' : ''}` : comment.rangeLabel;
  return `${basename(comment.filePath)} ${label}`;
}
/** Folded into the context-id grammar, as reviewCommentContextId does. */
export function reviewCommentContextId(commentId: string): string { return contextId('review-comment', commentId); }
export function reviewCommentContextRecord(comment: ReviewCommentContext) {
  return {
    version: 1, contextId: reviewCommentContextId(comment.id), kind: 'review-comment', label: contextLabel(reviewCommentContextLabel(comment), 'review-comment'),
    sectionId: comment.sectionId, sectionTitle: comment.sectionTitle, filePath: comment.filePath, startIndex: comment.startIndex, endIndex: comment.endIndex,
    rangeLabel: comment.rangeLabel, text: clamp(comment.text, REVIEW_TEXT_MAX), diff: clamp(comment.diff, REVIEW_DIFF_MAX),
    ...(comment.fenceLanguage !== undefined ? { fenceLanguage: comment.fenceLanguage } : {}),
  };
}

// fileCommentAnnotations.ts
export type FileCommentEntry = { id: string; kind: 'draft' | 'comment'; startLine: number; endLine: number; text: string };
export type FileCommentAnnotation = { lineNumber: number; entries: FileCommentEntry[] };
let fileCommentSequence = 0;
/** `file-comment-<ms>-<n>`; the caller passes the time (a data source has no ambient clock it may trust). */
export function nextFileCommentId(now: number): string { fileCommentSequence += 1; return `file-comment-${now}-${fileCommentSequence}`; }
export function normalizeFileCommentRange(range: { start: number; end: number }) { return { startLine: Math.min(range.start, range.end), endLine: Math.max(range.start, range.end) }; }
export function formatFileCommentRange(startLine: number, endLine: number): string { return startLine === endLine ? `L${startLine}` : `L${startLine} to L${endLine}`; }
/** After an edit moved an annotation to `lineNumber`, its entries keep their length and end there. */
export function remapFileCommentAnnotations(annotations: ReadonlyArray<FileCommentAnnotation>): FileCommentAnnotation[] {
  return annotations.map(annotation => ({ ...annotation, entries: annotation.entries.map(entry => ({ ...entry, endLine: annotation.lineNumber, startLine: Math.max(1, annotation.lineNumber - (entry.endLine - entry.startLine)) })) }));
}

/** commentSubmitShortcut.ts: ⌘/Ctrl+Enter submits a non-empty comment that is not already sending. */
export function isCommentSubmitShortcut(event: { key: string; metaKey: boolean; ctrlKey: boolean }, value: string, pending: boolean): boolean {
  return !pending && (event.metaKey || event.ctrlKey) && event.key === 'Enter' && value.trim().length > 0;
}
