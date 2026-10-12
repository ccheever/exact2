// The Pull Requests list's useLiveRefresh (apps/web/src/routes/_chat.pull-requests.tsx: `useLiveRefresh(() =>
// refreshList(true), { enabled })`; apps/web/src/hooks/useLiveRefresh.ts): the list reads again when the reader
// comes back to the page or the window, and every five minutes while somebody reads it; never twice within 10 s,
// never while the window is hidden, and not once the reader has been idle for six minutes. Each read goes through
// the server's cache (no `pullRequests.invalidate`).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pullRequestsPage, type PrInput } from './pages-prs';
import { prCommand, pullRequestDetail } from './pages-pr-detail';
import { PR_REFRESH_KEY, prRefreshEvent } from './pages-pr-refresh';
import { noteNow } from './composer-controls';

const NOW = Date.parse('2026-10-08T12:00:00Z');
const MINUTE = 60_000;
const entries = [{ projectId: 'p1', host: 'github.com', repository: 'acme/playground', number: 158, title: 'Count the vowels in a line', state: 'open', isDraft: false,
  author: { login: 'second' }, updatedAt: '2026-10-08T11:00:00Z', additions: 1, deletions: 0 }];

/** A client whose reader last pointed or typed at `interactedAt` (the activity reporter's answer). */
function fixture(interactedAt = NOW, more: Record<string, (payload: Obj) => unknown> = {}) {
  const calls: string[] = [];
  let listed: Obj[] = entries;
  const client = {
    environmentId: 'env', ready: true, generation: 1, revision: 0, local: {} as Obj,
    config: { environment: { capabilities: { pullRequests: true } } }, shell: { projects: [{ id: 'p1', title: 'playground', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/playground' } }], threads: [] },
    rpc: async (_native: unknown, method: string, payload: Obj) => {
      calls.push(method);
      if (more[method]) return more[method]!(payload);
      if (method === 'pullRequests.list') return { entries: listed, viewers: {} };
      if (method === 'pullRequests.listStats') return { stats: [] };
      throw new Error(`no reply for ${method}`);
    },
    restAccess: () => ({ call: async () => ({ id: '1-1' }) }),
  } as unknown as T3Client;
  const native = { available: true, watch: () => {}, later: async (request: Obj) => request.op === 'activityLastInteraction' ? { ok: true, generation: 1, value: { at: interactedAt } } : { ok: true, generation: 1, value: {} } } as unknown as Native;
  const page = (input: Partial<PrInput> & { visible?: boolean; returns?: number } = {}) =>
    pullRequestsPage(client, native, { open: true, refresh: 0, now: NOW, selected: '', query: '', typed: false, ...input });
  const reads = () => calls.filter(method => method === 'pullRequests.list').length;
  return { client, native, calls, page, reads, list: (next: Obj[]) => { listed = next; } };
}

describe('the Pull Requests list reads again as the reference page does (useLiveRefresh)', () => {
  test('focusing the window again reads the list once; not again within 10 s; the first opening reads only its own read', async () => {
    const { calls, page, reads } = fixture();
    await page();
    expect(reads()).toBe(1); // the page's own first read; arriving at it costs nothing more
    await page({ now: NOW + 5_000 });
    expect(reads()).toBe(1);
    await page({ now: NOW + 30_000, returns: 1 }); // the window's `focus` (app.contract windowReturned: the clock read then)
    expect(reads()).toBe(2);
    await page({ now: NOW + 35_000, returns: 2 }); // 5 s after the last read: alt-tabbing is not a read per stop
    expect(reads()).toBe(2);
    await page({ now: NOW + 2 * MINUTE, visible: false });
    await page({ now: NOW + 2 * MINUTE, visible: true }); // `visibilitychange` to visible
    expect(reads()).toBe(3);
    expect(calls).not.toContain('pullRequests.invalidate'); // through the server's cache
  });

  test('coming back to the page reads the list again', async () => {
    const { page, reads } = fixture();
    await page();
    await page({ now: NOW + MINUTE, open: false }); // another page
    await page({ now: NOW + MINUTE });
    expect(reads()).toBe(2);
  });

  test('a 5-minute tick reads the list once', async () => {
    const { page, reads } = fixture(NOW + 4 * MINUTE);
    await page();
    for (const minutes of [1, 2, 3, 4]) await page({ now: NOW + minutes * MINUTE }); // the clock's minute ticks
    expect(reads()).toBe(1);
    await page({ now: NOW + 5 * MINUTE });
    expect(reads()).toBe(2);
    await page({ now: NOW + 5 * MINUTE });
    expect(reads()).toBe(2);
  });

  test('no reads while the reader has been idle past six minutes, or while the window is hidden', async () => {
    const { page, reads } = fixture(NOW); // the reader's last pointer or key: NOW
    await page();
    await page({ now: NOW + 5 * MINUTE });
    expect(reads()).toBe(2); // five minutes in, the reader was here within six
    await page({ now: NOW + 10 * MINUTE });
    expect(reads()).toBe(2); // ten minutes untouched: the interval stops
    await page({ now: NOW + 11 * MINUTE, returns: 1 });
    expect(reads()).toBe(2); // an arrival while idle reads nothing either
    const hidden = fixture(NOW + 5 * MINUTE);
    await hidden.page();
    await hidden.page({ now: NOW + 5 * MINUTE, visible: false });
    expect(hidden.reads()).toBe(1); // a hidden window costs the host nothing
  });

  test('close #158, the list read without it, reopen, an answer that has not seen the reopen yet, then the window focused again: the row is back', async () => {
    // The live check of 2026-10-08 (pr-writing-and-metadata): the server lists through GitHub search, so the read
    // right after the reopen can still lack the row; nothing read the list again. The focus read (through the
    // server's cache, its 30 s gone) brings it back, the reopen's note (notedListEntry, #261) agreeing with it.
    let state = 'open';
    const selected = JSON.stringify({ projectId: 'p1', host: 'github.com', repository: 'acme/playground', number: 158 });
    const detail = () => ({ provider: 'github', projectId: 'p1', repository: 'acme/playground', number: 158, title: 'Count the vowels in a line', state, isDraft: false, author: { login: 'second' },
      viewer: 'primary', checks: [], labels: [], reviewers: [], updatedAt: '2026-10-08T11:00:00Z', capabilities: { actions: ['close', 'reopen'], comment: true }, viewerPermissions: { actions: ['close', 'reopen'], comment: true } });
    const run = fixture(NOW, { 'pullRequests.detail': detail, 'pullRequests.activity': () => ({ comments: [], commits: [], reviewers: [], commentCount: 0 }), 'pullRequests.stack': () => null,
      'pullRequests.runAction': (payload) => { state = payload.action === 'close' ? 'closed' : 'open'; return {}; } });
    noteNow(run.client, NOW);
    const rows = async (input: Partial<PrInput> & { visible?: boolean; returns?: number } = {}) => (await run.page({ selected, ...input })).groups.flatMap(group => group.rows.map(row => `${row.number}:${row.state}`));
    let announced = 0;
    const announce = () => prRefreshEvent(run.client, { key: PR_REFRESH_KEY, subscriptionId: '1-1', value: ++announced }); // the server's word after each write
    expect(await rows()).toEqual(['158:open']);
    await pullRequestDetail(run.client, run.native, { selected, refresh: 0, now: NOW });
    expect(await prCommand(run.client, run.native, 'action', selected, 'close')).toBe('');
    run.list([]); announce();
    expect(await rows({ now: NOW + 10_000 })).toEqual([]); // read again without it
    expect(await prCommand(run.client, run.native, 'action', selected, 'reopen')).toBe('');
    announce();
    expect(await rows({ now: NOW + 20_000 })).toEqual([]); // the search has not seen the reopen yet
    run.list(entries);
    expect(await rows({ now: NOW + 50_000 })).toEqual([]); // nothing asks the list again on its own
    expect(await rows({ now: NOW + 50_000, returns: 1 })).toEqual(['158:open']); // the window's focus reads it, and it is back (inside the close note's minute)
  });

  test('a run that asks while a read is out joins it: one read, and its answer is what shows (the five ticks of the live check)', async () => {
    // Live check, 2026-10-08: `clock +300000` ran the page five times; the interval's read answered with #158 back,
    // but a later tick's run had already answered from the old list. Attempt 3: a refocus read the list twice.
    const later = [...entries, { ...entries[0]!, number: 159, title: 'Explain the vowel rule' }];
    let answers = 0, release: () => void = () => {};
    const run = fixture(NOW + 4 * MINUTE, { 'pullRequests.list': () => { answers++; if (answers === 1) return { entries, viewers: {} }; if (answers === 2) return new Promise(resolve => { release = () => resolve({ entries: later, viewers: {} }); }); return { entries: later, viewers: {} }; } });
    const numbers = (view: Awaited<ReturnType<typeof run.page>>) => view.groups.flatMap(group => group.rows.map(row => row.number)).sort();
    expect(numbers(await run.page())).toEqual([158]);
    const first = run.page({ now: NOW + 5 * MINUTE }); // the interval's read goes out
    await Bun.sleep(1);
    const second = run.page({ now: NOW + 6 * MINUTE }); // the next tick's run asks while it is out
    const third = run.page({ now: NOW + 6 * MINUTE, returns: 1 }); // and a focus
    await Bun.sleep(1);
    release();
    expect([numbers(await first), numbers(await second), numbers(await third)]).toEqual([[158, 159], [158, 159], [158, 159]]);
    expect(run.reads()).toBe(2); // the first read and the interval's; the tick and the focus joined it
    expect(numbers(await run.page({ now: NOW + 6 * MINUTE }))).toEqual([158, 159]);
    expect(run.reads()).toBe(2);
  });

  test('another answer does not await a read the runner let go with its answer: it reads for itself, and its rows show (a search typed after the first)', async () => {
    // pr-links-previews-and-routing retry, 2026-10-08: a data revision asked the list again while a search's read was
    // out; the runner let the first answer go and rejected its read, and the new answer, which had joined that read,
    // failed and kept the old rows for good. Each answer gets its own `native` (app.ts letGoAware).
    const later = [...entries, { ...entries[0]!, number: 159, title: 'Explain the vowel rule' }];
    let answers = 0, drop: () => void = () => {};
    const run = fixture(NOW, { 'pullRequests.list': () => { answers++; if (answers === 2) return new Promise((_resolve, reject) => { drop = () => reject(new ClientError('This operation was superseded.', 'superseded')); }); return { entries: answers === 1 ? entries : later, viewers: {} }; } });
    const numbers = (view: Awaited<ReturnType<typeof run.page>>) => view.groups.flatMap(group => group.rows.map(row => row.number)).sort();
    expect(numbers(await run.page())).toEqual([158]);
    const first = run.page({ now: NOW + 30_000, returns: 1 }); // a read goes out
    await Bun.sleep(1);
    const second = pullRequestsPage(run.client, { ...run.native } as Native, { open: true, refresh: 0, now: NOW + 30_000, selected: '', query: '', typed: false, returns: 1 }); // the resource asked again: a new answer
    await Bun.sleep(1);
    drop(); // the runner lets the first answer go, and its read with it
    await expect(first).rejects.toThrow('superseded');
    expect(numbers(await second)).toEqual([158, 159]);
    expect(run.reads()).toBe(3);
  });

  test('a server announcement while a live read is out joins it', async () => {
    let answers = 0, release: () => void = () => {};
    const run = fixture(NOW, { 'pullRequests.list': () => { answers++; if (answers === 2) return new Promise(resolve => { release = () => resolve({ entries, viewers: {} }); }); return { entries, viewers: {} }; } });
    await run.page();
    const focused = run.page({ now: NOW + 30_000, returns: 1 }); // the window's focus: a read goes out
    await Bun.sleep(1);
    prRefreshEvent(run.client, { key: PR_REFRESH_KEY, subscriptionId: '1-1', value: 1 }); // the server's word arrives meanwhile
    const announced = run.page({ now: NOW + 30_000, returns: 1 });
    await Bun.sleep(1);
    release(); await focused; await announced;
    expect(run.reads()).toBe(2);
  });

  test('a re-read that fails keeps the rows on screen (the reference\'s `answered ?? carried`), so the list keeps its place', async () => {
    let fail = false;
    const run = fixture(NOW, { 'pullRequests.list': () => { if (fail) throw new Error('GitHub CLI command failed: HTTP 502'); return { entries, viewers: {} }; } });
    expect((await run.page()).groups.flatMap(group => group.rows.map(row => row.number))).toEqual([158]);
    fail = true;
    const view = await run.page({ now: NOW + 30_000, returns: 1 });
    expect([view.groups.flatMap(group => group.rows.map(row => row.number)), view.empty]).toEqual([[158], '']);
  });
});
