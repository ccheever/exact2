// The Pull Requests page across servers and pages (pr-links-previews-and-routing): the list asks every connected
// server that reads pull requests (each repository by one of them), folds their answers, and pages by each server's
// `nextCursors` (_chat.pull-requests.tsx listTargets / loadMore / refreshList), driven through pullRequestsPage with
// injected host replies.
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { prLocal, pullRequestsPage, type PrInput } from './pages-prs';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { initialShell } from './domain';

const NOW = Date.parse('2026-10-08T12:00:00Z');
const row = (number: number, over: Obj = {}): Obj => ({ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number, title: `Change ${number}`, state: 'open', isDraft: false,
  author: { login: 'lane-primary' }, updatedAt: new Date(NOW - number * 60_000).toISOString(), additions: 1, deletions: 0, ...over });
const rows = (from: number, count: number, over: Obj = {}) => Array.from({ length: count }, (_, index) => row(from + index, over));
const CURSOR_KEY = 'github.com lane/sandbox';

function fixture(answer: (payload: Obj) => Obj) {
  const lists: Obj[] = [];
  const client = {
    environmentId: 'env', ready: true, generation: 1, revision: 0, local: {} as Obj,
    config: { environment: { label: 'Lane A', capabilities: { pullRequests: true } } },
    shell: { projects: [{ id: 'p1', title: 'sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox' } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj) => {
      if (method === 'pullRequests.list') { lists.push(payload); return answer(payload); }
      if (method === 'pullRequests.listStats') return { stats: [] };
      if (method === 'pullRequests.invalidate') return {};
      throw new Error(`no reply for ${method}`);
    },
    restAccess: () => ({ call: async () => ({ id: '1-1' }) }),
  } as unknown as T3Client;
  const native = { available: true, watch: () => {}, later: async () => ({ ok: true, generation: 1, value: {} }) } as unknown as Native;
  const page = (input: Partial<PrInput> = {}) => pullRequestsPage(client, native, { open: true, refresh: 0, now: NOW, selected: '', query: '', typed: false, ...input });
  return { client, lists, page };
}
const numbers = (view: Awaited<ReturnType<ReturnType<typeof fixture>['page']>>) => view.groups.flatMap(group => group.rows.map(entry => entry.number));

describe('list paging ("Load more pull requests", the per-repository cursors)', () => {
  test('a further page carries on from the cursors the last answer gave, and its rows land under the ones shown', async () => {
    const { client, lists, page } = fixture(payload => payload.cursors
      ? { entries: rows(100, 6), viewers: { 'github.com': 'lane-primary' }, providers: [], errors: [], truncated: false, nextCursors: {} }
      : { entries: rows(1, 99), viewers: { 'github.com': 'lane-primary' }, providers: [], errors: [], truncated: true, nextCursors: { [CURSOR_KEY]: 'Y3Vyc29yOjk5' } });
    const first = await page();
    expect(lists[0]).toMatchObject({ limit: 99, state: 'open' });
    expect(lists[0]!.cursors).toBeUndefined();
    expect(numbers(first)).toHaveLength(99);
    expect(first).toMatchObject({ footer: 'more', footerText: 'Load more pull requests', loadDisabled: false });
    prLocal(client, 'load-more', '');
    const loading = await page(); // the footer's spinner first, the rows unchanged
    expect(loading).toMatchObject({ footer: 'loading', footerText: 'Loading more' });
    expect(numbers(loading)).toHaveLength(99);
    const grown = await page();
    expect(lists[1]).toMatchObject({ limit: 99, cursors: { [CURSOR_KEY]: 'Y3Vyc29yOjk5' } });
    expect(numbers(grown).slice(0, 99)).toEqual(numbers(first));
    expect(numbers(grown)).toHaveLength(105);
    expect(numbers(grown).slice(99)).toEqual([100, 101, 102, 103, 104, 105]);
    expect(grown.footer).toBe('');
  });
  test('with no cursor to carry on from the page grows, and at 500 the reader is asked to narrow the search', async () => {
    const { client, lists, page } = fixture(payload => ({ entries: rows(1, Number(payload.limit)), viewers: {}, providers: [], errors: [], truncated: true, nextCursors: {} }));
    await page();
    for (const size of [198, 297, 396, 495, 500]) {
      prLocal(client, 'load-more', '');
      expect((await page()).footerText).toBe('Updating pull requests');
      await page();
      expect(lists.at(-1)).toMatchObject({ limit: size });
      expect(lists.at(-1)!.cursors).toBeUndefined();
    }
    expect(await page()).toMatchObject({ footer: 'narrow', footerText: 'Narrow your search to find more pull requests.' });
  });
  test('a page that fails keeps the rows shown and says so beside them', async () => {
    let fail = false;
    const { client, page } = fixture(payload => {
      if (payload.cursors && fail) throw new Error('GitHub rate limit exceeded.');
      return { entries: rows(1, 99), viewers: {}, providers: [], errors: [], truncated: true, nextCursors: { [CURSOR_KEY]: 'next' } };
    });
    await page();
    fail = true;
    prLocal(client, 'load-more', '');
    await page();
    const failed = await page();
    expect(numbers(failed)).toHaveLength(99);
    expect(failed.listError).toBe('GitHub rate limit exceeded. Showing the last pull requests loaded.');
  });
  test('a refresh after a continuation reads the whole visible list again, one page long enough for every row', async () => {
    const { client, lists, page } = fixture(payload => payload.cursors
      ? { entries: rows(100, 50), viewers: {}, providers: [], errors: [], truncated: true, nextCursors: { [CURSOR_KEY]: 'again' } }
      : { entries: rows(1, Number(payload.limit) === 99 ? 99 : 149), viewers: {}, providers: [], errors: [], truncated: true, nextCursors: { [CURSOR_KEY]: 'next' } });
    await page();
    prLocal(client, 'load-more', '');
    await page(); await page();
    await page({ refresh: 1 });
    await page({ refresh: 1 });
    expect(lists.at(-1)).toMatchObject({ limit: 198 });
    expect(lists.at(-1)!.cursors).toBeUndefined();
  });
});

describe('merged listings from several servers', () => {
  afterEach(() => { fleet.entries.delete('http://127.0.0.1:16361\nenv-b'); });
  test('a background server lists only the repositories the focused one does not hold, and its rows name it', async () => {
    const entry: FleetEntry = { key: 'http://127.0.0.1:16361\nenv-b', origin: 'http://127.0.0.1:16361', environmentId: 'env-b', phase: 'connected', message: '', traceId: '', generation: 3, synchronized: 3,
      lastEvent: 0, subscriptions: {}, config: { environment: { label: 'Lane B', capabilities: { pullRequests: true } } }, scopes: [], error: '', requested: true,
      shell: { ...initialShell(), projects: [
        { id: 'b1', title: 'sandbox', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/sandbox' } },
        { id: 'b2', title: 'notes', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/lane/notes' } },
      ] } };
    fleet.entries.set(entry.key, entry);
    const { lists, page } = fixture(payload => payload.environmentId === 'env-b'
      ? { entries: [row(7, { projectId: 'b2', repository: 'lane/notes', author: { login: 'lane-primary' } })], viewers: { 'github.com': 'lane-primary' }, providers: [], errors: [], truncated: false, nextCursors: {} }
      : { entries: [row(3)], viewers: { 'github.com': 'lane-primary' }, providers: [], errors: [], truncated: false, nextCursors: {} });
    const view = await page();
    expect(lists).toEqual([expect.not.objectContaining({ environmentId: expect.anything() }), expect.objectContaining({ environmentId: 'env-b', projectIds: ['b2'] })]);
    const listed = view.groups.flatMap(group => group.rows);
    expect(listed.map(entry => [entry.number, entry.repository])).toEqual([[3, 'lane/sandbox'], [7, 'lane/notes']]);
    expect(JSON.parse(listed[1]!.ref)).toEqual({ projectId: 'b2', host: 'github.com', repository: 'lane/notes', number: 7, environmentId: 'env-b' });
    expect(JSON.parse(listed[0]!.ref).environmentId).toBeUndefined();
    expect(view.groups.map(group => group.label)).toEqual(['Authored']);
  });
});
