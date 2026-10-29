import { DiffFind, markDiffRows, findGap } from './diff-find';
// Changes panel state and projection, adapted from T3 Code (MIT); see LICENSE-T3.
// Sources (74541bab17): apps/web/src/components/DiffPanel.tsx, diffPanelStore.ts, components/diffs/*
// and the served @pierre/diffs rendering (stacked/split rows, bars). DiffPanel.tsx passes
// `lineDiffType: "none"`, but the served panel still marks changed words with Pierre's default
// "word-alt" (measured on the 9c161462c9 oracle), so a replaced line pair carries its marks.
// The server owns every patch: turn diffs come from orchestration.getTurnDiff and
// working-tree/branch diffs from review.getDiffPreview. The tree (diff-tree.ts), large
// diffs and hidden lines (diff-lazy.ts) and line comments (diff-comments.ts) are
// projected here; their commands are diff-review.ts.
import { arr, obj, str, num, type Obj } from '../client/domain';
import { diffFileTreeEntries, diffTreeRows, collectDirectoryPaths, allDirectoriesExpanded, ancestorDirectories } from './diff-tree';
import { diffSource, lazyPatches, settledFileCount, fileState, changeType, type DiffSource, type LazyPatches, type FileContents, type Expansion } from './diff-lazy';
import { diffReviewLines, type ReviewLine, type SelectedLineRange } from './diff-comments';
import { AUTOMATIC_BASE_REF, baseRefView, emptyBaseRefPicker, normalizeBaseRef, type BaseRefPicker } from './diff-base-ref';
import { compactCount } from '../timeline/timeline-tree';
import { ClientError } from '../client/protocol';
import { peekVcsStatus } from '../details/shell-vcs';
import type { T3Client } from '../client/client';
import { messageTime } from '../timeline/timeline-presentation';
import { lineTokens, overlay } from '../highlight/timeline-diff-syntax';
import { wordAltMarks } from '../appearance/settings-font-previews'; // Pierre's word-alt marks, ported
import type { Token } from '../highlight/timeline-highlight';
import type { HighlightView } from '../highlight/highlights';
import { utilityPlacement, type LineDrag } from './diff-line-drag';

/** `baseRef`: the Changes scope's comparison target, null for Automatic (diffPanelStore's branch selection). `reveal`: a turn's
 * revealRequestId, which each open of a turn diff bumps so its named file is revealed again. */
export type DiffSelection = { kind: 'unstaged' | 'branch' | 'turn'; runId: string; filePath: string; baseRef: string | null; reveal?: number };
/** View preferences mirror the reference defaults: stacked, wrapped, whitespace ignored, tree hidden, files collapsed. */
export class DiffState {
  find = new DiffFind();
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
  /** The tree's closed folders by scope (trailing slash), and the file it shows selected. */
  treeCollapsed: Record<string, string[]> = {};
  treeSelected = '';
  /** The named file of a turn diff last revealed (its scope, path and reveal request), and the reveal the list scrolls to. */
  revealed = '';
  reveal = 0;
  /** The git source that answered (diff-lazy.ts), its per-file patches when it was too large, contents and opened hidden lines. */
  source: DiffSource | null = null;
  lazy: LazyPatches | null = null;
  contents: Record<string, FileContents> = {};
  expansions: Record<string, Expansion> = {};
  /** Line comments (diff-review.ts): the selected lines, the open draft, and the saved comments behind composer chips. */
  selection: { scope: string; path: string; range: SelectedLineRange } | null = null;
  /** The gutter's drag in progress (diff-line-drag.ts), in the selection's scope. */
  drag: LineDrag | null = null;
  /** `previous` and `status`: the file's old path and change type as the draft opened, which name its file (AnnotatableCodeView's fileKey). */
  draft: { scope: string; path: string; previous: string; status: string; id: string; range: SelectedLineRange; rangeLabel: string } | null = null;
  saved: { contextId: string; scope: string; path: string; range: SelectedLineRange; rangeLabel: string; text: string }[] = [];
  /** The thread whose open draft's textarea took the focus ('' when it let go): the composer's send chords then leave ⌘↩ to it
   * (composer-presentation.ts). The host sends no blur when a focused view leaves the tree (LLP 1008), so it counts only
   * on that thread and while the draft's card is drawn (draftHoldsCommandEnter). */
  draftFocus = '';
  /** diffPanelStore branchBaseRefByThreadKey: each thread's comparison target, kept while another scope shows. */
  branchBaseRefs: Record<string, string | null> = {};
  /** The comparison target picker's ref lists (diff-base-ref.ts). */
  baseRefs: BaseRefPicker = emptyBaseRefPicker();
}
/** UI-only operations never touch the server and never mark a pending write uncertain. */
export const DIFF_LOCAL_OPS = ['diff-view', 'diffreview', 'diffbase'];
export const ROW_LIMIT = 4000;

export type Line = { kind: 'context' | 'addition' | 'deletion'; text: string; old: number; next: number };
export type Hunk = { oldStart: number; newStart: number; lines: Line[] };
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
  const saved = client.diffState.selections[threadKey(client)] ?? { kind: 'branch', runId: '', filePath: '', baseRef: client.diffState.branchBaseRefs[threadKey(client)] ?? null };
  if (saved.kind !== 'turn') return saved;
  const turns = turnSummaries(client.projection);
  // reconcileTurnSelection: a vanished turn falls back to the latest one.
  return turns.some(turn => turn.runId === saved.runId) || !turns.length ? saved : { ...saved, runId: turns[0]!.runId };
}
function select(client: T3Client, next: DiffSelection): void { client.diffState.selections[threadKey(client)] = next; }
/** diffPanelStore selectTurn: a named file (blank is none) and the next reveal request. */
function selectTurnFile(client: T3Client, runId: string, filePath: string): void {
  const previous = client.diffState.selections[threadKey(client)];
  select(client, { kind: 'turn', runId, filePath: filePath.trim(), baseRef: null, reveal: previous?.kind === 'turn' ? (previous.reveal ?? 0) + 1 : 1 });
}
/** Opening from a checkpoint selects that turn and names the file, which the panel then expands and scrolls to (diffSnapshot). */
export function selectCheckpoint(client: T3Client, ordinal: number, filePath = ''): void {
  const turn = turnSummaries(client.projection).find(entry => entry.count === ordinal);
  if (!turn) throw new ClientError('No completed checkpoint is available for this thread.');
  selectTurnFile(client, turn.runId, filePath);
}
/** onOpenTurnDiff(runId, filePath): a file change's Open diff in the work-log inspector. */
export function selectTurn(client: T3Client, runId: string, filePath = ''): void {
  if (!runId) throw new ClientError('That turn is no longer available.');
  selectTurnFile(client, runId, filePath);
}
/** selectGitScope: Changes takes back the thread's comparison target; leaving Changes keeps it for later. */
export function selectScope(client: T3Client, value: string): void {
  if (value === 'unstaged' || value === 'branch') {
    const key = threadKey(client), previous = client.diffState.selections[key];
    if (previous?.kind === 'branch') client.diffState.branchBaseRefs[key] = previous.baseRef;
    const baseRef = previous?.kind === 'branch' ? previous.baseRef : client.diffState.branchBaseRefs[key] ?? null;
    return select(client, { kind: value, runId: '', filePath: '', baseRef: value === 'branch' ? baseRef : null });
  }
  const turns = turnSummaries(client.projection);
  const runId = value === 'latest' ? turns[0]?.runId : value.startsWith('turn:') ? value.slice(5) : undefined;
  if (!runId || !turns.some(turn => turn.runId === runId)) throw new ClientError('That turn is no longer available.');
  select(client, { kind: 'turn', runId, filePath: '', baseRef: null });
}
/** selectBranchBaseRef: the comparison target (blank or Automatic is null) selects Changes and is kept for the thread. */
export function selectBaseRef(client: T3Client, value: string): void {
  const baseRef = value === AUTOMATIC_BASE_REF ? null : normalizeBaseRef(value), key = threadKey(client);
  client.diffState.branchBaseRefs[key] = baseRef;
  select(client, { kind: 'branch', runId: '', filePath: '', baseRef });
}
function workspace(client: T3Client): string {
  const thread = obj(client.projection.thread);
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  return str(thread.worktreePath) || str(obj(thread.workspace).path) || str(project?.workspaceRoot);
}
/**
 * DiffPanel's collapseScopeKey: the thread and its review section. Folded files, opened lines and drafts are kept by it
 * for the session (diffPanelStore useDiffPanelSessionStore), so they survive switching right-panel tabs or threads.
 */
function scopeKey(client: T3Client, selection: DiffSelection): string { return `${threadKey(client)}:${selection.kind === 'turn' ? `turn:${selection.runId}` : selection.kind}`; }
/** The current selection has an answer (the query's data): a refresh then keeps it on show instead of the loading state. */
export function diffAnswered(client: T3Client): boolean { return client.diffState.scopeKey === scopeKey(client, currentSelection(client)); }
export const NOT_GIT_REPO = 'Turn diffs are unavailable because this project is not a git repository.';
/** DiffPanel's !isGitRepo: the thread's workspace streams a status that is not a repository (unknown counts as one). */
export const diffNotGit = (client: T3Client): boolean => !!client.threadId && !!client.projection && peekVcsStatus(client, workspace(client))?.isRepo === false;
/** gitStatusQuery.data?.isRepo: whether the thread's workspace is a repository, undefined until its status streams. */
export function gitRepoState(client: T3Client): boolean | undefined {
  const isRepo = peekVcsStatus(client, workspace(client))?.isRepo;
  return typeof isRepo === 'boolean' ? isRepo : undefined;
}
/** The request for the current selection, or an explanation when there is nothing to ask. */
export function diffRequest(client: T3Client): { method: string; payload: Obj; scope: string; source: string } {
  const selection = currentSelection(client), state = client.diffState;
  if (!client.threadId) throw new ClientError('Select a thread to inspect turn diffs.');
  // DiffPanel: isGitRepo (gitStatusQuery.data?.isRepo ?? true) gates every scope, turns included.
  if (diffNotGit(client)) throw new ClientError(NOT_GIT_REPO);
  if (selection.kind === 'turn') {
    const turn = turnSummaries(client.projection).find(entry => entry.runId === selection.runId);
    if (!turn) throw new ClientError('No completed turns yet.');
    return { method: 'orchestration.getTurnDiff', scope: scopeKey(client, selection), source: '',
      payload: { threadId: client.threadId, fromTurnCount: Math.max(0, turn.count - 1), toTurnCount: turn.count, ignoreWhitespace: state.ignoreWhitespace } };
  }
  const cwd = workspace(client);
  if (!cwd) throw new ClientError('Choose a project with a workspace to inspect its changes.');
  return { method: 'review.getDiffPreview', scope: scopeKey(client, selection), source: selection.kind === 'unstaged' ? 'working-tree' : 'branch-range',
    payload: { cwd, ...(selection.kind === 'branch' && selection.baseRef ? { baseRef: selection.baseRef } : {}), ignoreWhitespace: state.ignoreWhitespace } };
}
/** Adopts the server's answer for the scope that asked for it. */
export function adoptDiff(client: T3Client, request: { scope: string; source: string }, result: Obj): string {
  const state = client.diffState;
  state.scopeKey = request.scope;
  // Opened file contents are read again for a scope that shows later; only the shown scope's are kept.
  for (const key of Object.keys(state.contents)) if (!key.includes(`::${request.scope}::`)) delete state.contents[key];
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
  if (action === 'layout' && (value === 'stacked' || value === 'split')) state.layout = value;
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

export type Segment = { id: string; text: string; mark: boolean; syntax: string };
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
export type Row = { tone: string; number: string; segments: Segment[]; oldLine: number; newLine: number };
export const plain = (text: string): Segment[] => [{ id: '0', text, mark: false, syntax: '' }];
/**
 * One hunk as stacked rows and split pairs; the pull request Code tab draws the same rows (pages-pr-code-rows.ts).
 * With `marks`, each deletion and the addition at its index in a change block carry word-alt marks (wordAltMarks).
 */
export function hunkRows(hunk: Hunk, path: string, marks = false, view: HighlightView | null = null): { stacked: Row[]; split: { left: Row; right: Row }[] } {
  const stacked: Row[] = [], split: { left: Row; right: Row }[] = [];
  const empty: Row = { tone: 'empty', number: '', segments: [], oldLine: 0, newLine: 0 };
  // Each side is highlighted as one text (timeline-diff-syntax.ts).
  const oldSide = hunk.lines.filter(line => line.kind !== 'addition'), newSide = hunk.lines.filter(line => line.kind !== 'deletion');
  const oldTokens = lineTokens(oldSide.map(line => line.text), path, view), newTokens = lineTokens(newSide.map(line => line.text), path, view);
  const syntax = new Map<Line, { old: Token[]; next: Token[] }>();
  oldSide.forEach((line, at) => syntax.set(line, { old: oldTokens[at] ?? [], next: [] }));
  newSide.forEach((line, at) => syntax.set(line, { old: syntax.get(line)?.old ?? [], next: newTokens[at] ?? [] }));
  const paint = (line: Line, side: 'old' | 'next', marked?: Segment[]) => overlay(marked ?? plain(line.text), syntax.get(line)?.[side] ?? []);
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
    // computeLineDiffDecorations: no marks when either line is longer than maxLineDiffLength (1000).
    const pairs = deletions.map((entry, at) => marks && additions[at] && entry.text.length <= 1000 && additions[at]!.text.length <= 1000 ? wordAltMarks(entry.text, additions[at]!.text) : null);
    const left = deletions.map((entry, at) => ({ tone: 'deletion', number: String(entry.old), segments: paint(entry, 'old', pairs[at]?.[0]), oldLine: entry.old, newLine: 0 }));
    const right = additions.map((entry, at) => ({ tone: 'addition', number: String(entry.next), segments: paint(entry, 'next', pairs[at]?.[1]), oldLine: 0, newLine: entry.next }));
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
  /** Line items: the file has a selection, so its "+" is pinned to the selection's bottom line and the pointer does not move it;
   * which of the row's cells draws it there (diff-line-drag.ts utilityPlacement). */
  pinned: boolean; pin: boolean; leftPin: boolean; rightPin: boolean;
  /** File headers: Retry, the partial mark, a header whose patch is not there (chevron off). Gaps: a press opens hidden lines. */
  error: boolean; partial: boolean; unavailable: boolean; expandable: boolean;
  /** Comment items (`note`, `draft`): the entry's id (a saved one's context id) and text. */
  entry: string; text: string;
  /** File headers' counts: Pierre's own ("-d" then "+a", each shown when it is not zero or the other is) or, for a
   * large source read file by file, DiffPanel's DiffStatLabel (additions first, both always, compact, 4ch columns). */
  statAligned: boolean; addText: string; delText: string };
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
  // AnnotatableCodeView: a draft whose file is no longer listed (another scope, or a refresh that dropped or changed the
  // file) is released, so it stops holding the gutter. A file is its path, old path and change type; a lazy file still
  // loading has a provisional change type, so its two paths are enough.
  const draftFile = (file: FileView) => file.path === state.draft!.path && file.previous === state.draft!.previous && (file.pending || file.status === state.draft!.status);
  if (state.draft && !client.diffLoading && (state.draft.scope !== scope || !files.some(draftFile))) {
    state.draft = null; state.selection = null; noteDraftFocus(client, false);
  }
  state.find.reset(scope);
  state.find.search(files.map(file => ({ path: file.path, lines: reviewLinesOf(state, file) })));
  const findMatch = state.find.active;
  if (state.find.open && findMatch) {
    state.expanded[`${scope}::${findMatch.path}`] = true;
    const file = files.find(file => file.path === findMatch.path), gap = file && findMatch.context ? findGap(file, findMatch.line) : null;
    if (gap) { const key = `${scope}::${findMatch.path}::${gap.gap}`, previous = state.expansions[key]; state.expansions[key] = { fromStart: Math.max(previous?.fromStart ?? 0, gap.through), fromEnd: previous?.fromEnd ?? 0 }; }
  }
  revealNamedFile(client, selection, files);
  const gitSource = selection.kind !== 'turn' && state.source !== null;
  const turn = turns.find(entry => entry.runId === selection.runId);
  const scopeLabel = selection.kind === 'unstaged' ? 'Uncommitted' : selection.kind === 'branch' ? 'Changes'
    : turn && turn.runId === turns[0]?.runId ? 'Latest turn' : `Turn ${turn?.count ?? '?'}`;
  const items: DiffItemView[] = [];
  let rows = 0;
  const base = { tone: '', number: '', segments: [] as Segment[], leftTone: '', leftNumber: '', leftSegments: [] as Segment[], rightTone: '', rightNumber: '', rightSegments: [] as Segment[],
    label: '', gutter: 33.3, name: '', status: '', letter: '', additions: 0, deletions: 0, expanded: false, markdown: false, side: '', line: 0, selected: false, leftLine: 0, rightLine: 0,
    leftSelected: false, rightSelected: false, pinned: false, pin: false, leftPin: false, rightPin: false, error: false, partial: false, unavailable: false, expandable: false, entry: '', text: '',
    statAligned: false, addText: '', delText: '' };
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
      additions: file.additions, deletions: file.deletions, expanded, markdown: /\.(md|mdx|markdown)$/i.test(file.path), error: status.error, partial: status.truncated, unavailable, ...headerStat(file, lazy !== null) });
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
    const range = draft?.path === file.path ? draft.range : picked?.path === file.path ? picked.range : undefined, chosen = span(range);
    const isSelected = (side: string, line: number) => { const index = line > 0 ? at.get(sideKey(side, line)) : undefined; return !!chosen && index !== undefined && index >= chosen[0] && index <= chosen[1]; };
    const utility = utilityPlacement(range ?? null, point => at.get(sideKey(point.side, point.line)) ?? null);
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
        // Painted as selected under a selected line (Pierre's renderSelection).
        items.push({ ...base, id: `${note.kind}:${file.path}:${note.id}`, kind: note.kind, path: file.path, expanded, entry: note.id, label: note.label, text: note.text,
          selected: isSelected(note.range.endSide, note.range.end) });
      }
    };
    const push = (row: Row | { left: Row; right: Row }, key: string) => {
      if (++rows > (state.find.open ? 100_000 : ROW_LIMIT)) return;
      const common = { ...base, id: `line:${file.path}:${key}`, path: file.path, expanded, gutter, pinned: utility.pinned };
      if ('left' in row) {
        items.push({ ...common, kind: 'split', leftTone: row.left.tone, leftNumber: row.left.number, leftSegments: row.left.segments, rightTone: row.right.tone, rightNumber: row.right.number,
          rightSegments: row.right.segments, leftLine: row.left.oldLine, rightLine: row.right.newLine, leftSelected: isSelected('deletions', row.left.oldLine), rightSelected: isSelected('additions', row.right.newLine),
          leftPin: utility.holds('deletions', row.left.oldLine, true), rightPin: utility.holds('additions', row.right.newLine, true) });
        annotate(row.left.oldLine, row.right.newLine);
      } else {
        const side = row.tone === 'deletion' ? 'deletions' : 'additions', line = side === 'deletions' ? row.oldLine : row.newLine;
        items.push({ ...common, kind: 'line', tone: row.tone, number: row.number, segments: row.segments, side, line, selected: isSelected(side, line), pin: utility.holds(side, line, false) });
        annotate(row.tone === 'addition' ? 0 : row.oldLine, row.tone === 'deletion' ? 0 : row.newLine);
      }
    };
    // Unchanged lines a hidden range has opened (Pierre's expandHunk), drawn as context rows.
    const context = (newLine: number, delta: number, key: string) => {
      const text = newLines[newLine - 1] ?? '', oldLine = newLine - delta;
      push(state.layout === 'split' ? { left: { tone: 'context', number: String(oldLine), segments: plain(text), oldLine, newLine: 0 }, right: { tone: 'context', number: String(newLine), segments: plain(text), oldLine: 0, newLine } }
        : { tone: 'context', number: String(newLine), segments: plain(text), oldLine, newLine }, key);
    };
    const hidden = (count: number, gapIndex: number, newFirst: number, delta: number) => {
      if (count <= 0) return;
      const key = `${scope}::${file.path}::${gapIndex}`, opened = state.expansions[key] ?? { fromStart: 0, fromEnd: 0 };
      const fromStart = Math.min(count, opened.fromStart), fromEnd = Math.min(count - fromStart, opened.fromEnd), left = count - fromStart - fromEnd;
      for (let k = 0; k < fromStart; k++) context(newFirst + k, delta, `x${gapIndex}:${k}`);
      const failed = contents?.state === 'error';
      if (left > 0) items.push({ ...base, id: `gap:${file.path}:${gapIndex}`, kind: 'gap', path: file.path, expanded, line: gapIndex, expandable: gitSource && !failed,
        label: failed ? contents.error || 'Unable to load file contents.' : `${left} unmodified ${left === 1 ? 'line' : 'lines'}` });
      for (let k = left + fromStart; k < count; k++) context(newFirst + k, delta, `x${gapIndex}:${k}`);
    };
    let oldNext = 1, newNext = 1;
    file.hunks.forEach((hunk, hunkIndex) => {
      const oldCount = hunk.lines.filter(line => line.kind !== 'addition').length, newCount = hunk.lines.filter(line => line.kind !== 'deletion').length;
      const oldStart = hunk.oldStart + (oldCount === 0 ? 1 : 0), newStart = hunk.newStart + (newCount === 0 ? 1 : 0);
      hidden(newStart - newNext, hunkIndex, newNext, newStart - oldStart);
      // Past the row limit nothing more is drawn: the hunk only counts, and its sides ask the lexers for nothing.
      if (rows >= (state.find.open ? 100_000 : ROW_LIMIT)) rows += hunk.lines.length;
      else {
        const { stacked, split } = hunkRows(hunk, file.path, true, 'data');
        (state.layout === 'split' ? split : stacked).forEach((row, index) => push(row, `${hunkIndex}:${index}`));
      }
      oldNext = oldStart + oldCount; newNext = newStart + newCount;
    });
    // After the last hunk: counted once the contents are in. Before that, a changed file of a git source (Pierre's
    // canHydrateCollapsedContext: a partial patch, a file loader, a change or a rename with changes) has a row for an
    // unknown count, which a press loads.
    if (contents?.state === 'loaded' && file.hunks.length > 0) hidden(Math.max(0, newLines.length - (contents.newContents.endsWith('\n') ? 1 : 0) - newNext + 1), file.hunks.length, newNext, newNext - oldNext);
    else if (gitSource && file.hunks.length > 0 && ['change', 'rename-changed'].includes(changeType(file.status, file.hunks.length))) {
      const failed = contents?.state === 'error';
      items.push({ ...base, id: `gap:${file.path}:${file.hunks.length}`, kind: 'gap', path: file.path, expanded, line: file.hunks.length, expandable: !failed,
        label: failed ? contents.error || 'Unable to load file contents.' : 'More unchanged context may be available' });
    }
    // The code block ends with 8pt of padding before the next file header.
    items.push({ ...base, id: `pad:${file.path}`, kind: 'pad', path: file.path, expanded });
  }
  // DiffFileLoadingBoundary: up to four header skeletons while files remain.
  const remaining = lazy ? lazy.files.length - settledFileCount(lazy) : 0;
  for (let index = 0; index < Math.min(remaining, 4); index++) items.push({ ...base, id: `ghost:${index}`, kind: 'ghost', path: '' });
  const totals = selection.kind !== 'turn' && state.source?.files ? state.source.files : files;
  const entries = diffFileTreeEntries(files.map(file => ({ path: file.path, status: file.status })));
  const closed = new Set(state.treeCollapsed[scope] ?? []);
  const additions = totals.reduce((sum, file) => sum + file.additions, 0), deletions = totals.reduce((sum, file) => sum + file.deletions, 0);
  // The Changes header's "<head> → <base>" and its picker, for the branch source that answered (DiffPanel selectedGitSource).
  const branchSource = selection.kind === 'branch' && !notGit && state.source?.kind === 'branch-range' ? state.source : null;
  const picker = baseRefView(state.baseRefs, branchSource?.cwd ?? '', branchSource?.headRef ?? null, selection.kind === 'branch' ? selection.baseRef : null);
  const marked = markDiffRows(items, state.find);
  const pending = client.diffLoading && !diffAnswered(client);
  return {
    diffCompare: !!branchSource?.baseRef, diffHead: branchSource?.headRef || 'HEAD', diffBase: branchSource?.baseRef ?? '',
    diffBaseRefs: picker.rows, diffBaseAutomatic: picker.automatic, diffBaseEmpty: picker.empty,
    diffScopeLabel: scopeLabel, diffScope: selection.kind === 'turn' ? `turn:${selection.runId}` : selection.kind,
    diffTurns: turns.map(entry => ({ id: `turn:${entry.runId}`, label: `Turn ${entry.count}`, time: messageTime(entry.completedAt, now, client.local.deviceSettings.timestampFormat), selected: selection.kind === 'turn' && selection.runId === entry.runId })),
    diffLatestSelected: selection.kind === 'turn' && selection.runId === turns[0]?.runId,
    diffCanRefresh: selection.kind !== 'turn' && !notGit, diffLayout: state.layout, diffWrap: state.wrap, diffIgnoreWhitespace: state.ignoreWhitespace, diffTree: state.tree,
    // The size banner shows only for a truncated preview the panel cannot read file by file (older servers).
    // A project outside git reads as the centred empty state, as DiffPanel's !isGitRepo branch, not as an error.
    diffTruncated: (state.truncated && !lazy) || rows > (state.find.open ? 100_000 : ROW_LIMIT), diffEmpty: notGit || (!pending && (!client.diffError || client.diffError === NOT_GIT_REPO) && files.length === 0),
    // isLoadingSelectedPatch: loading with no answer for this selection yet; a refresh keeps the answer (or its empty state) on show.
    diffPending: pending,
    diffEmptyLabel: notGit ? NOT_GIT_REPO : client.diffText.trim() || lazy ? 'No patch available for this selection.' : 'No net changes in this selection.',
    diffAdditions: additions, diffDeletions: deletions, diffAdditionsText: `+${compactCount(additions)}`, diffDeletionsText: `-${compactCount(deletions)}`,
    diffAllCollapsed: files.every(file => (state.expanded[`${scope}::${file.path}`] ?? state.defaultExpanded) !== true),
    diffFiles: files.map(file => ({ id: file.path, name: file.path.split('/').pop() ?? file.path, path: file.path, status: file.status, letter: statusLetter[file.status] ?? 'M',
      additions: file.additions, deletions: file.deletions, markdown: /\.(md|mdx|markdown)$/i.test(file.path) })),
    diffTreeRows: diffTreeRows(entries, closed, state.treeSelected), diffTreeLabel: `${section.title} files`,
    diffTreeFolders: collectDirectoryPaths(entries.map(entry => entry.path)).length > 0, diffTreeAllOpen: allDirectoriesExpanded(entries.map(entry => entry.path), closed),
    diffItems: marked.items, diffFind: state.find.view(marked.target), diffCommentOpen: draft !== null,
    diffReveal: state.reveal, diffRevealTarget: selection.kind === 'turn' && selection.filePath ? `file:${selection.filePath}` : '',
    diffLoadingLabel: selection.kind === 'turn' ? 'Loading checkpoint diff...' : selection.kind === 'unstaged' ? 'Loading uncommitted changes...' : 'Loading changes...',
  };
}
/**
 * DiffPanel revealDiffFile for the turn diff's named file (onOpenTurnDiff(runId, filePath)): once the answer lists it,
 * the file unfolds, the tree selects it with its folders open, and the list scrolls to it, once per reveal request.
 */
function revealNamedFile(client: T3Client, selection: DiffSelection, files: readonly DiffFileModel[]): void {
  const state = client.diffState;
  if (selection.kind !== 'turn' || !selection.filePath) { state.revealed = ''; return; }
  const key = JSON.stringify([state.scopeKey, selection.filePath, selection.reveal ?? 0]);
  if (state.revealed === key || !diffAnswered(client) || !files.some(file => file.path === selection.filePath)) return;
  state.revealed = key;
  state.expanded[`${state.scopeKey}::${selection.filePath}`] = true;
  state.treeSelected = selection.filePath;
  const open = new Set(ancestorDirectories(selection.filePath));
  state.treeCollapsed[state.scopeKey] = (state.treeCollapsed[state.scopeKey] ?? []).filter(path => !open.has(path));
  state.reveal++;
}
/** A file header's counts (see DiffItemView). */
export function headerStat(file: { additions: number; deletions: number }, aligned: boolean): { statAligned: boolean; addText: string; delText: string } {
  const { additions, deletions } = file;
  if (aligned) return { statAligned: true, addText: `+${compactCount(additions)}`, delText: `-${compactCount(deletions)}` };
  return { statAligned: false, addText: additions > 0 || deletions === 0 ? `+${additions.toLocaleString()}` : '', delText: deletions > 0 || additions === 0 ? `-${deletions.toLocaleString()}` : '' };
}
/** The open draft's textarea took the focus (true) or let it go (false), on the thread that shows. */
export function noteDraftFocus(client: T3Client, focused: boolean): void {
  client.diffState.draftFocus = focused && client.diffState.draft !== null ? threadKey(client) : '';
}
/** diffSnapshot draws the draft's card: its scope shows, its file is listed with its patch in and expanded, and its end line is one the file numbers. */
function draftDrawn(client: T3Client): boolean {
  const state = client.diffState, draft = state.draft;
  if (!client.diffOpen || !draft || draft.scope !== state.scopeKey || diffNotGit(client)) return false;
  const file = diffFiles(client).find(entry => entry.path === draft.path);
  if (!file || file.pending || file.binary || (state.lazy !== null && state.lazy.patches.get(file.path)?.state !== 'loaded')) return false;
  if ((state.expanded[`${state.scopeKey}::${file.path}`] ?? state.defaultExpanded) !== true) return false;
  return lineIndex(reviewLinesOf(state, file)).has(sideKey(draft.range.endSide, draft.range.end));
}
/**
 * A Diff comment draft on show holds the focus, so ⌘↩ is its own (composer-presentation.ts gives the composer no send chords
 * then). A draft that is no longer drawn (another thread, a diff without its line, its file collapsed) holds nothing, though
 * no blur said so.
 */
export function draftHoldsCommandEnter(client: T3Client): boolean {
  return client.diffState.draftFocus !== '' && client.diffState.draftFocus === threadKey(client) && draftDrawn(client);
}
export function diffPaths(client: T3Client): string[] { return diffFiles(client).map(file => file.path); }
export function turnNumber(projection: Obj, ordinal: number): boolean { return turnSummaries(projection).some(turn => turn.count === num(ordinal, -1)); }
