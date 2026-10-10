// 20261005-pr-code-tab: the Code tab through the panel's resource (pages-pr-detail.ts) and its
// presses and writes, against injected host replies — the slices of a 310-file change, a slice that
// fails, a commit scope, the Viewed ticks and their one write, line comments into the review, and
// the conversations' commands — the delays and failures real GitHub cannot be made to produce on
// request. The live rows ran against the real-GitHub lane (the task record).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { prCodeLocalFor, prCommand, pullRequestDetail, type DetailInput } from './pages-pr-detail';
import { diffFileContentsInput, draftSelection } from './pages-pr-code';
import { pullRequestReviewKey, pullRequestReviewStore } from './pages-pr-writes-logic';
import { toasts } from './toast';
import { renderablePatch } from './pages-pr-code-logic';
import { composerReplyEvent } from './composer-replies';
import { presentTimeline } from './pages-pr-timeline';

const NOW = Date.parse('2026-10-08T12:00:00Z');
const at = (minutesAgo: number) => new Date(NOW - minutesAgo * 60_000).toISOString();
const person = (login: string) => ({ login, name: null, avatarUrl: null });
const selected = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'acme/playground', number: 114 });
const reference = { projectId: 'p1', host: 'github.com', repository: 'acme/playground', number: 114 };
const commits = Array.from({ length: 14 }, (_, index) => ({ oid: `${String(index + 1).padStart(2, '0')}${'c'.repeat(38)}`, messageHeadline: `Commit ${index + 1}`, committedDate: at(200 - index * 10) }));
const detail = (over: Obj = {}): Obj => ({
  provider: 'github', projectId: 'p1', repository: 'acme/playground', number: 114, title: 'Add sample data files', body: '', url: 'https://github.com/acme/playground/pull/114',
  author: person('primary'), state: 'open', isDraft: false, changedFiles: 310, additions: 320, deletions: 2, headBranch: 'feature/sample-data', baseBranch: 'main',
  createdAt: at(600), updatedAt: at(60), checks: [], labels: [], reviewers: [],
  capabilities: { diff: true, comment: true, reactions: true, viewedFiles: 'host', review: { inlineComment: true, reply: true, resolve: true, verdicts: ['comment', 'approve', 'request-changes'] }, actions: [] },
  viewerPermissions: { comment: true, resolve: true, verdicts: ['comment', 'approve', 'request-changes'], actions: [] }, ...over,
});
const comment = (id: string, login = 'second', body = `Remark ${id}`) => ({ id, author: person(login), body, createdAt: at(30), url: null, reactions: [] });
const activity = (): Obj => ({
  author: person('primary'), comments: [], commentCount: 0, commits,
  reviewThreads: [
    { id: 't-open', path: 'src/catalog.js', line: 2, side: 'right', isResolved: false, isOutdated: false, comments: [comment('c-open')] },
    { id: 't-resolved', path: 'src/catalog.js', line: 2, side: 'left', isResolved: true, isOutdated: false, comments: [comment('c-resolved', 'primary')] },
    { id: 't-outdated', path: 'src/catalog.js', line: null, side: 'right', isResolved: false, isOutdated: true, comments: [comment('c-outdated')] },
    { id: 't-long', path: 'src/catalog.js', line: 3, side: 'right', isResolved: false, isOutdated: false, commentCount: 12, nextCommentsCursor: 'page-2', comments: [comment('c-long-0', 'primary'), comment('c-long-1')] },
  ],
});
/** The change: src/catalog.js (modified), a rename, a binary file, then data files to 310, a hundred per slice. */
const sourcePatch = [
  'diff --git a/src/catalog.js b/src/catalog.js', '--- a/src/catalog.js', '+++ b/src/catalog.js', '@@ -1,4 +1,4 @@', ' export const items = [];', '-export const size = 0;', '+export const size = 1;', ' export const name = "catalog";', ' export default items;',
  'diff --git a/src/text.js b/src/strings.js', 'similarity index 90%', 'rename from src/text.js', 'rename to src/strings.js', '--- a/src/text.js', '+++ b/src/strings.js', '@@ -1,2 +1,2 @@', ' export const shout = (text) => text.toUpperCase();', '-export const whisper = (text) => text.toLowerCase();', '+export const whisper = (text) => text.toLocaleLowerCase();',
  'diff --git a/assets/logo.png b/assets/logo.png', 'new file mode 100644', 'Binary files /dev/null and b/assets/logo.png differ',
].join('\n');
const dataPatch = (from: number, to: number) => Array.from({ length: to - from }, (_, i) => { const n = String(from + i + 1).padStart(3, '0'); return `diff --git a/data/item-${n}.txt b/data/item-${n}.txt\nnew file mode 100644\n--- /dev/null\n+++ b/data/item-${n}.txt\n@@ -0,0 +1 @@\n+item ${from + i + 1}`; }).join('\n');
const SLICES: Record<string, Obj> = {
  first: { patch: `${sourcePatch}\n${dataPatch(0, 97)}\n`, truncated: true, nextCursor: '100', omittedFileStats: [] },
  '100': { patch: `${dataPatch(97, 197)}\n`, truncated: false, nextCursor: '200', omittedFileStats: [] },
  '200': { patch: `${dataPatch(197, 297)}\n`, truncated: false, nextCursor: '300', omittedFileStats: [] },
  '300': { patch: `${dataPatch(297, 307)}\n`, truncated: false, nextCursor: null, omittedFileStats: [] },
};

type Reply = (payload: Obj) => unknown;
function fixture(replies: Record<string, Reply> = {}, slices: (payload: Obj) => unknown = payload => SLICES[String(payload.cursor ?? 'first')]) {
  const calls: { method: string; payload: Obj }[] = [];
  // A detached write (composer-replies.ts): sent at once, its reply filed for the next drain.
  const delivered: { key: string; reply: Promise<unknown> }[] = [];
  const answer = async (method: string, payload: Obj) => {
    calls.push({ method, payload });
    if (method === 'prDiff') return slices(payload);
    const reply = replies[method] ?? DEFAULTS[method];
    if (!reply) throw new Error(`no reply for ${method}`);
    return reply(payload);
  };
  const client = {
    environmentId: 'env', ready: true, generation: 1, revision: 0, local: {} as Obj, diffState: {},
    config: { environment: { capabilities: { pullRequests: true } } }, shell: { projects: [{ id: 'p1', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/playground' } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj) => answer(method, payload),
    call: async (_native: unknown, request: Obj) => {
      if (request.deliver) { delivered.push({ key: String(request.deliver), reply: answer(String(request.method), (request.payload ?? {}) as Obj) }); return { id: 'sent' }; }
      return answer(request.op === 'prDiff' ? 'prDiff' : String(request.method), (request.payload ?? {}) as Obj);
    },
    ids: async () => [`r${delivered.length + 1}`],
    restAccess: () => ({ call: async () => ({ id: '1-1' }) }),
    savePreferences: async () => {},
  } as unknown as T3Client;
  let wakes = 0;
  const native = { available: true, watch: () => {}, later: async (request: Obj) => { if (request.op === 'r10Wake') wakes++; return { ok: true, generation: 1, value: { at: NOW } }; } } as unknown as Native;
  const ask = (input: Partial<DetailInput> = {}) => pullRequestDetail(client, native, { selected, refresh: 0, now: NOW, tab: 'code', ...input });
  const settle = async (input: Partial<DetailInput> = {}) => { let before = wakes, view = await ask(input); for (let i = 0; wakes > before && i < 12; i++) { before = wakes; view = await ask(input); } return view; };
  const press = (op: string, value = '') => prCodeLocalFor(client, native, op, selected, value);
  const act = (op: string, value = '') => prCommand(client, native, op, selected, value);
  /** The snapshot's drain: every detached reply lands (client.ts composerReplyEvent). */
  const drain = async () => {
    for (const entry of delivered.splice(0)) {
      const value = await entry.reply.then(reply => ({ _reply: reply ?? {} }), (error: unknown) => ({ _replyError: { kind: error instanceof ClientError ? error.kind : 'RPC', message: error instanceof Error ? error.message : String(error) } }));
      composerReplyEvent(client, { key: entry.key, value });
    }
    await Bun.sleep(0);
  };
  return { client, calls, settle, ask, press, act, drain, wakes: () => wakes, of: (method: string) => calls.filter(call => call.method === method) };
}
const DEFAULTS: Record<string, Reply> = {
  'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.filesViewed': () => ({ files: [], truncated: false }),
  'pullRequests.setFilesViewed': () => ({}), 'pullRequests.stack': () => ({}), 'pullRequests.nativeStack': () => null,
};
/** Every slice, as reaching the end of the list asks. */
async function walk(f: ReturnType<typeof fixture>) {
  let view = await f.settle();
  for (let i = 0; i < 6 && view.codeTab.items.some(item => item.kind === 'pr-footer'); i++) { await f.press('next'); view = await f.settle(); }
  return view;
}

describe('the Code tab and its slices (PullRequestCodeTab, POST /api/pull-requests/diff)', () => {
  test('is offered where the host has a patch, and reads nothing until it is opened', async () => {
    const f = fixture();
    const summary = await f.settle({ tab: 'summary' });
    expect([summary.codeTab.available, summary.codeTab.mounted, f.of('prDiff').length]).toEqual([true, false, 0]);
    const azure = fixture({ 'pullRequests.detail': () => detail({ capabilities: { ...detail().capabilities as Obj, diff: false } }) });
    const noPatch = await azure.settle();
    expect([noPatch.codeTab.available, azure.of('prDiff').length]).toEqual([false, 0]);
  });
  test('the first slice, then "Loading more files..." as the end is reached, then the rest: 310 files', async () => {
    const f = fixture();
    const first = await f.ask(); // the ghost, then the detail …
    expect(first.codeTab.mounted).toBe(false);
    const loading = await (async () => { let view = first; for (let i = 0; i < 8 && !view.codeTab.mounted; i++) view = await f.ask(); return view; })();
    expect([loading.codeTab.mounted, loading.codeTab.phase]).toEqual([true, 'loading']); // shown before the read
    let view = await f.settle();
    expect([view.codeTab.phase, view.codeTab.filesLabel, view.codeTab.withheld]).toEqual(['files', '100 files+', true]);
    expect(view.codeTab.items.at(-1)).toMatchObject({ kind: 'pr-footer', label: '', error: false }); // the sentinel
    await f.press('next');
    const asking = await f.ask();
    expect(asking.codeTab.items.at(-1)).toMatchObject({ kind: 'pr-footer', label: 'Loading more files...' });
    view = await walk(f);
    expect([view.codeTab.filesLabel, view.codeTab.items.some(item => item.kind === 'pr-footer')]).toEqual(['310 files', false]);
    expect(f.of('prDiff').map(call => call.payload.cursor ?? 'first')).toEqual(['first', '100', '200', '300']);
    expect(f.of('prDiff')[0]!.payload).toEqual(reference);
    // Files arrive folded (General → Diff files collapsed), each with its Viewed tick; the binary file and the rename say what they are.
    const headers = view.codeTab.items.filter(item => item.kind === 'pr-file');
    expect(headers).toHaveLength(310);
    // orderDiffFiles within a slice: one tier here (no tests, nothing generated), so path order.
    expect([headers[0]!.path, headers[1]!.path, headers.at(-2)!.path, headers.at(-1)!.path]).toEqual(['assets/logo.png', 'data/item-001.txt', 'src/catalog.js', 'src/strings.js'].slice(0, 2).concat(['data/item-306.txt', 'data/item-307.txt']));
    const header = (path: string) => headers.find(item => item.path === path)!;
    expect(['src/catalog.js', 'src/strings.js', 'assets/logo.png'].map(path => [header(path).status, header(path).expanded, header(path).label, header(path).entry])).toEqual([
      ['modified', false, 'unviewed', ''], ['renamed', false, 'unviewed', 'src/text.js'], ['added', false, 'unviewed', '']]);
  });
  test('a slice that fails says so at the end with Retry, is not asked for again on its own, and Retry recovers', async () => {
    let fail = true;
    const f = fixture({}, payload => { if (payload.cursor === '100' && fail) throw new ClientError('HTTP 502: GitHub did not answer', 'HTTP'); return SLICES[String(payload.cursor ?? 'first')]; });
    await f.settle(); await f.press('next');
    let view = await f.settle();
    expect(view.codeTab.items.at(-1)).toMatchObject({ kind: 'pr-footer', label: 'The rest of this diff could not be loaded.', error: true });
    expect([view.codeTab.filesLabel, view.codeTab.treeFooter]).toEqual(['100 files+', 'Retry']);
    await f.press('next');
    await f.settle();
    expect(f.of('prDiff')).toHaveLength(2);
    fail = false;
    await f.press('retry');
    view = await f.settle();
    expect([view.codeTab.filesLabel, view.codeTab.items.at(-1)?.error]).toEqual(['200 files+', false]);
  });
  test('a first slice that fails takes the list\'s place, under the toolbar', async () => {
    const f = fixture({}, () => { throw new ClientError('This pull request diff is too large.', 'PullRequestOperationError'); });
    const view = await f.settle();
    expect([view.codeTab.phase, view.codeTab.message, view.codeTab.hasCommits]).toEqual(['error', 'This pull request diff is too large.', true]);
  });
});

describe('the scope menu (commits newest first, ten at a time)', () => {
  test('a commit scopes the diff and its reads; line comments wait for the whole change; All commits goes back', async () => {
    const f = fixture();
    let view = await f.settle();
    expect([view.codeTab.scopeLabel, view.codeTab.commits.length, view.codeTab.moreCommits, view.codeTab.commits[0]?.headline]).toEqual(['All commits', 10, 4, 'Commit 14']);
    await f.press('more-commits');
    view = await f.settle();
    expect([view.codeTab.commits.length, view.codeTab.moreCommits]).toEqual([14, 0]);
    const oid = commits[5]!.oid;
    await f.press('scope', oid);
    view = await f.settle();
    expect([view.codeTab.scopeLabel, view.codeTab.commentsOff, view.codeTab.canComment, view.codeTab.commits.length]).toEqual(['Commit 6', true, false, 10]);
    expect(f.of('prDiff').at(-1)!.payload).toEqual({ ...reference, commit: oid });
    // Under a commit every conversation is listed off the diff.
    expect(view.codeTab.orphanCount).toBe(4);
    await f.press('begin', `additions|2|src/catalog.js`);
    expect((await f.settle()).codeTab.draftOpen).toBe(false);
    await f.press('scope', '');
    view = await f.settle();
    expect([view.codeTab.scopeLabel, view.codeTab.commentsOff, f.of('prDiff').at(-1)!.payload.commit]).toEqual(['All commits', false, undefined]);
  });
  test('a commit the change no longer has puts the scope back on the whole change', async () => {
    let list = commits;
    const f = fixture({ 'pullRequests.activity': () => ({ ...activity(), commits: list }) });
    await f.settle();
    await f.press('scope', commits[0]!.oid);
    await f.settle();
    list = commits.slice(1);
    await f.act('activity-retry', 'timeline');
    const view = await f.settle();
    expect(view.codeTab.scopeLabel).toBe('All commits');
  });
});

describe('Viewed ticks (usePullRequestFilesViewed)', () => {
  test('three quick ticks are one write after the flush; the count and the folds follow', async () => {
    // The host keeps what it was told (GitHub's markFileAsViewed).
    const host = new Map<string, string>();
    const f = fixture({ 'pullRequests.setFilesViewed': payload => { for (const file of payload.files as Obj[]) host.set(String(file.path), file.viewed ? 'viewed' : 'unviewed'); return {}; },
      'pullRequests.filesViewed': () => ({ files: [...host].map(([path, state]) => ({ path, state })), truncated: false }) });
    let view = await walk(f);
    await f.press('fold-all'); // everything open: a tick folds its file
    for (const path of ['data/item-001.txt', 'data/item-002.txt', 'data/item-003.txt']) await f.press('viewed', `${path}|true`);
    view = await f.settle();
    expect(view.codeTab.viewedCount).toBe('3 / 310');
    expect(view.codeTab.items.filter(item => item.kind === 'pr-file' && item.label === 'viewed').map(item => [item.path, item.expanded])).toEqual([
      ['data/item-001.txt', false], ['data/item-002.txt', false], ['data/item-003.txt', false]]);
    // The resource waited (the native sleep) and sent the three in one write.
    expect(f.of('pullRequests.setFilesViewed').map(call => call.payload)).toEqual([{ ...reference, files: [
      { path: 'data/item-001.txt', viewed: true }, { path: 'data/item-002.txt', viewed: true }, { path: 'data/item-003.txt', viewed: true }] }]);
    expect(view.codeTab.viewedQueued).toBe(0);
    await f.drain();
    view = await f.settle();
    expect([view.codeTab.viewedCount, f.of('pullRequests.filesViewed').length]).toEqual(['3 / 310', 2]); // read again after the write
  });
  test('a failed write takes the ticks back and says "Could not update viewed files"', async () => {
    const f = fixture({ 'pullRequests.setFilesViewed': () => { throw new ClientError('Resource not accessible by integration', 'PullRequestOperationError'); } });
    await walk(f);
    await f.press('viewed', 'data/item-001.txt|true');
    await f.settle(); await f.drain();
    const view = await f.settle();
    expect([view.codeTab.viewedCount, toasts(f.client).map(toast => toast.title)]).toEqual(['0 / 310', ['Could not update viewed files']]);
  });
  test('a file pushed to since it was ticked reads "Changed"; unreadable ticks say so', async () => {
    let fail = false;
    const f = fixture({ 'pullRequests.filesViewed': () => { if (fail) throw new ClientError('HTTP 502', 'HTTP'); return { files: [{ path: 'src/catalog.js', state: 'dismissed' }, { path: 'src/strings.js', state: 'viewed' }], truncated: true }; } });
    let view = await walk(f);
    const mark = (path: string) => view.codeTab.items.find(item => item.kind === 'pr-file' && item.path === path)?.label;
    expect([mark('src/catalog.js'), mark('src/strings.js')]).toEqual(['changed', 'viewed']);
    expect([view.codeTab.viewedCount, view.codeTab.viewedTruncated]).toEqual(['1 / 310', true]);
    fail = true;
    await f.act('activity-retry', 'summary');
    view = await f.settle({ refresh: 1 });
    // Refresh goes around the host's cache: the diff from its first page, the ticks read again (and kept through the failure).
    expect([view.codeTab.viewedError, view.codeTab.viewedCount]).toEqual(['HTTP 502', '1 / 100']);
  });
});

describe('line comments into the review (PullRequestCodeTab beginComment, Add to review)', () => {
  test('an added, a deleted and a context line (split) each become their position; the rename sends its old path; Submit sends them', async () => {
    const f = fixture({ 'pullRequests.submitReview': () => ({}) });
    await walk(f);
    await f.press('fold', 'src/catalog.js'); await f.press('fold', 'src/strings.js'); await f.press('layout', 'split');
    let view = await f.settle();
    expect(view.codeTab.items.filter(item => item.path === 'src/catalog.js' && item.kind === 'split').map(item => [item.leftLine, item.rightLine])).toEqual([[1, 1], [2, 2], [3, 3], [4, 4]]);
    // The gutter's "+" on the added line 2 (right), the deleted line 2 (left), and the context line 4 (left copy).
    for (const [side, line, path, body] of [['additions', 2, 'src/catalog.js', 'Why one?'], ['deletions', 2, 'src/catalog.js', 'Was zero on purpose?'], ['deletions', 4, 'src/catalog.js', 'Keep this default.'], ['additions', 2, 'src/strings.js', 'Locale-aware now.']] as const) {
      await f.press('begin', `${side}|${line}|${path}`);
      view = await f.settle();
      expect(view.codeTab.draftOpen).toBe(true);
      expect(view.codeTab.items.find(item => item.kind === 'pr-draft')?.label).toBe(`${path}:${line}`);
      await f.press('add', body);
    }
    const key = pullRequestReviewKey(reference);
    expect(pullRequestReviewStore(f.client).pending(key).map(({ id: _id, ...rest }) => rest)).toEqual([
      { path: 'src/catalog.js', position: { kind: 'added', newLine: 2 }, body: 'Why one?' },
      { path: 'src/catalog.js', position: { kind: 'deleted', oldLine: 2 }, body: 'Was zero on purpose?' },
      { path: 'src/catalog.js', position: { kind: 'context', oldLine: 4, newLine: 4, side: 'left' }, body: 'Keep this default.' },
      { path: 'src/strings.js', oldPath: 'src/text.js', position: { kind: 'added', newLine: 2 }, body: 'Locale-aware now.' },
    ]);
    view = await f.settle();
    // Under the split row of line 2, the old side's comment first, then the new side's.
    expect(view.codeTab.items.filter(item => item.kind === 'pr-pending').map(item => item.text)).toEqual(['Was zero on purpose?', 'Why one?', 'Keep this default.', 'Locale-aware now.']);
    expect(await f.act('review', 'comment|Notes inline.')).toBe('');
    const sent = f.of('pullRequests.submitReview')[0]!.payload;
    expect([sent.verdict, sent.body, (sent.comments as Obj[]).length, (sent.comments as Obj[])[3]]).toEqual(['comment', 'Notes inline.', 4, { path: 'src/strings.js', oldPath: 'src/text.js', position: { kind: 'added', newLine: 2 }, body: 'Locale-aware now.' }]);
    expect(pullRequestReviewStore(f.client).pending(key)).toHaveLength(0);
  });
  test('Escape (cancel) drops the draft; a pending comment is discarded', async () => {
    const f = fixture();
    await walk(f);
    await f.press('fold', 'src/catalog.js');
    await f.press('begin', 'additions|2|src/catalog.js'); await f.press('cancel');
    expect((await f.settle()).codeTab.draftOpen).toBe(false);
    await f.press('begin', 'additions|2|src/catalog.js'); await f.press('add', 'Keep');
    const id = pullRequestReviewStore(f.client).pending(pullRequestReviewKey(reference))[0]!.id;
    await f.press('discard', id);
    expect(pullRequestReviewStore(f.client).pending(pullRequestReviewKey(reference))).toHaveLength(0);
  });
});

// The drags come on their own queued send (`chatlocal:pr-code-drag`, app.contract lineDragChanged).
describe('the gutter\'s drags (realinput-1010f RF-3: onLineSelectionEnd and onGutterUtilityClick are both beginComment)', () => {
  const row = (op: string, line: number, value: string) => `diffreview|${op}|${line}|${value}`;
  const marks = (view: Awaited<ReturnType<ReturnType<typeof fixture>['settle']>>) => view.codeTab.items.filter(item => item.path === 'src/catalog.js' && item.kind === 'line').map(item => `${item.side}:${item.line}${item.selected ? '*' : ''}`);
  test('a drag on the numbers paints its lines as it goes and opens the draft on its last line at the release', async () => {
    const f = fixture();
    await walk(f);
    await f.press('fold', 'src/catalog.js');
    await f.press('drag', row('drag:additions', 1, 'src/catalog.js'));
    await f.press('drag', row('to', 0, 'dl:deletions:2:src/catalog.js'));
    await f.press('drag', row('to', 0, 'pull-request-code-file-src/strings.js')); // not a line: nothing
    await f.press('drag', row('to', 0, 'dl:additions:3:src/catalog.js'));
    let view = await f.settle();
    expect(marks(view)).toEqual(['additions:1*', 'deletions:2*', 'additions:2*', 'additions:3*', 'additions:4']);
    expect(view.codeTab.draftOpen).toBe(false);
    await f.press('drag', row('end', 1, 'src/catalog.js'));
    view = await f.settle();
    expect([view.codeTab.draftOpen, view.codeTab.items.find(item => item.kind === 'pr-draft')?.label]).toEqual([true, 'src/catalog.js:3']);
    expect(marks(view)).toEqual(['additions:1*', 'deletions:2*', 'additions:2*', 'additions:3*', 'additions:4']);
    // An open draft holds the gutter (enableLineSelection: false): another drag changes nothing.
    await f.press('drag', row('drag:additions', 4, 'src/catalog.js')); await f.press('drag', row('end', 4, 'src/catalog.js'));
    view = await f.settle();
    expect(view.codeTab.items.find(item => item.kind === 'pr-draft')?.label).toBe('src/catalog.js:3');
  });
  test('a click on a number opens the draft on that line, as the reference does', async () => {
    const f = fixture();
    await walk(f);
    await f.press('fold', 'src/catalog.js');
    await f.press('drag', row('drag:deletions', 2, 'src/catalog.js'));
    await f.press('drag', row('end', 2, 'src/catalog.js'));
    const view = await f.settle();
    expect([view.codeTab.draftOpen, view.codeTab.items.find(item => item.kind === 'pr-draft')?.label, marks(view)]).toEqual([
      true, 'src/catalog.js:2', ['additions:1', 'deletions:2*', 'additions:2', 'additions:3', 'additions:4']]);
    await f.press('add', 'Was zero on purpose?');
    expect(pullRequestReviewStore(f.client).pending(pullRequestReviewKey(reference)).map(entry => entry.position)).toEqual([{ kind: 'deleted', oldLine: 2 }]);
  });
  test('a drag from the "+" comments on the range it ends on; under a commit scope the gutter is off', async () => {
    const f = fixture();
    await walk(f);
    await f.press('fold', 'src/catalog.js');
    await f.press('drag', row('gutter:additions', 2, 'src/catalog.js'));
    await f.press('drag', row('to', 0, 'dl:additions:4:src/catalog.js'));
    await f.press('drag', row('end', 2, 'src/catalog.js'));
    let view = await f.settle();
    expect([view.codeTab.items.find(item => item.kind === 'pr-draft')?.label, marks(view)]).toEqual(['src/catalog.js:4', ['additions:1', 'deletions:2', 'additions:2*', 'additions:3*', 'additions:4*']]);
    await f.press('cancel');
    await f.press('scope', commits[0]!.oid);
    await f.settle();
    await f.press('drag', row('drag:additions', 1, 'src/catalog.js')); await f.press('drag', row('end', 1, 'src/catalog.js'));
    view = await f.settle();
    expect(view.codeTab.draftOpen).toBe(false);
  });
});

describe('conversations (ReviewThreadCard, the list off the diff)', () => {
  test('a thread sits under its line inside a hunk; one outside every hunk is listed, grouped by file', async () => {
    const f = fixture();
    let view = await walk(f);
    expect([view.codeTab.orphanCount, view.codeTab.orphansLabel, view.codeTab.orphans.map(group => [group.path, group.threads.map(entry => [entry.id, entry.line])])]).toEqual([
      1, 'Conversations not on the current diff', [['src/catalog.js', [['t-outdated', '']]]]]);
    await f.press('fold', 'src/catalog.js');
    view = await f.settle();
    const rows = view.codeTab.items.filter(item => item.path === 'src/catalog.js').map(item => (item.kind === 'line' ? `${item.side}:${item.line}` : item.kind === 'pr-thread' ? `thread ${item.entry}` : item.kind));
    expect(rows).toEqual(['pr-file', 'additions:1', 'deletions:2', 'thread t-resolved', 'additions:2', 'thread t-open', 'additions:3', 'thread t-long', 'additions:4', 'pad']);
    const cards = new Map(view.codeTab.threads.map(card => [card.id, card]));
    expect([cards.get('t-open')?.countLabel, cards.get('t-resolved')?.countLabel, cards.get('t-outdated')?.outdated, cards.get('t-long')?.countLabel, cards.get('t-long')?.more]).toEqual([
      'Open · 1 comment', 'Resolved · 1 comment', true, 'Open · 12 comments', true]);
    expect(view.bodies.some(body => body.id === 'pr-thread-comment:c-open')).toBe(true);
  });
  test('Reply, Resolve, Unresolve, edit one\'s own comment and Load more each send their write, then read the conversation again', async () => {
    const f = fixture({ 'pullRequests.replyToThread': () => ({}), 'pullRequests.setThreadResolution': () => ({}), 'pullRequests.updateComment': () => ({}),
      'pullRequests.threadComments': payload => ({ comments: payload.cursor === 'page-2' ? [comment('c-long-2'), comment('c-long-1')] : [], nextCursor: payload.cursor === 'page-2' ? 'page-3' : null }),
      'pullRequests.detail': () => detail({ viewer: { login: 'primary' } }) });
    await f.settle();
    const activities = () => f.of('pullRequests.activity').length, before = activities();
    expect(await f.act('thread-reply', 't-open|Good point.')).toBe('');
    expect(await f.act('thread-resolve', 't-open|true')).toBe('');
    expect(await f.act('thread-resolve', 't-resolved|false')).toBe('');
    expect(await f.act('thread-edit', 't-long|c-long-0|Edited words.')).toBe('');
    expect(await f.act('thread-more', 't-long')).toBe('');
    expect([f.of('pullRequests.replyToThread')[0]!.payload, f.of('pullRequests.setThreadResolution').map(call => call.payload.resolved), f.of('pullRequests.updateComment')[0]!.payload,
      f.of('pullRequests.threadComments')[0]!.payload]).toEqual([
      { ...reference, threadId: 't-open', body: 'Good point.' }, [true, false], { ...reference, commentId: 'c-long-0', kind: 'review-comment', body: 'Edited words.' }, { ...reference, threadId: 't-long', cursor: 'page-2' }]);
    const view = await f.settle();
    expect(activities()).toBeGreaterThan(before);
    const long = view.codeTab.threads.find(card => card.id === 't-long')!;
    expect([long.comments.map(entry => entry.id), long.more]).toEqual([['c-long-0', 'c-long-1', 'c-long-2'], true]);
    expect([view.codeTab.threads.find(card => card.id === 't-open')?.repliedSerial, long.comments[0]?.savedSerial]).toEqual([1, 1]);
  });
  test('failures toast the reference\'s words and keep the card as it was', async () => {
    const refuse = () => { throw new ClientError('Forbidden', 'PullRequestOperationError'); };
    const f = fixture({ 'pullRequests.replyToThread': refuse, 'pullRequests.setThreadResolution': refuse, 'pullRequests.updateComment': refuse, 'pullRequests.threadComments': refuse });
    await f.settle();
    await f.act('thread-reply', 't-open|Good point.'); await f.act('thread-resolve', 't-open|true'); await f.act('thread-edit', 't-long|c-long-0|x'); await f.act('thread-more', 't-long');
    expect(toasts(f.client).map(toast => toast.title)).toEqual(['Reply could not be posted', 'The conversation could not be updated', 'The comment could not be saved', 'More comments could not be loaded']);
    const view = await f.settle();
    expect(view.codeTab.threads.find(card => card.id === 't-open')?.repliedSerial).toBe(0);
  });
  test('a reader who may not resolve sees no Resolve; a host without reply offers no Reply', async () => {
    const f = fixture({ 'pullRequests.detail': () => detail({ viewerPermissions: { comment: true, resolve: false, verdicts: [], actions: [] }, capabilities: { ...detail().capabilities as Obj, review: { inlineComment: true, reply: false, resolve: true, verdicts: [] } } }) });
    const view = await f.settle();
    expect(view.codeTab.threads.map(card => [card.canResolve, card.canReply])).toEqual([[false, false], [false, false], [false, false], [false, false]]);
  });
});

describe('hidden lines (createPullRequestDiffFileContentsLoader)', () => {
  test('keeps concurrent diff file reads on different hosts separate', () => {
    const parsed = renderablePatch(sourcePatch, false), file = parsed?.kind === 'files' ? parsed.files.find(entry => entry.path === 'src/strings.js')! : null;
    const one = diffFileContentsInput({ ...reference, host: 'github.com' }, null, file!), other = diffFileContentsInput({ ...reference, host: 'github.example.com' }, null, file!);
    expect(one).toEqual({ ...reference, host: 'github.com', changeType: 'rename-changed', oldPath: 'src/text.js', newPath: 'src/strings.js' });
    expect(JSON.stringify(one)).not.toBe(JSON.stringify(other)); // the shared read's key is the whole payload
    expect(diffFileContentsInput(reference, 'abc1234', file!)).toMatchObject({ commit: 'abc1234' });
  });
  test('a hidden range reads the file once (in the resource) and opens', async () => {
    const f = fixture({ 'pullRequests.diffFileContents': () => ({ oldContents: 'export const shout = (text) => text.toUpperCase();\nexport const whisper = (text) => text.toLowerCase();\nexport const third = 3;\n', newContents: 'export const shout = (text) => text.toUpperCase();\nexport const whisper = (text) => text.toLocaleLowerCase();\nexport const third = 3;\n' }) });
    await walk(f);
    await f.press('fold', 'src/strings.js'); await f.press('expand', 'src/strings.js|1');
    const view = await f.settle();
    expect(f.of('pullRequests.diffFileContents').map(call => call.payload.changeType)).toEqual(['rename-changed']);
    expect(view.codeTab.items.filter(item => item.path === 'src/strings.js' && item.kind === 'line').map(item => item.number)).toEqual(['1', '2', '2', '3']);
  });
});

describe('the Timeline opens a commit in the Code tab (PullRequestTimelineTab CommitEvent onOpen)', () => {
  test('a commit row carries its whole oid, which scopes the Code tab', async () => {
    const timeline = presentTimeline({ detail: detail(), activity: activity(), activityPending: false, activityError: '', now: NOW });
    const row = timeline.newest.find(entry => entry.kind === 'commit' && entry.headline === 'Commit 6')!;
    expect([row.id, row.sha]).toEqual([commits[5]!.oid, commits[5]!.oid.slice(0, 7)]);
    const f = fixture();
    await f.settle({ tab: 'timeline' });
    await f.press('scope', row.id); // PrdBody openCommit: the scope, then setTab("code")
    const view = await f.settle({ tab: 'code' });
    expect([view.codeTab.mounted, view.codeTab.scopeLabel, f.of('prDiff').map(call => call.payload.commit)]).toEqual([true, 'Commit 6', [row.id]]);
  });
});

describe('Refresh (the panel\'s, which the Code tab follows)', () => {
  test('one press is one invalidate, even when the read after it is let go and asked again', async () => {
    let letGoOnce = true;
    const f = fixture({ 'pullRequests.invalidate': () => ({}), 'pullRequests.detail': () => { if (letGoOnce) { letGoOnce = false; throw new ClientError('superseded', 'superseded'); } return detail(); } });
    letGoOnce = false;
    await f.settle();
    letGoOnce = true;
    // The announcement asks again while the detail is out: that read is let go (the ask throws), then asked again.
    for (let i = 0; i < 4 && letGoOnce; i++) await f.ask({ refresh: 1 }).catch(() => null);
    expect(letGoOnce).toBe(false);
    await f.settle({ refresh: 1 });
    expect(f.of('pullRequests.invalidate')).toHaveLength(1);
  });
});

describe('"Add to agent" (PullRequestCodeTab finishSelection)', () => {
  test('the draft\'s lines become the review-comment chip the hand-off carries, and the draft closes', async () => {
    const f = fixture();
    await walk(f);
    await f.press('fold', 'src/catalog.js');
    await f.press('begin', 'additions|2|src/catalog.js');
    const comment = draftSelection(f.client, reference, detail(), 'Why one?');
    expect(comment).toMatchObject({ id: 'pull-request-selection:src/catalog.js:2:2', sectionId: 'pull-request:114', sectionTitle: 'PR #114 review', filePath: 'src/catalog.js', rangeLabel: '+2', text: 'Why one?' });
    expect(comment!.diff.split('\n')).toEqual(['@@ -0,0 +2,1 @@', '+export const size = 1;']);
    expect((await f.settle()).codeTab.draftOpen).toBe(false);
  });
});
