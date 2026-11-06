// Review guide excerpts, ported from T3 Code 9c161462c9 (MIT; see LICENSE-T3):
// packages/client-runtime/src/reviewGuideTour.ts. The clone's parser retains Git's
// zero-count header offsets, so the empty side starts one line after that offset.
import type { PrDiffFile } from './pages-pr-code-logic';
import type { Line } from '../diff/diff';

export type GuideExcerptRange = { readonly side: 'old' | 'new'; readonly start: number; readonly end: number };
export type GuideLineNote = { readonly path: string; readonly line: number; readonly title: string };
type StopFiles = { readonly excerpts: readonly { readonly path: string }[]; readonly files: readonly { readonly path: string }[] };

export function stopFilePaths(stop: StopFiles): string[] {
  return [...new Set([...stop.excerpts.map(excerpt => excerpt.path), ...stop.files.map(file => file.path)])];
}
export function stopSubline(stop: StopFiles): string {
  const count = stopFilePaths(stop).length;
  return count === 0 ? 'Context, no code' : count === 1 ? '1 file' : `${count} files`;
}

export type DiffRow = {
  readonly kind: Line['kind']; readonly oldLine: number | null; readonly newLine: number | null;
  readonly oldAt: number; readonly newAt: number; readonly text: string; readonly hunk: number;
};

export function fileDiffRows(file: Pick<PrDiffFile, 'hunks'>): DiffRow[] {
  const rows: DiffRow[] = [];
  file.hunks.forEach((hunk, hunkIndex) => {
    let oldAt = hunk.deletionStart + (hunk.deletionCount === 0 ? 1 : 0);
    let newAt = hunk.additionStart + (hunk.additionCount === 0 ? 1 : 0);
    for (const line of hunk.lines) {
      rows.push({ kind: line.kind, oldLine: line.kind === 'addition' ? null : oldAt,
        newLine: line.kind === 'deletion' ? null : newAt, oldAt, newAt, text: line.text, hunk: hunkIndex });
      if (line.kind !== 'addition') oldAt++;
      if (line.kind !== 'deletion') newAt++;
    }
  });
  return rows;
}

export function excerptRows(rows: readonly DiffRow[], excerpt: GuideExcerptRange): readonly DiffRow[] {
  const inRange = (row: DiffRow) => {
    const line = excerpt.side === 'new' ? row.newLine : row.oldLine;
    return line !== null && line >= excerpt.start && line <= excerpt.end;
  };
  let first = rows.findIndex(inRange);
  if (first === -1) return [];
  let last = rows.findLastIndex(inRange);
  const sameHunk = (index: number, anchor: number) => rows[index]?.hunk === rows[anchor]?.hunk;
  if (excerpt.side === 'new' && rows[first]?.kind === 'addition') {
    while (first > 0 && rows[first - 1]?.kind === 'deletion' && sameHunk(first - 1, first)) first--;
  }
  if (excerpt.side === 'old' && rows[last]?.kind === 'deletion') {
    while (last < rows.length - 1 && rows[last + 1]?.kind === 'addition' && sameHunk(last + 1, last)) last++;
  }
  return rows.slice(first, last + 1);
}

const ROW_PREFIX: Record<DiffRow['kind'], string> = { context: ' ', addition: '+', deletion: '-' };

export function rowsToPatch(path: string, previousPath: string, rows: readonly DiffRow[]): string {
  if (rows.length === 0) return '';
  const lines = [`diff --git a/${previousPath} b/${path}`, `--- a/${previousPath}`, `+++ b/${path}`];
  let start = 0;
  while (start < rows.length) {
    const head = rows[start];
    if (head === undefined) break;
    let end = start;
    while (end + 1 < rows.length && rows[end + 1]?.hunk === head.hunk) end++;
    const group = rows.slice(start, end + 1);
    const oldCount = group.filter(row => row.oldLine !== null).length, newCount = group.filter(row => row.newLine !== null).length;
    // An empty side names the line before the change in Git's unified format.
    const oldStart = oldCount === 0 ? Math.max(head.oldAt - 1, 0) : head.oldAt;
    const newStart = newCount === 0 ? Math.max(head.newAt - 1, 0) : head.newAt;
    lines.push(`@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`);
    for (const row of group) lines.push(`${ROW_PREFIX[row.kind]}${row.text.replace(/\r?\n$/, '')}`);
    start = end + 1;
  }
  return `${lines.join('\n')}\n`;
}

/** Reconstruct only this excerpt, keeping the new file's other changes on both sides. */
export function excerptExpansionOldContents(newContents: string, rows: readonly DiffRow[]): string | null {
  const head = rows[0];
  if (head === undefined || rows.some(row => row.hunk !== head.hunk)) return null;
  const newLines = newContents.match(/[^\n]*\n|[^\n]+$/g) ?? [];
  const before = head.newAt - 1, oldBefore = head.oldAt - 1;
  const newCount = rows.filter(row => row.newLine !== null).length;
  const prefix = newLines.slice(0, before);
  const oldPrefix = oldBefore >= prefix.length
    ? [...Array.from({ length: oldBefore - prefix.length }, () => '\n'), ...prefix]
    : prefix.slice(prefix.length - oldBefore);
  const oldRows = rows.filter(row => row.oldLine !== null).map(row => row.text.endsWith('\n') ? row.text : `${row.text}\n`);
  return [...oldPrefix, ...oldRows, ...newLines.slice(before + newCount)].join('');
}

export type LetteredNote<Note> = { readonly marker: string; readonly note: Note };
export type ExcerptNoteMarker = { readonly line: number; readonly markers: readonly { readonly marker: string; readonly title: string }[] };

export function letterStopNotes<Note extends GuideLineNote>(stop: {
  readonly excerpts: readonly (GuideExcerptRange & { readonly path: string })[]; readonly notes: readonly Note[];
}): { lettered: LetteredNote<Note>[]; byExcerpt: ExcerptNoteMarker[][] } {
  const lettered = stop.notes.map((note, index) => ({ marker: index < 26 ? String.fromCharCode(65 + index) : String(index + 1), note }));
  const byExcerpt = stop.excerpts.map(excerpt => {
    const lines = new Map<number, { marker: string; title: string }[]>();
    for (const { marker, note } of lettered) {
      if (note.path !== excerpt.path || note.line < excerpt.start || note.line > excerpt.end) continue;
      const entry = { marker, title: note.title }, list = lines.get(note.line);
      if (list === undefined) lines.set(note.line, [entry]); else list.push(entry);
    }
    return [...lines].map(([line, markers]) => ({ line, markers }));
  });
  return { lettered, byExcerpt };
}
