// The Pull Requests page's row extras (pr-handoffs-and-quick-actions): the Shift quick actions
// (PullRequestSpeedActions: which buttons a row offers, the merge's own checks, the toasts and the row
// it writes), speed mode read from the native monitor, and the checks and stack popovers read when
// opened. Driven through the list resource (pages-prs.ts pullRequestsPage) and the panel's command
// route (pages-pr-detail.ts prCommand) against injected host replies.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { toasts } from './toast';
import { prCommand } from './pages-pr-detail';
import { pullRequestsPage, rowRef } from './pages-prs';
import { prQuickLocal, speedActions } from './pages-pr-quick';
import { listRelist } from './pages-pr-actions';
import { noteNow } from './composer-controls';

const entry = (number: number, extra: Obj = {}): Obj => ({
  provider: 'github', host: 'github.com', projectId: 'p1', projectTitle: 'sandbox', repository: 'lane/sandbox', number, title: `Change ${number}`,
  url: `https://github.com/lane/sandbox/pull/${number}`, author: { login: 'lane-primary' }, headBranch: `feature/${number}`, baseBranch: 'main',
  state: 'open', isDraft: false, mergeability: 'mergeable', additions: 3, deletions: 1, checksState: 'failing',
  createdAt: '2026-10-07T00:00:00.000Z', updatedAt: '2026-10-08T00:00:00.000Z', viewerReviewRequested: false, labels: [], ...extra,
});
const detail = (number: number, extra: Obj = {}): Obj => ({ ...entry(number), workspaceRoot: '/repos/sandbox', changedFiles: 1,
  capabilities: { actions: ['merge', 'close', 'reopen', 'ready'], mergeMethods: ['merge', 'squash', 'rebase'], stacks: true, stackActions: false },
  viewerPermissions: { actions: ['merge', 'close', 'reopen', 'ready'] }, mergeCapabilities: { merge: false, squash: true, rebase: true },
  checks: [{ name: 'ci/build', status: 'success', url: 'https://ci/1' }, { name: 'ci/test', status: 'failure', description: '2 tests failed', url: 'https://ci/2' }], ...extra });
type Reply = (payload: Obj) => unknown;
function fakeClient(replies: Record<string, Reply>, speed = false) {
  const calls: { method: string; payload: Obj; write: boolean }[] = [];
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, revision: 0, generation: 1, local: {} as Obj,
    config: { environment: { capabilities: { pullRequests: true, threadPullRequests: true, pullRequestChecks: true } }, settings: { pullRequestMergeMethod: 'rebase' } },
    shell: { projects: [{ id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox' }], threads: [] as Obj[] },
    rpc: async (_native: unknown, method: string, payload: Obj, write = false) => {
      calls.push({ method, payload, write });
      const reply = replies[method];
      if (!reply) throw new Error(`no reply for ${method}`);
      return reply(payload);
    },
    restAccess: () => ({ call: async (request: Obj) => (request.op === 'sidebarSpeedMode' ? { speedMode: speed } : {}) }),
  } as unknown as T3Client;
  return { client, calls };
}
const listOf = (...entries: Obj[]): Record<string, Reply> => ({
  'pullRequests.list': () => ({ entries, viewers: {}, providers: [{ host: 'github.com', kind: 'github', configured: true }], errors: [], truncated: false }),
  'pullRequests.listStats': () => ({ stats: [] }),
});
let wakes = 0;
const native = { available: true, watch: () => {}, later: async () => { wakes++; return { ok: true }; } } as unknown as Native;
async function page(client: T3Client) {
  const input = { open: true, refresh: 0, now: Date.parse('2026-10-08T01:00:00.000Z'), selected: '', query: '', typed: false };
  let seen = wakes, view = await pullRequestsPage(client, native, input);
  for (let asked = 0; wakes > seen && asked < 6; asked++) { seen = wakes; view = await pullRequestsPage(client, native, input); }
  return view;
}
const rows = (view: Awaited<ReturnType<typeof page>>) => view.groups.flatMap(group => group.rows);
const quick = (client: T3Client, row: Obj, action: string) => prCommand(client, native, 'quick', '', JSON.stringify({ ref: rowRef(row), action }));

describe('the Shift quick actions (PullRequestSpeedActions)', () => {
  test('a closed row reopens, a draft closes or readies, an open one closes or merges; merged and other hosts offer none', () => {
    const keys = (row: Obj) => speedActions(row, false).map(action => action.action);
    expect(keys(entry(1, { state: 'closed' }))).toEqual(['reopen']);
    expect(keys(entry(2, { isDraft: true }))).toEqual(['close', 'ready']);
    expect(keys(entry(3))).toEqual(['close', 'merge']);
    expect(keys(entry(4, { state: 'merged' }))).toEqual([]);
    expect(keys(entry(5, { provider: 'gitlab' }))).toEqual([]);
    expect(speedActions(entry(3), false)).toEqual([
      expect.objectContaining({ label: 'Close', icon: 'git-pull-request-closed', destructive: true, disabled: false, tooltip: 'Close immediately', aria: 'Close #3' }),
      expect.objectContaining({ label: 'Merge', icon: 'git-merge', destructive: false, disabled: false, tooltip: 'Merge immediately', aria: 'Merge #3' }),
    ]);
    // A stacked row merges from the panel, where the whole stack is.
    expect(speedActions(entry(6, { stack: { number: 120, position: 1, size: 2, base: 'main' } }), false)[1]).toMatchObject({ disabled: true, tooltip: 'Open this pull request to merge its stack' });
    expect(speedActions(entry(3), true).every(action => action.disabled)).toBe(true);
  });
  test('speed mode is the native monitor\'s, read with every answer of the page', async () => {
    expect((await page(fakeClient(listOf(entry(1)), true).client)).speedMode).toBe(true);
    expect((await page(fakeClient(listOf(entry(1)), false).client)).speedMode).toBe(false);
  });
  test('Close runs at once, says "Pull request closed", writes the row closed and reads the list again', async () => {
    const { client, calls } = fakeClient({ ...listOf(entry(7)), 'pullRequests.runAction': () => ({}) });
    noteNow(client, Date.parse('2026-10-08T01:00:00.000Z')); // the reader's clock, which the row's note is written at
    const [row] = rows(await page(client));
    const relist = listRelist(client);
    expect(await quick(client, row!, 'close')).toBe('');
    expect(calls.filter(call => call.method === 'pullRequests.runAction')).toEqual([{ method: 'pullRequests.runAction', write: true,
      payload: { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7, action: 'close' } }]);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Pull request closed' });
    expect(listRelist(client)).toBe(relist + 1);
    // The list's open filter no longer holds a closed row until the host says otherwise.
    expect(rows(await page(client)).map(entry => entry.number)).toEqual([]);
  });
  test('Merge reads the detail around the cache and merges with the project default where the repository allows it', async () => {
    // No stack: the server answers null, which the client hands back as an empty record (the real-input session's find).
    const { client, calls } = fakeClient({ ...listOf(entry(8)), 'pullRequests.detail': () => detail(8, { capabilities: { actions: ['merge'], mergeMethods: ['merge', 'squash', 'rebase'], stackActions: true } }),
      'pullRequests.stack': () => ({}), 'pullRequests.runAction': () => ({}) });
    const [row] = rows(await page(client));
    await quick(client, row!, 'merge');
    expect(calls.find(call => call.method === 'pullRequests.detail')!.payload).toMatchObject({ number: 8, allowStale: false });
    expect(calls.find(call => call.method === 'pullRequests.runAction')!.payload).toMatchObject({ action: 'merge', mergeMethod: 'rebase' });
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Merge requested' });
  });
  test('a merge the row cannot know is refused in the reference\'s words, and nothing is sent', async () => {
    const cases: [Obj, Record<string, Reply>, string][] = [
      [{ isDraft: true }, {}, 'This pull request cannot be merged.'],
      [{ state: 'closed' }, {}, 'This pull request cannot be merged.'],
      [{ viewerPermissions: { actions: ['close'] } }, {}, 'This pull request cannot be merged.'],
      [{ capabilities: { actions: ['merge'], mergeMethods: ['merge'], stackActions: true } }, { 'pullRequests.stack': () => ({ number: 120, base: 'main', layers: [{ number: 9, headBranch: 'feature/9', state: 'open' }] }) }, 'Open this pull request to merge its stack.'],
      [{ mergeCapabilities: { merge: false, squash: false, rebase: false } }, {}, 'No merge method is available for this repository.'],
    ];
    for (const [over, extra, sentence] of cases) {
      const { client, calls } = fakeClient({ ...listOf(entry(9)), 'pullRequests.detail': () => detail(9, over), 'pullRequests.runAction': () => ({}), ...extra });
      const [row] = rows(await page(client));
      await quick(client, row!, 'merge');
      expect(calls.some(call => call.method === 'pullRequests.runAction')).toBe(false);
      expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not merge this pull request', description: sentence });
    }
  });
  test('a refused action says the host\'s sentence and leaves the row as it was; the row stays visible while it runs', async () => {
    let release: (value: unknown) => void = () => {};
    const { client } = fakeClient({ ...listOf(entry(10, { isDraft: true })), 'pullRequests.runAction': () => new Promise((_, reject) => { release = () => reject(new Error('Pull request operation runAction failed: Resource not accessible by integration')); }) });
    const [row] = rows(await page(client));
    const running = quick(client, row!, 'ready');
    await new Promise(resolve => setTimeout(resolve, 5));
    const [pending] = rows(await page(client));
    expect(pending).toMatchObject({ speedPending: true });
    expect(pending!.speed.every(action => action.disabled)).toBe(true);
    release(null);
    await running;
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not mark this ready for review', description: 'Resource not accessible by integration' });
    expect(rows(await page(client))[0]).toMatchObject({ number: 10, state: 'draft', speedPending: false });
  });
});

describe('the row popovers (PullRequestChecksPopover, PullRequestStackPopover)', () => {
  test('the checks popover says its headline, shows "Loading checks…" first, then the checks needing attention with "Show all"', async () => {
    const { client, calls } = fakeClient({ ...listOf(entry(11)), 'pullRequests.detail': () => detail(11), 'pullRequests.checks': () => ({ checks: detail(11).checks }) });
    const [row] = rows(await page(client));
    expect(row).toMatchObject({ checks: 'failing', checksLabel: 'Checks: Some checks were not successful', checksHeadline: 'Some checks were not successful' });
    expect(calls.some(call => call.method === 'pullRequests.detail')).toBe(false); // nothing is read for a row at rest
    prQuickLocal(client, 'checks', row!.ref);
    const input = { open: true, refresh: 0, now: Date.parse('2026-10-08T01:00:00.000Z'), selected: '', query: '', typed: false };
    const first = await pullRequestsPage(client, native, input);
    expect(first.checksPopover).toMatchObject({ ref: row!.ref, headline: 'Some checks were not successful', loading: true });
    const view = await page(client);
    expect(view.checksPopover).toMatchObject({ loading: false, collapsible: true, expanded: false });
    expect(view.checksPopover.rows.map(check => [check.name, check.label])).toEqual([['ci/test', 'Failed']]);
    prQuickLocal(client, 'checks-all', row!.ref);
    expect((await page(client)).checksPopover.rows.map(check => check.name)).toEqual(['ci/test', 'ci/build']);
  });
  test('the stack popover names the stack, reads it when opened, lists the layers top-down with this one checked and "↳ base"', async () => {
    const stack = { id: 's', number: 120, url: '', base: 'main', layers: [{ number: 118, headBranch: 'feature/greeting-options', state: 'open', title: 'Let greet take options' },
      { number: 119, headBranch: 'docs/greeting-options', state: 'open', title: 'Document the greeting options', isDraft: true }] };
    const { client, calls } = fakeClient({ ...listOf(entry(118, { stack: { number: 120, position: 1, size: 2, base: 'main' } })), 'pullRequests.stack': () => stack });
    const [row] = rows(await page(client));
    expect(row).toMatchObject({ stackLabel: '1/2', stackAria: 'Stack 120, layer 1 of 2', stackTip: 'View stack #120, layer 1 of 2' });
    prQuickLocal(client, 'stack', row!.ref);
    const input = { open: true, refresh: 0, now: 0, selected: '', query: '', typed: false };
    expect((await pullRequestsPage(client, native, input)).stackPopover).toMatchObject({ label: 'Stack #120', message: 'Loading stack…' });
    const view = await page(client);
    expect(calls.filter(call => call.method === 'pullRequests.stack').map(call => call.payload)).toEqual([{ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 118 }]);
    expect(view.stackPopover).toMatchObject({ label: 'Stack #120', base: '↳ main', message: '' });
    expect(view.stackPopover.layers.map(layer => [layer.number, layer.detail, layer.current])).toEqual([[119, '#119 · docs/greeting-options · Draft', false], [118, '#118 · feature/greeting-options · Open', true]]);
  });
  test('a stack that is gone says so; a failed read says the error, and a held stack offers Retry', async () => {
    const gone = fakeClient({ ...listOf(entry(118, { stack: { number: 120, position: 1, size: 2, base: 'main' } })), 'pullRequests.stack': () => null });
    const [row] = rows(await page(gone.client));
    prQuickLocal(gone.client, 'stack', row!.ref);
    expect((await page(gone.client)).stackPopover.message).toBe('This pull request is no longer in a stack.');
    const failed = fakeClient({ ...listOf(entry(118, { stack: { number: 120, position: 1, size: 2, base: 'main' } })), 'pullRequests.stack': () => { throw new Error('GitHub CLI command failed.'); } });
    const [failing] = rows(await page(failed.client));
    prQuickLocal(failed.client, 'stack', failing!.ref);
    expect((await page(failed.client)).stackPopover).toMatchObject({ message: 'GitHub CLI command failed.', retry: false });
  });
});
