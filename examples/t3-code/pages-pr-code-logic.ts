// The pull request Code tab's pure logic (20261005-pr-code-tab), ported from T3 Code 1e2ecbd975
// (MIT, see LICENSE-T3): apps/web/src/components/pullRequest/{pullRequestDiff.logic,
// pullRequestFilesViewed.logic,pullRequestFileOrder.logic}.ts, pullRequestDetail.logic.ts (the
// thread comment pages), apps/web/src/reviewCommentContext.ts resolveDiffReviewPosition,
// apps/web/src/lib/{diffRendering.ts hideWhitespaceChanges and getRenderablePatch, diffCollapse.ts}
// and PullRequestCodeTab.tsx's slice adoption. Changes from the reference: a file is the clone's
// parsed patch (diff.ts parsePatch) carrying the two hunk ranges Pierre's FileDiffMetadata has
// (deletionStart/Count, additionStart/Count) and the patch lines the import graph reads
// (additionLines, deletionLines); the whitespace filter re-diffs each hunk with a line LCS where
// Pierre calls its own diff.
import { parsePatch, type DiffFileModel, type Hunk, type Line } from './diff';
import { diffReviewLines, findDiffReviewLineIndex, type ReviewContents, type SelectionSide } from './diff-comments';

// ── Files and hunks ─────────────────────────────────────────────────────────

export type PrDiffSide = 'left' | 'right';
export type PrHunk = Hunk & { deletionStart: number; deletionCount: number; additionStart: number; additionCount: number };
/** One file of a slice: the parsed patch, its hunk ranges, and its patch lines by side. */
export type PrDiffFile = Omit<DiffFileModel, 'hunks'> & { name: string; hunks: PrHunk[]; additionLines: string[]; deletionLines: string[] };

function withRanges(hunk: Hunk): PrHunk {
  const deletionCount = hunk.lines.filter(line => line.kind !== 'addition').length, additionCount = hunk.lines.filter(line => line.kind !== 'deletion').length;
  return { ...hunk, deletionStart: hunk.oldStart, deletionCount, additionStart: hunk.newStart, additionCount };
}
function prFile(file: DiffFileModel): PrDiffFile {
  const hunks = file.hunks.map(withRanges);
  const lines = (kind: Line['kind']) => file.hunks.flatMap(hunk => hunk.lines.filter(line => line.kind === kind).map(line => line.text));
  return { ...file, name: file.path, hunks, additionLines: lines('addition'), deletionLines: lines('deletion') };
}

/** getRenderablePatch: the slice's files, or the raw text when nothing in it parses as files. */
export type RenderablePatch = { kind: 'files'; files: PrDiffFile[] } | { kind: 'raw'; text: string; reason: string } | null;
export function renderablePatch(patch: string, ignoreWhitespace: boolean): RenderablePatch {
  const normalized = patch.trim();
  if (normalized.length === 0) return null;
  try {
    const files = parsePatch(normalized).map(prFile).map(file => (ignoreWhitespace ? hideWhitespaceChanges(file) : file));
    return files.length > 0 ? { kind: 'files', files } : { kind: 'raw', text: normalized, reason: 'Unsupported diff format. Showing raw patch.' };
  } catch {
    return { kind: 'raw', text: normalized, reason: 'Failed to parse patch. Showing raw patch.' };
  }
}

/**
 * hideWhitespaceChanges: each hunk compared again with every whitespace character taken out, so a
 * line whose only change is its spacing reads as context (shown as the new line) and the counts
 * follow. A hunk too large to align (over a million line pairs) is left as the host sent it.
 */
export function hideWhitespaceChanges<T extends { hunks: PrHunk[]; additions: number; deletions: number }>(file: T): T {
  let additions = 0, deletions = 0;
  const hunks = file.hunks.map(hunk => {
    const oldSide = hunk.lines.filter(line => line.kind !== 'addition'), newSide = hunk.lines.filter(line => line.kind !== 'deletion');
    if (oldSide.length * newSide.length > 1_000_000) {
      additions += hunk.lines.filter(line => line.kind === 'addition').length; deletions += hunk.lines.filter(line => line.kind === 'deletion').length;
      return hunk;
    }
    const strip = (line: Line) => line.text.replace(/\s/g, '');
    const steps = alignLines(oldSide.map(strip), newSide.map(strip));
    const lines: Line[] = [];
    let removed: Line[] = [], added: Line[] = [];
    const flush = () => { lines.push(...removed, ...added); removed = []; added = []; };
    for (const step of steps) {
      if (step[0] === '=') {
        flush();
        const before = oldSide[step[1]]!, after = newSide[step[2]]!;
        lines.push({ kind: 'context', text: after.text, old: before.old, next: after.next });
      } else if (step[0] === '-') { const before = oldSide[step[1]]!; removed.push({ kind: 'deletion', text: before.text, old: before.old, next: 0 }); }
      else { const after = newSide[step[1]]!; added.push({ kind: 'addition', text: after.text, old: 0, next: after.next }); }
    }
    flush();
    additions += added.length + lines.filter(line => line.kind === 'addition').length;
    deletions += lines.filter(line => line.kind === 'deletion').length;
    return { ...hunk, lines };
  });
  return { ...file, hunks, additions, deletions };
}
type Step = ['=', number, number] | ['-', number] | ['+', number];
/** A longest-common-subsequence walk; on a tie the old line goes first, as git writes a change. */
function alignLines(a: readonly string[], b: readonly string[]): Step[] {
  const n = a.length, m = b.length, width = m + 1;
  const table = new Uint32Array((n + 1) * width);
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) {
    table[i * width + j] = a[i] === b[j] ? table[(i + 1) * width + j + 1]! + 1 : Math.max(table[(i + 1) * width + j]!, table[i * width + j + 1]!);
  }
  const steps: Step[] = [];
  let i = 0, j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) steps.push(['=', i++, j++]);
    else if (table[(i + 1) * width + j]! >= table[i * width + j + 1]!) steps.push(['-', i++]);
    else steps.push(['+', j++]);
  }
  while (i < n) steps.push(['-', i++]);
  while (j < m) steps.push(['+', j++]);
  return steps;
}

// ── pullRequestDiff.logic.ts ────────────────────────────────────────────────

type HunkRanges = { deletionStart: number; deletionCount: number; additionStart: number; additionCount: number };
/** Whether a conversation's line is really in this file's hunks (on the side it names). */
export function isLineInFileDiff(file: { hunks: readonly HunkRanges[] }, side: PrDiffSide, line: number): boolean {
  return file.hunks.some(hunk => (side === 'left'
    ? line >= hunk.deletionStart && line < hunk.deletionStart + hunk.deletionCount
    : line >= hunk.additionStart && line < hunk.additionStart + hunk.additionCount));
}
/** What the toolbar last asked of every file at once, null being the reader asking nothing yet. */
export type DiffFoldOverride = 'expanded' | 'folded' | null;
/** Whether a file is drawn folded: the reader's toggles are the difference from the toolbar's last word. */
export function isFileDiffCollapsed(fileKey: string, foldOverride: DiffFoldOverride, toggledFileKeys: ReadonlySet<string>): boolean {
  const foldedByDefault = foldOverride === 'folded';
  return toggledFileKeys.has(fileKey) ? !foldedByDefault : foldedByDefault;
}
/** The reader's fold choices after a file was ticked off (it folds) or put back (it opens). */
export function toggleFileDiffFoldForViewed(fileKey: string, viewed: boolean, foldOverride: DiffFoldOverride, toggledFileKeys: ReadonlySet<string>): ReadonlySet<string> {
  if (isFileDiffCollapsed(fileKey, foldOverride, toggledFileKeys) === viewed) return toggledFileKeys;
  const next = new Set(toggledFileKeys);
  if (next.has(fileKey)) next.delete(fileKey); else next.add(fileKey);
  return next;
}
/** lib/diffCollapse.ts areAllDiffFilesCollapsed. */
export function areAllDiffFilesCollapsed(fileKeys: readonly string[], collapsedFileKeys: ReadonlySet<string>): boolean {
  return fileKeys.length > 0 && fileKeys.every(fileKey => collapsedFileKeys.has(fileKey));
}

// ── pullRequestFilesViewed.logic.ts ─────────────────────────────────────────

export type PullRequestFileViewedState = 'unviewed' | 'viewed' | 'dismissed';
/** What the host last said about each file, by path. Absent means the host said nothing. */
export type FileViewedStates = ReadonlyMap<string, PullRequestFileViewedState>;
/** Presses the host has not confirmed yet, by path. */
export type FileViewedOverlay = ReadonlyMap<string, boolean>;
export function toFileViewedStates(result: { files: readonly { path: string; state: PullRequestFileViewedState }[] } | null): FileViewedStates | null {
  if (result === null) return null;
  return new Map(result.files.map(file => [file.path, file.state]));
}
/** `dismissed` (pushed to since it was cleared) reads as unseen. */
const isViewedState = (state: PullRequestFileViewedState | undefined): boolean => state === 'viewed';
/** Whether the file was cleared and has since moved, which the header says out loud. */
export function isStaleViewedState(state: PullRequestFileViewedState | undefined): boolean { return state === 'dismissed'; }
/** The press the reader made if it has not landed, and the host's answer otherwise. */
export function isFileViewed(path: string, states: FileViewedStates | null, overlay: FileViewedOverlay): boolean {
  const pressed = overlay.get(path);
  return pressed ?? isViewedState(states?.get(path));
}
export function countViewedFiles(paths: readonly string[], states: FileViewedStates | null, overlay: FileViewedOverlay): number {
  return paths.reduce((total, path) => (isFileViewed(path, states, overlay) ? total + 1 : total), 0);
}
/**
 * The overlay with everything the host has caught up on removed. `pending` presses the host cannot
 * have heard yet stay; `answered` paths go on the read that answered for them, whatever it says.
 */
export function settleFileViewedOverlay(overlay: FileViewedOverlay, states: FileViewedStates | null, pending: ReadonlySet<string>, answered: ReadonlySet<string>): FileViewedOverlay {
  if (states === null || overlay.size === 0) return overlay;
  const next = new Map(overlay);
  for (const [path, pressed] of overlay) {
    if (pending.has(path)) continue;
    if (answered.has(path) || isViewedState(states.get(path)) === pressed) next.delete(path);
  }
  return next.size === overlay.size ? overlay : next;
}
/** The overlay with a failed request's presses taken back, only the paths that request still `owned`. */
export function revertFileViewedOverlay(overlay: FileViewedOverlay, batch: readonly { path: string; viewed: boolean }[], owned: ReadonlySet<string>): FileViewedOverlay {
  const next = new Map(overlay);
  for (const { path, viewed } of batch) {
    if (!owned.has(path)) continue;
    if (next.get(path) === viewed) next.delete(path);
  }
  return next.size === overlay.size ? overlay : next;
}
/** The presses in an overlay as the batch the host is told about. */
export function toFileViewedBatch(overlay: FileViewedOverlay): { path: string; viewed: boolean }[] {
  return [...overlay].map(([path, viewed]) => ({ path, viewed }));
}

// ── pullRequestFileOrder.logic.ts ───────────────────────────────────────────

export type DiffFileTier = 'source' | 'test' | 'generated';
const GENERATED_FILE_NAMES = new Set(['pnpm-lock.yaml', 'yarn.lock', 'package-lock.json', 'bun.lockb', 'Cargo.lock', 'go.sum', 'composer.lock', 'Gemfile.lock']);
const GENERATED_DIRECTORIES = new Set(['__snapshots__', '__generated__', 'dist', 'build', 'vendor']);
const TEST_DIRECTORIES = new Set(['__tests__', 'tests', 'test']);
const MODULE_EXTENSIONS = ['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs'];
function diffFileTier(path: string): DiffFileTier {
  const segments = path.split('/'), name = segments.at(-1) ?? '';
  if (GENERATED_FILE_NAMES.has(name) || name.endsWith('.snap') || name.endsWith('.min.js') || name.endsWith('.min.css') || /\.generated\./.test(name)
    || segments.slice(0, -1).some(segment => GENERATED_DIRECTORIES.has(segment))) return 'generated';
  if (/\.(?:test|spec)\./.test(name) || segments.slice(0, -1).some(segment => TEST_DIRECTORIES.has(segment))) return 'test';
  return 'source';
}
function stripExtension(path: string): string { const dot = path.lastIndexOf('.'), slash = path.lastIndexOf('/'); return dot > slash ? path.slice(0, dot) : path; }
const baseName = (path: string) => stripExtension(path.split('/').at(-1) ?? '');
function resolveRelative(fromPath: string, specifier: string): string {
  const segments = fromPath.split('/').slice(0, -1);
  for (const part of specifier.split('/')) {
    if (part === '.' || part === '') continue;
    if (part === '..') segments.pop(); else segments.push(part);
  }
  return segments.join('/');
}
const IMPORT_SPECIFIER = /(?:\bfrom|\bimport|\brequire\s*\()\s*\(?\s*["']([^"']+)["']/g;
function importedPaths(path: string, lines: readonly string[], byModulePath: ReadonlyMap<string, string>, byBaseName: ReadonlyMap<string, string>): ReadonlySet<string> {
  const imported = new Set<string>();
  for (const line of lines) {
    IMPORT_SPECIFIER.lastIndex = 0;
    let match = IMPORT_SPECIFIER.exec(line);
    while (match !== null) {
      const specifier = match[1] ?? '';
      const withExtension = specifier.startsWith('.') ? resolveRelative(path, specifier) : specifier;
      const extension = MODULE_EXTENSIONS.find(candidate => withExtension.endsWith(candidate));
      const resolved = extension === undefined ? withExtension : withExtension.slice(0, -extension.length);
      const target = byModulePath.get(resolved) ?? byModulePath.get(`${resolved}/index`) ?? byBaseName.get(baseName(resolved));
      if (target !== undefined && target !== path) imported.add(target);
      match = IMPORT_SPECIFIER.exec(line);
    }
  }
  return imported;
}
/** Kahn's, always taking the lowest remaining path; a cycle falls back to path order. */
function orderByImports(paths: readonly string[], imports: ReadonlyMap<string, ReadonlySet<string>>): string[] {
  const remaining = new Set(paths), ordered: string[] = [];
  const sorted = [...paths].sort((left, right) => left.localeCompare(right));
  while (remaining.size > 0) {
    const candidates = sorted.filter(path => remaining.has(path));
    const next = candidates.find(path => [...(imports.get(path) ?? [])].every(dependency => !remaining.has(dependency))) ?? candidates[0]!;
    remaining.delete(next); ordered.push(next);
  }
  return ordered;
}
const testedBaseName = (path: string) => (path.split('/').at(-1) ?? '').replace(/\.(?:test|spec)\..*$/, '').replace(/\.[^.]+$/, '');
/** Diff files in reading order: source in dependency order, then the tests that cover it, then what a tool wrote. */
export function orderDiffFiles<T extends { name: string; additionLines: readonly string[]; deletionLines: readonly string[] }>(files: readonly T[]): T[] {
  const byPath = new Map<string, T>();
  for (const file of files) byPath.set(file.name, file);
  const tiers = new Map<string, DiffFileTier>();
  for (const path of byPath.keys()) tiers.set(path, diffFileTier(path));
  const sourcePaths = [...byPath.keys()].filter(path => tiers.get(path) === 'source');
  const byModulePath = new Map<string, string>(), ambiguous = new Set<string>(), byBaseName = new Map<string, string>();
  for (const path of sourcePaths) {
    byModulePath.set(stripExtension(path), path);
    const base = baseName(path);
    if (byBaseName.has(base)) ambiguous.add(base);
    byBaseName.set(base, path);
  }
  for (const base of ambiguous) byBaseName.delete(base);
  const imports = new Map<string, ReadonlySet<string>>();
  for (const path of sourcePaths) { const file = byPath.get(path)!; imports.set(path, importedPaths(path, [...file.additionLines, ...file.deletionLines], byModulePath, byBaseName)); }
  const orderedSource = orderByImports(sourcePaths, imports);
  const sourcePositions = new Map<string, number>();
  orderedSource.forEach((path, index) => { const base = baseName(path); if (!sourcePositions.has(base)) sourcePositions.set(base, index); });
  const orderedTests = [...byPath.keys()].filter(path => tiers.get(path) === 'test').sort((left, right) => {
    const leftPosition = sourcePositions.get(testedBaseName(left)) ?? Number.MAX_SAFE_INTEGER, rightPosition = sourcePositions.get(testedBaseName(right)) ?? Number.MAX_SAFE_INTEGER;
    return leftPosition - rightPosition || left.localeCompare(right);
  });
  const orderedGenerated = [...byPath.keys()].filter(path => tiers.get(path) === 'generated').sort((left, right) => left.localeCompare(right));
  return [...orderedSource, ...orderedTests, ...orderedGenerated].map(path => byPath.get(path)!);
}

// ── Review positions (reviewCommentContext.ts, PullRequestCodeTab.tsx) ──────

export type PullRequestReviewPosition = { kind: 'added'; newLine: number } | { kind: 'deleted'; oldLine: number } | { kind: 'context'; oldLine: number; newLine: number; side: PrDiffSide };
/** resolveDiffReviewPosition: the host's coordinates of the line a selection ends on, or null where there is no such row. */
export function resolveDiffReviewPosition(file: { hunks: Hunk[] }, lineNumber: number, side: SelectionSide | undefined, contents?: ReviewContents | null): PullRequestReviewPosition | null {
  const lines = diffReviewLines(file, contents);
  const index = findDiffReviewLineIndex(lines, lineNumber, side);
  const line = index < 0 ? undefined : lines[index];
  if (line === undefined) return null;
  if (line.change === 'add') return line.newLineNumber === null ? null : { kind: 'added', newLine: line.newLineNumber };
  if (line.change === 'delete') return line.oldLineNumber === null ? null : { kind: 'deleted', oldLine: line.oldLineNumber };
  return line.oldLineNumber === null || line.newLineNumber === null ? null : { kind: 'context', oldLine: line.oldLineNumber, newLine: line.newLineNumber, side: side === 'deletions' ? 'left' : 'right' };
}
/** getReviewPositionAnchor: the line and side a position is pinned under. */
export function reviewPositionAnchor(position: PullRequestReviewPosition): { line: number; side: PrDiffSide } {
  if (position.kind === 'added') return { line: position.newLine, side: 'right' };
  if (position.kind === 'deleted') return { line: position.oldLine, side: 'left' };
  return { line: position.side === 'left' ? position.oldLine : position.newLine, side: position.side };
}

// ── Thread comment pages (pullRequestDetail.logic.ts) ───────────────────────

/** The refreshed base comments, then each loaded comment once. */
export function mergePullRequestThreadComments<T extends { readonly id: string }>(base: readonly T[], loaded: readonly T[]): T[] {
  const seen = new Set(base.map(comment => comment.id));
  return [...base, ...loaded.filter(comment => { if (seen.has(comment.id)) return false; seen.add(comment.id); return true; })];
}
export function editPullRequestThreadComment<T extends { readonly id: string; readonly body: string }>(comments: readonly T[], commentId: string, body: string): T[] {
  return comments.map(comment => (comment.id === commentId ? { ...comment, body } : comment));
}

// ── Slices (PullRequestCodeTab sliceState) ──────────────────────────────────

export type OmittedFileStat = { path: string; additions: number; deletions: number };
/** One answer from the host: a whole number of files, and where the next one carries on. */
export type DiffSlice = { cursor: string | null; patch: string; truncated: boolean; nextCursor: string | null; omittedFileStats: OmittedFileStat[] };
const sameStats = (left: readonly OmittedFileStat[], right: readonly OmittedFileStat[]) =>
  left.length === right.length && left.every((file, index) => { const other = right[index]; return other !== undefined && other.path === file.path && other.additions === file.additions && other.deletions === file.deletions; });
/**
 * An answer for `cursor` added to the slices: a new one is appended; the same one again changes
 * nothing; a page that came back different replaces it and drops every slice after it (their
 * cursors were positions in the old diff).
 */
export function adoptDiffSlice(slices: readonly DiffSlice[], next: DiffSlice): readonly DiffSlice[] {
  const index = slices.findIndex(slice => slice.cursor === next.cursor);
  if (index === -1) return [...slices, next];
  const existing = slices[index]!;
  if (existing.patch === next.patch && existing.truncated === next.truncated && existing.nextCursor === next.nextCursor && sameStats(existing.omittedFileStats, next.omittedFileStats)) return slices;
  return [...slices.slice(0, index), next];
}
/** fnv1a32: a short hash of a slice's patch for its parse cache key. */
export function fnv1a32(text: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < text.length; index++) { hash ^= text.charCodeAt(index); hash = Math.imul(hash, 0x01000193) >>> 0; }
  return hash.toString(36);
}
