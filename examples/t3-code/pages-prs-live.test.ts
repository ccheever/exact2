// The Pull Requests list's useLiveRefresh (apps/web/src/routes/_chat.pull-requests.tsx: `useLiveRefresh(() =>
// refreshList(true), { enabled })`; apps/web/src/hooks/useLiveRefresh.ts): the list reads again when the reader
// comes back to the page or the window, and every five minutes while somebody reads it; never twice within 10 s,
// never while the window is hidden, and not once the reader has been idle for six minutes. Each read goes through
// the server's cache (no `pullRequests.invalidate`).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { pullRequestsPage, type PrInput } from './pages-prs';

const NOW = Date.parse('2026-10-08T12:00:00Z');
const MINUTE = 60_000;
const entries = [{ projectId: 'p1', host: 'github.com', repository: 'acme/playground', number: 158, title: 'Count the vowels in a line', state: 'open', isDraft: false,
  author: { login: 'second' }, updatedAt: '2026-10-08T11:00:00Z', additions: 1, deletions: 0 }];

/** A client whose reader last pointed or typed at `interactedAt` (the activity reporter's answer). */
function fixture(interactedAt = NOW) {
  const calls: string[] = [];
  const client = {
    environmentId: 'env', ready: true, generation: 1, revision: 0, local: {} as Obj,
    config: { environment: { capabilities: { pullRequests: true } } }, shell: { projects: [{ id: 'p1', title: 'playground', repositoryIdentity: { provider: 'github', canonicalKey: 'github.com/acme/playground' } }], threads: [] },
    rpc: async (_native: unknown, method: string) => {
      calls.push(method);
      if (method === 'pullRequests.list') return { entries, viewers: {} };
      if (method === 'pullRequests.listStats') return { stats: [] };
      throw new Error(`no reply for ${method}`);
    },
    restAccess: () => ({ call: async () => ({ id: '1-1' }) }),
  } as unknown as T3Client;
  const native = { available: true, watch: () => {}, later: async (request: Obj) => request.op === 'activityLastInteraction' ? { ok: true, generation: 1, value: { at: interactedAt } } : { ok: true, generation: 1, value: {} } } as unknown as Native;
  const page = (input: Partial<PrInput> & { visible?: boolean; focused?: boolean } = {}) =>
    pullRequestsPage(client, native, { open: true, refresh: 0, now: NOW, selected: '', query: '', typed: false, ...input });
  const reads = () => calls.filter(method => method === 'pullRequests.list').length;
  return { calls, page, reads };
}

describe('the Pull Requests list reads again as the reference page does (useLiveRefresh)', () => {
  test('focusing the window again reads the list once; not again within 10 s; the first opening reads only its own read', async () => {
    const { calls, page, reads } = fixture();
    await page();
    expect(reads()).toBe(1); // the page's own first read; arriving at it costs nothing more
    await page({ now: NOW + 5_000 });
    expect(reads()).toBe(1);
    await page({ now: NOW + 30_000, focused: false });
    await page({ now: NOW + 30_000, focused: true }); // the window's `focus`
    expect(reads()).toBe(2);
    await page({ now: NOW + 35_000, focused: false });
    await page({ now: NOW + 35_000, focused: true }); // 5 s after the last read: alt-tabbing is not a read per stop
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
    await page({ now: NOW + 11 * MINUTE, focused: false });
    await page({ now: NOW + 11 * MINUTE, focused: true });
    expect(reads()).toBe(2); // an arrival while idle reads nothing either
    const hidden = fixture(NOW + 5 * MINUTE);
    await hidden.page();
    await hidden.page({ now: NOW + 5 * MINUTE, visible: false });
    expect(hidden.reads()).toBe(1); // a hidden window costs the host nothing
  });
});
