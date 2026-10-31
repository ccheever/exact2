// The open thread's visit against T3 Code 7c3b3b3605ef (MIT, LICENSE-T3), threadVisits.ts createThreadVisitTracker: it
// runs whenever the open thread's shell changes, sends thread.visit to the thread's own environment at its updatedAt,
// dedupes per `${routeThreadKey}:${updatedAt}`, and sends at once when a completion is unseen. Showing a thread is its
// open (claimThreadOpen), and the open rides on the first visit the server hears. A thread opened from a background
// environment's row arrives by the reconnect to that environment (settings-b-fleet.ts focusFleetThread), so the
// refresh after it is where the visit is sent.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { sidebarRefreshed } from '../../src/sidebar/sidebar-commands';
import { lastVisited, unseenCompletion } from '../../src/sidebar/sidebar-model';
import { adoptCommandTime, setRuntimeClock } from '../../src/sidebar/sidebar-state';
import { claimThreadOpen, threadSearchSubject } from '../../src/palette/palette-search';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';

beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => setRuntimeClock(() => Number.NaN));

const NOW = Date.parse('2026-10-15T22:40:00.000Z');
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true, threadVisitedTracking: true };
// 'Background thread (completed turn)' after Mark unread: the server rewound lastVisitedAt below the completion and
// left updatedAt where the rewind's write put it.
const markedUnread = (id: string): Obj => ({ id, projectId: 'p1', title: 'Background thread (completed turn)', status: 'idle', latestRunId: 'r1',
  latestRunCompletedAt: iso(-1000), lastVisitedAt: iso(-1001), createdAt: iso(-60_000), updatedAt: iso(37_374), archivedAt: null,
  settledOverride: null, settledAt: null, modelSelection: { instanceId: 'acp', model: 'm' }, lineage: { relationshipToParent: null } });

function focused(environmentId: string, threads: Obj[], threadId: string) {
  const dispatched: Obj[] = [];
  let ids = 0;
  const client = {
    shell: { projects: [{ id: 'p1', title: 'background-only', workspaceRoot: '/background-only', projectIcon: { kind: 'emoji', emoji: 'B' } }], threads, sequence: 1 },
    config: { environment: { capabilities: CAPS }, providers: [], keybindings: [] },
    environmentId, threadId, projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, generation: 2, origin: 'http://127.0.0.1:16310',
    presentation: {}, local: { drafts: {}, snapshotDrafts: {}, snapshotReleases: [], deviceSettings: { timestampFormat: '24-hour' }, clientSettings: {}, sidebarWidth: 256 },
    projectGroups() { return []; },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, () => `c${ids++}`),
      request: async (method: string, payload: Obj) => { if (method === 'orchestration.dispatchCommand') dispatched.push({ environmentId: client.environmentId, ...payload }); return {}; },
      call: async () => ({}),
    }),
    rpc: async () => ({}),
  } as unknown as T3Client & { environmentId: string; threadId: string };
  return { client, dispatched };
}
const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
const settle = () => new Promise(resolve => setTimeout(resolve, 5));
const strip = (payloads: Obj[]) => payloads.map(({ commandId: _id, ...rest }) => rest);

describe('opening a background environment\'s thread visits it there', () => {
  test('the refresh after the reconnect sends thread.visit, the open, to that environment once, and the row\'s Completed goes', async () => {
    const thread = markedUnread('t-bg');
    expect(unseenCompletion(thread, lastVisited(thread, undefined))).toBe(true);
    const { client, dispatched } = focused('env-second', [thread], 't-bg');
    await sidebarRefreshed(client, native);
    await sidebarRefreshed(client, native);
    await settle();
    expect(strip(dispatched)).toEqual([{ environmentId: 'env-second', type: 'thread.visit', threadId: 't-bg', visitedAt: iso(37_374), opened: true }]);
    // The server's thread.visited echo moves the watermark to updatedAt: no unseen completion, no status.
    const visited = { ...thread, lastVisitedAt: iso(37_374) };
    client.shell.threads = [visited];
    await sidebarRefreshed(client, native);
    await settle();
    expect(dispatched).toHaveLength(1);
    expect(unseenCompletion(visited, lastVisited(visited, undefined))).toBe(false);
  });

  test('the same thread id on another environment is another thread: its visit is not deduped away', async () => {
    const { client, dispatched } = focused('env-first', [markedUnread('t1')], 't1');
    await sidebarRefreshed(client, native);
    await settle();
    (client as unknown as { environmentId: string }).environmentId = 'env-second';
    client.shell.threads = [markedUnread('t1')];
    await sidebarRefreshed(client, native);
    await settle();
    expect(strip(dispatched).map(payload => `${String(payload.environmentId)}:${String(payload.type)}:${String(payload.threadId)}`))
      .toEqual(['env-first:thread.visit:t1', 'env-second:thread.visit:t1']);
  });

  test('Mark unread on the thread already visited at this watermark stays unread (the reference\'s dedupe)', async () => {
    const thread = markedUnread('t-open');
    const { client, dispatched } = focused('env-second', [thread], 't-open');
    await sidebarRefreshed(client, native);
    client.shell.threads = [{ ...thread, lastVisitedAt: iso(37_374) }];
    await sidebarRefreshed(client, native);
    // Mark unread while it is open: the rewind leaves updatedAt alone, so the dispatched key skips a fresh visit.
    client.shell.threads = [thread];
    await sidebarRefreshed(client, native);
    await settle();
    expect(dispatched).toHaveLength(1);
  });

  test('another thread\'s visit inside the throttle\'s window is sent, so a thread visited before, then marked unread, is visited when opened again', async () => {
    // The window's wall time moves only with commands: here it stays where the first open put it.
    setRuntimeClock(() => NOW);
    const background = markedUnread('t-bg');
    // A thread on the other environment that was never visited and has no run: no unseen completion to send at once.
    const third: Obj = { ...markedUnread('t-third'), title: 'Shared notes on third', latestRunId: null, latestRunCompletedAt: null, lastVisitedAt: null, updatedAt: iso(5_000) };
    const { client, dispatched } = focused('env-second', [background], 't-bg');
    const open = async (environmentId: string, threadId: string, threads: Obj[]) => {
      Object.assign(client, { environmentId, threadId });
      client.shell.threads = threads;
      await sidebarRefreshed(client, native);
      await settle();
    };
    await open('env-second', 't-bg', [background]);
    await open('env-third', 't-third', [third]);
    // One thread's mid-turn bump inside the window is still held back.
    await open('env-third', 't-third', [{ ...third, lastVisitedAt: iso(5_000), updatedAt: iso(6_000) }]);
    // Mark unread on the background thread (the rewind leaves updatedAt), then open it again.
    await open('env-second', 't-bg', [background]);
    expect(strip(dispatched).map(payload => `${String(payload.environmentId)}:${String(payload.threadId)}:${String(payload.visitedAt)}`))
      .toEqual([`env-second:t-bg:${iso(37_374)}`, `env-third:t-third:${iso(5_000)}`, `env-second:t-bg:${iso(37_374)}`]);
  });

  test('no thread open, or a read-only connection: nothing is sent', async () => {
    const none = focused('env-second', [markedUnread('t-bg')], '');
    await sidebarRefreshed(none.client, native);
    const readOnly = focused('env-second', [markedUnread('t-bg')], 't-bg');
    (readOnly.client as unknown as { writable: boolean }).writable = false;
    await sidebarRefreshed(readOnly.client, native);
    await settle();
    expect([...none.dispatched, ...readOnly.dispatched]).toEqual([]);
  });
});

// Opens are kept per environment and thread for the whole window, so each test here names its own environment.
describe('showing a thread counts as opening it (claimThreadOpen)', () => {
  const visited = (id: string, extra: Obj = {}): Obj => ({ ...markedUnread(id), latestRunCompletedAt: iso(-5000), lastVisitedAt: iso(37_374), ...extra });
  const payloads = (dispatched: Obj[]) => strip(dispatched).map(payload => `${String(payload.threadId)}:${String(payload.visitedAt)}:${String(payload.opened ?? '')}`);

  test('the open rides on the first visit the server hears, even with the watermark current, however late the shell arrives', async () => {
    const { client, dispatched } = focused('env-opens-late', [], 't-late');
    await sidebarRefreshed(client, native); // shown before its shell: nothing to send yet
    await settle();
    expect(dispatched).toEqual([]);
    client.shell.threads = [visited('t-late')];
    await sidebarRefreshed(client, native);
    await sidebarRefreshed(client, native);
    await settle();
    expect(payloads(dispatched)).toEqual([`t-late:${iso(37_374)}:true`]);
  });

  test('a thread opened again within ten minutes sends no second open, nor does one a ⌘K pick claimed', async () => {
    const { client, dispatched } = focused('env-opens-again', [visited('t-a'), visited('t-b'), visited('t-picked')], 't-a');
    const show = async (threadId: string) => { Object.assign(client, { threadId }); await sidebarRefreshed(client, native); await settle(); };
    await show('t-a');
    await show('t-b');
    await show('t-a');
    claimThreadOpen('env-opens-again', 't-picked', Date.now()); // palette-commands.ts claimPick: a ⌘K query pick claims its open first
    await show('t-picked');
    expect(payloads(dispatched)).toEqual([`t-a:${iso(37_374)}:true`, `t-b:${iso(37_374)}:true`]);
  });

  test('a server that keeps no visits hears no open; a read-only connection sends it once writable', async () => {
    const untracked = focused('env-opens-untracked', [{ ...visited('t-u'), lastVisitedAt: undefined }], 't-u');
    await sidebarRefreshed(untracked.client, native);
    const readOnly = focused('env-opens-read-only', [visited('t-r')], 't-r');
    (readOnly.client as unknown as { writable: boolean }).writable = false;
    await sidebarRefreshed(readOnly.client, native);
    await settle();
    expect([...untracked.dispatched, ...readOnly.dispatched]).toEqual([]);
    (readOnly.client as unknown as { writable: boolean }).writable = true;
    await sidebarRefreshed(readOnly.client, native);
    await settle();
    expect(payloads(readOnly.dispatched)).toEqual([`t-r:${iso(37_374)}:true`]);
  });

  test('a view with no thread ends the last one\'s: shown again fifteen minutes on, the thread is opened again', async () => {
    let now = NOW;
    setRuntimeClock(() => now);
    const { client, dispatched } = focused('env-opens-draft', [visited('t-d')], 't-d');
    await sidebarRefreshed(client, native);
    Object.assign(client, { threadId: '' }); // ⌘N: a draft
    await sidebarRefreshed(client, native);
    now += 15 * 60_000;
    Object.assign(client, { threadId: 't-d' });
    await sidebarRefreshed(client, native);
    await settle();
    expect(payloads(dispatched)).toEqual([`t-d:${iso(37_374)}:true`, `t-d:${iso(37_374)}:true`]);
  });

  test('the open is stamped with the later of the last sidebar snapshot\'s time and the last command\'s', async () => {
    setRuntimeClock(() => Number.NaN); // the data runtime: no clock of its own
    const { client } = focused('env-opens-stamp', [visited('t-s1'), visited('t-s2')], '');
    const helpers = { projectIdentity: () => ({ projectMark: 'P', projectInk: 'ink', projectSurface: 'surface' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
    const openedAt = (id: string) => threadSearchSubject({ id }, null, null, 'env-opens-stamp').opens?.openedAt;
    sidebarSnapshot(client, NOW + 60_000, helpers); // the root's minute clock
    Object.assign(client, { threadId: 't-s1' });
    await sidebarRefreshed(client, native);
    expect(openedAt('t-s1')).toBe(NOW + 60_000);
    adoptCommandTime(client, NOW + 90_000); // a sidebar command's window time, later than the last snapshot
    sidebarSnapshot(client, NOW, helpers); // an older snapshot time never lowers it
    Object.assign(client, { threadId: 't-s2' });
    await sidebarRefreshed(client, native);
    expect(openedAt('t-s2')).toBe(NOW + 90_000);
  });
});
