// Ported from T3 Code 5d3d78ab93 apps/web/src/components/ThreadStatusIndicators.linkedPr.browser.test.tsx (MIT, see
// LICENSE-T3), over the row's fields instead of the rendered labels; the status pills follow Sidebar.logic.ts
// resolveThreadStatusPill.
import { expect, test } from 'bun:test';
import { leadingStatus, threadStatusPill } from '../../src/palette/palette-thread-status';
import { opened } from '../support/composer-controls-fixture';

const linksConfig = { environment: { capabilities: { threadPullRequests: true } } };
const link = {
  host: 'github.com', repository: 'example/repo', number: 7, url: 'https://github.com/example/repo/pull/7', source: 'manual',
  linkedAt: '2026-01-01T00:00:00Z', stack: null,
  snapshot: { state: 'merged', title: 'Linked change', headBranch: 'deleted-branch', baseBranch: 'main', isDraft: false, updatedAt: null, syncedAt: '2026-01-03T00:00:00Z' },
};
const thread = (pullRequest: object) => ({ id: 'palette-lead', projectId: 'p1', linkedPullRequest: null, branchPullRequest: null, pullRequests: [pullRequest] });

test('updates palette PR status from linked snapshots without a legacy link or branch PR', async () => {
  const { client } = await opened();
  const merged = leadingStatus(client, thread(link), linksConfig);
  expect(merged).toMatchObject({ leadPrIcon: 'git-merge', leadPrState: 'merged', leadPrLabel: 'PR #7 - Merged: Linked change', leadPending: '' });

  const pending = leadingStatus(client, thread({ ...link, snapshot: null }), linksConfig);
  expect(pending).toMatchObject({ leadPrIcon: '', leadPrLabel: '', leadPending: 'PR #7, status pending' });

  const draft = leadingStatus(client, thread({ ...link, snapshot: { ...link.snapshot, state: 'open', isDraft: true } }), linksConfig);
  expect(draft).toMatchObject({ leadPrIcon: 'git-pull-request-draft', leadPrState: 'draft', leadPrLabel: 'PR #7 - Draft: Linked change', leadPending: '' });
});

test('an older server reads no links, so a link shows no pending glyph', async () => {
  const { client } = await opened();
  expect(leadingStatus(client, thread({ ...link, snapshot: null }), {})).toMatchObject({ leadPrIcon: '', leadPending: '' });
});

test('the status pill follows resolveThreadStatusPill', () => {
  const visited = '2026-10-01T00:00:00.000Z';
  expect(threadStatusPill({ pendingRuntimeRequest: { kind: 'approval' } }, visited)?.label).toBe('Pending Approval');
  expect(threadStatusPill({ pendingRuntimeRequest: { kind: 'user_input' } }, visited)?.label).toBe('Awaiting Input');
  expect(threadStatusPill({ pendingGuestMessageCount: 2 }, visited)?.label).toBe('Guest Request');
  expect(threadStatusPill({ latestRunId: 'r1', status: 'running' }, visited)).toMatchObject({ label: 'Working', pulse: true });
  expect(threadStatusPill({ latestRunId: 'r1', status: 'starting' }, visited)).toMatchObject({ label: 'Connecting', pulse: true });
  expect(threadStatusPill({ latestRunId: 'r1', status: 'running', blockedOnThreadWait: true }, visited)?.label).toBe('Waiting');
  const failed = { latestRunId: 'r1', status: 'failed', latestRunCompletedAt: '2026-10-02T00:00:00.000Z' };
  expect(threadStatusPill(failed, visited)?.label).toBe('Failed');
  // An unopened thread's failure is unseen; its completion is not.
  expect(threadStatusPill(failed, undefined)?.label).toBe('Failed');
  const completed = { latestRunId: 'r1', status: 'completed', latestRunCompletedAt: '2026-10-02T00:00:00.000Z' };
  expect(threadStatusPill(completed, visited)?.label).toBe('Completed');
  expect(threadStatusPill(completed, undefined)).toBeNull();
  expect(threadStatusPill({ ...completed, interactionMode: 'plan', hasActionableProposedPlan: true }, visited)?.label).toBe('Plan Ready');
  expect(threadStatusPill(completed, '2026-10-03T00:00:00.000Z')).toBeNull();
});
