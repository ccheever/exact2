// The pull request Code tab's virtualized list (20261005-pr-code-tab): one flat list of the shared
// diff items (shapes.contract DiffItem, drawn by diff.contract DiffRow) — the file headers with their
// Viewed tick, the stacked or split rows, the hidden ranges, and what is pinned under a line: the
// host's conversations, the pending comments of the review being written, and the open draft
// (PullRequestCodeTab.tsx annotatedFiles/items over @pierre/diffs CodeView, T3 Code 1e2ecbd975, MIT,
// see LICENSE-T3). The clone's thread diff panel draws the same rows (diff.ts diffSnapshot); the
// kinds this tab adds are `pr-file`, `pr-thread`, `pr-pending` and `pr-draft` (pages-pr-code.contract).
import { hunkRows, plain, ROW_LIMIT, type Row, type Segment } from './diff';
import { diffReviewLines, type ReviewLine, type SelectedLineRange } from './diff-comments';
import type { FileContents, Expansion } from './diff-lazy';
import { isFileDiffCollapsed, reviewPositionAnchor, type DiffFoldOverride, type PrDiffFile, type PrDiffSide, type PullRequestReviewPosition } from './pages-pr-code-logic';

export type CodeItem = {
  id: string; kind: string; path: string; name: string; status: string; letter: string; additions: number; deletions: number; expanded: boolean; markdown: boolean;
  tone: string; number: string; segments: Segment[]; leftTone: string; leftNumber: string; leftSegments: Segment[]; rightTone: string; rightNumber: string; rightSegments: Segment[];
  label: string; gutter: number; side: string; line: number; selected: boolean; leftLine: number; rightLine: number; leftSelected: boolean; rightSelected: boolean;
  error: boolean; partial: boolean; unavailable: boolean; expandable: boolean; entry: string; text: string;
  /** The Diff panel's header counts (diff.ts headerStat); the Code tab draws its own header. */
  statAligned: boolean; addText: string; delText: string;
};
export const BASE: CodeItem = { id: '', kind: '', path: '', name: '', status: '', letter: '', additions: 0, deletions: 0, expanded: false, markdown: false, tone: '', number: '', segments: [],
  leftTone: '', leftNumber: '', leftSegments: [], rightTone: '', rightNumber: '', rightSegments: [], label: '', gutter: 33.3, side: '', line: 0, selected: false, leftLine: 0, rightLine: 0,
  leftSelected: false, rightSelected: false, error: false, partial: false, unavailable: false, expandable: false, entry: '', text: '', statAligned: false, addText: '', delText: '' };
const LETTER: Record<string, string> = { modified: 'M', added: 'A', deleted: 'D', renamed: 'R' };

/** What one file's rows read beside the file itself. */
export type FileAnnotations = {
  threads: { id: string; side: PrDiffSide; line: number }[];
  pending: { id: string; body: string; position: PullRequestReviewPosition }[];
  draft: { position: PullRequestReviewPosition; label: string } | null;
};
export type RowsInput = {
  files: PrDiffFile[];
  layout: 'stacked' | 'split';
  foldOverride: DiffFoldOverride;
  toggled: ReadonlySet<string>;
  /** The Viewed mark a header draws: '' (no tick offered), 'unviewed', 'viewed' or 'changed'. */
  viewedMark: (path: string) => string;
  /** The host's own counts for a file whose hunks it withheld. */
  omitted: ReadonlyMap<string, { additions: number; deletions: number }>;
  annotations: (path: string) => FileAnnotations;
  /** The selected lines (or the draft's), in one file. */
  selection: { path: string; range: SelectedLineRange } | null;
  contents: Record<string, FileContents>;
  contentsKey: (path: string) => string;
  expansions: Record<string, Expansion>;
};
/** A file's key: its path (a slice holds one file per path; the reference's buildFileDiffRenderKey). */
export const fileKey = (file: PrDiffFile) => file.path;
export const isCollapsed = (input: Pick<RowsInput, 'foldOverride' | 'toggled'>, file: PrDiffFile) => isFileDiffCollapsed(fileKey(file), input.foldOverride, input.toggled);

const sideKey = (side: string, line: number) => `${side === 'deletions' || side === 'left' ? 'd' : 'a'}:${line}`;
function lineIndex(lines: ReviewLine[]): Map<string, number> {
  const index = new Map<string, number>();
  lines.forEach((line, at) => {
    if (line.change !== 'add' && line.oldLineNumber !== null && !index.has(sideKey('deletions', line.oldLineNumber))) index.set(sideKey('deletions', line.oldLineNumber), at);
    if (line.change !== 'delete' && line.newLineNumber !== null && !index.has(sideKey('additions', line.newLineNumber))) index.set(sideKey('additions', line.newLineNumber), at);
  });
  return index;
}

/** The list: every file's header and, unless it is folded, its rows with what is pinned under them. */
export function codeRows(input: RowsInput): { items: CodeItem[]; truncated: boolean } {
  const items: CodeItem[] = [];
  let rows = 0;
  for (const file of input.files) {
    const expanded = !isCollapsed(input, file);
    // PullRequestCodeTab renderHeaderMetadata: the hunks' sums, or the host's counts for a withheld file.
    let additions = file.additions, deletions = file.deletions;
    if (additions === 0 && deletions === 0) { const withheld = input.omitted.get(file.path); if (withheld) ({ additions, deletions } = withheld); }
    items.push({ ...BASE, id: `file:${file.path}`, kind: 'pr-file', path: file.path, name: file.path, status: file.status, letter: LETTER[file.status] ?? 'M', additions, deletions, expanded,
      markdown: /\.(md|mdx|markdown)$/i.test(file.path), label: input.viewedMark(file.path), entry: file.previous && file.previous !== file.path ? file.previous : '' });
    if (!expanded) continue;
    if (file.binary) { items.push({ ...BASE, id: `binary:${file.path}`, kind: 'gap', path: file.path, expanded, label: 'Binary file not shown' }); items.push({ ...BASE, id: `pad:${file.path}`, kind: 'pad', path: file.path, expanded }); continue; }
    const contents = input.contents[input.contentsKey(file.path)];
    const loaded = contents?.state === 'loaded' ? contents : null;
    const newLines = loaded ? loaded.newContents.split('\n') : [];
    const review = diffReviewLines(file, loaded), at = lineIndex(review);
    const picked = input.selection?.path === file.path ? input.selection.range : null;
    const span = picked ? (() => { const from = at.get(sideKey(picked.side, picked.start)), to = at.get(sideKey(picked.endSide, picked.end)); return from === undefined || to === undefined ? null : [Math.min(from, to), Math.max(from, to)] as const; })() : null;
    const isSelected = (side: string, line: number) => { const index = line > 0 ? at.get(sideKey(side, line)) : undefined; return !!span && index !== undefined && index >= span[0] && index <= span[1]; };
    const digits = Math.max(1, ...file.hunks.flatMap(hunk => hunk.lines.map(line => String(Math.max(line.old, line.next)).length)));
    const gutter = Math.round((digits * 7.8267 + 15.6533 + 9.83) * 10) / 10;
    // One group per line, on its side: conversations, then pending comments, then the draft (PullRequestCodeTab groupAt).
    const notes = input.annotations(file.path);
    const groups = new Map<string, CodeItem[]>();
    const group = (side: PrDiffSide, line: number) => { const key = sideKey(side, line); let list = groups.get(key); if (!list) { list = []; groups.set(key, list); } return list; };
    for (const thread of notes.threads) group(thread.side, thread.line).push({ ...BASE, id: `thread:${thread.id}`, kind: 'pr-thread', path: file.path, expanded, entry: thread.id });
    for (const comment of notes.pending) { const anchor = reviewPositionAnchor(comment.position); group(anchor.side, anchor.line).push({ ...BASE, id: `pending:${comment.id}`, kind: 'pr-pending', path: file.path, expanded, entry: comment.id, text: comment.body }); }
    if (notes.draft) { const anchor = reviewPositionAnchor(notes.draft.position); group(anchor.side, anchor.line).push({ ...BASE, id: `draft:${file.path}`, kind: 'pr-draft', path: file.path, expanded, label: notes.draft.label }); }
    const annotate = (oldLine: number, newLine: number) => {
      for (const key of [oldLine > 0 ? sideKey('left', oldLine) : '', newLine > 0 ? sideKey('right', newLine) : '']) {
        const list = key ? groups.get(key) : undefined;
        if (list) { items.push(...list); groups.delete(key); }
      }
    };
    const push = (row: Row | { left: Row; right: Row }, key: string) => {
      if (++rows > ROW_LIMIT) return;
      const common = { ...BASE, id: `line:${file.path}:${key}`, path: file.path, expanded, gutter };
      if ('left' in row) {
        items.push({ ...common, kind: 'split', leftTone: row.left.tone, leftNumber: row.left.number, leftSegments: row.left.segments, rightTone: row.right.tone, rightNumber: row.right.number,
          rightSegments: row.right.segments, leftLine: row.left.oldLine, rightLine: row.right.newLine, leftSelected: isSelected('deletions', row.left.oldLine), rightSelected: isSelected('additions', row.right.newLine) });
        annotate(row.left.oldLine, row.right.newLine);
      } else {
        const side = row.tone === 'deletion' ? 'deletions' : 'additions', line = side === 'deletions' ? row.oldLine : row.newLine;
        items.push({ ...common, kind: 'line', tone: row.tone, number: row.number, segments: row.segments, side, line, selected: isSelected(side, line) });
        annotate(row.tone === 'addition' ? 0 : row.oldLine, row.tone === 'deletion' ? 0 : row.newLine);
      }
    };
    // Unchanged lines a hidden range has opened (Pierre's expandHunk over the file's contents).
    const context = (newLine: number, delta: number, key: string) => {
      const text = newLines[newLine - 1] ?? '', oldLine = newLine - delta;
      push(input.layout === 'split' ? { left: { tone: 'context', number: String(oldLine), segments: plain(text), oldLine, newLine: 0 }, right: { tone: 'context', number: String(newLine), segments: plain(text), oldLine: 0, newLine } }
        : { tone: 'context', number: String(newLine), segments: plain(text), oldLine, newLine }, key);
    };
    const hidden = (count: number, gapIndex: number, newFirst: number, delta: number, trailing: boolean) => {
      if (count <= 0) return;
      const opened = input.expansions[`${input.contentsKey(file.path)}::${gapIndex}`] ?? { fromStart: 0, fromEnd: 0 };
      const fromStart = Math.min(count, opened.fromStart), fromEnd = Math.min(count - fromStart, opened.fromEnd), left = count - fromStart - fromEnd;
      for (let k = 0; k < fromStart; k++) context(newFirst + k, delta, `x${gapIndex}:${k}`);
      const failed = contents?.state === 'error';
      if (left > 0) items.push({ ...BASE, id: `gap:${file.path}:${gapIndex}`, kind: 'gap', path: file.path, expanded, line: gapIndex, expandable: !failed,
        label: failed ? contents.error || 'Unable to load file contents.' : trailing && left === count && !loaded ? 'More unchanged context may be available' : `${left} unmodified ${left === 1 ? 'line' : 'lines'}` });
      for (let k = left + fromStart; k < count; k++) context(newFirst + k, delta, `x${gapIndex}:${k}`);
    };
    let oldNext = 1, newNext = 1;
    file.hunks.forEach((hunk, hunkIndex) => {
      const oldStart = hunk.oldStart + (hunk.deletionCount === 0 ? 1 : 0), newStart = hunk.newStart + (hunk.additionCount === 0 ? 1 : 0);
      hidden(newStart - newNext, hunkIndex, newNext, newStart - oldStart, false);
      const { stacked, split } = hunkRows(hunk, file.path);
      (input.layout === 'split' ? split : stacked).forEach((row, index) => push(row, `${hunkIndex}:${index}`));
      oldNext = oldStart + hunk.deletionCount; newNext = newStart + hunk.additionCount;
    });
    // After the last hunk: known once the contents are in (as the thread diff panel draws it).
    if (loaded && file.hunks.length > 0) hidden(Math.max(0, newLines.length - (loaded.newContents.endsWith('\n') ? 1 : 0) - newNext + 1), file.hunks.length, newNext, newNext - oldNext, true);
    items.push({ ...BASE, id: `pad:${file.path}`, kind: 'pad', path: file.path, expanded });
  }
  return { items, truncated: rows > ROW_LIMIT };
}

/** The hidden range a gap press opens: how many lines it hides, from the file's contents. */
export function hiddenCount(file: PrDiffFile, gapIndex: number, contents: FileContents | undefined): number {
  const loaded = contents?.state === 'loaded' ? contents : null;
  let newNext = 1;
  for (const [index, hunk] of file.hunks.entries()) {
    const start = hunk.newStart + (hunk.additionCount === 0 ? 1 : 0);
    if (index === gapIndex) return start - newNext;
    newNext = start + hunk.additionCount;
  }
  if (!loaded) return 0;
  const total = loaded.newContents.split('\n').length - (loaded.newContents.endsWith('\n') ? 1 : 0);
  return Math.max(0, total - newNext + 1);
}
