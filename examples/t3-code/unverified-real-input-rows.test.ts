// unverified-real-input-rows (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): UV-1, the age tip that never showed under a real
// pointer in the pull request Code tab (realinput-1009). The reference has none there: PullRequestReviewAnnotation.tsx's
// ReviewThreadCard draws the author as PullRequestActorLabel (a tooltip with the login) and the age as a bare
// `<span>{formatRelativeTimeLabel(comment.createdAt)}</span>`; PullRequestTimelineTab.tsx draws every age the same way.
// Only the Summary tab's comment age (PullRequestSummaryTab.tsx CommentMeta) has the full date as a tooltip. UV-3: a commit
// link's tip (its URL, one long word) ran past its bubble and the window; the reference's TooltipPopup is `max-w-80
// wrap-anywhere`. UV-2 and UV-4 change no code: their fixtures and real-input steps are in the task record.
import { describe, expect, test } from 'bun:test';
import type { Obj } from './domain';
import { presentTimeline } from './pages-pr-timeline';
import { presentSummary } from './pages-pr-summary';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of one component of a contract file, from its `component` line to the next top-level line. */
async function component(file: string, name: string): Promise<string[]> {
  const lines = (await source(file)).split('\n'), start = lines.findIndex(line => line === `component ${name}`);
  expect(start).toBeGreaterThan(-1);
  const end = lines.findIndex((line, index) => index > start && line !== '' && !line.startsWith(' ') && !line.startsWith('//'));
  return lines.slice(start, end === -1 ? undefined : end);
}

describe('UV-1: the Code tab thread card has the author tip only, as ReviewThreadCard', () => {
  test('the author keeps its LayerTip; the age is plain text with a test id and no tip', async () => {
    const card = await component('pages-pr-threads.contract', 'PrdThreadComment');
    expect(card.some(line => line.includes('LayerTip(label=comment.authorTip') && line.includes('pull-request-thread-author-tip-'))).toBe(true);
    const age = card.findIndex(line => line.trim().startsWith('text comment.age '));
    expect(age).toBeGreaterThan(-1);
    expect(card[age]).toContain('testId=`pull-request-thread-age-${comment.id}`');
    // Nothing wraps it: the line above is a comment or the author's tip, never a Tip or LayerTip.
    expect(card.filter(line => /\b(Layer)?Tip\(label=comment\.age/.test(line))).toEqual([]);
    expect(card.join('\n')).not.toContain('pull-request-thread-age-tip-');
    expect((await source('pages-pr-threads.contract')).includes('ageTip')).toBe(false);
  });
});

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

describe('UV-1: no age on the Timeline has a tip, as PullRequestTimelineTab', () => {
  test('the comment cards, commits, lifecycle and verdict rows draw their age bare', async () => {
    const file = await source('pages-pr-timeline.contract');
    expect(file.includes('ageTip')).toBe(false);
    const ages = file.split('\n').map((line, index, lines) => ({ line, above: lines[index - 1] ?? '' })).filter(({ line }) => /^\s*text (row|card)\.age /.test(line));
    expect(ages.length).toBe(4);
    for (const { above } of ages) expect(above).not.toMatch(/\bTip\(/);
    // The stale verdict's tooltip is the reference's one Timeline tooltip, and stays.
    expect(file).toContain('Tip(label=(row.stale ? row.staleLabel : "")');
  });
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

describe('UV-3: a tip on the hover layer breaks a long word inside its bubble, as TooltipPopup (wrap-anywhere)', () => {
  test('HoverText wraps anywhere within its 20rem bubble', async () => {
    const tip = await component('hover-layer.contract', 'HoverText');
    expect(tip.some(line => line.includes('max-width="20rem"'))).toBe(true);
    expect(tip.find(line => line.trim().startsWith('text tip.text '))).toContain('overflow-wrap="anywhere"');
  });
  test('a commit link\'s tip is its URL on that layer (kind "tip", drawn by HoverText)', async () => {
    const run = await component('markdown.contract', 'PrLinkRun');
    expect(run.join('\n')).toContain('chip.target != "" ? "pr-preview" : "tip", run.href');
  });
});
