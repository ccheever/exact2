import { describe, expect, test } from 'bun:test';
import { currentPullRequestLink, pullRequestPanelTarget } from './shell-pr';
import { surfaces } from './shell';
import type { T3Client } from './client';
import type { Obj } from './domain';

const link = (number: number, extra: Obj = {}): Obj => ({ host: 'github.com', repository: 'acme/app', number, url: `https://github.com/acme/app/pull/${number}`,
  source: 'agent', linkedAt: `2026-10-0${number}T10:00:00.000Z`, snapshot: { state: 'open', updatedAt: `2026-10-0${number}T10:00:00.000Z` }, ...extra });

describe('Pull request panel target (upstream f90b77d809)', () => {
  test('the current link wins, reusing the legacy object only for the same PR', () => {
    const legacy = { projectId: 'p1', repository: 'acme/app', number: 3, url: 'https://github.com/acme/app/pull/3', title: 'Legacy' };
    expect(pullRequestPanelTarget({ projectId: 'p1', pullRequests: [link(3)], linkedPullRequest: legacy })).toBe(legacy);
    expect(pullRequestPanelTarget({ projectId: 'p1', pullRequests: [link(4)], linkedPullRequest: legacy }))
      .toEqual({ projectId: 'p1', host: 'github.com', repository: 'acme/app', number: 4, url: 'https://github.com/acme/app/pull/4' });
    expect(pullRequestPanelTarget({ projectId: 'p1', pullRequests: [], linkedPullRequest: null, branchPullRequest: { number: 9 } })).toEqual({ number: 9 });
    expect(pullRequestPanelTarget({ projectId: 'p1' })).toBeNull();
  });
  test('open work first; tombstoned stack members are never current', () => {
    expect(currentPullRequestLink([link(1, { snapshot: { state: 'merged', updatedAt: '2026-10-09T00:00:00.000Z' } }), link(2)])).toMatchObject({ number: 2 });
    expect(currentPullRequestLink([link(1), link(2)])).toMatchObject({ number: 2 });
    expect(currentPullRequestLink([link(1, { source: 'stack-dismissed' })])).toBeNull();
    expect(currentPullRequestLink([link(1, { snapshot: { state: 'closed', updatedAt: '2026-10-08T00:00:00.000Z' } }), link(2, { snapshot: { state: 'merged', updatedAt: '2026-10-03T00:00:00.000Z' } })])).toMatchObject({ number: 1 });
  });
  test('a PR known only through pullRequests makes the Pull request (P) surface available where the environment reads pull requests (r5-panels)', () => {
    const config = { environment: { capabilities: { pullRequests: true, threadPullRequests: true } } };
    const client = { threadId: 't1', projectId: 'p1', ready: true, config, shell: { threads: [{ id: 't1', projectId: 'p1', pullRequests: [link(5)] }], projects: [{ id: 'p1' }] } } as unknown as T3Client;
    const row = surfaces(client).find(entry => entry.id === 'pull-request')!;
    expect(row).toMatchObject({ available: true, reason: '' });
    // ChatView pullRequestSurfaceAvailable needs supportsPullRequests; the reason is always the one hint.
    const unread = { ...client, config: { environment: { capabilities: { threadPullRequests: true } } } } as unknown as T3Client;
    expect(surfaces(unread).find(entry => entry.id === 'pull-request')).toMatchObject({ available: false, reason: 'No pull request on this branch yet.' });
    const bare = { threadId: 't1', projectId: 'p1', ready: true, config, shell: { threads: [{ id: 't1', projectId: 'p1', pullRequests: [link(5, { source: 'stack-dismissed' })] }], projects: [{ id: 'p1' }] } } as unknown as T3Client;
    expect(surfaces(bare).find(entry => entry.id === 'pull-request')!.reason).toBe('No pull request on this branch yet.');
    expect(surfaces(bare).find(entry => entry.id === 'pull-requests')!.reason).toBe('No linked pull requests available.');
  });
});
