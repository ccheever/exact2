import { afterEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { branchPullRequest, syncBranchPullRequests } from '../../src/sidebar/sidebar-branch-pr';
import { largestPullRequestReadCount } from '../../src/pull-requests/pages-pr-paging';
import { sortProjectsByGroupOrder } from '../../src/pull-requests/pages-prs';
import { newestPullRequestSummary, observePullRequestSummary, sharedPullRequestSummary } from '../../src/pull-requests/pages-pr-shared';
import { queueQuickAction, queueCloseBatch, quickRow } from '../../src/pull-requests/pages-pr-quick';
import { composerReplyEvent } from '../../src/composer/composer-replies';
import { toasts } from '../../src/shell/toast';
import { resolveLinkInput } from '../../src/palette/palette-linkpr';
import { contractComponent } from '../support/contract-action-fixture';
import { pullRequestRouter } from '../../src/pull-requests/pages-pr-routing';

const ref = { projectId: 'p', host: 'github.com', repository: 'acme/repo', number: 7 };
const summary = (over: Obj = {}): Obj => ({ ...ref, title: 'Change seven', state: 'open', updatedAt: '2026-10-18T10:00:00Z', ...over });
afterEach(() => pullRequestRouter.reset());

describe('pull request list bounds and project order', () => {
  test('refresh size is the largest host read, with non-GitHub repositories counted separately', () => {
    const rows = (count: number, host: string, provider: string, repository: string, environmentId = 'env') => Array.from({ length: count }, () => ({ host, provider, repository, environmentId }));
    expect(largestPullRequestReadCount([...rows(80, 'github.com', 'github', 'a/b'), ...rows(40, 'github.com', 'github', 'c/d'), ...rows(100, 'forge.local', 'forgejo', 'a/b'), ...rows(100, 'forge.local', 'forgejo', 'c/d'), ...rows(50, 'github.com', 'github', 'a/b', 'other')])).toBe(120);
  });
  test('filter groups use sidebar order and preserve the order inside each group', () => {
    expect(sortProjectsByGroupOrder([{ id: 'a' }, { id: 'b' }, { id: 'c' }, { id: 'd' }], [{ memberProjectIds: ['c', 'a'] }, { memberProjectIds: ['b'] }]).map(project => project.id)).toEqual(['a', 'c', 'b', 'd']);
  });
  test('a full URL links from a folder with no Git identity, while a number still needs its repository', () => {
    expect(resolveLinkInput('https://github.example/team/repo/pull/17', [{ id: 'folder', workspaceRoot: '/notes' }], 'folder', 'multiple')).toMatchObject({ link: { host: 'github.example', repository: 'team/repo', number: 17 } });
    expect(resolveLinkInput('#17', [{ id: 'folder' }], 'folder', 'multiple')).toMatchObject({ error: expect.any(String) });
  });
});

describe('shared pull request observations', () => {
  test('merged is final; newer host updates beat a later receipt of stale state', () => {
    const merged = summary({ state: 'merged' }), newer = summary({ state: 'closed', updatedAt: '2026-10-18T11:00:00Z' });
    expect(newestPullRequestSummary(merged, newer)).toBe(merged);
    expect(newestPullRequestSummary(newer, summary())).toBe(newer);
  });
  test('a saved host-qualified summary updates another control without bringing old detail counts with it', () => {
    const client = new T3Client(); client.environmentId = 'env';
    client.shell.threads = [{ id: 't', projectId: 'p', pullRequests: [{ ...ref, url: 'https://github.com/acme/repo/pull/7', snapshot: summary({ state: 'merged', updatedAt: '2026-10-18T11:00:00Z' }) }] }];
    observePullRequestSummary(client, 'env', ref, summary({ additions: 100, checksState: 'passing' }));
    expect(sharedPullRequestSummary(client, ref, summary())).toMatchObject({ state: 'merged' });
    expect(sharedPullRequestSummary(client, { ...ref, host: 'other.example' }, null)).toMatchObject({ checksState: 'passing' });
    expect(sharedPullRequestSummary(client, { ...ref, projectId: 'other' }, null)?.state).toBe('merged');
    expect(sharedPullRequestSummary(client, ref, null, 'other-env')).toBeNull();
    expect(sharedPullRequestSummary(client, { ...ref, host: '' }, null)).not.toHaveProperty('additions');
  });
  test('a snapshot without the host update time does not replace a fresh read', () => {
    const client = new T3Client(); client.environmentId = 'env';
    client.shell.threads = [{ id: 't', projectId: 'p', pullRequests: [{ ...ref, snapshot: { title: 'Old saved title', state: 'closed' } }] }];
    expect(sharedPullRequestSummary(client, ref, summary())?.title).toBe('Change seven');
  });
});

function queuedFixture() {
  const client = new T3Client(); Object.assign(client, { origin: 'http://127.0.0.1:16947', environmentId: 'env', connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1 });
  client.config = { environment: { capabilities: { pullRequests: true } } }; client.shell.projects = [{ id: 'p', workspaceRoot: '/repo' }];
  const sent: { key: string; payload: Obj }[] = [];
  client.ids = async () => [`id-${sent.length}`];
  client.call = async (_native, input) => { sent.push({ key: String(input.deliver), payload: obj(input.payload) }); return {}; };
  const native = { available: true, watch() {}, async later() { return { ok: true, generation: 1, value: {} }; } } as Native;
  const entry = (number: number) => summary({ number, provider: 'github', state: 'open' });
  return { client, native, sent, entry };
}

test('two fast closes queue before either host reply arrives and repeated clicks on one row are ignored', async () => {
  const { client, native, sent, entry } = queuedFixture();
  let refreshed = 0;
  const run = (number: number) => queueQuickAction(client, native, JSON.stringify({ action: 'close', ref: String(number) }), id => entry(Number(id)), () => { refreshed++; });
  await run(7); await run(8); await run(7);
  expect(sent.map(item => item.payload.number)).toEqual([7, 8]);
  expect(quickRow(client, entry(7)).speedPending).toBe(true);
  composerReplyEvent(client, { key: sent[0]!.key, value: { _reply: {} } });
  composerReplyEvent(client, { key: sent[1]!.key, value: { _reply: {} } });
  await Promise.resolve();
  expect(refreshed).toBe(2);
  expect(quickRow(client, entry(7)).speedPending).toBe(false);
  expect(toasts(client).filter(toast => toast.title === 'Pull request closed')).toHaveLength(2);
});

const list = contractComponent('src/pull-requests/pages-prs.contract', 'PrList');
test('a close sweep includes eligible rows in its group, stops at another group, and Escape cancels it', () => {
  const calls: unknown[][] = [];
  const row = (key: string, disabled = false) => ({ key, ref: key, speed: [{ action: 'close', disabled }] });
  const scope = { page: { groups: [{ key: 'authored', rows: [row('a'), row('b'), row('c', true)] }, { key: 'others', rows: [row('d')] }] }, frame: (id: string) => ({ y: { a: 100, b: 156, c: 212, d: 268 }[id.slice('pr-row-wrap-'.length)] ?? 0, height: 56 }), min: Math.min, max: Math.max, abs: Math.abs, act: (...args: unknown[]) => calls.push(args) };
  const state = list.state(scope);
  list.call('sweepDown', state, scope, ['a', 'authored', 'close', { buttons: 1, clientY: 120 }]);
  list.call('sweepMove', state, scope, [{ buttons: 1, clientY: 280 }]);
  expect(state.sweepRefs).toEqual(['a', 'b']);
  list.call('sweepCancel', state, scope, []);
  list.call('sweepUp', state, scope, [{ buttons: 0, clientY: 280 }]);
  expect(calls).toEqual([]);
});

test('a merged fallback summary is not polled again after its minute expires', async () => {
  const client = new T3Client(); Object.assign(client, { environmentId: 'env', connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1 });
  client.config = { environment: { capabilities: { pullRequests: true, threadPullRequests: true } } };
  client.shell.threads = [{ id: 't', projectId: 'p', pullRequests: [], branchPullRequest: { ...ref, url: 'https://github.com/acme/repo/pull/7' } }];
  let reads = 0;
  client.rpc = async () => { reads++; return summary({ state: 'merged', url: 'https://github.com/acme/repo/pull/7' }); };
  const native = { available: true } as Native;
  await syncBranchPullRequests(client, native, 100_000);
  await syncBranchPullRequests(client, native, 161_000);
  await syncBranchPullRequests(client, native, 300_000);
  expect(reads).toBe(1);
  expect(branchPullRequest(client, client.shell.threads[0]!)?.state).toBe('merged');
});

const linkedRow = contractComponent('panel.contract', 'LinkedPrRowView');
test('middle click leaves a linked row in the browser, while its drag inserts context only inside the composer', () => {
  const sent: unknown[][] = [];
  const state = linkedRow.state();
  const scope = { row: { number: 7, url: 'https://github.com/a/b/pull/7', context: '{"number":7}' }, local: (...args: unknown[]) => sent.push(args), frame: (id: string) => id === 'composer-surface' ? { x: 100, y: 300, width: 400, height: 100 } : { x: 0, y: 0, width: 80, height: 40 } };
  linkedRow.call('down', state, scope, [{ buttons: 4, clientX: 20, clientY: 20 }]);
  linkedRow.call('up', state, scope, [{ buttons: 0, clientX: 20, clientY: 20 }]);
  expect(sent).toEqual([['surface-linked-pr-open', 'held', 'https://github.com/a/b/pull/7']]);
  sent.length = 0;
  linkedRow.call('down', state, scope, [{ buttons: 1, clientX: 20, clientY: 20 }]);
  linkedRow.call('drag', state, scope, [180, 330]);
  linkedRow.call('drop', state, scope, [0, 0]);
  expect(sent).toEqual([['surface-pr-insert', '', '{"number":7}']]);
});

test('a close batch deduplicates eligible rows and reports one combined result after both replies', async () => {
  const { client, native, sent, entry } = queuedFixture();
  await queueCloseBatch(client, native, ['7', '8', '7', '9'], ref => entry(Number(ref)) && { ...entry(Number(ref)), ...(ref === '9' ? { state: 'merged' } : {}) }, () => {});
  expect(sent.map(item => item.payload.number)).toEqual([7, 8]);
  composerReplyEvent(client, { key: sent[0]!.key, value: { _reply: {} } });
  await Promise.resolve();
  expect(toasts(client)).toHaveLength(0);
  composerReplyEvent(client, { key: sent[1]!.key, value: { _reply: {} } });
  await Promise.resolve();
  expect(toasts(client)).toEqual([expect.objectContaining({ kind: 'success', title: 'Closed 2 pull requests' })]);
});
