// The detail panel's links between pull requests and threads, and its link previews (pr-links-previews-and-routing),
// driven through pages-pr-detail.ts and pages-pr-links.ts against injected host replies: the linked-thread count and
// its 10 s re-read, the Link / Unlink item and the picker, the palette pages, the autolinked text and a link's click.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { toasts } from './toast';
import { prCommand, pullRequestDetail } from './pages-pr-detail';
import { linkedThreadItems, openLink, readPreview, threadPickerView } from './pages-pr-links';
import { paletteView } from './palette-view';

type Reply = (payload: Obj) => unknown;
const URL7 = 'https://github.com/lane/sandbox/pull/7';
const detail = (over: Obj = {}): Obj => ({
  provider: 'github', projectId: 'p1', repository: 'lane/sandbox', number: 7, title: 'Add a changelog', url: URL7, state: 'open', isDraft: false, mergeability: 'mergeable',
  baseComparison: 'up-to-date', changedFiles: 1, additions: 3, deletions: 0, headBranch: 'feature/changelog', baseBranch: 'main', author: { login: 'lane-primary', name: 'Lane Primary' },
  checks: [], labels: [], capabilities: { actions: [] }, viewerPermissions: { actions: [] }, createdAt: '2026-10-08T08:00:00Z', updatedAt: '2026-10-08T10:00:00Z',
  body: 'Follows #101 and 0123456789abcdef0123456789abcdef01234567, not `#2` and not a#3. See https://github.com/lane/sandbox/pull/9 too.', ...over,
});
const sandbox = { id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox', displayName: 'lane/sandbox', owner: 'lane', name: 'sandbox' } };

function fakeClient(replies: Record<string, Reply>, over: Obj = {}) {
  const calls: { method: string; payload: Obj; write: boolean }[] = [], dispatched: Obj[] = [];
  const answer = async (method: string, payload: Obj, write = false) => {
    calls.push({ method, payload, write });
    const reply = replies[method];
    if (!reply) throw new Error(`no reply for ${method}`);
    return reply(payload) as Obj;
  };
  const client = {
    environmentId: 'env', origin: 'http://127.0.0.1:16360', connection: 'connected', statusMessage: '', scopes: [], threadId: '', projectId: 'p1', ready: true, revision: 0, generation: 1, error: '',
    get draftKey() { return `env:${this.threadId || `new:${this.projectId}`}`; },
    local: { drafts: {} as Record<string, string>, composerControls: { contexts: {} } },
    config: { environment: { label: 'Lane A', capabilities: { pullRequests: true, threadPullRequests: true } }, settings: {} },
    shell: { projects: [sandbox], threads: [
      { id: 't1', projectId: 'p1', title: 'Write the changelog', updatedAt: '2026-10-08T09:00:00Z', archivedAt: null, pullRequests: [] },
      { id: 't2', projectId: 'p1', title: 'Release notes', updatedAt: '2026-10-08T09:30:00Z', archivedAt: null, pullRequests: [{ host: 'github.com', repository: 'lane/sandbox', number: 7, url: URL7, source: 'manual' }] },
      { id: 't3', projectId: 'p1', title: 'Old work', updatedAt: '2026-10-01T09:30:00Z', archivedAt: '2026-10-02T00:00:00Z', pullRequests: [] },
    ] as Obj[] },
    rpc: (_native: unknown, method: string, payload: Obj, write = false) => answer(method, payload, write),
    request: (_native: unknown, method: string, payload: Obj, _generation: number, write = false) => answer(method, payload, write),
    call: (_native: unknown, request: Obj, _generation: number, write = false) => answer(String(request.method), request.payload as Obj, write),
    async dispatch(_native: unknown, _storage: unknown, payload: Obj) { dispatched.push(payload); if (replies['dispatch']) await replies['dispatch'](payload); return {}; },
    restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `command-${index + 1}`) }),
    projectGroups: () => [{ key: 'p1', name: 'sandbox', members: [sandbox] }],
    ...over,
  } as unknown as T3Client;
  return { client, calls, dispatched };
}
const relations = { threads: [{ id: 't2', projectId: 'p1', title: 'Release notes', archivedAt: null }, { id: 't3', projectId: 'p1', title: 'Old work', archivedAt: '2026-10-02T00:00:00Z' }] };
const defaults = (extra: Record<string, Reply> = {}): Record<string, Reply> => ({
  'pullRequests.detail': () => detail(), 'pullRequests.activity': () => ({ reviewers: [], commits: [], comments: [], reviewThreads: [] }), 'pullRequests.stack': () => null,
  'pullRequests.linkedThreads': () => relations, ...extra,
});
const gestures: Obj[] = [];
const opened: string[] = [];
const native = { available: true, watch: () => {}, later: async (request: Obj) => {
  if (request.op === 'composerSendIntent') return { ok: true, generation: 1, value: gestures.shift() ?? {} };
  if (request.op === 'remoteEditorsOpen') { opened.push(String(request.url)); return { ok: true, generation: 1, value: {} }; }
  return { ok: true, generation: 1, value: {} };
} } as unknown as Native;
const storage = {} as Files;
const selected = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 });
async function open(client: T3Client, input: Obj = {}) {
  let view = await pullRequestDetail(client, native, { selected, refresh: 1, now: Date.parse('2026-10-08T12:00:00Z'), ...input });
  for (let asked = 0; asked < 6; asked++) view = await pullRequestDetail(client, native, { selected, refresh: 1, now: Date.parse('2026-10-08T12:00:00Z'), ...input });
  return view;
}

describe('the linked threads (PullRequestThreadLinks display="count")', () => {
  test('on the page the count names the threads and reads again only when the 10 s tick moves', async () => {
    const { client, calls } = fakeClient(defaults());
    const view = await open(client, { page: true, linkTick: 1 });
    expect(view.links).toMatchObject({ countShown: true, countText: '2', countLabel: 'Linked from 2 threads', countTip: 'Linked from 2 threads. Search in the command palette.', polling: true });
    const reads = () => calls.filter(call => call.method === 'pullRequests.linkedThreads');
    expect(reads()).toHaveLength(1);
    expect(reads()[0]!.payload).toEqual({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 });
    await open(client, { page: true, linkTick: 1 });
    expect(reads()).toHaveLength(1);
    await open(client, { page: true, linkTick: 2 });
    expect(reads()).toHaveLength(2);
  });
  test('a failed read keeps the count it had, and with nothing read shows "?"', async () => {
    let fail = false;
    const { client } = fakeClient(defaults({ 'pullRequests.linkedThreads': () => { if (fail) throw new Error('offline'); return relations; } }));
    expect((await open(client, { page: true, linkTick: 1 })).links.countText).toBe('2');
    fail = true;
    expect((await open(client, { page: true, linkTick: 2 })).links).toMatchObject({ countShown: true, countText: '2' });
    const { client: other } = fakeClient(defaults({ 'pullRequests.linkedThreads': () => { throw new Error('offline'); } }));
    expect((await open(other, { page: true, linkTick: 1 })).links).toMatchObject({ countShown: true, countText: '?', countLabel: 'Linked threads' });
  });
  test('beside a thread there is no count and nothing is polled', async () => {
    const { client, calls } = fakeClient(defaults(), { threadId: 't1' });
    const view = await open(client, { page: false, linkTick: 1 });
    expect(view.links).toMatchObject({ countShown: false, polling: false });
    expect(calls.some(call => call.method === 'pullRequests.linkedThreads')).toBe(false);
  });
  test('the count opens the palette on the URL, whose thread results are the linked threads, archived ones too', async () => {
    const { client } = fakeClient(defaults());
    await open(client, { page: true, linkTick: 1 });
    expect(await prCommand(client, native, 'links', selected, 'linked', storage)).toBe(`palette:pr-linked|${URL7}`);
    const items = linkedThreadItems(client, `pr-linked|${URL7}`, URL7)!;
    expect(items.map(item => [item.row.title, item.row.description, item.row.op, item.row.arg])).toEqual([['Release notes', 'Linked thread', 'thread', 't2'], ['Old work', 'Archived thread', 'thread', 't3']]);
    expect(linkedThreadItems(client, `pr-linked|${URL7}`, `${URL7} x`)).toBeNull();
    const view = await paletteView(client, native, [true, 'command', `pr-linked|${URL7}`, URL7, '', false, 0, 'light']);
    expect(view.rows.filter(entry => entry.op === 'thread').map(entry => entry.title)).toEqual(['Release notes', 'Old work']);
    expect(view.back).toBe(false);
  });
});

describe('Link to thread (PullRequestThreadLinks display="menu-item" and the picker)', () => {
  test('on the page there is no thread: "Link to thread" opens the picker, which marks a linked thread and leaves archived ones out', async () => {
    const { client } = fakeClient(defaults());
    const view = await open(client, { page: true, linkTick: 1 });
    expect(view.links).toMatchObject({ menuShown: true, menuLabel: 'Link to thread', menuIcon: 'link-2', menuPicker: true });
    expect(await prCommand(client, native, 'links', selected, 'picker', storage)).toBe('palette:pr-link-thread');
    const picker = threadPickerView(client, '', false);
    expect(picker).toMatchObject({ label: 'Choose a thread', placeholder: 'Search threads or projects...', empty: '' });
    expect(picker.rows.map(entry => [entry.title, entry.description, entry.kind, entry.trailing])).toEqual([['Release notes', 'sandbox', 'disabled', 'Linked'], ['Write the changelog', 'sandbox', 'action', '']]);
    expect(threadPickerView(client, 'nothing like it', false).empty).toBe('No active threads found.');
  });
  test('the picker\'s choice links the pull request to that thread, and the count is read again', async () => {
    const { client, dispatched, calls } = fakeClient(defaults());
    await open(client, { page: true, linkTick: 1 });
    const view = await paletteView(client, native, [true, 'command', 'pr-link-thread', 'changelog', '', false, 0, 'light']);
    expect(view.rows.map(entry => [entry.op, entry.arg, entry.arg2])).toEqual([['flow', 'pr-link-thread', 't1']]);
    const { paletteCommand } = await import('./palette-commands');
    expect(await paletteCommand(client, native, storage, 'pr-link-thread', 't1', 'pr-link-thread')).toMatchObject({ close: true });
    expect(dispatched).toEqual([{ type: 'thread.pull-request.link', commandId: 'command-1', threadId: 't1', host: 'github.com', repository: 'lane/sandbox', number: 7, url: URL7, source: 'manual' }]);
    await open(client, { page: true, linkTick: 1 });
    expect(calls.filter(call => call.method === 'pullRequests.linkedThreads')).toHaveLength(2);
  });
  test('beside a linked thread the item unlinks it; beside another it links to it', async () => {
    const linked = fakeClient(defaults(), { threadId: 't2' });
    expect((await open(linked.client)).links).toMatchObject({ menuLabel: 'Unlink from this thread', menuIcon: 'unlink-2', menuPicker: false });
    expect(await prCommand(linked.client, native, 'links', selected, 'toggle', storage)).toBe('');
    expect(linked.dispatched).toEqual([{ type: 'thread.pull-request.unlink', commandId: 'command-1', threadId: 't2', host: 'github.com', repository: 'lane/sandbox', number: 7 }]);
    const other = fakeClient(defaults(), { threadId: 't1' });
    expect((await open(other.client)).links).toMatchObject({ menuLabel: 'Link to this thread', menuPicker: false });
    await prCommand(other.client, native, 'links', selected, 'toggle', storage);
    expect(other.dispatched[0]).toMatchObject({ type: 'thread.pull-request.link', threadId: 't1' });
  });
  test('an unsent draft has no thread yet, so its item opens the picker (c8d7d50a73)', async () => {
    const { client } = fakeClient(defaults(), { threadId: '' });
    expect((await open(client, { page: false })).links).toMatchObject({ menuLabel: 'Link to thread', menuPicker: true });
    expect(await prCommand(client, native, 'links', selected, 'toggle', storage)).toBe('palette:pr-link-thread');
  });
  test('a refused dispatch is a toast, not the transcript banner', async () => {
    const { client } = fakeClient(defaults({ dispatch: () => { throw new ClientError('Thread is archived.'); } }), { threadId: 't1' });
    await open(client);
    await prCommand(client, native, 'links', selected, 'toggle', storage);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not link the pull request', description: 'Thread is archived.' });
    expect(client.error).toBe('');
  });
  test('the back arrow shows beside a thread with more than one pull request', async () => {
    const { client } = fakeClient(defaults(), { threadId: 't2' });
    (client.shell.threads[1] as Obj).pullRequests = [...((client.shell.threads[1] as Obj).pullRequests as Obj[]), { host: 'github.com', repository: 'lane/sandbox', number: 8, url: 'https://github.com/lane/sandbox/pull/8', source: 'manual' }];
    expect((await open(client)).links.backShown).toBe(true);
    const page = fakeClient(defaults());
    expect((await open(page.client, { page: true })).links.backShown).toBe(false);
  });
});

describe('pull request text (remarkPullRequestAutolinks, PullRequestLinkPreview)', () => {
  test('#N and a commit become links; code and word-attached forms do not; each pull request link has a chip', async () => {
    const { client } = fakeClient(defaults());
    const view = await open(client, { page: true });
    const body = view.bodies[0]!.body;
    expect(body).toContain('[#101](https://github.com/lane/sandbox/issues/101)');
    expect(body).toContain('[0123456](https://github.com/lane/sandbox/commit/0123456789abcdef0123456789abcdef01234567)');
    expect(body).toContain('`#2`');
    expect(body).toContain('a#3');
    expect(view.md.chips.map(chip => [chip.href, chip.detail, chip.target ? JSON.parse(chip.target).number : null])).toEqual([
      ['https://github.com/lane/sandbox/issues/101', 'reference', 101],
      ['https://github.com/lane/sandbox/commit/0123456789abcdef0123456789abcdef01234567', 'commit', null],
      ['https://github.com/lane/sandbox/pull/9', '', 9],
    ]);
  });
  test('the hovered link\'s card reads the detail once, and a failure leaves the URL tooltip', async () => {
    const target = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 9 });
    const { client, calls } = fakeClient(defaults({ 'pullRequests.detail': payload => detail({ number: payload.number, title: 'Fix the parser', url: 'https://github.com/lane/sandbox/pull/9', state: 'merged' }) }));
    const card = await readPreview(client, native, target);
    expect(card).toMatchObject({ phase: 'content', repository: 'lane/sandbox', number: '#9', stateKey: 'merged', stateLabel: 'Merged', title: 'Fix the parser', author: 'Lane Primary (@lane-primary)' });
    expect(card.opened).toMatch(/^opened /);
    await readPreview(client, native, target);
    expect(calls.filter(call => call.method === 'pullRequests.detail')).toHaveLength(1);
    const failing = fakeClient(defaults({ 'pullRequests.detail': () => { throw new Error('404'); } }));
    expect((await readPreview(failing.client, native, target)).phase).toBe('error');
  });
  test('a click opens the pull request in the panel; a #N asks the host first; a modifier keeps the browser', async () => {
    const target = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 101 });
    const { client } = fakeClient(defaults({ 'pullRequests.preview': () => ({ url: 'https://github.com/lane/sandbox/pull/101' }) }));
    const surfaces: string[] = [];
    const surface = async (url: string) => { surfaces.push(url); };
    expect(await openLink(client, native, true, `reference\n${target}\nhttps://github.com/lane/sandbox/issues/101`, surface)).toBe(`pr-select:${target}`);
    expect(await openLink(client, native, false, `\n${target}\nhttps://github.com/lane/sandbox/pull/101`, surface)).toBe('');
    expect(surfaces).toEqual(['https://github.com/lane/sandbox/pull/101']);
    gestures.push({ modifiers: 'meta', source: 'pointer' });
    opened.length = 0;
    expect(await openLink(client, native, true, `\n${target}\nhttps://github.com/lane/sandbox/pull/101`, surface)).toBe('');
    expect(opened).toEqual(['https://github.com/lane/sandbox/pull/101']);
    const issue = fakeClient(defaults({ 'pullRequests.preview': () => { throw new Error('not a pull request'); } }));
    opened.length = 0;
    expect(await openLink(issue.client, native, true, `reference\n${target}\nhttps://github.com/lane/sandbox/issues/101`, surface)).toBe('');
    expect(opened).toEqual(['https://github.com/lane/sandbox/issues/101']);
  });
});
