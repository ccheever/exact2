// T3 Code 1e2ecbd975 (MIT; see LICENSE-T3): the age tip that never showed under a real pointer in the pull
// request Code tab. The reference has none there: PullRequestReviewAnnotation.tsx's
// ReviewThreadCard draws the author as PullRequestActorLabel (a tooltip with the login) and the age as a bare
// `<span>{formatRelativeTimeLabel(comment.createdAt)}</span>`; PullRequestTimelineTab.tsx draws every age the same way.
// Only the Summary tab's comment age (PullRequestSummaryTab.tsx CommentMeta) has the full date as a tooltip. A commit
// link's tip (its URL, one long word) ran past its bubble and the window; the reference's TooltipPopup is `max-w-80
// wrap-anywhere`.
import { describe, expect, test } from 'bun:test';
import type { Obj } from './domain';
import { presentTimeline } from './pages-pr-timeline';
import { presentSummary } from './pages-pr-summary';

const NOW = Date.parse('2026-10-08T12:00:00Z');
const at = (minutesAgo: number) => new Date(NOW - minutesAgo * 60_000).toISOString();
const person = (login: string): Obj => ({ login, name: null, avatarUrl: null });
const detail: Obj = {
  provider: 'github', projectId: 'p1', repository: 'acme/playground', number: 168, title: 'Build the catalog', body: '', url: 'https://github.com/acme/playground/pull/168',
  author: person('primary'), state: 'open', isDraft: false, createdAt: at(600), updatedAt: at(60), mergedAt: null, closedAt: null, reviewers: [], labels: [], checks: [],
  capabilities: {}, viewerPermissions: {},
};
const remark = (id: string, minutesAgo: number, over: Obj = {}): Obj => ({ id, kind: 'issue-comment', author: person('second'), body: `Remark ${id}`, createdAt: at(minutesAgo),
  url: `https://github.com/acme/playground/pull/168#${id}`, path: null, reviewState: null, ...over });
const activity: Obj = {
  author: person('primary'), reviewers: [person('second')], commentsTruncated: false, reviewThreads: [],
  comments: [remark('c1', 300), remark('c2', 290), remark('approve', 100, { kind: 'review', reviewState: 'APPROVED', body: 'Looks good.' })],
  commits: [{ oid: 'aaaaaaa1', messageHeadline: 'Add the catalog', committedDate: at(500) }],
};

describe('No age on the Timeline has a tip, as PullRequestTimelineTab', () => {
  test('the rows carry the relative age and nothing for a tip', () => {
    const view = presentTimeline({ detail, activity, activityPending: false, activityError: '', now: NOW });
    const rows = view.newest;
    expect(rows.map(row => row.kind)).toEqual(['verdict', 'comments', 'commit', 'opened']);
    for (const row of rows) { expect(row).not.toHaveProperty('ageTip'); for (const card of row.cards) expect(card).not.toHaveProperty('ageTip'); }
    expect(rows.find(row => row.kind === 'commit')?.age).toBe('8h ago');
    expect(rows.find(row => row.kind === 'comments')?.cards.map(card => card.age)).toEqual(['4h ago', '5h ago']); // newest first
  });
  test('the Summary tab keeps its full-date tip (CommentMeta)', () => {
    const view = presentSummary({ detail, activity, activityPending: false, activityError: '', now: NOW, listEntry: null });
    const card = view.newest.find(entry => entry.id === 'c1')!;
    expect(card.ageTip).toContain('Open comment on host');
    expect(card.ageTip.length).toBeGreaterThan(' · Open comment on host'.length);
  });
});
