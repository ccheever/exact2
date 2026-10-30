import { DiffFind, markDiffRows, findGap } from '../diff/diff-find';
// The pull request panel's Code tab: PullRequestCodeTab.tsx (T3 Code
// 6d450a4284, MIT, see LICENSE-T3) — the change or one commit's change read a slice at a time
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
// the resource asked again (`wake`), as the panel's other reads do.
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { capitalizeTerm, terminologyOf } from '../details/git-logic';
import { ClientError, type Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { sleep } from '../client/command-waits';
import { pushToast } from '../shell/toast';
import { buildDiffReviewComment, diffReviewLines, findDiffReviewLineIndex, type SelectedLineRange, type SelectionSide } from '../diff/diff-comments';
import { dragTo, gutterClick, parseLineCellId, pressGutter, pressLine, releaseDrag, type LineDrag, type LinePoint, type RowIndex } from '../diff/diff-line-drag';
import { changeType, expandRange, type Expansion, type FileContents } from '../diff/diff-lazy';
import { diffFileTreeEntries, diffTreeRows, ancestorDirectories, collectDirectoryPaths, allDirectoriesExpanded, orderDiffFilesLikeTree, type DiffTreeRow } from '../diff/diff-tree';
import { hideTestFiles, setHideTestFiles, shownDiffFiles, testFileCount } from '../diff/hide-test-files';
import { decodeClientPrefs } from '../settings/settings-core';
import { rememberDiffLayout } from '../appearance/settings-appearance-look';
import { pagesPrefs } from '../pages/pages-prefs';
import { sourceControlHostName } from '../shared/source-control-hosts';
import { nextPendingReviewCommentId, pullRequestReviewKey, pullRequestReviewStore } from './pages-pr-writes-logic';
import type { WriteReference } from './pages-pr-writes';
import {
  adoptDiffSlice, areAllDiffFilesCollapsed, describePullRequestDiffError, fnv1a32, getFileViewedRange, isLineInFileDiff, orderDiffFiles, renderablePatch, resolveDiffReviewAnchor, reviewPositionAnchor, toggleFileDiffFoldForViewed,
  type DiffFoldOverride, type DiffSlice, type PrDiffFile, type PrDiffSide, type PullRequestReviewPosition, type RenderablePatch,
} from './pages-pr-code-logic';
import { BASE, codeRows, fileKey, hiddenCount, isCollapsed, type CodeItem } from './pages-pr-code-rows';
import { FLUSH_DELAY_MS, FilesViewedStore } from './pages-pr-viewed';
import { startDetached, type DetachedReply } from '../composer/composer-replies';
import { startFleetDetached } from '../usage/usage-replies';
import { environmentCall, environmentTarget, pullRequestServer, routedPullRequestRequest, routerHost } from './pages-pr-environments';
import { pullRequestRouter } from './pages-pr-routing';
import { emptyThreadState, presentThreads, threadBodies, threadCommand, type PrThreadCard, type ThreadState } from './pages-pr-threads';

/** Commits per press of "Show more" in the scope menu. */
export const COMMIT_PAGE_SIZE = 10;

type Draft = { fileKey: string; path: string; oldPath: string | null; position: PullRequestReviewPosition; startPosition?: PullRequestReviewPosition; range: SelectedLineRange };
export type CodeState = {
  find: DiffFind;
  key: string; mounted: boolean; commit: string | null; refresh: number;
  /** The server's refresh epoch last seen (pullRequests.subscribeRefreshes), null before the first read. */
  epoch: number | null;
  slices: readonly DiffSlice[]; cursor: string | null; diffDue: boolean; diffError: string;
  parsed: Map<string, RenderablePatch>;
  toggled: ReadonlySet<string>; foldOverride: DiffFoldOverride; visibleCommits: number;
  ignoreWhitespace: boolean; wrap: boolean; orphansOpen: boolean; treeClosed: Set<string>; treeSelected: string;
  reveal: { path: string; nonce: number; pending: boolean } | null;
  selection: { path: string; range: SelectedLineRange } | null; drag: LineDrag | null; draft: Draft | null;
  contents: Record<string, FileContents>; expansions: Record<string, Expansion>; contentsDue: Map<string, { path: string; gap: number }>;
  viewed: FilesViewedStore; threads: ThreadState; shown: string;
  /** The file whose Viewed tick was pressed last: a Shift-press marks every file from it to the pressed one. */
  lastViewed: string | null;
};
const states = new WeakMap<object, CodeState>();
const prefs = (client: { local: object }) => decodeClientPrefs((client.local as { clientSettings?: unknown }).clientSettings);
export function createCodeState(client: { local: object }, key: string): CodeState {
  const settings = prefs(client);
  return { find: new DiffFind(), key, mounted: false, commit: null, refresh: 0, epoch: null, slices: [], cursor: null, diffDue: true, diffError: '', parsed: new Map(), toggled: new Set(), foldOverride: null,
    visibleCommits: COMMIT_PAGE_SIZE, ignoreWhitespace: settings.diffIgnoreWhitespace, wrap: settings.wordWrap, orphansOpen: false, treeClosed: new Set(), treeSelected: '',
    reveal: null, selection: null, drag: null, draft: null, contents: {}, expansions: {}, contentsDue: new Map(), viewed: new FilesViewedStore(), threads: emptyThreadState(), shown: '', lastViewed: null };
}
/** The Code tab of the pull request on screen; another pull request starts over. */
export function codeState(client: { local: object }, key: string): CodeState {
  let state = states.get(client);
  if (!state || state.key !== key) { state = createCodeState(client, key); states.set(client, state); }
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
  code.draft = null; code.selection = null; code.drag = null; code.toggled = new Set(); code.foldOverride = null; code.visibleCommits = COMMIT_PAGE_SIZE; code.orphansOpen = false; code.lastViewed = null;
  restart(code);
}

/**
 * `environmentId`: the server the pull request was listed on ('' for the focused one). The
 * tab's reads and its Viewed writes go to that server, as the reference's do (pullRequests.ts: the diff is the pull request
 * environment's own HTTP read, filesViewed, setFilesViewed and diffFileContents its routed requests); sent to the focused
 * server instead, a pull request listed on a paired server answered HTTP 503 (provider-unsupported) from the embedded one.
 */
export type CodeContext = { client: T3Client; reference: WriteReference; detail: Obj; activity: Obj | null; environmentId?: string; state?: CodeState;
  /** Whether "Add to agent" can take the lines (onAddToAgentSelection): agent context is local, so lines are selectable without comment permission. */
  allowAsk?: boolean };
const host = (reference: WriteReference) => ({ projectId: reference.projectId, ...(reference.host ? { host: reference.host } : {}), repository: reference.repository, number: reference.number });
const commitsOf = (activity: Obj | null) => arr(activity?.commits).slice().sort((left, right) => Date.parse(str(right.committedDate)) - Date.parse(str(left.committedDate)));
const contentsKeyOf = (code: CodeState, path: string) => `${scopeKey(code)}::${path}`;

// ── Reads ───────────────────────────────────────────────────────────────────

/** What the tab shows that a read changes; a change is shown before the next read. */
function signature(code: CodeState): string {
  return [scopeKey(code), code.slices.length, code.cursor, code.diffDue, code.diffError, code.viewed.due, code.viewed.key(), code.contentsDue.size].join('|');
}
/**
 * The tab's reads, after the panel's own: the ticks, the slice owed, the contents a hidden range
 * asked for. 'wake' when something new should be shown before they run (the caller presents and
 * asks again); the reads run otherwise.
 */
export async function readCode(ctx: CodeContext & { native: Native; tab: string; refresh: number; epoch?: number }): Promise<'wake' | 'done'> {
  const { client, native, reference, detail, activity } = ctx, key = pullRequestReviewKey(reference), listedOn = ctx.environmentId ?? '';
  const code = ctx.state ?? codeState(client, `${client.environmentId}|${key}`);
  await tellReaders(client, native);
  // backgroundRefreshToken: a change the server announced (a turn finishing) reads the first slice and
  // the ticks again with the loaded slices kept, so the list keeps its scroll; adoptDiffSlice drops what changed.
  if (ctx.epoch !== undefined && ctx.epoch !== code.epoch) {
    // A first slice that failed is read again too (its error gives way to the answer).
    if (code.epoch !== null && code.mounted && (code.slices.length > 0 || code.diffError)) { code.cursor = null; code.diffDue = true; code.diffError = ''; code.viewed.due = true; }
    code.epoch = ctx.epoch;
  }
  if (ctx.tab === 'code') code.mounted = true;
  if (!code.mounted || obj(detail.capabilities).diff !== true) return 'done';
  // The panel's Refresh goes around the host's cache: the diff from its first page, and the ticks.
  if (ctx.refresh !== code.refresh) { const first = code.refresh === 0 && code.slices.length === 0; code.refresh = ctx.refresh; if (!first) { restart(code); code.viewed.due = true; } }
  // A rebase can take the scoped commit out of the change: the scope goes back to the whole change.
  if (code.commit !== null && activity && !commitsOf(activity).some(entry => str(entry.oid) === code.commit)) scope(code, null);
  const viewedOn = obj(detail.capabilities).viewedFiles !== undefined;
  if (!(code.diffDue || (viewedOn && code.viewed.due) || code.contentsDue.size > 0 || code.viewed.flushable())) { code.shown = signature(code); return 'done'; }
  if (code.shown !== signature(code)) { code.shown = signature(code); return 'wake'; }
  // The Viewed ticks gather FLUSH_DELAY_MS after the last press, then go in one write.
  if (code.viewed.flushable()) {
    const presses = code.viewed.key();
    await sleep(FLUSH_DELAY_MS);
    if (code.viewed.key() === presses) await flushViewed(client, native, code, reference, listedOn);
  }
  if (viewedOn && code.viewed.due) {
    try { code.viewed.adopt(obj(await routedPullRequestRequest(client, native, 'pullRequests.filesViewed', { ...host(reference), environmentId: listedOn }, false, true)) as never); }
    catch (error) { if (letGo(error)) throw error; code.viewed.failed(error instanceof Error && error.message ? error.message : 'The host did not answer.'); }
  }
  if (code.diffDue) {
    const asked = scopeKey(code), cursor = code.cursor;
    try {
      const payload = { ...host(reference), ...(cursor === null ? {} : { cursor }), ...(code.commit === null ? {} : { commit: code.commit }) };
      const answer = obj(await environmentCall(client, native, pullRequestServer(client, listedOn, payload), { op: 'prDiff', payload }));
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
      const answer = obj(await routedPullRequestRequest(client, native, 'pullRequests.diffFileContents', { ...diffFileContentsInput(reference, code.commit, file), environmentId: listedOn }, false, true));
      code.contents[contentsKey] = { state: 'loaded', oldContents: str(answer.oldContents), newContents: str(answer.newContents), error: '' };
      const gapKey = `${contentsKey}::${want.gap}`;
      code.expansions[gapKey] = expandRange(code.expansions[gapKey], hiddenCount(file, want.gap, code.contents[contentsKey]));
    } catch (error) {
      if (letGo(error)) { code.contentsDue.set(contentsKey, want); throw error; }
      code.contents[contentsKey] = { state: 'error', oldContents: '', newContents: '', error: error instanceof Error && error.message ? error.message : 'Unable to load file contents.' };
    }
  }
  if (code.reveal?.pending && !applyReveal(client, code)) {
    const next = code.slices.at(-1)?.nextCursor;
    if (next && !code.diffError) { code.cursor = next; code.diffDue = true; return 'wake'; }
  }
  code.shown = signature(code);
  return 'done';
}
/** Apply a reveal once; later I/O preserves the reader's folds, folders and selection. */
function applyReveal(client: T3Client, code: CodeState): boolean {
  const reveal = code.reveal;
  if (!reveal?.pending) return false;
  const target = filesOf(code).find(file => file.path === reveal.path);
  if (!target) return false;
  if (isCollapsed({ foldOverride: effectiveFold(client, code), toggled: code.toggled }, target)) {
    const toggled = new Set(code.toggled);
    if (toggled.has(target.path)) toggled.delete(target.path); else toggled.add(target.path);
    code.toggled = toggled;
  }
  code.treeSelected = target.path;
  for (const directory of ancestorDirectories(target.path)) code.treeClosed.delete(directory);
  reveal.pending = false;
  return true;
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
/** Every loaded file, in the file tree's order (orderDiffFilesLikeTree): a test listed last sits in its directory. */
export function filesOf(code: CodeState): PrDiffFile[] { return orderDiffFilesLikeTree(parsedSlices(code).flatMap(parsed => (parsed?.kind === 'files' ? orderDiffFiles(parsed.files) : [])), file => file.path); }
/** The files the list shows: while tests are hidden, a file asked for stays so that a jump to it still lands. */
const shownFiles = (client: { local: object }, code: CodeState) => shownDiffFiles(filesOf(code), file => file.path, hideTestFiles(client), code.reveal?.path ?? '');

// ── The view ────────────────────────────────────────────────────────────────

export type PrCodeView = ReturnType<typeof emptyCode>;
export function emptyCode() {
  return {
    available: false, mounted: false, key: '', phase: 'loading', message: '', raw: [] as { id: string; reason: string; text: string }[],
    // PullRequestDiffUnavailableState: what a failed first slice says, and where the host shows the change.
    errorTitle: '', errorDescription: '', providerName: '', url: '',
    // DiffPanelLoadingState's label, in the host's term.
    loadingLabel: 'Loading pull request diff...',
    scopeLabel: 'All commits', allSelected: true, hasCommits: false, commits: [] as { oid: string; short: string; headline: string; selected: boolean }[], moreCommits: 0,
    filesLabel: '0 files', viewedShown: false, viewedCount: '', viewedWord: 'viewed', viewedHere: false, viewedError: '', viewedTruncated: false, withheld: false, commentsOff: false,
    ignoreWhitespace: true, allCollapsed: false, hasFiles: false, layout: 'stacked', wrap: true, tree: false,
    // Hide test files: the loaded tests, whether they are hidden, and the meta line's "N test files hidden".
    testFiles: 0, hideTests: false, hiddenTests: '',
    find: new DiffFind().view(), items: [] as CodeItem[], truncated: false, threads: [] as PrThreadCard[], footer: '', footerFailed: false,
    orphansLabel: '', orphanCount: 0, orphanSummary: '', orphansOpen: false, orphans: [] as { path: string; threads: { id: string; line: string }[] }[],
    treeRows: [] as DiffTreeRow[], treeLabel: '', treeCount: 0, treeFolders: false, treeAllOpen: true, treeFooter: '', treeBusy: false,
    // `canComment`: lines can be selected (the gutter is on); `reviewable`: the draft can join the review, else it goes to the agent only.
    revealPath: '', revealNonce: 0, draftOpen: false, canComment: false, reviewable: false, viewedQueued: 0, threadPending: false,
  };
}
type Body = { id: string; kind: string; title: string; body: string };
/** The tab as it stands, and the markdown documents its conversations render. */
export function presentCode(ctx: CodeContext & { now: number }): { view: PrCodeView; bodies: Body[] } {
  const { client, reference, detail, activity } = ctx, view = emptyCode();
  const capabilities = obj(detail.capabilities), permissions = obj(detail.viewerPermissions);
  const term = terminologyOf(str(detail.provider) || undefined).singular;
  view.loadingLabel = `Loading ${term} diff...`;
  view.available = capabilities.diff === true;
  const code = ctx.state ?? peekCode(client);
  if (!code || (!ctx.state && code.key !== `${client.environmentId}|${pullRequestReviewKey(reference)}`) || !code.mounted || !view.available) return { view, bodies: [] };
  view.mounted = true; view.key = scopeKey(code);
  if (code.reveal && filesOf(code).some(file => file.path === code.reveal?.path)) { view.revealPath = code.reveal.path; view.revealNonce = code.reveal.nonce; }
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
  const canSelectLines = code.commit === null && (inlineComment || ctx.allowAsk === true);
  view.commentsOff = code.commit !== null && inlineComment;
  const parsed = parsedSlices(code), loaded = filesOf(code), files = shownFiles(client, code), paths = files.map(file => file.path);
  view.testFiles = testFileCount(loaded, file => file.path); view.hideTests = hideTestFiles(client);
  const hiddenTests = loaded.length - files.length;
  view.hiddenTests = hiddenTests > 0 ? `${hiddenTests} test ${hiddenTests === 1 ? 'file' : 'files'} hidden` : '';
  const nextCursor = code.slices.at(-1)?.nextCursor ?? null;
  view.withheld = code.slices.some(slice => slice.truncated) || parsed.some(entry => entry?.kind === 'raw');
  view.filesLabel = `${files.length} ${files.length === 1 ? 'file' : 'files'}${nextCursor === null ? '' : '+'}`;
  const store = obj(capabilities).viewedFiles;
  view.viewedShown = store !== undefined && files.length > 0;
  view.viewedCount = `${code.viewed.count(paths)} / ${files.length}`; view.viewedHere = store === 'environment';
  view.viewedWord = store === 'environment' ? 'viewed in T3 Code' : 'viewed';
  view.viewedError = code.viewed.error ?? ''; view.viewedTruncated = code.viewed.truncated;
  view.viewedQueued = code.viewed.queuedCount();
  // A conversation is placed only on a line a rendered hunk holds, and never under a commit scope.
  const threads = arr(activity?.reviewThreads), placed = new Set<string>();
  if (code.commit === null) for (const file of files) for (const thread of threads) {
    if (str(thread.path) === file.path && num(thread.line) > 0 && isLineInFileDiff(file, thread.side === 'left' ? 'left' : 'right', num(thread.line))) placed.add(str(thread.id));
  }
  const pending = code.commit === null ? pullRequestReviewStore(client).pending(pullRequestReviewKey(reference)) : [];
  const draft = code.draft, omitted = new Map(code.slices.flatMap(slice => slice.omittedFileStats.map(file => [file.path, file] as const)));
  const sourceStats = new Map(parsedSlices(code).flatMap(parsed => parsed?.kind === 'files' ? parsed.sourceFiles.map(file => [file.path, file] as const) : []));
  code.find.reset(scopeKey(code));
  code.find.search(files.map(file => {
    const contents = code.contents[contentsKeyOf(code, file.path)];
    return { path: file.path, lines: diffReviewLines(file, contents?.state === 'loaded' ? contents : null) };
  }));
  const findMatch = code.find.active;
  if (code.find.open && findMatch) {
    const target = files.find(file => file.path === findMatch.path);
    const gap = target && findMatch.context ? findGap(target, findMatch.line) : null;
    if (gap) { const key = `${contentsKeyOf(code, findMatch.path)}::${gap.gap}`, previous = code.expansions[key]; code.expansions[key] = { fromStart: Math.max(previous?.fromStart ?? 0, gap.through), fromEnd: previous?.fromEnd ?? 0 }; }
    if (target && isCollapsed({ foldOverride: effectiveFold(client, code), toggled: code.toggled }, target)) {
      const toggled = new Set(code.toggled); if (toggled.has(target.path)) toggled.delete(target.path); else toggled.add(target.path); code.toggled = toggled;
    }
  }
  const rows = codeRows({
    finding: code.find.open, sourceStats, files, layout: view.layout as 'stacked' | 'split', foldOverride: effectiveFold(client, code), toggled: code.toggled, omitted,
    viewedMark: path => (!view.viewedShown ? '' : code.viewed.isStale(path) ? 'changed' : code.viewed.isViewed(path) ? 'viewed' : 'unviewed'),
    annotations: path => ({
      threads: threads.filter(thread => placed.has(str(thread.id)) && str(thread.path) === path).map(thread => ({ id: str(thread.id), side: (thread.side === 'left' ? 'left' : 'right') as PrDiffSide, line: num(thread.line) })),
      pending: pending.filter(comment => comment.path === path).map(comment => ({ id: comment.id, body: comment.body, position: comment.position as PullRequestReviewPosition })),
      draft: draft && canSelectLines && draft.path === path ? { position: draft.position, label: `${draft.path}:${reviewPositionAnchor(draft.position).line}` } : null,
    }),
    selection: draft ? { path: draft.path, range: draft.range } : code.selection, contents: code.contents, contentsKey: path => contentsKeyOf(code, path), expansions: code.expansions,
  });
  const marked = markDiffRows(rows.items, code.find); view.find = code.find.view(marked.target);
  view.items = marked.items; view.truncated = rows.truncated; view.draftOpen = draft !== null && canSelectLines; view.canComment = canSelectLines; view.reviewable = canCommentOnLines;
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
  view.treeRows = diffTreeRows(entries, code.treeClosed, code.treeSelected); view.treeLabel = `${capitalizeTerm(term)} #${num(detail.number)} files`; view.treeCount = entries.length;
  view.treeFolders = collectDirectoryPaths(paths).length > 0; view.treeAllOpen = allDirectoriesExpanded(paths, code.treeClosed);
  view.treeFooter = nextCursor === null ? '' : code.diffError ? 'Retry' : code.diffDue ? 'Loading more files...' : 'Load more files';
  view.treeBusy = nextCursor !== null && code.diffDue;
  // The states that take the list's place: loading, the error, the raw text, nothing changed.
  const raw = nextCursor === null ? parsed.flatMap((entry, index) => (entry?.kind === 'raw' ? [{ id: `raw:${index}`, reason: entry.reason, text: entry.text }] : [])) : [];
  view.raw = raw;
  if (code.slices.length === 0 && code.diffError) {
    view.phase = 'error'; view.message = code.diffError;
    view.providerName = sourceControlHostName(str(detail.provider, 'github')); view.url = str(detail.url);
    const words = describePullRequestDiffError(code.diffError, view.providerName, term);
    view.errorTitle = words.title; view.errorDescription = words.description;
  }
  else if (code.slices.length === 0) view.phase = 'loading';
  else if (files.length === 0 && raw.length > 0) view.phase = 'raw';
  else if (rows.items.length === 0 && nextCursor === null) { view.phase = 'empty'; view.message = code.commit === null ? `This ${term} has no file changes.` : 'This commit has no file changes.'; }
  else view.phase = 'files';
  return { view, bodies: threadBodies(code.threads, threads) };
}
/** The toolbar's last word, else the saved default (General → Diff files collapsed). */
const effectiveFold = (client: { local: object }, code: CodeState): DiffFoldOverride => code.foldOverride ?? (prefs(client).diffFilesCollapsed ? 'folded' : 'expanded');

// ── The tab's presses (chatlocal:pr-code-*) ─────────────────────────────────

const fields = (value: string, count: number) => { const parts: string[] = []; let rest = value; for (let i = 1; i < count; i++) { const bar = rest.indexOf('|'); if (bar < 0) break; parts.push(rest.slice(0, bar)); rest = rest.slice(bar + 1); } parts.push(rest); return parts; };
export type CodeLocalContext = { client: T3Client; reference: WriteReference; detail: Obj | null; state?: CodeState; allowAsk?: boolean };
/**
 * `chatlocal:pr-code-<op>`: what the tab's controls change. None of them reads or writes the host:
 * the resource does, once they made something due (the ticks' write too).
 */
export async function prCodeLocal(ctx: CodeLocalContext & { native: Native }, op: string, value: string): Promise<string> {
  const { client, reference } = ctx;
  const code = ctx.state ?? codeState(client, `${client.environmentId}|${pullRequestReviewKey(reference)}`);
  if (op.startsWith('find-')) { code.find.action(op.slice(5), value); return ''; }
  const files = () => filesOf(code), file = (path: string) => files().find(entry => entry.path === path);
  switch (op) {
    case 'scope': code.mounted = true; scope(code, value || null); return '';
    case 'more-commits': code.visibleCommits += COMMIT_PAGE_SIZE; return '';
    case 'whitespace': code.ignoreWhitespace = !code.ignoreWhitespace; code.draft = null; code.selection = null; return '';
    case 'wrap': code.wrap = !code.wrap; return '';
    case 'layout': if (value === 'stacked' || value === 'split') rememberDiffLayout(client, value); return '';
    case 'tree': { const prefs = pagesPrefs(client); prefs.prFileTree = prefs.prFileTree !== true; return ''; }
    case 'fold-all': {
      const fold = effectiveFold(client, code), all = shownFiles(client, code);
      code.foldOverride = areAllDiffFilesCollapsed(all.map(fileKey), new Set(all.filter(entry => isCollapsed({ foldOverride: fold, toggled: code.toggled }, entry)).map(fileKey))) ? 'expanded' : 'folded';
      code.toggled = new Set();
      return '';
    }
    case 'fold': { const next = new Set(code.toggled); if (next.has(value)) next.delete(value); else next.add(value); code.toggled = next; return ''; }
    case 'viewed': {
      // The tick and the fold are one gesture: clearing a file puts it away, un-clearing brings it back. A Shift-press
      // gives every file shown from the last pressed one to this one the same tick.
      const [path = '', viewed = '', shift = ''] = fields(value, 3), on = viewed === 'true';
      if (!path) return '';
      const range = getFileViewedRange(shownFiles(client, code).map(entry => ({ fileKey: entry.path })), path, shift === 'shift' ? code.lastViewed : null);
      code.lastViewed = path;
      const fold = effectiveFold(client, code);
      for (const entry of range) { code.viewed.setViewed(entry.fileKey, on); code.toggled = toggleFileDiffFoldForViewed(entry.fileKey, on, fold, code.toggled); }
      return '';
    }
    case 'hide-tests': setHideTestFiles(client, value === '' ? !hideTestFiles(client) : value === 'true'); return '';
    case 'next': {
      const nextCursor = code.slices.at(-1)?.nextCursor ?? null;
      // A failed slice is not asked for again on its own: Retry asks.
      if (nextCursor !== null && nextCursor !== code.cursor && !code.diffDue && !code.diffError) { code.cursor = nextCursor; code.diffDue = true; }
      return '';
    }
    case 'retry': if (code.diffError) { code.diffError = ''; code.diffDue = true; } return '';
    case 'orphans': code.orphansOpen = !code.orphansOpen; return '';
    case 'folder': { if (code.treeClosed.has(value)) code.treeClosed.delete(value); else code.treeClosed.add(value); return ''; }
    case 'folders': code.treeClosed = value === 'collapse' ? new Set(collectDirectoryPaths(shownFiles(client, code).map(entry => entry.path))) : new Set(); return '';
    case 'reveal': {
      // revealFile: a folded file opens; its folders open; the list scrolls to it (the contract's scrollIntoView).
      code.reveal = { path: value, nonce: (code.reveal?.nonce ?? 0) + 1, pending: true };
      if (!applyReveal(client, code)) {
        const next = code.slices.at(-1)?.nextCursor;
        if (next && !code.diffDue) { code.cursor = next; code.diffDue = true; }
      }
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
    case 'row': case 'drag': {
      // DiffRow's commands: `diffreview|expand|<gap>|<path>`, and the gutter's drags (diff-line-drag.ts): `drag:<side>[:shift]`
      // and `gutter:<side>` on a press on a line number or the "+", `to` with the cell id under the pointer, `end` on the release.
      const [, id = '', n = '', path = ''] = fields(value, 4), [action = '', side = '', shift = ''] = id.split(':');
      if (action === 'expand') return prCodeLocal(ctx, 'expand', `${path}|${n}`);
      if (action === 'drag' || action === 'gutter' || action === 'to' || action === 'end') return lineDrag(ctx, code, action, side, shift === 'shift', Number(n), path);
      return '';
    }
    case 'begin': return beginComment(ctx, code, value);
    case 'cancel': code.draft = null; code.selection = null; return '';
    case 'add': {
      // "Add to review": the comment joins the pending review (pages-pr-writes-logic.ts PullRequestReviewStore).
      const draft = code.draft, body = value.trim();
      if (!draft || body.length === 0 || obj(obj(ctx.detail?.capabilities).review).inlineComment !== true || obj(ctx.detail?.viewerPermissions).comment !== true) return '';
      pullRequestReviewStore(client).addComment(pullRequestReviewKey(reference), { id: nextPendingReviewCommentId(), path: draft.path, ...(draft.oldPath === null ? {} : { oldPath: draft.oldPath }),
        ...(draft.startPosition === undefined ? {} : { startPosition: draft.startPosition }), position: draft.position, body });
      code.draft = null; code.selection = null;
      return '';
    }
    case 'discard': pullRequestReviewStore(client).removeComment(pullRequestReviewKey(reference), value); return '';
    default: throw new ClientError(`Unknown Code tab action: ${op}`);
  }
}
/** Whether the viewer can comment on lines now (PullRequestCodeTab's canCommentOnLines and no draft open): the gutter is on. */
function linesOpen(code: CodeState, detail: Obj | null, allowAsk = false): boolean {
  const capabilities = obj(detail?.capabilities), review = obj(capabilities.review), permissions = obj(detail?.viewerPermissions);
  return !code.draft && code.commit === null && (allowAsk || (review.inlineComment === true && permissions.comment === true));
}
const loadedContents = (code: CodeState, path: string) => { const contents = code.contents[contentsKeyOf(code, path)]; return contents?.state === 'loaded' ? contents : null; };
/**
 * beginComment from the gutter's "+" press (`chatlocal:pr-code-begin`, on the drags' queued send). Its pointer gesture owns
 * the click: a press while that gesture is in flight is its release, one after it finds the draft open. A press no gesture
 * carried (a keyboard or accessibility press) comments on what the gesture would: the selection's top to bottom, else its
 * line (diff-line-drag.ts gutterClick); the draft goes on the range's last line, on the whole change only.
 */
function beginComment(ctx: CodeLocalContext, code: CodeState, value: string): string {
  if (code.drag?.mode === 'gutter') return lineDrag(ctx, code, 'end', '', false, 0, '');
  if (!linesOpen(code, ctx.detail, ctx.allowAsk)) return '';
  const [side = '', line = '', path = ''] = fields(value, 3), at: LinePoint = { side: (side === 'deletions' ? 'deletions' : 'additions') as SelectionSide, line: Number(line) };
  const target = filesOf(code).find(entry => entry.path === path);
  if (!target || !(at.line > 0)) return '';
  const lines = diffReviewLines(target, loadedContents(code, path));
  const index: RowIndex = point => { const found = findDiffReviewLineIndex(lines, point.line, point.side); return found < 0 ? null : found; };
  return beginRange(code, ctx.detail, path, gutterClick(code.selection, path, at, index), ctx.allowAsk);
}
/**
 * The draft on a range (beginComment's body). On a host without ranged comments (`review.rangeComment`) a range collapses
 * to its last line: one that silently lost its first line there would be worse than one line.
 */
function beginRange(code: CodeState, detail: Obj | null, path: string, range: SelectedLineRange, allowAsk = false): string {
  const target = filesOf(code).find(entry => entry.path === path);
  if (!target || !linesOpen(code, detail, allowAsk)) return '';
  const anchor = resolveDiffReviewAnchor(target, range, obj(obj(detail?.capabilities).review).rangeComment === true, loadedContents(code, path));
  if (anchor === null) return '';
  code.selection = { path, range };
  code.draft = { fileKey: fileKey(target), path, oldPath: target.previous && target.previous !== path && target.status === 'renamed' ? target.previous : null, ...anchor, range };
  return '';
}
/**
 * The gutter's drags (diff-line-drag.ts): a press on a line number or on the "+" starts one, each
 * line the pointer reaches moves its end, and the release opens the draft on what it ended on (PullRequestCodeTab's
 * onGutterUtilityClick and onLineSelectionEnd are both beginComment). Off while a draft is open or a commit is scoped.
 */
function lineDrag(ctx: CodeLocalContext, code: CodeState, action: string, side: string, shift: boolean, line: number, value: string): string {
  if (action === 'end') {
    const { selection, gutter, ended } = releaseDrag(code.drag, code.selection), path = code.drag?.path ?? '';
    code.drag = null; code.selection = selection;
    const range = gutter ?? ended;
    return range ? beginRange(code, ctx.detail, path, range, ctx.allowAsk) : '';
  }
  if (action === 'to') {
    const hit = parseLineCellId(value);
    if (hit) ({ drag: code.drag, selection: code.selection } = dragTo(code.drag, code.selection, hit.path, hit.at));
    return '';
  }
  if (!linesOpen(code, ctx.detail, ctx.allowAsk) || !(line > 0)) { code.drag = null; return ''; }
  const path = value, at: LinePoint = { line, side: side === 'deletions' ? 'deletions' : 'additions' };
  const target = filesOf(code).find(entry => entry.path === path);
  if (!target) return '';
  const lines = diffReviewLines(target, loadedContents(code, path));
  const index: RowIndex = point => { const found = findDiffReviewLineIndex(lines, point.line, point.side); return found < 0 ? null : found; };
  ({ drag: code.drag, selection: code.selection } = action === 'gutter' ? pressGutter(code.selection, path, at, index) : pressLine(code.selection, path, at, shift, index));
  return '';
}
const SET_FILES_VIEWED = 'pullRequests.setFilesViewed';
/** Viewed writes the host answered whose readers are not told yet: the router's `afterWrite`, run by the panel's next answer. */
const accepted = new WeakMap<object, { origin: string; payload: Obj }[]>();
/**
 * The reference's `finish` for each answered Viewed write (pullRequestRouting.ts), accepted or failed, since a failed batch
 * may have applied some paths: the servers read through for the pull request get `filesViewedOnly` invalidations, before
 * the read that follows the write. One the answer let go stays for the next.
 */
async function tellReaders(client: T3Client, native: Native): Promise<void> {
  const pending = accepted.get(client);
  while (pending && pending.length > 0) {
    const write = pending[0]!;
    await pullRequestRouter.afterWrite(routerHost(client, native, write.origin), SET_FILES_VIEWED, write.payload);
    pending.shift();
  }
}
/**
 * The flush: one `setFilesViewed` of the gathered presses, sent detached (its reply lands with the
 * next snapshot's drain, composer-replies.ts), so no answer waits on it and none can lose it. A
 * write that never left (its answer let go first) is put back for the next flush: the write is
 * idempotent, so sending it again is safe.
 *
 * It is routed from the pull request's own server, as the reference's
 * command is (`routedRequest`, pullRequestRouting.ts lists it in `writes`): the identity probes,
 * then the first server in the reference's order that shares the GitHub account and is trusted to
 * write, a server that cannot take it now passing it to the next (`dispatch`). No server that can
 * take it now (the pull request's own not connected and no other trusted): the batch waits for the
 * next flush. A refusal that comes back in the reply (a guard that saw another account after the
 * probe) fails the batch, as any failed write does: the route is behind the answer that sent it.
 */
async function flushViewed(client: T3Client, native: Native, code: CodeState, reference: WriteReference, listedOn: string): Promise<void> {
  const taken = code.viewed.takeBatch();
  if (!taken) return;
  const key = code.key, payload = { ...host(reference), files: taken.batch }, origin = pullRequestServer(client, listedOn, payload);
  const out: { sent?: { reply: Promise<DetachedReply> } } = {};
  const send = async (environmentId: string, routed: Obj): Promise<Obj> => {
    const remote = environmentTarget(client, environmentId);
    if (environmentId && environmentId !== client.environmentId && !remote) throw new ClientError('The environment was removed.', 'EnvironmentRpcUnavailableError');
    out.sent = remote ? await startFleetDetached(native, remote, SET_FILES_VIEWED, routed) : await startDetached(client, native, SET_FILES_VIEWED, routed);
    return {};
  };
  try { await pullRequestRouter.dispatch(routerHost(client, native, origin), SET_FILES_VIEWED, payload, send); }
  catch (error) {
    if (!out.sent) code.viewed.requeue(taken);
    if (letGo(error)) throw error;
  }
  if (!out.sent) return;
  void out.sent.reply.then(answer => {
    // A superseded write never reached the host; any other answer may have changed what the readers hold.
    if (answer.ok || answer.error.kind !== 'superseded') { const pending = accepted.get(client) ?? []; pending.push({ origin, payload }); accepted.set(client, pending); }
    // The reader has moved on: what is on screen has nothing to do with this answer.
    if (peekCode(client)?.key !== key) return;
    if (answer.ok) { code.viewed.landed(taken, 'ok'); return; }
    if (answer.error.kind === 'superseded') { code.viewed.requeue(taken); return; }
    // Silent when the connection went away mid-flight (the host may have it; the next read says).
    const { reverted } = code.viewed.landed(taken, answer.interrupted ? 'unknown' : 'failed');
    // A partly applied batch: the marks are read again rather than left where the revert put them.
    if (!answer.interrupted && reverted) { pushToast(client, { kind: 'error', title: 'Could not update viewed files' }); code.viewed.due = true; }
  });
}

// ── The tab's host writes (pageslocal:pr-act-code-agent, thread-*) ──────────

/** "Add to agent": the draft's lines and the reader's request, handed to the agent (pages-pr-handoffs.ts). */
export function draftSelection(client: T3Client, reference: WriteReference, detail: Obj, text: string, state?: CodeState) {
  const code = state ?? peekCode(client);
  const draft = code && (state || code.key === `${client.environmentId}|${pullRequestReviewKey(reference)}`) ? code.draft : null;
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
