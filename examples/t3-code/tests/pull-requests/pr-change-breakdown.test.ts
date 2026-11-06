// A pull request's +/- split by file kind (PullRequestChangeBreakdownStat, T3 Code 74541bab17): the lines it
// draws, a list row's read on its first hover (loading first), the eager read of a view of one pull request,
// a host that gives no split, an older server's missing docs kind, and when a split is read again.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { pullRequestsPage } from '../../src/pull-requests/pages-prs';
import { breakdownLines, presentChangeBreakdown, readChangeBreakdowns, releaseChangeBreakdowns, requestChangeBreakdown } from '../../src/pull-requests/pr-change-breakdown';
import { prRefreshEvent } from '../../src/pull-requests/pages-pr-refresh';
import { decodeChangeBreakdown } from '../../src/client/domain';
import { prCard } from '../../src/details/pr-row-logic';
import { contractComponent } from '../support/contract-action-fixture';

const NOW = Date.parse('2026-10-08T01:00:00.000Z');
const lines = (additions: number, deletions: number) => ({ additions, deletions });
// The fixture's change (tools/second-environment/github-api-detail.mjs PullRequestFileStats): README.md and
// notes/plan.md are docs, notes/outline.ts is code.
const SPLIT = { code: lines(80, 0), tests: lines(0, 0), mocks: lines(0, 0), generated: lines(0, 0), docs: lines(123, 1) };
const entry = (number: number, extra: Obj = {}): Obj => ({
  provider: 'github', host: 'github.com', projectId: 'p1', projectTitle: 'sandbox', repository: 'lane/sandbox', number, title: `Change ${number}`,
  url: `https://github.com/lane/sandbox/pull/${number}`, author: { login: 'lane-primary' }, headBranch: `feature/${number}`, baseBranch: 'main',
  state: 'open', isDraft: false, mergeability: 'mergeable', additions: 203, deletions: 1, checksState: 'passing',
  createdAt: '2026-10-07T00:00:00.000Z', updatedAt: '2026-10-08T00:00:00.000Z', viewerReviewRequested: false, labels: [], ...extra,
});
type Reply = (payload: Obj) => unknown;
function fakeClient(replies: Record<string, Reply>) {
  const calls: { method: string; payload: Obj }[] = [];
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, revision: 0, generation: 1, local: {} as Obj,
    config: { environment: { capabilities: { pullRequests: true, threadPullRequests: true } }, settings: {} },
    shell: { projects: [{ id: 'p1', title: 'sandbox', workspaceRoot: '/repos/sandbox' }], threads: [] as Obj[] },
    rpc: async (_native: unknown, method: string, payload: Obj) => {
      calls.push({ method, payload });
      const reply = replies[method];
      if (!reply) throw new Error(`no reply for ${method}`);
      return reply(payload);
    },
    restAccess: () => ({ call: async () => ({}) }),
  } as unknown as T3Client;
  client.projectGroups = () => client.shell.projects.map(project => ({ key: String(project.id), name: String(project.title ?? ''), members: [project] }));
  return { client, calls };
}
let wakes = 0;
const native = { available: true, watch: () => {}, later: async () => { wakes++; return { ok: true }; } } as unknown as Native;
const listOf = (...entries: Obj[]): Record<string, Reply> => ({
  'pullRequests.list': () => ({ entries, viewers: {}, providers: [{ host: 'github.com', kind: 'github', configured: true }], errors: [], truncated: false }),
  'pullRequests.listStats': () => ({ stats: [] }),
});
async function firstRow(client: T3Client) {
  const view = await pullRequestsPage(client, native, { open: true, refresh: 0, now: NOW, selected: '', query: '', typed: false });
  return view.groups.flatMap(group => group.rows)[0]!;
}
const target = (extra: Obj = {}) => ({ reference: { projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 }, additions: 203, deletions: 1, ...extra });

describe('the lines (ChangeBreakdownGrid)', () => {
  test('each kind that changed in the reference order, counts grouped, then the Total under its rule', () => {
    expect(breakdownLines({ ...SPLIT, tests: lines(1200, 34) })).toEqual([
      { key: 'code', label: 'Code', additions: '+80', deletions: '-0', row: 1 },
      { key: 'tests', label: 'Tests', additions: '+1,200', deletions: '-34', row: 2 },
      { key: 'docs', label: 'Docs', additions: '+123', deletions: '-1', row: 3 },
    ]);
  });
  test('a server from before the docs split sends no docs: it reads as none, its prose counted in code', () => {
    const older = decodeChangeBreakdown({ code: lines(203, 1), tests: lines(0, 0), mocks: lines(0, 0), generated: lines(0, 0) });
    expect(older?.docs).toEqual(lines(0, 0));
    expect(breakdownLines(older!).map(line => line.label)).toEqual(['Code']);
  });
});

describe('a list row (PullRequestRow)', () => {
  test('nothing is read until the +/- is hovered; then "Loading breakdown", then the split with its Total', async () => {
    const { client, calls } = fakeClient({ ...listOf(entry(7)), 'pullRequests.changeBreakdown': () => SPLIT });
    let row = await firstRow(client);
    expect(row.breakdown).toMatchObject({ requested: false, loading: false, rows: [] });
    expect(calls.some(call => call.method === 'pullRequests.changeBreakdown')).toBe(false);
    requestChangeBreakdown(client, row.breakdown.ref, 'list'); // pageslocal:pr-breakdown
    const before = wakes;
    row = await firstRow(client);
    expect(row.breakdown).toMatchObject({ requested: true, loading: true, rows: [] });
    expect(wakes).toBeGreaterThan(before);
    row = await firstRow(client);
    expect(calls.filter(call => call.method === 'pullRequests.changeBreakdown').map(call => call.payload)).toEqual([{ projectId: 'p1', host: 'github.com', repository: 'lane/sandbox', number: 7 }]);
    expect(row.breakdown.loading).toBe(false);
    expect(row.breakdown.rows.map(line => [line.label, line.additions, line.deletions])).toEqual([['Code', '+80', '-0'], ['Docs', '+123', '-1']]);
    expect(row.breakdown.total).toEqual([{ key: 'total', label: 'Total', additions: '+203', deletions: '-1', row: 4 }]);
    // The hover layer's estimate: three 16pt lines and the rule inside TooltipPopup's insets.
    expect([row.breakdown.width > 100, row.breakdown.height]).toEqual([true, 75]);
  });
  test('a searched row, whose counts come from listStats, asks for its split too', async () => {
    const { additions: _a, deletions: _d, ...searched } = entry(9);
    const { client } = fakeClient({ ...listOf(searched), 'pullRequests.listStats': () => ({ stats: [{ repository: 'lane/sandbox', number: 9, additions: 203, deletions: 1 }] }),
      'pullRequests.changeBreakdown': () => SPLIT });
    const row = await firstRow(client);
    expect([row.additions, row.breakdown.requested]).toEqual(['+203', false]);
  });
  test('a row without lines has no split to ask for', async () => {
    const { client } = fakeClient(listOf(entry(8, { additions: 0, deletions: 0 })));
    expect((await firstRow(client)).breakdown).toMatchObject({ requested: true, loading: false, rows: [] });
  });
});

describe('the read', () => {
  test('a view of one pull request reads at once (eager): loading first, then the split', async () => {
    const { client, calls } = fakeClient({ 'pullRequests.changeBreakdown': () => SPLIT });
    expect(await readChangeBreakdowns(client, native, [target()], { eager: 'detail' })).toBe('wake');
    expect(presentChangeBreakdown(client, target(), { eager: true })).toMatchObject({ loading: true, rows: [] });
    expect(await readChangeBreakdowns(client, native, [target()], { eager: 'detail' })).toBe('done');
    expect(calls).toHaveLength(1);
    expect(presentChangeBreakdown(client, target(), { eager: true }).rows).toHaveLength(2);
    // The details card's inline split carries no Total.
    expect(presentChangeBreakdown(client, target(), { eager: true, total: false }).total).toEqual([]);
  });
  test('a host with no per-file counts (null), or a server without the read, leaves the +/- alone', async () => {
    for (const reply of [() => null, () => { throw new Error('Unknown method pullRequests.changeBreakdown'); }]) {
      const { client } = fakeClient({ 'pullRequests.changeBreakdown': reply });
      await readChangeBreakdowns(client, native, [target()], { eager: 'detail' });
      await readChangeBreakdowns(client, native, [target()], { eager: 'detail' });
      expect(presentChangeBreakdown(client, target(), { eager: true })).toMatchObject({ loading: false, rows: [], total: [] });
    }
  });
  test('updates a minute apart with nothing announced read once; an announcement, other totals or a new request read again', async () => {
    const { client, calls } = fakeClient({ 'pullRequests.changeBreakdown': () => SPLIT });
    const reads = () => calls.filter(call => call.method === 'pullRequests.changeBreakdown').length;
    for (let tick = 0; tick < 3; tick++) await readChangeBreakdowns(client, native, [target()], { eager: 'detail' }); // the minute ticks
    expect(reads()).toBe(1);
    // pullRequests.subscribeRefreshes: a change announced; the split is read again, keeping the one shown meanwhile.
    prRefreshEvent(client, { key: 'pull-request-refreshes', subscriptionId: '1-1', value: 3 });
    expect(await readChangeBreakdowns(client, native, [target()], { eager: 'detail' })).toBe('done');
    expect(reads()).toBe(2);
    await readChangeBreakdowns(client, native, [target()], { eager: 'detail' });
    expect(reads()).toBe(2);
    // A surface with no announcement (a linked row's snapshot): the totals beside the split moved.
    await readChangeBreakdowns(client, native, [target({ additions: 210 })], { eager: 'detail' });
    expect(reads()).toBe(3);
    // The view closed and opened again: a new request reads once.
    releaseChangeBreakdowns(client, 'detail');
    await readChangeBreakdowns(client, native, [target({ additions: 210 })], { eager: 'detail' });
    await readChangeBreakdowns(client, native, [target({ additions: 210 })], { eager: 'detail' });
    expect(reads()).toBe(4);
  });
  test('a closed surface asks for nothing more: its hovered rows stop reading', async () => {
    const { client, calls } = fakeClient({ 'pullRequests.changeBreakdown': () => SPLIT });
    const key = presentChangeBreakdown(client, target()).ref;
    requestChangeBreakdown(client, key, 'linked');
    await readChangeBreakdowns(client, native, [target()]);
    await readChangeBreakdowns(client, native, [target()]);
    releaseChangeBreakdowns(client, 'linked');
    prRefreshEvent(client, { key: 'pull-request-refreshes', subscriptionId: '1-1', value: 4 });
    await readChangeBreakdowns(client, native, [target()]);
    expect(calls).toHaveLength(1);
    expect(presentChangeBreakdown(client, target()).requested).toBe(false);
  });
});

describe('the details card (ThreadDetailsPrRow, PullRequestChangeBreakdownRows)', () => {
  const detail = { title: 'Keep the notes in one place', number: 7, state: 'open', baseBranch: 'main', headBranch: 'feature/notes', changedFiles: 3, additions: 203, deletions: 1, checks: [] };
  test('lists the split under the diff size, with no Total, and widens to hold it', async () => {
    const { client } = fakeClient({ 'pullRequests.changeBreakdown': () => ({ ...SPLIT, generated: lines(12000, 3400) }) });
    await readChangeBreakdowns(client, native, [target()], { eager: 'detail' });
    await readChangeBreakdowns(client, native, [target()], { eager: 'detail' });
    const split = presentChangeBreakdown(client, target(), { eager: true, total: false });
    const bare = prCard(detail, 'open', 900), card = prCard(detail, 'open', 900, split);
    expect(card.breakdown.rows.map(line => line.label)).toEqual(['Code', 'Generated', 'Docs']);
    expect(card.breakdown.total).toEqual([]);
    // The hover layer's estimate: as wide as the widest line, and three more lines (16pt, 4pt apart, 2pt inside) on one of its own.
    expect(card.width).toBeGreaterThanOrEqual(bare.width);
    expect(card.height - bare.height).toBe(3 * 16 + 2 * 4 + 4 + 6);
  });
  test('a change without lines carries no split', () => {
    expect(prCard({ ...detail, additions: 0, deletions: 0 }, 'open', 900, { ref: 'x', requested: true, loading: false, rows: [{ key: 'code', label: 'Code', additions: '+1', deletions: '-0', row: 1 }], total: [] }).breakdown.rows).toEqual([]);
  });
});

describe('the details card in the window\'s hover layer (ThreadDetailsPrRow, its portalled TooltipPopup)', () => {
  // The hover-layer fns as JavaScript, as hover-layer.contract defines them.
  const hoverTipAtFrame = (key: string, kind: string, text: string, lines: string[], side: string, align: string, frame: Obj, estimate: number, testId: string) =>
    ({ key, kind, text, lines, side, align, x: frame.x, y: frame.y, width: frame.width, height: frame.height, estimate, wide: 0, testId });
  const hoverTipWide = (tip: Obj, wide: number) => ({ ...tip, wide });
  const card = { show: true, flipped: false, width: 290, height: 192, breakdown: { ref: 'k', requested: true } };
  const run = (extra: Obj, over: boolean) => {
    const component = contractComponent('src/panel/pr-attachment.contract', 'DetailsPrRowView'), state = component.state({});
    const tips: unknown[][] = [], locals: unknown[][] = [];
    const scope = { row: { key: 'pr-7', split: true, extra: { card: { ...card, ...extra } } }, local: (...args: unknown[]) => locals.push(args),
      hoverTipAt: (...args: unknown[]) => tips.push(args), hoverTipAtFrame, hoverTipWide, frame: () => ({ x: 1000, y: 300, width: 262, height: 32 }) };
    component.call('hover', state, scope, ['row', over]);
    return { tips, locals };
  };
  test('pointing at the row hands the card, its size and the link half\'s frame to the layer; leaving takes it back', () => {
    expect(run({}, true).tips).toEqual([[{ key: 'details-pr-card-pr-7', kind: 'pr-card', text: 'pr-7', lines: [], side: 'top', align: 'start', x: 1000, y: 300, width: 262, height: 32, estimate: 192, wide: 290, testId: 'details-pr-card' }, 'trigger', true]]);
    expect(run({}, false).tips[0]?.[2]).toBe(false);
    // Where it would cross the window, it ends where the link half ends.
    expect((run({ flipped: true }, true).tips[0]?.[0] as Obj).align).toBe('end');
  });
  test('the first point at the row asks for the split', () => {
    expect(run({ breakdown: { ref: 'k', requested: false } }, true).locals).toEqual([['surface-pr-breakdown', 'card', 'k']]);
  });
});
