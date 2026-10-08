// Ported tests (pr-writing-and-metadata): the names and cases of T3 Code 1e2ecbd975 (MIT, see
// LICENSE-T3) apps/web/src/components/pullRequest/pullRequestEditing.logic.test.ts
// ("canEditPullRequestChangeRequest", "canEditPullRequestComment"), pullRequestReactions.logic.test.ts
// ("reaction presentation", "reaction tooltip", "reaction tooltip, with a reaction in flight",
// "pending reactions") and pullRequestReviewStore.test.ts ("pull request review drafts"; the store is
// a plain object here, so `setState` is a fresh store).
import { describe, expect, it } from 'bun:test';
import type { Obj } from './domain';
import {
  PULL_REQUEST_REACTION_ORDER, PullRequestReviewStore, applyPendingPullRequestReactions, canEditPullRequestChangeRequest, canEditPullRequestComment, pullRequestReactionEmoji,
  pullRequestReactionName, pullRequestReactionTooltip, pullRequestReviewKey, type PendingReviewComment, type PullRequestReaction, type PullRequestReactionContent,
} from './pages-pr-writes-logic';

const actor = (login: string): Obj => ({ login, name: null, avatarUrl: null });
function capabilities(edit?: Obj): Obj {
  return { diff: true, comment: true, actions: [], mergeMethods: [], search: true, reactions: true, review: { inlineComment: true, reply: true, resolve: true, verdicts: [] },
    reviewers: { request: true, listCandidates: true }, ...(edit === undefined ? {} : { edit }) };
}
const permissions = (overrides: Obj = {}): Obj => ({ actions: [], comment: true, resolve: true, verdicts: [], requestReviewers: true, ...overrides });
function subject(overrides: Obj = {}): Obj {
  return { author: actor('octocat'), capabilities: capabilities({ changeRequest: true, comment: true }), viewer: 'octocat', viewerPermissions: permissions(), ...overrides };
}
const comment = (overrides: Obj = {}): Obj => ({ id: 'c1', kind: 'issue-comment', author: actor('octocat'), body: 'words', createdAt: '2026-01-01T00:00:00.000Z', url: null, path: null, reviewState: null, ...overrides });
const without = (value: Obj, key: string): Obj => { const { [key]: _gone, ...rest } = value; return rest; };

describe('canEditPullRequestChangeRequest', () => {
  it('lets the author rewrite their own change request', () => {
    expect(canEditPullRequestChangeRequest(subject())).toBe(true);
  });
  it('matches the author regardless of case', () => {
    expect(canEditPullRequestChangeRequest(subject({ viewer: 'OctoCat' }))).toBe(true);
  });
  it('lets somebody who may merge rewrite a change request they did not write', () => {
    expect(canEditPullRequestChangeRequest(subject({ author: actor('someone-else'), viewerPermissions: permissions({ actions: ['merge'] }) }))).toBe(true);
  });
  it('refuses a reader who neither wrote it nor may merge it', () => {
    expect(canEditPullRequestChangeRequest(subject({ author: actor('someone-else'), viewerPermissions: permissions({ actions: ['close'] }) }))).toBe(false);
  });
  it('refuses where the host cannot rewrite a change request at all', () => {
    expect(canEditPullRequestChangeRequest(subject({ capabilities: capabilities({ changeRequest: false, comment: true }) }))).toBe(false);
    expect(canEditPullRequestChangeRequest(subject({ capabilities: capabilities() }))).toBe(false);
  });
  it('refuses where the host did not say who the reader is', () => {
    expect(canEditPullRequestChangeRequest(without(subject(), 'viewer'))).toBe(false);
  });
  it('still allows a merger the host could not name', () => {
    expect(canEditPullRequestChangeRequest(without(subject({ viewerPermissions: permissions({ actions: ['merge'] }) }), 'viewer'))).toBe(true);
  });
});

describe('canEditPullRequestComment', () => {
  it('lets the reader rewrite their own conversation comment', () => {
    expect(canEditPullRequestComment(subject(), comment())).toBe(true);
  });
  it('lets the reader rewrite their own remark on a line', () => {
    expect(canEditPullRequestComment(subject(), comment({ kind: 'review-comment' }))).toBe(true);
  });
  it('matches the author regardless of case', () => {
    expect(canEditPullRequestComment(subject({ viewer: 'OCTOCAT' }), comment({ author: actor('octocat') }))).toBe(true);
  });
  it("refuses somebody else's remark", () => {
    expect(canEditPullRequestComment(subject(), comment({ author: actor('someone-else') }))).toBe(false);
  });
  it('refuses a remark the host attributes to nobody', () => {
    expect(canEditPullRequestComment(subject(), comment({ author: null }))).toBe(false);
  });
  it("refuses a review's own summary", () => {
    expect(canEditPullRequestComment(subject(), comment({ kind: 'review' }))).toBe(false);
  });
  it('refuses where the host cannot rewrite a remark at all', () => {
    expect(canEditPullRequestComment(subject({ capabilities: capabilities({ changeRequest: true, comment: false }) }), comment())).toBe(false);
    expect(canEditPullRequestComment(subject({ capabilities: capabilities() }), comment())).toBe(false);
  });
  it('refuses where the host did not say who the reader is', () => {
    expect(canEditPullRequestComment(without(subject(), 'viewer'), comment())).toBe(false);
  });
});

const reaction = (overrides: Partial<PullRequestReaction> = {}): PullRequestReaction => ({ content: 'heart', count: 1, actors: ['octocat'], viewerHasReacted: false, ...overrides });

describe('reaction presentation', () => {
  it("names and draws all eight, in GitHub's picker order", () => {
    expect(PULL_REQUEST_REACTION_ORDER).toEqual(['thumbs-up', 'thumbs-down', 'laugh', 'hooray', 'confused', 'heart', 'rocket', 'eyes']);
    expect(PULL_REQUEST_REACTION_ORDER.map(pullRequestReactionEmoji)).toEqual(['👍', '👎', '😄', '🎉', '😕', '❤️', '🚀', '👀']);
    expect(pullRequestReactionName('thumbs-up')).toBe('thumbs up');
    expect(pullRequestReactionName('eyes')).toBe('eyes');
  });
});

describe('reaction tooltip', () => {
  it("reads as GitHub's sentence for one, two and three names", () => {
    expect(pullRequestReactionTooltip(reaction({ content: 'thumbs-up', count: 1, actors: ['Bil0000'] }))).toBe('Bil0000 reacted with thumbs up emoji');
    expect(pullRequestReactionTooltip(reaction({ count: 2, actors: ['Bil0000', 'octocat'] }))).toBe('Bil0000 and octocat reacted with heart emoji');
    expect(pullRequestReactionTooltip(reaction({ content: 'eyes', count: 3, actors: ['Bil0000', 'octocat'], viewerHasReacted: true }))).toBe('You, Bil0000, and octocat reacted with eyes emoji');
  });
  it('counts everyone past the third name, including the ones the host never named', () => {
    expect(pullRequestReactionTooltip(reaction({ content: 'rocket', count: 15, actors: ['a', 'b', 'c', 'd'], viewerHasReacted: true }))).toBe('You, a, b, and 12 others reacted with rocket emoji');
    expect(pullRequestReactionTooltip(reaction({ count: 2, actors: ['octocat'] }))).toBe('octocat and 1 other reacted with heart emoji');
    expect(pullRequestReactionTooltip(reaction({ count: 4, actors: [] }))).toBe('4 people reacted with heart emoji');
    expect(pullRequestReactionTooltip(reaction({ count: 1, actors: [] }))).toBe('1 person reacted with heart emoji');
  });
  it('names the viewer as You, ahead of the other people who reacted', () => {
    expect(pullRequestReactionTooltip(reaction({ count: 3, actors: ['Bil0000', 'octocat'], viewerHasReacted: true }))).toBe('You, Bil0000, and octocat reacted with heart emoji');
  });
  it('names nobody as You when the viewer has not reacted', () => {
    expect(pullRequestReactionTooltip(reaction({ count: 2, actors: ['Bil0000', 'octocat'], viewerHasReacted: false }))).toBe('Bil0000 and octocat reacted with heart emoji');
  });
  it('names actors as given, and leaves off You, for a host with no room for the viewer', () => {
    expect(pullRequestReactionTooltip(reaction({ count: 2, actors: ['Bil0000', 'octocat'], viewerHasReacted: true }))).toBe('Bil0000 and octocat reacted with heart emoji');
    expect(pullRequestReactionTooltip(reaction({ count: 4, actors: ['Bil0000', 'octocat', 'hubot', 'zzz'], viewerHasReacted: true }))).toBe('Bil0000, octocat, hubot, and 1 other reacted with heart emoji');
  });
  it('keeps naming the viewer You when the host does leave them room', () => {
    expect(pullRequestReactionTooltip(reaction({ count: 2, actors: ['octocat'], viewerHasReacted: true }))).toBe('You and octocat reacted with heart emoji');
  });
});

describe('reaction tooltip, with a reaction in flight', () => {
  it('still says You after an optimistic react, even at full capacity', () => {
    const applied = applyPendingPullRequestReactions([reaction({ count: 2, actors: ['a', 'b'], viewerHasReacted: false })], new Map([['heart', true] as const]));
    expect(pullRequestReactionTooltip(applied[0]!)).toBe('You, a, and b reacted with heart emoji');
  });
  it('drops You after an optimistic un-react, without treating the host as non-compliant', () => {
    const applied = applyPendingPullRequestReactions([reaction({ count: 2, actors: ['octocat'], viewerHasReacted: true })], new Map([['heart', false] as const]));
    expect(pullRequestReactionTooltip(applied[0]!)).toBe('octocat reacted with heart emoji');
  });
});

describe('pending reactions', () => {
  const pending = (entries: ReadonlyArray<readonly [PullRequestReactionContent, boolean]>): ReadonlyMap<PullRequestReactionContent, boolean> => new Map(entries);
  it("returns the host's list untouched while nothing is in flight", () => {
    const reactions = [reaction()];
    expect(applyPendingPullRequestReactions(reactions, pending([]))).toBe(reactions);
  });
  it('adds a reaction nobody had yet, in picker order', () => {
    const applied = applyPendingPullRequestReactions([reaction({ content: 'rocket', count: 2, actors: ['a', 'b'] })], pending([['thumbs-up', true]]));
    expect(applied).toEqual([{ content: 'thumbs-up', count: 1, actors: [], viewerHasReacted: true }, { content: 'rocket', count: 2, actors: ['a', 'b'], viewerHasReacted: false }]);
  });
  it('joins and leaves an existing reaction, and drops the pill nobody is left on', () => {
    expect(applyPendingPullRequestReactions([reaction({ count: 2, actors: ['a', 'b'] })], pending([['heart', true]]))).toEqual([{ content: 'heart', count: 3, actors: ['a', 'b'], viewerHasReacted: true }]);
    expect(applyPendingPullRequestReactions([reaction({ count: 2, actors: ['a', 'b'], viewerHasReacted: true })], pending([['heart', false]]))).toEqual([{ content: 'heart', count: 1, actors: ['a', 'b'], viewerHasReacted: false }]);
    expect(applyPendingPullRequestReactions([reaction({ count: 1, viewerHasReacted: true })], pending([['heart', false]]))).toEqual([]);
  });
  it('ignores a pending state the host has already caught up with', () => {
    const reactions = [reaction({ count: 3, viewerHasReacted: true })];
    expect(applyPendingPullRequestReactions(reactions, pending([['heart', true]]))).toEqual(reactions);
    expect(applyPendingPullRequestReactions([], pending([['heart', false]]))).toEqual([]);
  });
});

const draft = (id: string, body = id): PendingReviewComment => ({ id, body, path: 'src/app.ts', position: { kind: 'added', newLine: 1 } });

describe('pull request review drafts', () => {
  it('removes only the line comments included in a submitted snapshot', () => {
    const store = new PullRequestReviewStore();
    store.addComment('review-a', draft('submitted'));
    const submittedIds = store.drafts['review-a']?.map(entry => entry.id) ?? [];
    store.addComment('review-a', draft('added-in-flight'));
    store.removeComments('review-a', submittedIds);
    expect(store.drafts['review-a']).toEqual([draft('added-in-flight')]);
  });
  it('keeps summary bodies isolated by review key', () => {
    const store = new PullRequestReviewStore();
    store.setSummary('review-a', 'Summary A');
    store.setSummary('review-b', 'Summary B');
    store.clearSummary('review-a', 'Summary A');
    expect(store.summaries).toEqual({ 'review-b': 'Summary B' });
  });
  it('keeps drafts on different hosts separate when a thread reviews the same repository and number', () => {
    const reference = { projectId: 'project-a', repository: 'owner/repo', number: 7 };
    const publicKey = pullRequestReviewKey({ ...reference, host: 'github.com' });
    const enterpriseKey = pullRequestReviewKey({ ...reference, host: 'github.example.com' });
    const store = new PullRequestReviewStore();
    store.addComment(publicKey, draft('public'));
    store.setSummary(publicKey, 'Public review');
    expect(store.drafts[enterpriseKey]).toBeUndefined();
    expect(store.summaries[enterpriseKey]).toBeUndefined();
    store.addComment(enterpriseKey, draft('enterprise'));
    store.setSummary(enterpriseKey, 'Enterprise review');
    store.clear(enterpriseKey);
    store.clearSummary(enterpriseKey, 'Enterprise review');
    expect(store.drafts[publicKey]).toEqual([draft('public')]);
    expect(store.summaries[publicKey]).toBe('Public review');
  });
  it('does not clear a summary revised while submission is in flight', () => {
    const store = new PullRequestReviewStore();
    store.setSummary('review-a', 'Submitted body');
    store.setSummary('review-a', 'Revised body');
    store.clearSummary('review-a', 'Submitted body');
    expect(store.summaries['review-a']).toBe('Revised body');
  });
});
