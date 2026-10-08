// The pull request panel's Code tab (20261005-pr-code-tab): PullRequestCodeTab.tsx (T3 Code
// 1e2ecbd975, MIT, see LICENSE-T3) — the change or one commit's change read a slice at a time
// (`POST /api/pull-requests/diff`, the native `prDiff` op), Viewed ticks, line comments into the
// review being written, the host's conversations on their lines or listed off the diff, and the
// toolbar. The panel's resource (pages-pr-detail.ts) reads here once the tab was opened for this
// pull request (`tab`), and draws `presentCode`; the tab's presses are `chatlocal:pr-code-*`
// (`prCodeLocal`), its host writes `pageslocal:pr-act-code-*`/`thread-*` (`prCodeCommand`).
//
// State lives per client for the pull request on screen and starts over for another one (the
// reference's tab is keyed by the pull request): the scope, the slices, the folds, the draft, the
// opened hidden ranges, the ticks and the conversations' pages. Every read is the resource's, one
// at a time, and stays due when Exact lets its answer go; what a read changes is shown first and
// the resource asked again (`r10Wake`), as the panel's other reads do.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { letGo } from './let-go';
import { pushToast } from './toast';
import { buildDiffReviewComment, diffReviewLines, type SelectedLineRange, type SelectionSide } from './diff-comments';
import { changeType, expandRange, type Expansion, type FileContents } from './diff-lazy';
import { diffFileTreeEntries, diffTreeRows, ancestorDirectories, collectDirectoryPaths, allDirectoriesExpanded, type DiffTreeRow } from './diff-tree';
import { decodeClientPrefs } from './settings-core';
import { rememberDiffLayout } from './settings-appearance-look';
import { pagesPrefs } from './pages-prefs';
import { nextPendingReviewCommentId, pullRequestReviewKey, pullRequestReviewStore } from './pages-pr-writes-logic';
import type { WriteReference } from './pages-pr-writes';
import {
  adoptDiffSlice, areAllDiffFilesCollapsed, fnv1a32, isLineInFileDiff, orderDiffFiles, renderablePatch, resolveDiffReviewPosition, reviewPositionAnchor, toggleFileDiffFoldForViewed,
  type DiffFoldOverride, type DiffSlice, type PrDiffFile, type PrDiffSide, type PullRequestReviewPosition, type RenderablePatch,
} from './pages-pr-code-logic';
import { BASE, codeRows, fileKey, hiddenCount, isCollapsed, type CodeItem } from './pages-pr-code-rows';
import { FilesViewedStore } from './pages-pr-viewed';
import { emptyThreadState, presentThreads, threadBodies, threadCommand, type PrThreadCard, type ThreadState } from './pages-pr-threads';

/** Commits per press of "Show more" in the scope menu. */
export const COMMIT_PAGE_SIZE = 10;

type Draft = { fileKey: string; path: string; oldPath: string | null; position: PullRequestReviewPosition; range: SelectedLineRange };
export type CodeState = {
  key: string; mounted: boolean; commit: string | null; refresh: number;
  slices: readonly DiffSlice[]; cursor: string | null; diffDue: boolean; diffError: string;
  parsed: Map<string, RenderablePatch>;
  toggled: ReadonlySet<string>; foldOverride: DiffFoldOverride; visibleCommits: number;
  ignoreWhitespace: boolean; wrap: boolean; orphansOpen: boolean; treeClosed: Set<string>; treeSelected: string;
  selection: { path: string; range: SelectedLineRange } | null; draft: Draft | null;
  contents: Record<string, FileContents>; expansions: Record<string, Expansion>; contentsDue: Map<string, { path: string; gap: number }>;
  viewed: FilesViewedStore; threads: ThreadState; shown: string;
};
const states = new WeakMap<object, CodeState>();
const prefs = (client: { local: object }) => decodeClientPrefs((client.local as { clientSettings?: unknown }).clientSettings);
function fresh(client: { local: object }, key: string): CodeState {
  const settings = prefs(client);
  return { key, mounted: false, commit: null, refresh: 0, slices: [], cursor: null, diffDue: true, diffError: '', parsed: new Map(), toggled: new Set(), foldOverride: null,
    visibleCommits: COMMIT_PAGE_SIZE, ignoreWhitespace: settings.diffIgnoreWhitespace, wrap: settings.wordWrap, orphansOpen: false, treeClosed: new Set(), treeSelected: '',
    selection: null, draft: null, contents: {}, expansions: {}, contentsDue: new Map(), viewed: new FilesViewedStore(), threads: emptyThreadState(), shown: '' };
}
/** The Code tab of the pull request on screen; another pull request starts over. */
export function codeState(client: { local: object }, key: string): CodeState {
  let state = states.get(client);
  if (!state || state.key !== key) { state = fresh(client, key); states.set(client, state); }
  return state;
}
export const peekCode = (client: object): CodeState | null => states.get(client) ?? null;
/** One commit's diff and the whole change are two different diffs, paged separately. */
const scopeKey = (code: CodeState) => `${code.key}@${code.commit ?? 'all'}`;
/** A new scope (or Refresh): back to the first slice. */
function restart(code: CodeState): void {
  code.slices = []; code.cursor = null; code.diffDue = true; code.diffError = ''; code.parsed.clear();
  code.contents = {}; code.expansions = {}; code.contentsDue.clear();
}
function scope(code: CodeState, commit: string | null): void {
  if (code.commit === commit) return;
  code.commit = commit;
  // PullRequestCodeTab's reset on scopeKey: the draft, the selection, the folds, the menu page, the orphans.
  code.draft = null; code.selection = null; code.toggled = new Set(); code.foldOverride = null; code.visibleCommits = COMMIT_PAGE_SIZE; code.orphansOpen = false;
  restart(code);
}

export type CodeContext = { client: T3Client; reference: WriteReference; detail: Obj; activity: Obj | null };
const host = (reference: WriteReference) => ({ projectId: reference.projectId, ...(reference.host ? { host: reference.host } : {}), repository: reference.repository, number: reference.number });
const commitsOf = (activity: Obj | null) => arr(activity?.commits).slice().sort((left, right) => Date.parse(str(right.committedDate)) - Date.parse(str(left.committedDate)));
const contentsKeyOf = (code: CodeState, path: string) => `${scopeKey(code)}::${path}`;

// ── Reads ───────────────────────────────────────────────────────────────────

/** What the tab shows that a read changes; a change is shown before the next read. */
function signature(code: CodeState): string {
  return [scopeKey(code), code.slices.length, code.cursor, code.diffDue, code.diffError, code.viewed.due, code.contentsDue.size].join('|');
}
/**
 * The tab's reads, after the panel's own: the ticks, the slice owed, the contents a hidden range
 * asked for. 'wake' when something new should be shown before they run (the caller presents and
 * asks again); the reads run otherwise.
 */
export async function readCode(ctx: CodeContext & { native: Native; tab: string; refresh: number }): Promise<'wake' | 'done'> {
  const { client, native, reference, detail, activity } = ctx, key = pullRequestReviewKey(reference);
  const code = codeState(client, `${client.environmentId}|${key}`);
  if (ctx.tab === 'code') code.mounted = true;
  if (!code.mounted || obj(detail.capabilities).diff !== true) return 'done';
  // The panel's Refresh goes around the host's cache: the diff from its first page, and the ticks.
  if (ctx.refresh !== code.refresh) { const first = code.refresh === 0 && code.slices.length === 0; code.refresh = ctx.refresh; if (!first) { restart(code); code.viewed.due = true; } }
  // A rebase can take the scoped commit out of the change: the scope goes back to the whole change.
  if (code.commit !== null && activity && !commitsOf(activity).some(entry => str(entry.oid) === code.commit)) scope(code, null);
  const viewedOn = obj(detail.capabilities).viewedFiles !== undefined;
  if (!(code.diffDue || (viewedOn && code.viewed.due) || code.contentsDue.size > 0)) { code.shown = signature(code); return 'done'; }
  if (code.shown !== signature(code)) { code.shown = signature(code); return 'wake'; }
  if (viewedOn && code.viewed.due) {
    try { code.viewed.adopt(obj(await client.call(native, { op: 'request', method: 'pullRequests.filesViewed', payload: host(reference), share: true })) as never); }
    catch (error) { if (letGo(error)) throw error; code.viewed.failed(error instanceof Error && error.message ? error.message : 'The host did not answer.'); }
  }
  if (code.diffDue) {
    const asked = scopeKey(code), cursor = code.cursor;
    try {
      const answer = obj(await client.call(native, { op: 'prDiff', payload: { ...host(reference), ...(cursor === null ? {} : { cursor }), ...(code.commit === null ? {} : { commit: code.commit }) } }));
      if (scopeKey(code) === asked && code.cursor === cursor) {
        code.slices = adoptDiffSlice(code.slices, { cursor, patch: str(answer.patch), truncated: answer.truncated === true, nextCursor: str(answer.nextCursor) || null,
          omittedFileStats: arr(answer.omittedFileStats).map(file => ({ path: str(file.path), additions: num(file.additions), deletions: num(file.deletions) })) });
        code.diffDue = false; code.diffError = '';
      }
    } catch (error) {
      if (letGo(error)) throw error;
      if (scopeKey(code) === asked && code.cursor === cursor) { code.diffDue = false; code.diffError = error instanceof Error && error.message ? error.message : 'The diff could not be loaded.'; }
    }
  }
  for (const [contentsKey, want] of [...code.contentsDue]) {
    const file = filesOf(code).find(entry => entry.path === want.path);
    code.contentsDue.delete(contentsKey);
    if (!file) continue;
    try {
      const answer = obj(await client.call(native, { op: 'request', method: 'pullRequests.diffFileContents', share: true, payload: diffFileContentsInput(reference, code.commit, file) }));
      code.contents[contentsKey] = { state: 'loaded', oldContents: str(answer.oldContents), newContents: str(answer.newContents), error: '' };
      const gapKey = `${contentsKey}::${want.gap}`;
      code.expansions[gapKey] = expandRange(code.expansions[gapKey], hiddenCount(file, want.gap, code.contents[contentsKey]));
    } catch (error) {
      if (letGo(error)) { code.contentsDue.set(contentsKey, want); throw error; }
      code.contents[contentsKey] = { state: 'error', oldContents: '', newContents: '', error: error instanceof Error && error.message ? error.message : 'Unable to load file contents.' };
    }
  }
  code.shown = signature(code);
  return 'done';
}
/**
 * createPullRequestDiffFileContentsLoader's request: the change type and both names of the file,
 * under the commit scope when there is one. The single-flight key is the whole request (the
 * environment, project, host, repository, number, commit, change type and paths), so two hosts'
 * reads of one path stay apart (Swift's shared reads join only identical payloads).
 */
export function diffFileContentsInput(reference: WriteReference, commit: string | null, file: PrDiffFile): Obj {
  const type = changeType(file.status, file.hunks.length);
  return { ...host(reference), ...(commit === null ? {} : { commit }), changeType: type, oldPath: file.previous && file.status === 'renamed' ? file.previous : file.path, newPath: file.path };
}

// ── The files on screen ─────────────────────────────────────────────────────

function parsedSlices(code: CodeState): RenderablePatch[] {
  return code.slices.map(slice => {
    const cacheKey = `${scopeKey(code)}:${code.ignoreWhitespace}:${slice.cursor ?? 'first'}:${fnv1a32(slice.patch)}`;
    let parsed = code.parsed.get(cacheKey);
    if (parsed === undefined) { parsed = renderablePatch(slice.patch, code.ignoreWhitespace); code.parsed.set(cacheKey, parsed); }
    return parsed;
  });
}
/** Ordered within a slice, not across them: a late slice never moves a file the reader is part way through. */
function filesOf(code: CodeState): PrDiffFile[] { return parsedSlices(code).flatMap(parsed => (parsed?.kind === 'files' ? orderDiffFiles(parsed.files) : [])); }

// ── The view ────────────────────────────────────────────────────────────────

export type PrCodeView = ReturnType<typeof emptyCode>;
export function emptyCode() {
  return {
    available: false, mounted: false, key: '', phase: 'loading', message: '', raw: [] as { id: string; reason: string; text: string }[],
    scopeLabel: 'All commits', allSelected: true, hasCommits: false, commits: [] as { oid: string; short: string; headline: string; selected: boolean }[], moreCommits: 0,
    filesLabel: '0 files', viewedShown: false, viewedCount: '', viewedWord: 'viewed', viewedHere: false, viewedError: '', viewedTruncated: false, withheld: false, commentsOff: false,
    ignoreWhitespace: true, allCollapsed: false, hasFiles: false, layout: 'stacked', wrap: true, tree: false,
    items: [] as CodeItem[], truncated: false, threads: [] as PrThreadCard[], footer: '', footerFailed: false,
    orphansLabel: '', orphanCount: 0, orphanSummary: '', orphansOpen: false, orphans: [] as { path: string; threads: { id: string; line: string }[] }[],
    treeRows: [] as DiffTreeRow[], treeLabel: '', treeCount: 0, treeFolders: false, treeAllOpen: true, treeFooter: '', treeBusy: false,
    draftOpen: false, canComment: false, viewedQueued: 0, viewedKey: '', threadPending: false,
  };
}
type Body = { id: string; kind: string; title: string; body: string };
/** The tab as it stands, and the markdown documents its conversations render. */
export function presentCode(ctx: CodeContext & { now: number }): { view: PrCodeView; bodies: Body[] } {
  const { client, reference, detail, activity } = ctx, view = emptyCode();
  const capabilities = obj(detail.capabilities), permissions = obj(detail.viewerPermissions);
  view.available = capabilities.diff === true;
  const code = peekCode(client);
  if (!code || code.key !== `${client.environmentId}|${pullRequestReviewKey(reference)}` || !code.mounted || !view.available) return { view, bodies: [] };
  view.mounted = true; view.key = scopeKey(code);
  const settings = prefs(client);
  view.layout = settings.diffLayout === 'split' ? 'split' : 'stacked'; view.wrap = code.wrap; view.ignoreWhitespace = code.ignoreWhitespace; view.tree = pagesPrefs(client).prFileTree === true;
  // The scope menu: newest first, ten at a time.
  const commits = commitsOf(activity), selected = commits.find(entry => str(entry.oid) === code.commit);
  view.hasCommits = commits.length > 0; view.allSelected = code.commit === null;
  view.scopeLabel = selected ? str(selected.messageHeadline) : 'All commits';
  view.commits = commits.slice(0, code.visibleCommits).map(entry => ({ oid: str(entry.oid), short: str(entry.oid).slice(0, 7), headline: str(entry.messageHeadline), selected: str(entry.oid) === code.commit }));
  view.moreCommits = Math.max(0, commits.length - code.visibleCommits);
  // What this host can do, and what this account may.
  const review = obj(capabilities.review), inlineComment = review.inlineComment === true && permissions.comment === true;
  const canCommentOnLines = inlineComment && code.commit === null;
  view.commentsOff = code.commit !== null && inlineComment;
  const parsed = parsedSlices(code), files = filesOf(code), paths = files.map(file => file.path);
  const nextCursor = code.slices.at(-1)?.nextCursor ?? null;
  view.withheld = code.slices.some(slice => slice.truncated) || parsed.some(entry => entry?.kind === 'raw');
  view.filesLabel = `${files.length} ${files.length === 1 ? 'file' : 'files'}${nextCursor === null ? '' : '+'}`;
  const store = obj(capabilities).viewedFiles;
  view.viewedShown = store !== undefined && files.length > 0;
  view.viewedCount = `${code.viewed.count(paths)} / ${files.length}`; view.viewedHere = store === 'environment';
  view.viewedWord = store === 'environment' ? 'viewed in T3 Code' : 'viewed';
  view.viewedError = code.viewed.error ?? ''; view.viewedTruncated = code.viewed.truncated;
  view.viewedQueued = code.viewed.queuedCount(); view.viewedKey = code.viewed.key();
  // A conversation is placed only on a line a rendered hunk holds, and never under a commit scope.
  const threads = arr(activity?.reviewThreads), placed = new Set<string>();
  if (code.commit === null) for (const file of files) for (const thread of threads) {
    if (str(thread.path) === file.path && num(thread.line) > 0 && isLineInFileDiff(file, thread.side === 'left' ? 'left' : 'right', num(thread.line))) placed.add(str(thread.id));
  }
  const pending = code.commit === null ? pullRequestReviewStore(client).pending(pullRequestReviewKey(reference)) : [];
  const draft = code.draft, omitted = new Map(code.slices.flatMap(slice => slice.omittedFileStats.map(file => [file.path, file] as const)));
  const rows = codeRows({
    files, layout: view.layout as 'stacked' | 'split', foldOverride: effectiveFold(client, code), toggled: code.toggled, omitted,
    viewedMark: path => (!view.viewedShown ? '' : code.viewed.isStale(path) ? 'changed' : code.viewed.isViewed(path) ? 'viewed' : 'unviewed'),
    annotations: path => ({
      threads: threads.filter(thread => placed.has(str(thread.id)) && str(thread.path) === path).map(thread => ({ id: str(thread.id), side: (thread.side === 'left' ? 'left' : 'right') as PrDiffSide, line: num(thread.line) })),
      pending: pending.filter(comment => comment.path === path).map(comment => ({ id: comment.id, body: comment.body, position: comment.position as PullRequestReviewPosition })),
      draft: draft && draft.path === path ? { position: draft.position, label: `${draft.path}:${reviewPositionAnchor(draft.position).line}` } : null,
    }),
    selection: draft ? { path: draft.path, range: draft.range } : code.selection, contents: code.contents, contentsKey: path => contentsKeyOf(code, path), expansions: code.expansions,
  });
  view.items = rows.items; view.truncated = rows.truncated; view.draftOpen = draft !== null && canCommentOnLines; view.canComment = canCommentOnLines;
  view.hasFiles = files.length > 0;
  view.allCollapsed = areAllDiffFilesCollapsed(paths, new Set(files.filter(file => isCollapsed({ foldOverride: effectiveFold(client, code), toggled: code.toggled }, file)).map(fileKey)));
  // The footer only while something is still owed.
  view.footer = nextCursor === null ? '' : code.diffError ? 'The rest of this diff could not be loaded.' : code.diffDue ? 'Loading more files...' : '';
  view.footerFailed = nextCursor !== null && code.diffError !== '';
  // The sentinel the list's reachend reaches, inside the list, at the end of the files.
  if (nextCursor !== null) view.items = [...view.items, { ...BASE, id: 'footer', kind: 'pr-footer', label: view.footer, error: view.footerFailed }];
  // Conversations off the diff, a file once with its threads.
  const orphans = threads.filter(thread => !placed.has(str(thread.id))), byPath = new Map<string, { id: string; line: string }[]>();
  for (const thread of orphans) { const list = byPath.get(str(thread.path)) ?? []; list.push({ id: str(thread.id), line: num(thread.line) > 0 ? `Line ${num(thread.line)}` : '' }); byPath.set(str(thread.path), list); }
  view.orphans = [...byPath].map(([path, list]) => ({ path, threads: list }));
  view.orphanCount = orphans.length; view.orphansOpen = code.orphansOpen;
  view.orphansLabel = nextCursor === null ? 'Conversations not on the current diff' : 'Conversations not on the diff loaded so far';
  view.orphanSummary = orphans.length === 1 ? '1 conversation' : `${orphans.length} conversations`;
  view.threads = presentThreads(client, code.threads, reference, detail, threads, ctx.now);
  view.threadPending = code.threads.pending;
  // The tree lists only what has arrived; its footer pulls the rest in.
  const entries = diffFileTreeEntries(files.map(file => ({ path: file.path, status: file.status })));
  view.treeRows = diffTreeRows(entries, code.treeClosed, code.treeSelected); view.treeLabel = `Pull request #${num(detail.number)} files`; view.treeCount = entries.length;
  view.treeFolders = collectDirectoryPaths(paths).length > 0; view.treeAllOpen = allDirectoriesExpanded(paths, code.treeClosed);
  view.treeFooter = nextCursor === null ? '' : code.diffError ? 'Retry' : code.diffDue ? 'Loading more files...' : 'Load more files';
  view.treeBusy = nextCursor !== null && code.diffDue;
  // The states that take the list's place: loading, the error, the raw text, nothing changed.
  const raw = nextCursor === null ? parsed.flatMap((entry, index) => (entry?.kind === 'raw' ? [{ id: `raw:${index}`, reason: entry.reason, text: entry.text }] : [])) : [];
  view.raw = raw;
  if (code.slices.length === 0 && code.diffError) { view.phase = 'error'; view.message = code.diffError; }
  else if (code.slices.length === 0) view.phase = 'loading';
  else if (files.length === 0 && raw.length > 0) view.phase = 'raw';
  else if (rows.items.length === 0 && nextCursor === null) { view.phase = 'empty'; view.message = code.commit === null ? 'This pull request has no file changes.' : 'This commit has no file changes.'; }
  else view.phase = 'files';
  return { view, bodies: threadBodies(code.threads, threads) };
}
/** The toolbar's last word, else the saved default (General → Diff files collapsed). */
const effectiveFold = (client: { local: object }, code: CodeState): DiffFoldOverride => code.foldOverride ?? (prefs(client).diffFilesCollapsed ? 'folded' : 'expanded');

// ── The tab's presses (chatlocal:pr-code-*) ─────────────────────────────────

const fields = (value: string, count: number) => { const parts: string[] = []; let rest = value; for (let i = 1; i < count; i++) { const bar = rest.indexOf('|'); if (bar < 0) break; parts.push(rest.slice(0, bar)); rest = rest.slice(bar + 1); } parts.push(rest); return parts; };
export type CodeLocalContext = { client: T3Client; reference: WriteReference; detail: Obj | null };
/**
 * `chatlocal:pr-code-<op>`: what the tab's controls change. None of them reads or writes the host
 * (the resource does, once they made something due) except `flush`, the gathered ticks' write.
 */
export async function prCodeLocal(ctx: CodeLocalContext & { native: Native }, op: string, value: string): Promise<string> {
  const { client, reference } = ctx;
  const code = codeState(client, `${client.environmentId}|${pullRequestReviewKey(reference)}`);
  const files = () => filesOf(code), file = (path: string) => files().find(entry => entry.path === path);
  switch (op) {
    case 'scope': code.mounted = true; scope(code, value || null); return '';
    case 'more-commits': code.visibleCommits += COMMIT_PAGE_SIZE; return '';
    case 'whitespace': code.ignoreWhitespace = !code.ignoreWhitespace; code.draft = null; code.selection = null; return '';
    case 'wrap': code.wrap = !code.wrap; return '';
    case 'layout': if (value === 'stacked' || value === 'split') rememberDiffLayout(client, value); return '';
    case 'tree': { const prefs = pagesPrefs(client); prefs.prFileTree = prefs.prFileTree !== true; return ''; }
    case 'fold-all': {
      const fold = effectiveFold(client, code), all = files();
      code.foldOverride = areAllDiffFilesCollapsed(all.map(fileKey), new Set(all.filter(entry => isCollapsed({ foldOverride: fold, toggled: code.toggled }, entry)).map(fileKey))) ? 'expanded' : 'folded';
      code.toggled = new Set();
      return '';
    }
    case 'fold': { const next = new Set(code.toggled); if (next.has(value)) next.delete(value); else next.add(value); code.toggled = next; return ''; }
    case 'viewed': {
      // The tick and the fold are one gesture: clearing a file puts it away, un-clearing brings it back.
      const [path = '', viewed = ''] = fields(value, 2), on = viewed === 'true';
      if (!path) return '';
      code.viewed.setViewed(path, on);
      code.toggled = toggleFileDiffFoldForViewed(path, on, effectiveFold(client, code), code.toggled);
      return '';
    }
    case 'flush': return flushViewed(ctx.client, ctx.native, code, reference);
    case 'next': {
      const nextCursor = code.slices.at(-1)?.nextCursor ?? null;
      // A failed slice is not asked for again on its own: Retry asks.
      if (nextCursor !== null && nextCursor !== code.cursor && !code.diffDue && !code.diffError) { code.cursor = nextCursor; code.diffDue = true; }
      return '';
    }
    case 'retry': if (code.diffError) { code.diffError = ''; code.diffDue = true; } return '';
    case 'orphans': code.orphansOpen = !code.orphansOpen; return '';
    case 'folder': { if (code.treeClosed.has(value)) code.treeClosed.delete(value); else code.treeClosed.add(value); return ''; }
    case 'folders': code.treeClosed = value === 'collapse' ? new Set(collectDirectoryPaths(files().map(entry => entry.path))) : new Set(); return '';
    case 'reveal': {
      // revealFile: a folded file opens; its folders open; the list scrolls to it (the contract's scrollIntoView).
      const target = file(value);
      if (!target) return '';
      if (isCollapsed({ foldOverride: effectiveFold(client, code), toggled: code.toggled }, target)) { const next = new Set(code.toggled); if (next.has(value)) next.delete(value); else next.add(value); code.toggled = next; }
      code.treeSelected = value;
      for (const directory of ancestorDirectories(value)) code.treeClosed.delete(directory);
      return '';
    }
    case 'expand': {
      // A hidden range opens from the file's contents, read once per file (the resource reads it).
      const [path = '', gap = ''] = fields(value, 2), target = file(path), index = Number(gap);
      if (!target || !Number.isInteger(index)) return '';
      const contentsKey = contentsKeyOf(code, path), contents = code.contents[contentsKey];
      if (contents?.state === 'loaded') { const gapKey = `${contentsKey}::${index}`; code.expansions[gapKey] = expandRange(code.expansions[gapKey], hiddenCount(target, index, contents)); }
      else if (contents?.state !== 'error') code.contentsDue.set(contentsKey, { path, gap: index });
      return '';
    }
    case 'select': case 'select-shift': {
      // Pierre's line selection: a press selects a line, a Shift-press extends it in the same file.
      if (code.draft || code.commit !== null) return '';
      const [side = '', line = '', path = ''] = fields(value, 3), at = { side: (side === 'deletions' ? 'deletions' : 'additions') as SelectionSide, line: Number(line) };
      const anchor = op === 'select-shift' && code.selection?.path === path ? code.selection.range : null;
      code.selection = { path, range: anchor ? { ...anchor, end: at.line, endSide: at.side } : { start: at.line, side: at.side, end: at.line, endSide: at.side } };
      return '';
    }
    case 'row': {
      // DiffRow's commands: `diffreview|line:<side>[:shift]|<line>|<path>` and `diffreview|expand|<gap>|<path>`.
      const [, id = '', n = '', path = ''] = fields(value, 4), [action = '', side = '', shift = ''] = id.split(':');
      if (action === 'line') return prCodeLocal(ctx, shift === 'shift' ? 'select-shift' : 'select', `${side}|${n}|${path}`);
      if (action === 'expand') return prCodeLocal(ctx, 'expand', `${path}|${n}`);
      return '';
    }
    case 'begin': return beginComment(client, code, ctx.detail, value);
    case 'cancel': code.draft = null; code.selection = null; return '';
    case 'add': {
      // "Add to review": the comment joins the pending review (pages-pr-writes-logic.ts PullRequestReviewStore).
      const draft = code.draft, body = value.trim();
      if (!draft || body.length === 0) return '';
      pullRequestReviewStore(client).addComment(pullRequestReviewKey(reference), { id: nextPendingReviewCommentId(), path: draft.path, ...(draft.oldPath === null ? {} : { oldPath: draft.oldPath }), position: draft.position, body });
      code.draft = null; code.selection = null;
      return '';
    }
    case 'discard': pullRequestReviewStore(client).removeComment(pullRequestReviewKey(reference), value); return '';
    default: throw new ClientError(`Unknown Code tab action: ${op}`);
  }
}
/** beginComment: the gutter's "+" (or a selection's end) opens the draft on the line it ends on, on the whole change only. */
function beginComment(client: T3Client, code: CodeState, detail: Obj | null, value: string): string {
  const capabilities = obj(detail?.capabilities), review = obj(capabilities.review), permissions = obj(detail?.viewerPermissions);
  if (code.draft || code.commit !== null || review.inlineComment !== true || permissions.comment !== true) return '';
  const [side = '', line = '', path = ''] = fields(value, 3), at = { side: (side === 'deletions' ? 'deletions' : 'additions') as SelectionSide, line: Number(line) };
  const target = filesOf(code).find(entry => entry.path === path);
  if (!target || !(at.line > 0)) return '';
  // The gutter's button comments on the selection when its line is in it, else on that line alone.
  const selected = code.selection?.path === path ? code.selection.range : null;
  const contents = code.contents[contentsKeyOf(code, path)], loaded = contents?.state === 'loaded' ? contents : null;
  const lines = diffReviewLines(target, loaded);
  const probe = (range: SelectedLineRange) => buildDiffReviewComment({ id: 'probe', sectionId: '', sectionTitle: '', filePath: path, lines, range, text: '' });
  const inside = selected && (() => { const whole = probe(selected), one = probe({ start: at.line, side: at.side, end: at.line, endSide: at.side }); return !!whole && !!one && one.startIndex >= whole.startIndex && one.startIndex <= whole.endIndex; })();
  const range: SelectedLineRange = inside && selected ? selected : { start: at.line, side: at.side, end: at.line, endSide: at.side };
  // A range collapses to its last line: only GitHub carries a multi-line comment.
  const position = resolveDiffReviewPosition(target, range.end, range.endSide ?? range.side, loaded);
  if (position === null) return '';
  code.selection = { path, range };
  code.draft = { fileKey: fileKey(target), path, oldPath: target.previous && target.previous !== path && target.status === 'renamed' ? target.previous : null, position, range };
  void client;
  return '';
}
/** The flush (app.contract `prViewedFlush`, 400 ms after the last press): one write, after the one in flight. */
async function flushViewed(client: T3Client, native: Native, code: CodeState, reference: WriteReference): Promise<string> {
  const taken = code.viewed.takeBatch();
  if (!taken) return '';
  const key = code.key;
  const run = async () => {
    let outcome: 'ok' | 'failed' | 'unknown' = 'ok', interrupted = false;
    try { await client.rpc(native, 'pullRequests.setFilesViewed', { ...host(reference), files: taken.batch }, true); }
    catch (error) {
      // Let go (a newer press's answer) or the connection gone mid-flight: the host may have it; the next read says.
      const kind = error instanceof ClientError ? error.kind : '';
      interrupted = letGo(error) || ['stale', 'Disconnected', 'Closed', 'Replaced', 'transport'].includes(kind);
      outcome = interrupted ? 'unknown' : 'failed';
    }
    // The reader has moved on: what is on screen has nothing to do with this answer.
    if (peekCode(client)?.key !== key) return;
    const { reverted } = code.viewed.landed(taken, outcome);
    if (outcome === 'failed' && reverted) pushToast(client, { kind: 'error', title: 'Could not update viewed files' });
  };
  const previous = code.viewed.writing;
  code.viewed.writing = previous.then(run, run);
  await code.viewed.writing;
  return '';
}

// ── The tab's host writes (pageslocal:pr-act-code-agent, thread-*) ──────────

/** "Add to agent": the draft's lines and the reader's request, handed to the agent (pages-pr-handoffs.ts). */
export function draftSelection(client: T3Client, reference: WriteReference, detail: Obj, text: string) {
  const code = peekCode(client);
  const draft = code && code.key === `${client.environmentId}|${pullRequestReviewKey(reference)}` ? code.draft : null;
  if (!code || !draft) return null;
  const file = filesOf(code).find(entry => entry.path === draft.path);
  const contents = code.contents[contentsKeyOf(code, draft.path)];
  const comment = file ? buildDiffReviewComment({ id: `pull-request-selection:${draft.fileKey}:${draft.range.start}:${draft.range.end}`, sectionId: `pull-request:${num(detail.number)}`,
    sectionTitle: `PR #${num(detail.number)} review`, filePath: draft.path, lines: diffReviewLines(file, contents?.state === 'loaded' ? contents : null), range: draft.range, text }) : null;
  code.draft = null; code.selection = null;
  return comment;
}
export function codeThreadContext(client: T3Client, native: Native, reference: WriteReference, activity: () => Obj | null, refresh: () => void) {
  const code = codeState(client, `${client.environmentId}|${pullRequestReviewKey(reference)}`);
  return { client, native, state: code.threads, reference, threads: () => arr(activity()?.reviewThreads), refresh };
}
export { threadCommand };
