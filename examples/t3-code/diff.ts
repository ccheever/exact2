// Changes panel state and projection, adapted from T3 Code (MIT); see LICENSE-T3.
// Sources: apps/web/src/components/DiffPanel.tsx, diffPanelStore.ts, components/diffs/*
// and the served @pierre/diffs rendering (stacked/split rows, bars). The panel passes
// `lineDiffType: "none"` (DiffPanel.tsx), so changed words are not marked.
// The server owns every patch: turn diffs come from orchestration.getTurnDiff and
// working-tree/branch diffs from review.getDiffPreview. The tree (diff-tree.ts), large
// diffs and hidden lines (diff-lazy.ts) and line comments (diff-comments.ts) are
// projected here; their commands are diff-review.ts.
import { arr, obj, str, num, type Obj } from './domain';
import { diffFileTreeEntries, diffTreeRows, collectDirectoryPaths, allDirectoriesExpanded } from './diff-tree';
import { diffSource, lazyPatches, settledFileCount, fileState, type DiffSource, type LazyPatches, type FileContents, type Expansion } from './diff-lazy';
import { diffReviewLines, type ReviewLine, type SelectedLineRange } from './diff-comments';
import { ClientError } from './protocol';
import { peekVcsStatus } from './shell-vcs';
import type { T3Client } from './client';
import { messageTime } from './timeline-presentation';
import { lineTokens, overlay } from './timeline-diff-syntax';
import type { Token } from './timeline-highlight';

export type DiffSelection = { kind: 'unstaged' | 'branch' | 'turn'; runId: string; filePath: string };
/** View preferences mirror the reference defaults: stacked, wrapped, whitespace ignored, tree hidden, files collapsed. */
export class DiffState {
  selections: Record<string, DiffSelection> = {};
  layout: 'stacked' | 'split' = 'stacked';
  wrap = true;
  ignoreWhitespace = true;
  tree = false;
  expanded: Record<string, boolean> = {};
  /** Default diff file state (General → Behavior), seeded by settings-appearance-look.ts. */
  defaultExpanded = false;
  truncated = false;
  scopeKey = '';
  menu = '';
  /** The tree's closed folders by scope (trailing slash), and the file it shows selected. */
  treeCollapsed: Record<string, string[]> = {};
  treeSelected = '';
  /** The git source that answered (diff-lazy.ts), its per-file patches when it was too large, contents and opened hidden lines. */
  source: DiffSource | null = null;
  lazy: LazyPatches | null = null;
  contents: Record<string, FileContents> = {};
  expansions: Record<string, Expansion> = {};
  /** Line comments (diff-review.ts): the selected lines, the open draft, and the saved comments behind composer chips. */
  selection: { scope: string; path: string; range: SelectedLineRange } | null = null;
  draft: { scope: string; path: string; id: string; range: SelectedLineRange; rangeLabel: string } | null = null;
  saved: { contextId: string; scope: string; path: string; range: SelectedLineRange; rangeLabel: string; text: string }[] = [];
}
/** UI-only operations never touch the server and never mark a pending write uncertain. */
export const DIFF_LOCAL_OPS = ['diff-view', 'diffreview'];
export const ROW_LIMIT = 4000;

type Line = { kind: 'context' | 'addition' | 'deletion'; text: string; old: number; next: number };
type Hunk = { oldStart: number; newStart: number; lines: Line[] };
export type DiffFileModel = { path: string; previous: string; status: 'modified' | 'added' | 'deleted' | 'renamed'; additions: number; deletions: number; binary: boolean; hunks: Hunk[] };

function threadKey(client: T3Client): string { return `${client.environmentId}:${client.threadId}`; }
/** Checkpoints by run, newest turn first (deriveThreadCheckpointSummaries: every checkpoint with a run and a turn
 * ordinal, whatever its status, completed at its capture; DiffPanel orderedTurnDiffSummaries). A fork lists its inherited turns. */
export function turnSummaries(projection: Obj): { runId: string; count: number; completedAt: string }[] {
  const byRun = new Map<string, { runId: string; count: number; completedAt: string }>();
  for (const checkpoint of arr(projection.checkpoints)) {
    if (typeof checkpoint.runId !== 'string' || typeof checkpoint.appRunOrdinal !== 'number') continue;
    const run = arr(projection.runs).find(entry => entry.id === checkpoint.runId);
    byRun.set(checkpoint.runId, { runId: checkpoint.runId, count: checkpoint.appRunOrdinal,
      completedAt: str(checkpoint.capturedAt ?? run?.completedAt ?? checkpoint.updatedAt ?? checkpoint.createdAt) });
  }
  return [...byRun.values()].sort((a, b) => b.count - a.count || b.completedAt.localeCompare(a.completedAt));
}
export function currentSelection(client: T3Client): DiffSelection {
  // "branch" is the Changes view: everything this checkout changed since its base (upstream d1034d62b2).
  const saved = client.diffState.selections[threadKey(client)] ?? { kind: 'branch', runId: '', filePath: '' };
  if (saved.kind !== 'turn') return saved;
  const turns = turnSummaries(client.projection);
  // reconcileTurnSelection: a vanished turn falls back to the latest one.
  return turns.some(turn => turn.runId === saved.runId) || !turns.length ? saved : { ...saved, runId: turns[0]!.runId };
}
function select(client: T3Client, next: DiffSelection): void { client.diffState.selections[threadKey(client)] = next; }
/** Opening from a checkpoint selects that turn and names the file. The reference
 * (onOpenTurnDiff(runId, path)) leaves its files collapsed: 'Expand <file>' stays aria-expanded=false. */
export function selectCheckpoint(client: T3Client, ordinal: number, filePath = ''): void {
  const turn = turnSummaries(client.projection).find(entry => entry.count === ordinal);
  if (!turn) throw new ClientError('No completed checkpoint is available for this thread.');
  select(client, { kind: 'turn', runId: turn.runId, filePath });
}
export function selectScope(client: T3Client, value: string): void {
  client.diffState.menu = '';
  if (value === 'unstaged' || value === 'branch') return select(client, { kind: value, runId: '', filePath: '' });
  const turns = turnSummaries(client.projection);
  const runId = value === 'latest' ? turns[0]?.runId : value.startsWith('turn:') ? value.slice(5) : undefined;
  if (!runId || !turns.some(turn => turn.runId === runId)) throw new ClientError('That turn is no longer available.');
  select(client, { kind: 'turn', runId, filePath: '' });
}
function workspace(client: T3Client): string {
  const thread = obj(client.projection.thread);
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  return str(thread.worktreePath) || str(obj(thread.workspace).path) || str(project?.workspaceRoot);
}
function scopeKey(selection: DiffSelection): string { return selection.kind === 'turn' ? `turn:${selection.runId}` : selection.kind; }
export const NOT_GIT_REPO = 'Turn diffs are unavailable because this project is not a git repository.';
/** DiffPanel's !isGitRepo: the thread's workspace streams a status that is not a repository (unknown counts as one). */
export const diffNotGit = (client: T3Client): boolean => !!client.threadId && !!client.projection && peekVcsStatus(client, workspace(client))?.isRepo === false;
/** The request for the current selection, or an explanation when there is nothing to ask. */
export function diffRequest(client: T3Client): { method: string; payload: Obj; scope: string; source: string } {
  const selection = currentSelection(client), state = client.diffState;
  if (!client.threadId) throw new ClientError('Select a thread to inspect turn diffs.');
  // DiffPanel: isGitRepo (gitStatusQuery.data?.isRepo ?? true) gates every scope, turns included.
  if (diffNotGit(client)) throw new ClientError(NOT_GIT_REPO);
  if (selection.kind === 'turn') {
    const turn = turnSummaries(client.projection).find(entry => entry.runId === selection.runId);
    if (!turn) throw new ClientError('No completed turns yet.');
    return { method: 'orchestration.getTurnDiff', scope: scopeKey(selection), source: '',
      payload: { threadId: client.threadId, fromTurnCount: Math.max(0, turn.count - 1), toTurnCount: turn.count, ignoreWhitespace: state.ignoreWhitespace } };
  }
  const cwd = workspace(client);
  if (!cwd) throw new ClientError('Choose a project with a workspace to inspect its changes.');
  return { method: 'review.getDiffPreview', scope: scopeKey(selection), source: selection.kind === 'unstaged' ? 'working-tree' : 'branch-range',
    payload: { cwd, ignoreWhitespace: state.ignoreWhitespace } };
}
/** Adopts the server's answer for the scope that asked for it. */
export function adoptDiff(client: T3Client, request: { scope: string; source: string }, result: Obj): string {
  const state = client.diffState;
  state.scopeKey = request.scope;
  if (!request.source) { state.truncated = false; state.source = null; state.lazy = null; return str(result.diff); }
  const source = arr(result.sources).find(entry => entry.kind === request.source);
  state.truncated = source?.truncated === true;
  state.source = diffSource(result, request.source as 'working-tree' | 'branch-range');
  // useReviewFilePatches: a truncated source that lists its files is read file by file (diff-review.ts loads them).
  state.lazy = lazyPatches(state.source, state.ignoreWhitespace, state.lazy);
  return str(source?.diff);
}
export function diffView(client: T3Client, action: string, value: string, paths: string[]): void {
  const state = client.diffState, scope = state.scopeKey;
  if (action === 'menu') state.menu = client.diffOpen && ['scope', 'turns'].includes(value) && value !== state.menu ? value : '';
  else if (action === 'layout' && (value === 'stacked' || value === 'split')) state.layout = value;
  else if (action === 'wrap') state.wrap = !state.wrap;
  else if (action === 'tree') state.tree = !state.tree;
  else if (action === 'file') state.expanded[`${scope}::${value}`] = !(state.expanded[`${scope}::${value}`] ?? state.defaultExpanded);
  else if (action === 'reveal') state.expanded[`${scope}::${value}`] = true;
  else if (action === 'expand-all' || action === 'collapse-all') for (const path of paths) state.expanded[`${scope}::${path}`] = action === 'expand-all';
  else if (action === 'folder') {
    const closed = state.treeCollapsed[scope] ?? [];
    state.treeCollapsed[scope] = closed.includes(value) ? closed.filter(path => path !== value) : [...closed, value];
  } else if (action === 'folders') state.treeCollapsed[scope] = value === 'collapse' ? collectDirectoryPaths(paths) : [];
  else throw new ClientError('Unsupported diff view action.');
}

function header(path: string): string { return path.replace(/^"|"$/g, '').replace(/^[ab]\//, ''); }
/** Git unified patches (diff --git …) into files, hunks and numbered lines. */
export function parsePatch(patch: string): DiffFileModel[] {
  const files: DiffFileModel[] = [];
  let file: DiffFileModel | undefined, hunk: Hunk | undefined, old = 0, next = 0;
  const rawLines = patch.split('\n');
  for (const [lineIndex, raw] of rawLines.entries()) {
    if (raw.startsWith('diff --git ')) {
      const match = raw.match(/^diff --git (?:"?a\/)(.*?)"? (?:"?b\/)(.*?)"?$/);
      file = { path: match?.[2] ?? raw.slice(11), previous: match?.[1] ?? '', status: 'modified', additions: 0, deletions: 0, binary: false, hunks: [] };
      files.push(file); hunk = undefined; continue;
    }
    if (!file) continue;
    if (!hunk) {
      if (raw.startsWith('new file mode')) file.status = 'added';
      else if (raw.startsWith('deleted file mode')) file.status = 'deleted';
      else if (raw.startsWith('rename from ')) { file.status = 'renamed'; file.previous = raw.slice(12); }
      else if (raw.startsWith('rename to ')) file.path = raw.slice(10);
      else if (raw.startsWith('Binary files ')) file.binary = true;
      else if (raw.startsWith('--- ')) { if (raw === '--- /dev/null') file.status = 'added'; else file.previous = header(raw.slice(4)); }
      else if (raw.startsWith('+++ ')) { if (raw === '+++ /dev/null') file.status = 'deleted'; else file.path = header(raw.slice(4)); }
    }
    const range = raw.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (range) { old = Number(range[1]); next = Number(range[2]); hunk = { oldStart: old, newStart: next, lines: [] }; file.hunks.push(hunk); continue; }
    if (!hunk || raw.startsWith('\\')) continue;
    if (raw.startsWith('+')) { hunk.lines.push({ kind: 'addition', text: raw.slice(1), old: 0, next: next++ }); file.additions++; }
    else if (raw.startsWith('-')) { hunk.lines.push({ kind: 'deletion', text: raw.slice(1), old: old++, next: 0 }); file.deletions++; }
    // A bare empty line is a context row whose space was stripped, except the patch terminator.
    else if (raw.startsWith(' ') || raw === '' && lineIndex < rawLines.length - 1) hunk.lines.push({ kind: 'context', text: raw.slice(1), old: old++, next: next++ });
  }
  // The panel lists files by path, ignoring case, as the served viewer and its tree do.
  return files.sort((a, b) => a.path.toLowerCase().localeCompare(b.path.toLowerCase()));
}

type Segment = { id: string; text: string; mark: boolean; syntax: string };
const tokens = (text: string) => text.match(/\w+|\s+|[^\w\s]/g) ?? [];
/** Word marks for a replaced line pair: the changed middle between shared prefix and suffix tokens. */
export function wordMarks(before: string, after: string): [Segment[], Segment[]] {
  const a = tokens(before), b = tokens(after);
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let end = 0;
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end++;
  const shared = [...a.slice(0, start), ...a.slice(a.length - end)].some(token => /\S/.test(token));
  const split = (list: string[]): Segment[] => {
    if (!shared) return [{ id: '0', text: list.join(''), mark: false, syntax: '' }];
    return [{ id: '0', text: list.slice(0, start).join(''), mark: false, syntax: '' }, { id: '1', text: list.slice(start, list.length - end).join(''), mark: true, syntax: '' },
      { id: '2', text: list.slice(list.length - end).join(''), mark: false, syntax: '' }].filter(segment => segment.text !== '');
  };
  return [split(a), split(b)];
}
/** A drawn line: its tone and number, and the line it is on each side (0 for none), for line selection and comments. */
type Row = { tone: string; number: string; segments: Segment[]; oldLine: number; newLine: number };
const plain = (text: string): Segment[] => [{ id: '0', text, mark: false, syntax: '' }];
/** One hunk as stacked rows and split pairs (no word marks: `lineDiffType: "none"`). */
function hunkRows(hunk: Hunk, path: string): { stacked: Row[]; split: { left: Row; right: Row }[] } {
  const stacked: Row[] = [], split: { left: Row; right: Row }[] = [];
  const empty: Row = { tone: 'empty', number: '', segments: [], oldLine: 0, newLine: 0 };
  // Each side is highlighted as one text (timeline-diff-syntax.ts).
  const oldSide = hunk.lines.filter(line => line.kind !== 'addition'), newSide = hunk.lines.filter(line => line.kind !== 'deletion');
  const oldTokens = lineTokens(oldSide.map(line => line.text), path), newTokens = lineTokens(newSide.map(line => line.text), path);
  const syntax = new Map<Line, { old: Token[]; next: Token[] }>();
  oldSide.forEach((line, at) => syntax.set(line, { old: oldTokens[at] ?? [], next: [] }));
  newSide.forEach((line, at) => syntax.set(line, { old: syntax.get(line)?.old ?? [], next: newTokens[at] ?? [] }));
  const paint = (line: Line, side: 'old' | 'next') => overlay(plain(line.text), syntax.get(line)?.[side] ?? []);
  for (let index = 0; index < hunk.lines.length;) {
    const line = hunk.lines[index]!;
    if (line.kind === 'context') {
      stacked.push({ tone: 'context', number: String(line.next), segments: paint(line, 'next'), oldLine: line.old, newLine: line.next });
      split.push({ left: { tone: 'context', number: String(line.old), segments: paint(line, 'old'), oldLine: line.old, newLine: 0 },
        right: { tone: 'context', number: String(line.next), segments: paint(line, 'next'), oldLine: 0, newLine: line.next } });
      index++; continue;
    }
    const deletions: Line[] = [], additions: Line[] = [];
    while (hunk.lines[index]?.kind === 'deletion') deletions.push(hunk.lines[index++]!);
    while (hunk.lines[index]?.kind === 'addition') additions.push(hunk.lines[index++]!);
    const left = deletions.map(entry => ({ tone: 'deletion', number: String(entry.old), segments: paint(entry, 'old'), oldLine: entry.old, newLine: 0 }));
    const right = additions.map(entry => ({ tone: 'addition', number: String(entry.next), segments: paint(entry, 'next'), oldLine: 0, newLine: entry.next }));
    stacked.push(...left, ...right);
    for (let at = 0; at < Math.max(left.length, right.length); at++) split.push({ left: left[at] ?? empty, right: right[at] ?? empty });
  }
  return { stacked, split };
}

const statusLetter: Record<string, string> = { modified: 'M', added: 'A', deleted: 'D', renamed: 'R' };
type DiffItemView = { id: string; kind: string; path: string; name: string; status: string; letter: string; additions: number; deletions: number;
  expanded: boolean; markdown: boolean; tone: string; number: string; segments: Segment[]; leftTone: string; leftNumber: string; leftSegments: Segment[];
  rightTone: string; rightNumber: string; rightSegments: Segment[]; label: string; gutter: number;
  /** Line items: the side and line a press on the gutter selects (split: left deletions, right additions), and whether it is selected. */
  side: string; line: number; selected: boolean; leftLine: number; rightLine: number; leftSelected: boolean; rightSelected: boolean;
  /** File headers: Retry, the partial mark, a header whose patch is not there (chevron off). Gaps: a press opens hidden lines. */
  error: boolean; partial: boolean; unavailable: boolean; expandable: boolean;
  /** Comment items (`note`, `draft`): the entry's id (a saved one's context id) and text. */
  entry: string; text: string };
type FileView = DiffFileModel & { pending: boolean };

/** The files the panel lists: the parsed patch, or a large source's per-file stats with each patch once it answered. */
export function diffFiles(client: T3Client): FileView[] {
  const lazy = client.diffState.lazy;
  if (!lazy) return parsePatch(client.diffText).map(file => ({ ...file, pending: false }));
  return lazy.files.map(stat => {
    const patch = lazy.patches.get(stat.path);
    const parsed = patch?.state === 'loaded' ? parsePatch(patch.diff).find(file => file.path === stat.path) : undefined;
    const base: DiffFileModel = parsed ?? { path: stat.path, previous: stat.previousPath ?? '', status: stat.previousPath ? 'renamed' : 'modified', additions: 0, deletions: 0, binary: false, hunks: [] };
    return { ...base, additions: stat.additions, deletions: stat.deletions, pending: !patch || patch.state === 'loading' };
  });
}
/** Where a file's contents are kept: per source revision, so a refresh reads them again. */
export function contentsKey(state: DiffState, path: string): string { return `${state.source?.diffHash ?? ''}::${state.scopeKey}::${path}`; }
/** The rows the review comments number (diff-comments.ts), whole-file once the contents are in. */
export function reviewLinesOf(state: DiffState, file: DiffFileModel): ReviewLine[] {
  const contents = state.contents[contentsKey(state, file.path)];
  return diffReviewLines(file, contents?.state === 'loaded' ? contents : null);
}
/** The review section of the selection (DiffPanel reviewSectionId / reviewSectionTitle). */
export function reviewSection(client: T3Client): { id: string; title: string } {
  const selection = currentSelection(client);
  if (selection.kind !== 'turn') return { id: selection.kind, title: selection.kind === 'unstaged' ? 'Uncommitted' : 'Changes' };
  const turn = turnSummaries(client.projection).find(entry => entry.runId === selection.runId);
  return { id: `turn:${selection.runId}`, title: `Turn ${turn?.count ?? '?'}` };
}
const sideKey = (side: string, line: number) => `${side === 'deletions' ? 'd' : 'a'}:${line}`;
function lineIndex(lines: ReviewLine[]): Map<string, number> {
  const index = new Map<string, number>();
  lines.forEach((line, at) => {
    if (line.change !== 'add' && line.oldLineNumber !== null && !index.has(sideKey('deletions', line.oldLineNumber))) index.set(sideKey('deletions', line.oldLineNumber), at);
    if (line.change !== 'delete' && line.newLineNumber !== null && !index.has(sideKey('additions', line.newLineNumber))) index.set(sideKey('additions', line.newLineNumber), at);
  });
  return index;
}

/** The snapshot's panel fields: header labels, menus, the tree, one flat virtualized list of file, line, gap and comment items. */
export function diffSnapshot(client: T3Client, now: number) {
  const state = client.diffState, selection = currentSelection(client), turns = turnSummaries(client.projection);
  // Outside git nothing is listed, not even files a fetch before the status arrived returned.
  const notGit = diffNotGit(client), files = notGit ? [] : diffFiles(client), lazy = state.lazy, scope = state.scopeKey, section = reviewSection(client);
  const gitSource = selection.kind !== 'turn' && state.source !== null;
  const turn = turns.find(entry => entry.runId === selection.runId);
  const scopeLabel = selection.kind === 'unstaged' ? 'Uncommitted' : selection.kind === 'branch' ? 'Changes'
    : turn && turn.runId === turns[0]?.runId ? 'Latest turn' : `Turn ${turn?.count ?? '?'}`;
  const items: DiffItemView[] = [];
  let rows = 0;
  const base = { tone: '', number: '', segments: [] as Segment[], leftTone: '', leftNumber: '', leftSegments: [] as Segment[], rightTone: '', rightNumber: '', rightSegments: [] as Segment[],
    label: '', gutter: 33.3, name: '', status: '', letter: '', additions: 0, deletions: 0, expanded: false, markdown: false, side: '', line: 0, selected: false, leftLine: 0, rightLine: 0,
    leftSelected: false, rightSelected: false, error: false, partial: false, unavailable: false, expandable: false, entry: '', text: '' };
  // A draft or the selected lines belong to the scope they were made in; saved comments stay while their chip is in the prompt.
  const draft = state.draft?.scope === scope ? state.draft : null, picked = state.selection?.scope === scope ? state.selection : null;
  const saved = state.saved.filter(entry => entry.scope === scope && client.draft.includes(`review-comment/${entry.contextId})`));
  for (const file of files) {
    // Lazy files show once answered; the rest wait behind the loading boundary (DiffPanel codeViewFiles).
    if (lazy && file.pending) continue;
    const status = lazy ? fileState(lazy, file.path) : { error: false, truncated: false };
    const unavailable = lazy !== null && lazy.patches.get(file.path)?.state !== 'loaded';
    const expanded = !unavailable && (state.expanded[`${scope}::${file.path}`] ?? state.defaultExpanded) === true;
    const digits = Math.max(1, ...file.hunks.flatMap(hunk => hunk.lines.map(line => String(Math.max(line.old, line.next)).length)));
    // Numbers sit right-aligned after a 2ch inset, 1ch + 2pt before the code (measured 33.3pt for one digit).
    const gutter = Math.round((digits * 7.8267 + 15.6533 + 9.83) * 10) / 10;
    items.push({ ...base, id: `file:${file.path}`, kind: 'file', path: file.path, name: file.path, status: file.status, letter: statusLetter[file.status] ?? 'M',
      additions: file.additions, deletions: file.deletions, expanded, markdown: /\.(md|mdx|markdown)$/i.test(file.path), error: status.error, partial: status.truncated, unavailable });
    if (!expanded) continue;
    if (file.binary) { items.push({ ...base, id: `binary:${file.path}`, kind: 'gap', path: file.path, expanded, label: 'Binary file not shown' }); continue; }
    const contents = state.contents[contentsKey(state, file.path)];
    const newLines = contents?.state === 'loaded' ? contents.newContents.split('\n') : [];
    const review = reviewLinesOf(state, file), at = lineIndex(review);
    const span = (range: SelectedLineRange | undefined) => {
      if (!range) return null;
      const from = at.get(sideKey(range.side, range.start)), to = at.get(sideKey(range.endSide, range.end));
      return from === undefined || to === undefined ? null : [Math.min(from, to), Math.max(from, to)] as const;
    };
    const chosen = span(draft?.path === file.path ? draft.range : picked?.path === file.path ? picked.range : undefined);
    const isSelected = (side: string, line: number) => { const index = line > 0 ? at.get(sideKey(side, line)) : undefined; return !!chosen && index !== undefined && index >= chosen[0] && index <= chosen[1]; };
    // Comments sit under the range's end line, on its side; one card per line, entries stacked (AnnotatableCodeView).
    const notes = [...saved.filter(entry => entry.path === file.path).map(entry => ({ kind: 'note', id: entry.contextId, range: entry.range, label: entry.rangeLabel, text: entry.text })),
      ...(draft?.path === file.path ? [{ kind: 'draft', id: draft.id, range: draft.range, label: draft.rangeLabel, text: '' }] : [])];
    const placed = new Set<string>();
    const annotate = (oldLine: number, newLine: number) => {
      for (const note of notes) {
        if (placed.has(note.id)) continue;
        const anchored = note.range.endSide === 'deletions' ? oldLine > 0 && oldLine === note.range.end : newLine > 0 && newLine === note.range.end;
        if (!anchored) continue;
        placed.add(note.id);
        items.push({ ...base, id: `${note.kind}:${file.path}:${note.id}`, kind: note.kind, path: file.path, expanded, entry: note.id, label: note.label, text: note.text });
      }
    };
    const push = (row: Row | { left: Row; right: Row }, key: string) => {
      if (++rows > ROW_LIMIT) return;
      const common = { ...base, id: `line:${file.path}:${key}`, path: file.path, expanded, gutter };
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
    // Unchanged lines a hidden range has opened (Pierre's expandHunk), drawn as context rows.
    const context = (newLine: number, delta: number, key: string) => {
      const text = newLines[newLine - 1] ?? '', oldLine = newLine - delta;
      push(state.layout === 'split' ? { left: { tone: 'context', number: String(oldLine), segments: plain(text), oldLine, newLine: 0 }, right: { tone: 'context', number: String(newLine), segments: plain(text), oldLine: 0, newLine } }
        : { tone: 'context', number: String(newLine), segments: plain(text), oldLine, newLine }, key);
    };
    const hidden = (count: number, gapIndex: number, newFirst: number, delta: number, trailing: boolean) => {
      if (count <= 0) return;
      const key = `${scope}::${file.path}::${gapIndex}`, opened = state.expansions[key] ?? { fromStart: 0, fromEnd: 0 };
      const fromStart = Math.min(count, opened.fromStart), fromEnd = Math.min(count - fromStart, opened.fromEnd), left = count - fromStart - fromEnd;
      for (let k = 0; k < fromStart; k++) context(newFirst + k, delta, `x${gapIndex}:${k}`);
      const failed = contents?.state === 'error';
      if (left > 0) items.push({ ...base, id: `gap:${file.path}:${gapIndex}`, kind: 'gap', path: file.path, expanded, line: gapIndex, expandable: gitSource && !failed,
        label: failed ? contents.error || 'Unable to load file contents.' : trailing && left === count && !contents ? 'More unchanged context may be available' : `${left} unmodified ${left === 1 ? 'line' : 'lines'}` });
      for (let k = left + fromStart; k < count; k++) context(newFirst + k, delta, `x${gapIndex}:${k}`);
    };
    let oldNext = 1, newNext = 1;
    file.hunks.forEach((hunk, hunkIndex) => {
      const oldCount = hunk.lines.filter(line => line.kind !== 'addition').length, newCount = hunk.lines.filter(line => line.kind !== 'deletion').length;
      const oldStart = hunk.oldStart + (oldCount === 0 ? 1 : 0), newStart = hunk.newStart + (newCount === 0 ? 1 : 0);
      hidden(newStart - newNext, hunkIndex, newNext, newStart - oldStart, false);
      const { stacked, split } = hunkRows(hunk, file.path);
      (state.layout === 'split' ? split : stacked).forEach((row, index) => push(row, `${hunkIndex}:${index}`));
      oldNext = oldStart + oldCount; newNext = newStart + newCount;
    });
    // After the last hunk: known once the contents are in.
    if (contents?.state === 'loaded' && file.hunks.length > 0) hidden(Math.max(0, newLines.length - (contents.newContents.endsWith('\n') ? 1 : 0) - newNext + 1), file.hunks.length, newNext, newNext - oldNext, true);
    // The code block ends with 8pt of padding before the next file header.
    items.push({ ...base, id: `pad:${file.path}`, kind: 'pad', path: file.path, expanded });
  }
  // DiffFileLoadingBoundary: up to four header skeletons while files remain.
  const remaining = lazy ? lazy.files.length - settledFileCount(lazy) : 0;
  for (let index = 0; index < Math.min(remaining, 4); index++) items.push({ ...base, id: `ghost:${index}`, kind: 'ghost', path: '' });
  const totals = selection.kind !== 'turn' && state.source?.files ? state.source.files : files;
  const entries = diffFileTreeEntries(files.map(file => ({ path: file.path, status: file.status })));
  const closed = new Set(state.treeCollapsed[scope] ?? []);
  return {
    diffScopeLabel: scopeLabel, diffMenu: client.diffOpen ? state.menu : '', diffScope: selection.kind === 'turn' ? `turn:${selection.runId}` : selection.kind,
    diffTurns: turns.map(entry => ({ id: `turn:${entry.runId}`, label: `Turn ${entry.count}`, time: messageTime(entry.completedAt, now, client.local.deviceSettings.timestampFormat), selected: selection.kind === 'turn' && selection.runId === entry.runId })),
    diffLatestSelected: selection.kind === 'turn' && selection.runId === turns[0]?.runId,
    diffCanRefresh: selection.kind !== 'turn' && !notGit, diffLayout: state.layout, diffWrap: state.wrap, diffIgnoreWhitespace: state.ignoreWhitespace, diffTree: state.tree,
    // The size banner shows only for a truncated preview the panel cannot read file by file (older servers).
    // A project outside git reads as the centred empty state, as DiffPanel's !isGitRepo branch, not as an error.
    diffTruncated: (state.truncated && !lazy) || rows > ROW_LIMIT, diffEmpty: notGit || (!client.diffLoading && (!client.diffError || client.diffError === NOT_GIT_REPO) && files.length === 0),
    diffEmptyLabel: notGit ? NOT_GIT_REPO : client.diffText.trim() || lazy ? 'No patch available for this selection.' : 'No net changes in this selection.',
    diffAdditions: totals.reduce((sum, file) => sum + file.additions, 0), diffDeletions: totals.reduce((sum, file) => sum + file.deletions, 0),
    diffAllCollapsed: files.every(file => (state.expanded[`${scope}::${file.path}`] ?? state.defaultExpanded) !== true),
    diffFiles: files.map(file => ({ id: file.path, name: file.path.split('/').pop() ?? file.path, path: file.path, status: file.status, letter: statusLetter[file.status] ?? 'M',
      additions: file.additions, deletions: file.deletions, markdown: /\.(md|mdx|markdown)$/i.test(file.path) })),
    diffTreeRows: diffTreeRows(entries, closed, state.treeSelected), diffTreeLabel: `${section.title} files`,
    diffTreeFolders: collectDirectoryPaths(entries.map(entry => entry.path)).length > 0, diffTreeAllOpen: allDirectoriesExpanded(entries.map(entry => entry.path), closed),
    diffItems: items, diffCommentOpen: draft !== null,
    diffLoadingLabel: selection.kind === 'turn' ? 'Loading checkpoint diff...' : selection.kind === 'unstaged' ? 'Loading uncommitted changes...' : 'Loading changes...',
  };
}
export function diffPaths(client: T3Client): string[] { return diffFiles(client).map(file => file.path); }
export function turnNumber(projection: Obj, ordinal: number): boolean { return turnSummaries(projection).some(turn => turn.count === num(ordinal, -1)); }
