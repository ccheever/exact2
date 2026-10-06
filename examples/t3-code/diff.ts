// Changes panel state and projection, adapted from T3 Code (MIT); see LICENSE-T3.
// Sources: apps/web/src/components/DiffPanel.tsx, diffPanelStore.ts, components/diffs/*
// and the served @pierre/diffs rendering (stacked/split rows, word marks, bars).
// The server owns every patch: turn diffs come from orchestration.getTurnDiff and
// working-tree/branch diffs from review.getDiffPreview.
import { arr, obj, str, num, type Obj } from './domain';
import { ClientError } from './protocol';
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
}
/** UI-only operations never touch the server and never mark a pending write uncertain. */
export const DIFF_LOCAL_OPS = ['diff-view'];
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
/** The request for the current selection, or an explanation when there is nothing to ask. */
export function diffRequest(client: T3Client): { method: string; payload: Obj; scope: string; source: string } {
  const selection = currentSelection(client), state = client.diffState;
  if (!client.threadId) throw new ClientError('Select a thread to inspect turn diffs.');
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
  client.diffState.scopeKey = request.scope;
  if (!request.source) { client.diffState.truncated = false; return str(result.diff); }
  const source = arr(result.sources).find(entry => entry.kind === request.source);
  client.diffState.truncated = source?.truncated === true;
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
type Row = { tone: string; number: string; segments: Segment[] };
const plain = (text: string): Segment[] => [{ id: '0', text, mark: false, syntax: '' }];
/** One hunk as stacked rows and split pairs, with paired replacements word-marked. */
function hunkRows(hunk: Hunk, path: string): { stacked: Row[]; split: { left: Row; right: Row }[] } {
  const stacked: Row[] = [], split: { left: Row; right: Row }[] = [];
  const empty: Row = { tone: 'empty', number: '', segments: [] };
  // Each side is highlighted as one text (timeline-diff-syntax.ts), then overlaid on the word marks.
  const oldSide = hunk.lines.filter(line => line.kind !== 'addition'), newSide = hunk.lines.filter(line => line.kind !== 'deletion');
  const oldTokens = lineTokens(oldSide.map(line => line.text), path), newTokens = lineTokens(newSide.map(line => line.text), path);
  const syntax = new Map<Line, { old: Token[]; next: Token[] }>();
  oldSide.forEach((line, at) => syntax.set(line, { old: oldTokens[at] ?? [], next: [] }));
  newSide.forEach((line, at) => syntax.set(line, { old: syntax.get(line)?.old ?? [], next: newTokens[at] ?? [] }));
  const paint = (line: Line, segments: Segment[], side: 'old' | 'next') => overlay(segments, syntax.get(line)?.[side] ?? []);
  for (let index = 0; index < hunk.lines.length;) {
    const line = hunk.lines[index]!;
    if (line.kind === 'context') {
      stacked.push({ tone: 'context', number: String(line.next), segments: paint(line, plain(line.text), 'next') });
      split.push({ left: { tone: 'context', number: String(line.old), segments: paint(line, plain(line.text), 'old') }, right: { tone: 'context', number: String(line.next), segments: paint(line, plain(line.text), 'next') } });
      index++; continue;
    }
    const deletions: Line[] = [], additions: Line[] = [];
    while (hunk.lines[index]?.kind === 'deletion') deletions.push(hunk.lines[index++]!);
    while (hunk.lines[index]?.kind === 'addition') additions.push(hunk.lines[index++]!);
    const marks = deletions.map((entry, at) => additions[at] ? wordMarks(entry.text, additions[at]!.text) : [plain(entry.text), []] as [Segment[], Segment[]]);
    const left = deletions.map((entry, at) => ({ tone: 'deletion', number: String(entry.old), segments: paint(entry, marks[at]![0], 'old') }));
    const right = additions.map((entry, at) => ({ tone: 'addition', number: String(entry.next), segments: paint(entry, at < deletions.length ? marks[at]![1] : plain(entry.text), 'next') }));
    stacked.push(...left, ...right);
    for (let at = 0; at < Math.max(left.length, right.length); at++) split.push({ left: left[at] ?? empty, right: right[at] ?? empty });
  }
  return { stacked, split };
}

const statusLetter: Record<string, string> = { modified: 'M', added: 'A', deleted: 'D', renamed: 'R' };
type DiffItemView = { id: string; kind: string; path: string; name: string; status: string; letter: string; additions: number; deletions: number;
  expanded: boolean; markdown: boolean; tone: string; number: string; segments: Segment[]; leftTone: string; leftNumber: string; leftSegments: Segment[];
  rightTone: string; rightNumber: string; rightSegments: Segment[]; label: string; gutter: number };
/** The snapshot's panel fields: header labels, menus, one flat virtualized list of file and line items. */
export function diffSnapshot(client: T3Client, now: number) {
  const state = client.diffState, selection = currentSelection(client), turns = turnSummaries(client.projection);
  const files = parsePatch(client.diffText);
  const turn = turns.find(entry => entry.runId === selection.runId);
  const scopeLabel = selection.kind === 'unstaged' ? 'Uncommitted' : selection.kind === 'branch' ? 'Changes'
    : turn && turn.runId === turns[0]?.runId ? 'Latest turn' : `Turn ${turn?.count ?? '?'}`;
  const items: DiffItemView[] = [];
  let rows = 0;
  const base = { tone: '', number: '', segments: [] as Segment[], leftTone: '', leftNumber: '', leftSegments: [] as Segment[], rightTone: '', rightNumber: '', rightSegments: [] as Segment[], label: '', gutter: 33.3 };
  for (const file of files) {
    const expanded = (state.expanded[`${state.scopeKey}::${file.path}`] ?? state.defaultExpanded) === true;
    const digits = Math.max(1, ...file.hunks.flatMap(hunk => hunk.lines.map(line => String(Math.max(line.old, line.next)).length)));
    // Numbers sit right-aligned after a 2ch inset, 1ch + 2pt before the code (measured 33.3pt for one digit).
    const gutter = Math.round((digits * 7.8267 + 15.6533 + 9.83) * 10) / 10;
    items.push({ ...base, id: `file:${file.path}`, kind: 'file', path: file.path, name: file.path, status: file.status, letter: statusLetter[file.status] ?? 'M',
      additions: file.additions, deletions: file.deletions, expanded, markdown: /\.(md|mdx|markdown)$/i.test(file.path) });
    if (!expanded) continue;
    if (file.binary) { items.push({ ...base, id: `binary:${file.path}`, kind: 'gap', path: file.path, name: '', status: '', letter: '', additions: 0, deletions: 0, expanded, markdown: false, label: 'Binary file not shown' }); continue; }
    let previousEnd = 0;
    file.hunks.forEach((hunk, hunkIndex) => {
      const hidden = hunk.newStart - previousEnd - 1;
      if (hidden > 0) items.push({ ...base, id: `gap:${file.path}:${hunkIndex}`, kind: 'gap', path: file.path, name: '', status: '', letter: '', additions: 0, deletions: 0, expanded, markdown: false,
        label: `${hidden} unmodified ${hidden === 1 ? 'line' : 'lines'}` });
      const { stacked, split } = hunkRows(hunk, file.path);
      const list = state.layout === 'split' ? split : stacked;
      list.forEach((row, index) => {
        if (++rows > ROW_LIMIT) return;
        const common = { ...base, id: `line:${file.path}:${hunkIndex}:${index}`, path: file.path, name: '', status: '', letter: '', additions: 0, deletions: 0, expanded, markdown: false, gutter };
        if ('left' in row) items.push({ ...common, kind: 'split', leftTone: row.left.tone, leftNumber: row.left.number, leftSegments: row.left.segments,
          rightTone: row.right.tone, rightNumber: row.right.number, rightSegments: row.right.segments });
        else items.push({ ...common, kind: 'line', tone: row.tone, number: row.number, segments: row.segments });
      });
      previousEnd = hunk.newStart + hunk.lines.filter(line => line.kind !== 'deletion').length - 1;
    });
    // The code block ends with 8pt of padding before the next file header.
    items.push({ ...base, id: `pad:${file.path}`, kind: 'pad', path: file.path, name: '', status: '', letter: '', additions: 0, deletions: 0, expanded, markdown: false });
  }
  return {
    diffScopeLabel: scopeLabel, diffMenu: client.diffOpen ? state.menu : '', diffScope: selection.kind === 'turn' ? `turn:${selection.runId}` : selection.kind,
    diffTurns: turns.map(entry => ({ id: `turn:${entry.runId}`, label: `Turn ${entry.count}`, time: messageTime(entry.completedAt, now, client.local.deviceSettings.timestampFormat), selected: selection.kind === 'turn' && selection.runId === entry.runId })),
    diffLatestSelected: selection.kind === 'turn' && selection.runId === turns[0]?.runId,
    diffCanRefresh: selection.kind !== 'turn', diffLayout: state.layout, diffWrap: state.wrap, diffIgnoreWhitespace: state.ignoreWhitespace, diffTree: state.tree,
    diffTruncated: state.truncated || rows > ROW_LIMIT, diffEmpty: !client.diffLoading && !client.diffError && files.length === 0,
    diffEmptyLabel: client.diffText.trim() ? 'No patch available for this selection.' : 'No net changes in this selection.',
    diffAdditions: files.reduce((sum, file) => sum + file.additions, 0), diffDeletions: files.reduce((sum, file) => sum + file.deletions, 0),
    diffAllCollapsed: files.every(file => (state.expanded[`${state.scopeKey}::${file.path}`] ?? state.defaultExpanded) !== true),
    diffFiles: files.map(file => ({ id: file.path, name: file.path.split('/').pop() ?? file.path, path: file.path, status: file.status, letter: statusLetter[file.status] ?? 'M',
      additions: file.additions, deletions: file.deletions, markdown: /\.(md|mdx|markdown)$/i.test(file.path) })),
    diffItems: items,
    diffLoadingLabel: selection.kind === 'turn' ? 'Loading checkpoint diff...' : selection.kind === 'unstaged' ? 'Loading uncommitted changes...' : 'Loading changes...',
  };
}
export function diffPaths(client: T3Client): string[] { return parsePatch(client.diffText).map(file => file.path); }
export function turnNumber(projection: Obj, ordinal: number): boolean { return turnSummaries(projection).some(turn => turn.count === num(ordinal, -1)); }
