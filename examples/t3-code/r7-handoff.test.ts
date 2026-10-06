// Lane r7-handoff: the hand-off's thread id (useHandleNewThread + startHandoff's threadId) and the
// composer strip's pull request badge (BranchToolbarBranchSelector + ThreadPullRequestBadgeControl).
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from './domain';
import { composerBranches } from './composer-controls-branch';
import { decodeComposerControls } from './composer-controls';
import { connected } from './composer-controls-fixture';
import { draftThreadId, ensureDraftThreadId, forgetDraftThreadId, launchThreadId, withHandoffThread } from './r7-handoff-thread';
import { stripPr } from './r7-handoff-strip';
import type { T3Client } from './client';

const URL = (n: number) => `https://github.com/t3-fixture/pr-demo/pull/${n}`;
const pr = (n: number, extra: Obj = {}) => ({ number: n, title: `Change ${n}`, url: URL(n), baseRef: 'main', headRef: 'feature/conflict', state: 'open', ...extra });
const link = (n: number, extra: Obj = {}) => ({ host: 'github.com', repository: 't3-fixture/pr-demo', number: n, url: URL(n), source: 'manual', linkedAt: '2026-10-04T10:00:00.000Z',
  snapshot: { title: `Change ${n}`, state: 'open', isDraft: false, syncedAt: '2026-10-04T10:00:00.000Z' }, watch: null, ...extra });

describe('the draft launches as the thread its hand-off prepared', () => {
  test('one id per draft, kept with the draft, used by the launch and then forgotten', async () => {
    const { client, native, command } = await connected();
    const key = client.draftKey;
    expect(draftThreadId(client)).toBe('');
    const id = await ensureDraftThreadId(client, native);
    expect(id).toMatch(/^id-\d+$/);
    expect(await ensureDraftThreadId(client, native)).toBe(id);
    expect(launchThreadId(client, key, 'fresh')).toBe(id);
    expect(launchThreadId(client, 'env1:new:other', 'fresh')).toBe('fresh');
    // The preference file keeps it across a relaunch.
    expect(decodeComposerControls(JSON.parse(JSON.stringify(client.local.composerControls))).draftThreads).toEqual({ [key]: id });
    await command('send', '', 'Resolve the conflicts');
    expect(native.committed.at(-1)).toMatchObject({ method: 'orchestration.launchThread', threadId: id });
    expect(client.threadId).toBe(id);
    expect(draftThreadId(client, key)).toBe('');
  });
  test('a draft without a hand-off launches under a fresh id', async () => {
    const { client, native, command } = await connected();
    await command('send', '', 'Plain');
    const launched = native.committed.at(-1)!;
    expect(launched.method).toBe('orchestration.launchThread');
    expect(launched.threadId).toMatch(/^id-\d+$/);
    expect(client.local.composerControls.draftThreads ?? {}).toEqual({});
  });
  test('the prepare input carries the thread id only when there is one; a bad saved id is dropped', () => {
    expect(withHandoffThread({ cwd: '/r', reference: URL(1), mode: 'worktree' }, 't-9')).toEqual({ cwd: '/r', reference: URL(1), mode: 'worktree', threadId: 't-9' });
    expect(withHandoffThread({ cwd: '/r' }, '')).toEqual({ cwd: '/r' });
    expect(decodeComposerControls({ draftThreads: { a: 'ok-1', b: 'no spaces', c: 7 } }).draftThreads).toEqual({ a: 'ok-1' });
    const client = { local: { composerControls: { draftThreads: { k: 'x' } } } } as unknown as T3Client;
    forgetDraftThreadId(client, 'k');
    expect(client.local.composerControls.draftThreads).toEqual({});
  });
});

/** A fake whose status reports `pr` for the checkout at `cwd` (VcsStatusResult.pr). */
async function withStatus(status: Obj) {
  const context = await connected();
  const later = context.native.later.bind(context.native);
  context.native.later = async (input: unknown) => {
    const request = obj(input);
    if (request.op === 'request' && request.method === 'vcs.refreshStatus') return context.native.ok({ isRepo: true, refName: 'feature/conflict', ...status });
    return later(input);
  };
  obj(obj(context.client.config.environment).capabilities).pullRequests = true;
  obj(obj(context.client.config.environment).capabilities).threadPullRequests = true;
  context.client.shell.projects[0]!.repositoryIdentity = { provider: 'github', owner: 't3-fixture', name: 'pr-demo', host: 'github.com' };
  return context;
}

describe('the strip\'s pull request badge', () => {
  test('a hand-off draft on the pull request\'s checkout shows its number in the state\'s tone', async () => {
    const { client, native } = await withStatus({ pr: pr(101), sourceControlProvider: { kind: 'github', name: 'GitHub', baseUrl: 'https://github.com' } });
    client.local.composerControls.contexts[client.draftKey] = { envMode: 'worktree', branch: 'feature/conflict', worktreePath: '/home/worktrees/pr-demo/feature-conflict' };
    const strip = await composerBranches(client, native, false, '');
    expect(strip).toMatchObject({ show: true, branchLabel: 'feature/conflict', prShow: true, prText: '101', prIcon: 'git-pull-request-arrow', prState: 'open', prList: false,
      prLabel: 'PR #101 - Open: Change 101', prUrl: URL(101) });
    expect(strip.prTips).toEqual([expect.objectContaining({ number: '#101', title: 'Change 101', state: 'open' })]);
    const merged = stripPr(client, { cwd: '/x', threadBranch: 'feature/conflict', branch: 'feature/conflict', status: { refName: 'feature/conflict', pr: pr(101, { state: 'merged' }) } });
    expect(merged).toMatchObject({ prIcon: 'git-merge', prState: 'merged', prLabel: 'PR #101 - Merged: Change 101' });
    const draftPr = stripPr(client, { cwd: '/x', threadBranch: 'feature/conflict', branch: 'feature/conflict', status: { refName: 'feature/conflict', pr: pr(103, { isDraft: true }) } });
    expect(draftPr).toMatchObject({ prText: '103', prIcon: 'git-pull-request-draft', prState: 'draft' });
  });
  test('no badge when the status is about another branch, or reports no pull request', async () => {
    const { client, native } = await withStatus({ pr: null });
    client.local.composerControls.contexts[client.draftKey] = { envMode: 'local', branch: 'feature/conflict', worktreePath: '/wt' };
    expect((await composerBranches(client, native, false, '')).prShow).toBe(false);
    expect(stripPr(client, { cwd: '/x', threadBranch: 'feature/conflict', branch: 'main', status: { refName: 'main', pr: pr(9) } }).prShow).toBe(false);
    expect(stripPr(client, { cwd: '/x', threadBranch: 'a', branch: 'a', status: { refName: 'b', pr: pr(9) } }).prShow).toBe(false);
  });
  test('a thread\'s links win over the branch: one link, unrelated links as +N, a stack of N', async () => {
    const { client, native, command } = await withStatus({ pr: pr(200) });
    client.local.clientSettings.persistComposerContextStrip = true;
    const t1 = client.shell.threads.find(entry => entry.id === 't1')!;
    Object.assign(t1, { branch: 'feature/conflict', pullRequests: [link(101)] });
    await command('select-thread', 't1');
    obj(client.thread!.projection.thread).branch = 'feature/conflict';
    expect(await composerBranches(client, native, false, '')).toMatchObject({ prShow: true, prText: '101', prState: 'open', prList: false, prLabel: 'PR #101 - Open: Change 101' });
    t1.pullRequests = [link(101), link(102, { snapshot: { title: 'Change 102', state: 'merged', isDraft: false } })];
    expect(stripPr(client, { cwd: '/x', threadBranch: 'feature/conflict', branch: 'feature/conflict', status: null }))
      .toMatchObject({ prText: '+2', prList: true, prState: 'open', prLabel: 'PR #101 - Open: Change 101, and 1 more linked; overall open' });
    t1.pullRequests = [link(101, { stack: { id: 's', layers: [{ number: 101 }, { number: 102 }] } }), link(102, { stack: { id: 's', layers: [{ number: 101 }, { number: 102 }] } })];
    const stacked = stripPr(client, { cwd: '/x', threadBranch: 'feature/conflict', branch: 'feature/conflict', status: null });
    expect(stacked).toMatchObject({ prText: '2', prIcon: 'layers', prList: true, prLabel: 'Stack of 2 pull requests, open' });
    expect(stacked.prTips.map(tip => [tip.number, tip.depth, tip.stack])).toEqual([['#101', 0, 'stack · 2'], ['#102', 1, '']]);
    // A link without a snapshot stays muted: "status pending".
    t1.pullRequests = [link(104, { snapshot: null })];
    expect(stripPr(client, { cwd: '/x', threadBranch: 'feature/conflict', branch: 'feature/conflict', status: null }))
      .toMatchObject({ prText: '104', prState: '', prIcon: 'git-pull-request-arrow', prLabel: 'PR #104, status pending' });
  });
});
