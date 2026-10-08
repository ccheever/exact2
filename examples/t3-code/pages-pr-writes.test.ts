// pr-writing-and-metadata: the pull request panel's writes through pages:pr-act-* (prCommand ->
// pages-pr-writes.ts) against a recorded server, as the reference's components and client cache
// behave (T3 Code 1e2ecbd975, MIT, see LICENSE-T3). Ported names from
// packages/client-runtime/src/state/pullRequests.test.ts: "updates cached labels after successful
// edits without rereading the host", "updates reviewer requests and enriched reviewers without
// rereading the host", "refreshes pull request activity after a comment is updated"; the rest are
// this clone's rows (the task record's acceptance table), with injected failures real GitHub does not
// produce on request.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import type { Obj } from './domain';
import { toasts } from './toast';
import { prCommand, prLocalWrite, pullRequestDetail } from './pages-pr-detail';
import { pullRequestsPage } from './pages-prs';
import { noteNow } from './composer-controls';
import { heldCandidates, presentWrites } from './pages-pr-writes';
import { pullRequestReviewKey, pullRequestReviewStore } from './pages-pr-writes-logic';

const VERDICTS = ['comment', 'approve', 'request-changes'];
const CAPABILITIES = { diff: true, comment: true, actions: ['merge', 'ready', 'draft', 'close', 'reopen'], mergeMethods: ['merge', 'squash'], reactions: true, labels: true,
  reviewers: { request: true, listCandidates: true }, edit: { changeRequest: true, comment: true }, review: { inlineComment: true, reply: true, resolve: true, verdicts: VERDICTS } };
const ADMIN = { actions: ['merge', 'ready', 'draft', 'close', 'reopen'], comment: true, resolve: true, verdicts: VERDICTS, requestReviewers: true, labels: true };
const reference = { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 };
const selected = JSON.stringify(reference);
const key = pullRequestReviewKey(reference);

function detail(over: Obj = {}): Obj {
  return { provider: 'github', projectId: 'p1', repository: 'lane/sandbox', number: 7, title: 'Add input validation', body: 'Adds a validator.', url: 'https://github.com/lane/sandbox/pull/7',
    state: 'open', isDraft: false, author: { login: 'second' }, viewer: 'primary', checks: [], labels: [{ name: 'needs-review', color: 'fbca04' }], reviewers: [], updatedAt: '2026-10-08T00:00:00Z',
    capabilities: CAPABILITIES, viewerPermissions: ADMIN, ...over };
}
function activity(over: Obj = {}): Obj {
  return { author: { login: 'second' }, reviewers: [], commentCount: 2, commentsTruncated: false, reviewThreads: [], commits: [], reactions: [],
    comments: [
      { id: 'IC_1', kind: 'issue-comment', author: { login: 'primary' }, body: 'Thanks for this.', createdAt: '2026-10-07T00:00:00Z', url: null, path: null, reviewState: null,
        reactions: [{ content: 'thumbs-up', count: 1, actors: ['second'], viewerHasReacted: false }, { content: 'heart', count: 1, actors: ['second'], viewerHasReacted: false }] },
      { id: 'IC_2', kind: 'issue-comment', author: { login: 'second' }, body: 'Updated.', createdAt: '2026-10-07T01:00:00Z', url: null, path: null, reviewState: null, reactions: [] },
    ], ...over };
}
type Reply = (payload: Obj) => unknown;
function lane(replies: Record<string, Reply>) {
  const calls: { method: string; payload: Obj }[] = [];
  let wakes = 0;
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, revision: 0, local: {},
    config: { environment: { capabilities: { pullRequests: true } } },
    shell: { projects: [{ id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox' } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj) => { calls.push({ method, payload }); const reply = replies[method]; if (!reply) throw new Error(`no reply for ${method}`); return reply(payload); },
  } as unknown as T3Client;
  const native = { available: true, watch: () => {}, later: async () => { wakes++; return { ok: true }; } } as unknown as Native;
  const view = async () => {
    // Ask again while the panel asked to be woken (a refresh of a shown detail is drawn first, then read).
    let seen = wakes, shown = await pullRequestDetail(client, native, { selected, refresh: 0, now: 1 });
    for (let asked = 0; asked < 8 && (wakes > seen || shown.phase !== 'content' || shown.activityPending); asked++) { seen = wakes; shown = await pullRequestDetail(client, native, { selected, refresh: 0, now: 1 }); }
    return shown;
  };
  const act = (op: string, value: string) => prCommand(client, native, op, selected, value);
  const sent = (method: string) => calls.filter(call => call.method === method).map(call => call.payload);
  return { client, native, calls, view, act, sent, wakes: () => wakes };
}

describe('the composer: comment, close or reopen with comment (PullRequestCommentForm)', () => {
  test('a comment posts the words as typed, clears the box, closes the popover, refreshes, and toasts nothing', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.comment': () => ({}) });
    const before = (await run.view()).writes;
    expect([before.composer, before.canComment, before.verdicts, before.followUp]).toEqual([true, true, VERDICTS, 'close']);
    expect(await run.act('post-comment', '  Looks good, one nit.\n')).toBe('');
    expect(run.sent('pullRequests.comment')).toEqual([{ ...reference, body: 'Looks good, one nit.' }]);
    expect(run.wakes()).toBeGreaterThan(0); // "Posting..." was drawn while the host answered
    const after = await run.view();
    expect([after.writes.commentDraft, after.writes.submitting, after.writes.commentSerial - before.commentSerial, after.writes.closeSerial - before.closeSerial]).toEqual(['', '', 1, 1]);
    expect(run.sent('pullRequests.activity').length).toBe(2); // refreshDetail: the conversation is read again
    expect(toasts(run.client)).toEqual([]);
  });
  test('a failed comment keeps the words and the popover, and says so', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.comment': () => { throw new Error('HTTP 502'); } });
    await run.view();
    expect(await run.act('post-comment', 'Please rename this.')).toBe('HTTP 502');
    const writes = (await run.view()).writes;
    expect([writes.commentDraft, writes.commentSerial, writes.closeSerial, writes.submitting]).toEqual(['Please rename this.', 0, 0, '']);
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not post the comment' });
  });
  test('close with comment posts the comment, then closes; a refused close keeps the comment and explains', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.comment': () => ({}), 'pullRequests.runAction': () => { throw new Error('Pull request operation runAction failed: Resource not accessible by integration'); } });
    await run.view();
    expect(await run.act('comment-close', 'Closing: superseded.')).toBe('Pull request operation runAction failed: Resource not accessible by integration');
    expect(run.calls.filter(call => call.method !== 'pullRequests.detail' && call.method !== 'pullRequests.activity').map(call => [call.method, call.payload.body ?? call.payload.action])).toEqual([['pullRequests.comment', 'Closing: superseded.'], ['pullRequests.runAction', 'close']]);
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not close this pull request', description: 'Resource not accessible by integration' });
    const writes = (await run.view()).writes;
    expect([writes.commentDraft, writes.closeSerial]).toEqual(['', 1]); // the comment landed: the box is clear, the popover closed
  });
  test("close with comment finishes through the header's action runner: everything waits, the list row leaves on the press and comes back when refused", async () => {
    const LIST_NOW = Date.parse('2026-10-08T12:00:00Z');
    const entries = [{ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7, title: 'Add input validation', state: 'open', isDraft: false, author: { login: 'second' }, updatedAt: '2026-10-08T10:00:00Z', additions: 3, deletions: 0 }];
    let answer: (refused: boolean) => void = () => {};
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.list': () => ({ entries, viewers: {} }), 'pullRequests.listStats': () => ({ stats: [] }),
      'pullRequests.comment': () => ({}),
      'pullRequests.runAction': () => new Promise((resolve, reject) => { answer = refused => refused ? reject(new Error('Resource not accessible by integration')) : resolve({}); }) });
    noteNow(run.client, LIST_NOW);
    const rows = async () => (await pullRequestsPage(run.client, run.native, { open: true, refresh: 0, now: LIST_NOW, selected, query: '', typed: false })).groups.flatMap(group => group.rows.map(row => `${row.number}:${row.state}`));
    expect(await rows()).toEqual(['7:open']);
    await run.view();
    let closing = run.act('comment-close', 'Superseded by #8.');
    await Bun.sleep(5);
    const during = await run.view();
    expect([during.writes.submitting, during.writes.actionPending, during.actions.pending, during.actions.moreLabel]).toEqual(['close', true, true, 'More pull request actions']);
    expect(await rows()).toEqual([]); // "sent": the row left the open list on the press
    expect(await run.act('post-comment', 'And one more thing.')).toBe(''); // the form is locked while the action runs
    expect(await run.act('action', 'draft')).toBe(''); // and so is the header
    answer(true);
    expect(await closing).toBe('Resource not accessible by integration');
    expect(await rows()).toEqual(['7:open']); // "failed": the note is taken back
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not close this pull request', description: 'Resource not accessible by integration' });
    const after = await run.view();
    expect([after.writes.submitting, after.writes.actionPending, after.actions.pending, after.writes.commentDraft]).toEqual(['', false, false, '']);
    closing = run.act('comment-close', 'Superseded by #8, for real.');
    await Bun.sleep(5);
    answer(false);
    expect(await closing).toBe('');
    expect(await rows()).toEqual([]); // "done": the note stands until a list answer agrees
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'success', title: 'Pull request closed' });
    expect(run.sent('pullRequests.comment').map(payload => payload.body)).toEqual(['Superseded by #8.', 'Superseded by #8, for real.']);
    expect(run.sent('pullRequests.runAction').map(payload => payload.action)).toEqual(['close', 'close']);
  });
  test('reopen with comment on a closed pull request, and its toast', async () => {
    const run = lane({ 'pullRequests.detail': () => detail({ state: 'closed' }), 'pullRequests.activity': () => activity(), 'pullRequests.comment': () => ({}), 'pullRequests.runAction': () => ({}) });
    expect((await run.view()).writes.followUp).toBe('reopen');
    expect(await run.act('comment-reopen', 'Back on.')).toBe('');
    expect(run.sent('pullRequests.runAction')).toEqual([{ ...reference, action: 'reopen' }]);
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'success', title: 'Pull request reopened' });
  });
  test('an empty comment sends nothing; the drafts are kept per pull request', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity() });
    await run.view();
    expect(await run.act('post-comment', '   ')).toBe('');
    expect(run.sent('pullRequests.comment')).toEqual([]);
    await run.act('draft-comment', 'half a thought');
    await run.act('draft-summary', 'a summary');
    const writes = (await run.view()).writes;
    expect([writes.commentDraft, writes.summary]).toEqual(['half a thought', 'a summary']);
    const other = presentWrites(run.client, { ...reference, number: 8 }, detail({ number: 8 }));
    expect([other.commentDraft, other.summary]).toEqual(['', '']);
  });
  test('a blur that lands after the post (the press on Comment caused it) does not keep the posted words; the drafts go beside the write route', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.comment': () => ({}), 'pullRequests.submitReview': () => ({}) });
    await run.view();
    expect(await prLocalWrite(run.client, run.native, 'draft-comment', selected, 'Typed, then Comment pressed.')).toBe('');
    expect((await run.view()).writes.commentDraft).toBe('Typed, then Comment pressed.');
    await run.act('post-comment', 'Typed, then Comment pressed.');
    await prLocalWrite(run.client, run.native, 'draft-comment', selected, 'Typed, then Comment pressed.');
    await prLocalWrite(run.client, run.native, 'post-comment', selected, 'not a write this route takes');
    expect(run.sent('pullRequests.comment').length).toBe(1);
    await run.act('review', 'comment|Summary sent.');
    await prLocalWrite(run.client, run.native, 'draft-summary', selected, 'Summary sent.');
    const writes = (await run.view()).writes;
    expect([writes.commentDraft, writes.summary]).toEqual(['', '']);
    await prLocalWrite(run.client, run.native, 'draft-comment', selected, 'A new thought.');
    expect((await run.view()).writes.commentDraft).toBe('A new thought.');
  });
  test('the composer is hidden where neither a comment nor a verdict is allowed, and the follow-up only where the viewer may', () => {
    const none = presentWrites({}, reference, detail({ viewerPermissions: { ...ADMIN, comment: false, verdicts: [] } }));
    expect(none.composer).toBe(false);
    const reviewOnly = presentWrites({}, reference, detail({ capabilities: { ...CAPABILITIES, comment: false } }));
    expect([reviewOnly.composer, reviewOnly.canComment, reviewOnly.verdicts]).toEqual([true, false, VERDICTS]);
    expect(presentWrites({}, reference, detail({ viewerPermissions: { ...ADMIN, actions: [] } })).followUp).toBe('');
    expect(presentWrites({}, reference, detail({ provider: 'forgejo' })).summaryRequired).toBe(true);
  });
});

describe('review verdicts (PullRequestReviewForm)', () => {
  test.each([['comment', 'Review submitted'], ['approve', 'Pull request approved'], ['request-changes', 'Changes requested']])('%s sends its verdict and summary and toasts "%s"', async (verdict, toast) => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.submitReview': () => ({}) });
    await run.view();
    expect(await run.act('review', `${verdict}|Looks right to me.`)).toBe('');
    expect(run.sent('pullRequests.submitReview')).toEqual([{ ...reference, verdict, body: 'Looks right to me.', comments: [] }]);
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'success', title: toast });
    const writes = (await run.view()).writes;
    expect([writes.summary, writes.reviewSerial, writes.closeSerial]).toEqual(['', 1, 1]);
  });
  test('an author is offered Comment only; an empty Comment is not sent, an empty approval is', async () => {
    const authored = presentWrites({}, reference, detail({ author: { login: 'primary' }, viewerPermissions: { ...ADMIN, verdicts: ['comment'] } }));
    expect(authored.verdicts).toEqual(['comment']);
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.submitReview': () => ({}) });
    await run.view();
    expect(await run.act('review', 'comment|  ')).toBe('');
    expect(run.sent('pullRequests.submitReview')).toEqual([]);
    expect(await run.act('review', 'approve|')).toBe('');
    expect(run.sent('pullRequests.submitReview')).toEqual([{ ...reference, verdict: 'approve', body: '', comments: [] }]);
  });
  test('a failed review keeps the summary and the line comments, and says so', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.submitReview': () => { throw new Error('HTTP 422'); } });
    await run.view();
    pullRequestReviewStore(run.client).addComment(key, { id: 'pending-a', path: 'src/validate.js', position: { kind: 'added', newLine: 4 }, body: 'Here.' });
    expect(await run.act('review', 'request-changes|Not yet.')).toBe('HTTP 422');
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'The review could not be submitted' });
    const writes = (await run.view()).writes;
    expect([writes.summary, writes.pendingCount, writes.reviewSerial, writes.reviewPending]).toEqual(['Not yet.', 1, 0, false]);
  });
});

describe('the pending review store (the Code tab fills it)', () => {
  test('the count, the line comments sent with the review (without their ids), discard, and the summary across pull requests', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.submitReview': () => ({}) });
    await run.view();
    const store = pullRequestReviewStore(run.client);
    store.addComment(key, { id: 'pending-a', path: 'src/validate.js', position: { kind: 'added', newLine: 4 }, body: 'One.' });
    store.addComment(key, { id: 'pending-b', path: 'src/validate.js', position: { kind: 'deleted', oldLine: 2 }, body: 'Two.' });
    expect((await run.view()).writes.pendingCount).toBe(2);
    await run.act('discard-pending', '');
    expect((await run.view()).writes.pendingCount).toBe(0);
    store.addComment(key, { id: 'pending-c', path: 'src/validate.js', position: { kind: 'added', newLine: 6 }, body: 'Three.' });
    await run.act('draft-summary', 'Kept while I look at another one.');
    expect(presentWrites(run.client, { ...reference, number: 8 }, detail({ number: 8 })).summary).toBe('');
    expect((await run.view()).writes.summary).toBe('Kept while I look at another one.');
    expect(await run.act('review', 'comment|')).toBe(''); // a pending line comment is enough for a Comment
    expect(run.sent('pullRequests.submitReview')[0]).toEqual({ ...reference, verdict: 'comment', body: '', comments: [{ path: 'src/validate.js', position: { kind: 'added', newLine: 6 }, body: 'Three.' }] });
    expect((await run.view()).writes.pendingCount).toBe(0);
  });
});

describe('title, description and comment edits', () => {
  test('the title: trimmed, sent once; unchanged or empty sends nothing; a failure keeps the editor and explains', async () => {
    let refuse = false;
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.update': () => { if (refuse) throw new Error('Pull request operation update failed: title is too long (maximum is 256 characters)'); return {}; } });
    expect((await run.view()).writes.canEditChange).toBe(true); // an admin who may merge
    expect(await run.act('edit-title', '  Add input validation ')).toBe('');
    expect(await run.act('edit-title', '   ')).toBe('');
    expect(run.sent('pullRequests.update')).toEqual([]);
    expect((await run.view()).writes.titleSerial).toBe(2); // each closed the field
    expect(await run.act('edit-title', ' Validate the input ')).toBe('');
    expect(run.sent('pullRequests.update')).toEqual([{ ...reference, title: 'Validate the input' }]);
    refuse = true;
    await run.act('edit-title', 'x'.repeat(300));
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'The title could not be saved', description: 'title is too long (maximum is 256 characters)' });
    expect((await run.view()).writes.titleSerial).toBe(3);
  });
  test('the description: sent verbatim, empty allowed; a failure keeps the editor', async () => {
    let refuse = false;
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.update': () => { if (refuse) throw new Error('nope'); return {}; } });
    await run.view();
    expect(await run.act('edit-body', '  indented code\n\n- a list  ')).toBe('');
    expect(await run.act('edit-body', '')).toBe('');
    expect(run.sent('pullRequests.update')).toEqual([{ ...reference, body: '  indented code\n\n- a list  ' }, { ...reference, body: '' }]);
    refuse = true;
    await run.act('edit-body', 'again');
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not save the description' });
    expect((await run.view()).writes.bodySerial).toBe(2);
  });
  test('a comment: the pencil on the reader\'s own remarks only (not on others\' or on a review), the mutation\'s variables, a failure keeps the editor', async () => {
    let refuse = false;
    const comments = [...(activity().comments as Obj[]), { id: 'PRR_1', kind: 'review', author: { login: 'primary' }, body: 'Overall fine.', createdAt: '2026-10-07T02:00:00Z', url: null, path: null, reviewState: 'COMMENTED', reactions: [] }];
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity({ comments }), 'pullRequests.updateComment': () => { if (refuse) throw new Error('nope'); return {}; } });
    const view = await run.view();
    const card = (id: string) => view.summary.oldest.find(entry => entry.id === id)!;
    expect([card('IC_1').canEdit, card('IC_2').canEdit, card('PRR_1').canEdit]).toEqual([true, false, false]);
    expect(await run.act('edit-comment', 'IC_1|issue-comment|Thanks for this. Edited.')).toBe('');
    expect(run.sent('pullRequests.updateComment')).toEqual([{ ...reference, commentId: 'IC_1', kind: 'issue-comment', body: 'Thanks for this. Edited.' }]);
    expect(await run.act('edit-comment', 'PRR_1|review|Rewritten.')).toBe(''); // never sent: no host rewrites a review's summary here
    expect(run.sent('pullRequests.updateComment').length).toBe(1);
    refuse = true;
    await run.act('edit-comment', 'IC_1|issue-comment|Again.');
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not save the comment' });
    const after = await run.view();
    expect(after.summary.oldest.find(entry => entry.id === 'IC_1')!.savedSerial).toBe(1);
  });
  test('refreshes pull request activity after a comment is updated', async () => {
    let body = 'Thanks for this.';
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity({ comments: [{ ...(activity().comments as Obj[])[0], body }] }), 'pullRequests.updateComment': (payload) => { body = String(payload.body); return {}; } });
    expect((await run.view()).summary.oldest[0]!.raw).toBe('Thanks for this.');
    await run.act('edit-comment', 'IC_1|issue-comment|updated');
    expect((await run.view()).summary.oldest[0]!.raw).toBe('updated');
    expect(run.sent('pullRequests.activity').length).toBe(2);
  });
  test('a description preview is one more document, keyed by its editor', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity() });
    await run.view();
    await run.act('preview', 'body|# Heading\n\nWords.');
    expect((await run.view()).bodies.find(body => body.id === 'pr-preview:body')).toEqual({ id: 'pr-preview:body', kind: 'assistant', title: '', body: '# Heading\n\nWords.' });
  });
});

describe('reactions (PullRequestReactionBar)', () => {
  test('a press shows at once, sends the content enum, and is forgotten when the host\'s counts land', async () => {
    let reacted = false;
    const reactions = () => [{ content: 'thumbs-up', count: 1, actors: ['second'], viewerHasReacted: false }, { content: 'heart', count: reacted ? 2 : 1, actors: ['second'], viewerHasReacted: reacted }];
    let release: () => void = () => {};
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity({ comments: [{ ...(activity().comments as Obj[])[0], reactions: reactions() }] }),
      'pullRequests.setReaction': () => new Promise(resolve => { release = () => { reacted = true; resolve({}); }; }) });
    const first = (await run.view()).summary.oldest[0]!;
    expect(first.reactions.map(pill => [pill.emoji, pill.count, pill.pressed, pill.label])).toEqual([['👍', 1, false, 'thumbs up, 1'], ['❤️', 1, false, 'heart, 1']]);
    expect(first.reactions[1]!.tooltip).toBe('second reacted with heart emoji');
    const press = run.act('react', 'IC_1|heart|true');
    await Bun.sleep(5);
    const optimistic = (await run.view()).summary.oldest[0]!;
    expect([optimistic.reactions[1]!.count, optimistic.reactions[1]!.pressed, optimistic.reactions[1]!.tooltip, optimistic.reacted]).toEqual([2, true, 'You and second reacted with heart emoji', ['heart']]);
    release();
    expect(await press).toBe('');
    expect(run.sent('pullRequests.setReaction')).toEqual([{ ...reference, subjectId: 'IC_1', content: 'heart', reacted: true }]);
    const landed = (await run.view()).summary.oldest[0]!;
    expect([landed.reactions[1]!.count, landed.reactions[1]!.pressed]).toEqual([2, true]);
  });
  test('a refused press is rolled back and says so', async () => {
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.setReaction': () => { throw new Error('HTTP 403'); } });
    await run.view();
    expect(await run.act('react', 'IC_1|eyes|true')).toBe('HTTP 403');
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'The reaction could not be saved' });
    const card = (await run.view()).summary.oldest[0]!;
    expect(card.reactions.map(pill => pill.content)).toEqual(['thumbs-up', 'heart']);
  });
  test('read-only pills where the host takes no reactions, on the timeline too', async () => {
    const run = lane({ 'pullRequests.detail': () => detail({ capabilities: { ...CAPABILITIES, reactions: false } }), 'pullRequests.activity': () => activity() });
    const view = await run.view();
    expect([view.summary.oldest[0]!.canReact, view.summary.oldest[0]!.reactions.length]).toEqual([false, 2]);
    const cards = view.timeline.oldest.flatMap(row => row.cards);
    expect(cards.find(card => card.id === 'IC_1')).toMatchObject({ canReact: false, canEdit: true });
  });
});

describe('reviewers and labels (the candidate pickers)', () => {
  test('updates reviewer requests and enriched reviewers without rereading the host', async () => {
    let refuse = false;
    const run = lane({
      'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(),
      'pullRequests.reviewerCandidates': () => ({ candidates: [{ login: 'second', name: 'Second', avatarUrl: null, id: 'second', kind: 'user', isRequested: false }], truncated: false }),
      'pullRequests.requestReviewers': () => { if (refuse) throw new Error('Pull request operation requestReviewers failed: Reviews may only be requested from collaborators.'); return {}; },
    });
    await run.view();
    expect(await run.act('candidates', 'reviewers')).toBe('');
    expect(await run.act('candidates', 'reviewers')).toBe(''); // kept for a minute: one read
    expect(run.sent('pullRequests.reviewerCandidates').length).toBe(1);
    const picker = (await run.view()).writes.reviewers;
    expect([picker.shown, picker.allowed, picker.loading, picker.rows.map(row => [row.label, row.on, row.value])]).toEqual([true, true, false, [['second', false, 'user|second|request|second']]]);
    const reads = run.calls.length;
    expect(await run.act('reviewer', 'user|second|request|second')).toBe('');
    expect(run.sent('pullRequests.requestReviewers')).toEqual([{ ...reference, reviewers: [{ id: 'second', kind: 'user' }], requested: true }]);
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'success', title: 'Review requested from second' });
    const asked = await run.view();
    expect([asked.reviewers.map(reviewer => reviewer.login), asked.writes.reviewers.rows[0]!.on]).toEqual([['second'], true]);
    expect(run.calls.length).toBe(reads + 1); // the write alone: nothing was read again
    expect(await run.act('reviewer', 'user|second|remove|second')).toBe('');
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'success', title: 'Review request to second taken back' });
    expect((await run.view()).reviewers).toEqual([]);
    refuse = true;
    expect(await run.act('reviewer', 'user|second|request|second')).toContain('Reviews may only be requested from collaborators.');
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not ask second for a review', description: 'Reviews may only be requested from collaborators.' });
  });
  test('updates cached labels after successful edits without rereading the host', async () => {
    let refuse = false;
    const run = lane({
      'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(),
      'pullRequests.labelCandidates': () => ({ candidates: [{ name: 'needs-review', color: 'fbca04', description: null, isApplied: true }, { name: 'area/docs and help', color: '0e8a16', description: 'Docs', isApplied: false }], truncated: true }),
      // A refusal that says nothing of its own (the CLI's noise): the hint stands in (readableFailure).
      'pullRequests.setLabels': () => { if (refuse) throw new Error('GitHub CLI command failed'); return {}; },
    });
    await run.view();
    await run.act('candidates', 'labels');
    expect((await run.view()).writes.labels.truncated).toBe(true);
    const reads = run.calls.length;
    expect(await run.act('label', 'apply|area/docs and help')).toBe('');
    expect(run.sent('pullRequests.setLabels')).toEqual([{ ...reference, labels: ['area/docs and help'], applied: true }]);
    const applied = await run.view();
    // The label's colour comes from the candidate it was picked from (the chip tints it).
    expect([applied.labels.map(label => [label.name, label.background.includes(label.name === 'needs-review' ? '#fbca04' : '#0e8a16')]), applied.writes.labels.rows.map(row => row.on)]).toEqual([[['needs-review', true], ['area/docs and help', true]], [true, true]]);
    expect(await run.act('label', 'remove|needs-review')).toBe('');
    expect((await run.view()).labels.map(label => label.name)).toEqual(['area/docs and help']);
    expect(run.calls.length).toBe(reads + 2);
    expect(toasts(run.client)).toEqual([]); // no success toast for a label
    refuse = true;
    await run.act('label', 'apply|needs-review');
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not put needs-review on', description: 'The host refused it. Check that you have triage access on this repository.' });
    await run.act('label', 'remove|area/docs and help');
    expect(toasts(run.client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not take area/docs and help off' });
    expect(heldCandidates(run.client, reference, 'labels')!.list.map(candidate => candidate.isApplied)).toEqual([false, true]);
  });
  test('reader and triage: the reviewer picker disabled with its reason; triage may label, a reader may not', () => {
    const reader = presentWrites({}, reference, detail({ viewerPermissions: { actions: [], comment: true, resolve: false, verdicts: VERDICTS, requestReviewers: false, labels: false } }));
    expect([reader.reviewers.shown, reader.reviewers.allowed, reader.labels.shown, reader.labels.allowed]).toEqual([true, false, true, false]);
    const triage = presentWrites({}, reference, detail({ viewerPermissions: { actions: [], comment: true, resolve: false, verdicts: VERDICTS, requestReviewers: false, labels: true } }));
    expect([triage.reviewers.allowed, triage.labels.allowed]).toEqual([false, true]);
    // A host that cannot list candidates (Azure DevOps) offers no picker; a host without labels no label picker.
    const azure = presentWrites({}, reference, detail({ capabilities: { ...CAPABILITIES, reviewers: { request: true, listCandidates: false }, labels: false } }));
    expect([azure.reviewers.shown, azure.labels.shown]).toEqual([false, false]);
  });
  test('a failed candidate read: the menu says so, and a later open reads again', async () => {
    let fail = true;
    const run = lane({ 'pullRequests.detail': () => detail(), 'pullRequests.activity': () => activity(), 'pullRequests.labelCandidates': () => { if (fail) throw new Error('GitHub CLI is not signed in'); return { candidates: [], truncated: false }; } });
    await run.view();
    await run.act('candidates', 'labels');
    expect((await run.view()).writes.labels).toMatchObject({ loading: false, error: 'GitHub CLI is not signed in', rows: [] });
    fail = false;
    await run.act('candidates', 'labels');
    expect((await run.view()).writes.labels).toMatchObject({ loading: false, error: '', rows: [] });
  });
});
