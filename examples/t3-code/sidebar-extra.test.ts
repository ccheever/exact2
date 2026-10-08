import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { providerPillView, sidebarProviderPill, dismissProviderPill, scheduleProviderPill, SUCCESS_VISIBLE_MS } from './sidebar-provider-pill';
import { CARD, LABEL, dropIndex, parseDrop, planDrop, sidebarDrop } from './sidebar-drop';
import { sidebarSession, setRuntimeClock } from './sidebar-state';
import { primaryAt, primaryOff, resetPrimary } from './local-primary-fixture';

// The data runtime has no clock (sidebar-state.ts `clock`); these tests stand in for a host clock that reads Date.now.
beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => { setRuntimeClock(() => Number.NaN); resetPrimary(); });

const provider = (driver: string, update: Obj | null, extra: Obj = {}): Obj => ({ driver, instanceId: driver, enabled: true, checkedAt: '2026-10-04T10:00:00.000Z',
  version: '0.160.0', versionAdvisory: { latestVersion: '0.160.0' }, ...(update ? { updateState: update } : {}), ...extra });

describe('provider update pill (getProviderUpdateSidebarPillView)', () => {
  test('a queued or running update reads Updating; several providers count', () => {
    expect(providerPillView([provider('codex', { status: 'running' })], undefined, new Set())).toMatchObject({ tone: 'loading', title: 'Updating Codex',
      description: 'Codex update in progress.', key: 'loading:codex:running', dismissible: false });
    expect(providerPillView([provider('codex', { status: 'queued' }), provider('claudeAgent', { status: 'running' })], undefined, new Set()))
      .toMatchObject({ title: 'Updating 2 providers', description: 'Codex and Claude updates are in progress.' });
  });
  test('outcomes: failed and unchanged stay until dismissed; success leaves on its own; the newest outcome leads', () => {
    const failed = provider('codex', { status: 'failed', finishedAt: '2026-10-04T10:05:00.000Z', message: 'npm exited 1' });
    const unchanged = provider('claudeAgent', { status: 'unchanged', finishedAt: '2026-10-04T10:06:00.000Z' });
    const view = providerPillView([failed, unchanged], '2026-10-04T10:00:00.000Z', new Set());
    expect(view).toMatchObject({ tone: 'warning', title: 'Claude still needs an update', dismissible: true,
      description: 'Claude still appears outdated. Review provider settings for details.' });
    const next = providerPillView([failed, unchanged], '2026-10-04T10:00:00.000Z', new Set([view!.key]));
    expect(next).toMatchObject({ tone: 'error', title: 'Codex v0.160.0 update failed', description: 'npm exited 1', dismissible: true });
    expect(providerPillView([provider('codex', { status: 'succeeded', finishedAt: '2026-10-04T10:07:00.000Z' })], '2026-10-04T10:00:00.000Z', new Set()))
      .toMatchObject({ tone: 'success', title: 'Codex updated: v0.160.0', description: 'New sessions will use the updated provider.', dismissAfterMs: SUCCESS_VISIBLE_MS });
  });
  test('outcomes from before the window first looked never show; no update, no pill', () => {
    expect(providerPillView([provider('codex', { status: 'failed', finishedAt: '2026-10-04T09:00:00.000Z' })], '2026-10-04T10:00:00.000Z', new Set())).toBeNull();
    expect(providerPillView([provider('codex', null)], undefined, new Set())).toBeNull();
  });
  test('the window remembers its first check, retires a success after three seconds and keeps dismissals', () => {
    const providers = [provider('codex', null)];
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const client = { config: { providers }, ready: true, environmentId: 'env-local' } as unknown as T3Client;
    const delays: number[] = [];
    expect(sidebarProviderPill(client, 1_000)).toBeNull();
    (client.config as Obj).providers = [provider('codex', { status: 'succeeded', finishedAt: '2026-10-04T10:01:00.000Z' })];
    expect(sidebarProviderPill(client, 2_000)?.tone).toBe('success');
    scheduleProviderPill(client, 2_500, delay => delays.push(delay));
    scheduleProviderPill(client, 2_600, delay => delays.push(delay));
    expect(delays).toEqual([SUCCESS_VISIBLE_MS - 500 + 50]);
    expect(sidebarProviderPill(client, 2_000 + SUCCESS_VISIBLE_MS)).toBeNull();
    (client.config as Obj).providers = [provider('codex', { status: 'failed', finishedAt: '2026-10-04T10:02:00.000Z' })];
    const failed = sidebarProviderPill(client, 9_000);
    expect(failed?.tone).toBe('error');
    dismissProviderPill(client, failed!.key);
    expect(sidebarProviderPill(client, 9_000)).toBeNull();
  });
  test('the pill reads the primary\'s providers only (primaryServerProvidersAtom): a focused remote or no primary shows none', () => {
    const running = [provider('codex', { status: 'running' })];
    primaryAt('http://127.0.0.1:16437', 'env-local');
    expect(sidebarProviderPill({ config: { providers: running }, ready: true, environmentId: 'env-local' } as unknown as T3Client, 1_000)?.title).toBe('Updating Codex');
    // A remote environment in focus while the primary has no background connection: nothing to show.
    expect(sidebarProviderPill({ config: { providers: running }, ready: true, environmentId: 'env-box' } as unknown as T3Client, 1_000)).toBeNull();
    primaryOff();
    expect(sidebarProviderPill({ config: { providers: running }, ready: true, environmentId: 'env-box' } as unknown as T3Client, 1_000)).toBeNull();
  });
});

describe('sidebar drag and drop (planSidebarThreadDrop)', () => {
  const keys = (entries: [string, string | null][]) => new Map(entries);
  const base = { pinnedIds: ['p1', 'p2'], activeIds: ['a1', 'a2', 'a3'], pinnedKeys: keys([['p1', 'm'], ['p2', 't']]),
    activeKeys: keys([['a1', null], ['a2', null], ['a3', null]]), supportsSettlement: true, pinReorder: true, activeReorder: true };
  // Labels open while dragging: Pinned header at topY, the divider after the pinned cards.
  const topY = 100, dividerY = topY + LABEL + 2 * CARD;
  const at = (target: string, centerY: number) => parseDrop(`${target}|${centerY}|${topY}|${dividerY}|false`);
  test('dropping an active card on the Pinned block pins it with a key between its neighbours', () => {
    const plan = planDrop({ ...base, id: 'a2', from: 'active', target: 'pinned', geometry: at('pinned', topY + LABEL + CARD) });
    expect(plan.verb).toBe('pin');
    expect(plan.steps).toEqual([{ type: 'thread.pin', threadId: 'a2', orderKey: expect.any(String) }]);
    const key = plan.steps[0]!.orderKey!;
    expect(key > 'm' && key < 't').toBe(true);
    expect(plan.failure).toEqual(['Failed to pin thread']);
  });
  test('dragging out of Pinned unpins; out of Settled un-settles; onto the shelf settles', () => {
    expect(planDrop({ ...base, id: 'p1', from: 'pinned', target: 'active', geometry: at('active', dividerY + 2 * LABEL + 10) }).steps[0])
      .toEqual({ type: 'thread.unpin', threadId: 'p1' });
    expect(planDrop({ ...base, id: 's1', from: 'settled', target: 'active', geometry: at('active', dividerY + 2 * LABEL + 10) }))
      .toMatchObject({ verb: 'unsettle', steps: [{ type: 'thread.unsettle', threadId: 's1', reason: 'user' }, expect.anything(), expect.anything(), expect.anything(), expect.anything()] });
    expect(planDrop({ ...base, id: 'a1', from: 'active', target: 'settled', geometry: at('settled', 900) }))
      .toEqual({ verb: 'settle', steps: [{ type: 'thread.settle', threadId: 'a1' }], failure: ['Failed to settle thread'] });
  });
  test('reordering inside Pinned writes one key; a drop in place, on Working or Snoozed, or without settlement writes nothing', () => {
    const plan = planDrop({ ...base, id: 'p1', from: 'pinned', target: 'pinned', geometry: at('pinned', topY + LABEL + 2 * CARD - 4) });
    expect(plan.verb).toBe('');
    expect(plan.steps).toEqual([{ type: 'thread.pin.reorder', threadId: 'p1', orderKey: expect.any(String) }]);
    expect(plan.steps[0]!.orderKey! > 't').toBe(true);
    expect(planDrop({ ...base, id: 'p1', from: 'pinned', target: 'pinned', geometry: at('pinned', topY + LABEL + CARD / 2) }).steps).toEqual([]);
    expect(planDrop({ ...base, id: 'a1', from: 'active', target: 'working', geometry: at('working', 600) }).steps).toEqual([]);
    expect(planDrop({ ...base, id: 'a1', from: 'active', target: 'snoozed', geometry: at('snoozed', 600) }).steps).toEqual([]);
    expect(planDrop({ ...base, supportsSettlement: false, id: 'a1', from: 'active', target: 'settled', geometry: at('settled', 900) }).steps).toEqual([]);
  });
  test('the slot follows the lifted centre against the painted middles', () => {
    expect(dropIndex(['x', 'y', 'z'], 'z', 0, 50)).toEqual(['z', 'x', 'y']);
    expect(dropIndex(['x', 'y', 'z'], 'x', 50 + 2 * CARD + 1, 50)).toEqual(['y', 'z', 'x']);
  });
  test('past the list edge, a drop on the composer adds the thread (or the selection) as context', async () => {
    const inserted: Obj[] = [];
    const threads = [{ id: 't1', title: 'One' }, { id: 't2', title: 'Two' }];
    const client = { shell: { threads }, config: {} } as unknown as T3Client;
    const native = { available: true, watch() {}, later: async (request: Obj) => { inserted.push(request); return { ok: true, generation: 0, value: { applied: true } }; } } as unknown as Native;
    await sidebarDrop(client, native, 't1', 'context|0|0|0|false');
    expect(inserted).toEqual([]);
    sidebarSession(client).selection = ['t1', 't2'];
    await sidebarDrop(client, native, 't1', 'context|0|0|0|true');
    expect(inserted.map(request => request.op)).toEqual(['editorInsert', 'editorInsert']);
    expect(String(inserted[0]!.text)).toContain('One');
    expect(String(inserted[1]!.text)).toContain('Two');
  });
});

describe('subagent threads (filterSidebarV2VisibleThreads, parentThreadLink)', () => {
  test('a subagent timeline opens with "Subagent of · <parent>" (bot glyph) whose action opens the parent', async () => {
    const { subagentLead } = await import('./sidebar-lineage');
    const { sidebarVisible } = await import('./sidebar-model');
    const child = { id: 'kid', title: 'Explore', createdAt: '2026-10-04T10:00:00.000Z', lineage: { relationshipToParent: 'subagent', parentThreadId: 'mom' }, archivedAt: null };
    const client = { threadId: 'kid', shell: { threads: [{ id: 'mom', title: 'Parent work', lineage: {} }, child] } } as unknown as T3Client;
    expect(subagentLead(client)).toEqual([expect.objectContaining({ kind: 'fork', icon: 'bot', body: 'Subagent of · Parent work', actionLabel: 'Open parent thread', targetId: 'mom' })]);
    expect(sidebarVisible(child)).toBe(false);
    (client as unknown as { threadId: string }).threadId = 'mom';
    expect(subagentLead(client)).toEqual([]);
  });
});

describe('thread undo (showThreadUndoNotice)', () => {
  test('Undo dispatches through the undo command\'s own handle, never the finished settle\'s', async () => {
    const { sidebarCommand } = await import('./sidebar-commands');
    const used: string[] = [], dispatched: Obj[] = [];
    const threads = [{ id: 'a', projectId: 'p1', title: 'A', status: 'idle', createdAt: '2026-10-04T10:00:00.000Z', updatedAt: '2026-10-04T10:00:00.000Z', lineage: {} }];
    const client = {
      shell: { projects: [{ id: 'p1', title: 'P' }], threads }, config: { environment: { capabilities: { threadSettlement: true } }, providers: [], keybindings: [] },
      threadId: '', projectId: 'p1', query: '', writable: true, ready: true, connection: 'connected', environmentId: 'env', presentation: {},
      local: { drafts: {}, snapshotDrafts: {}, deviceSettings: { timestampFormat: 'locale' }, clientSettings: {} },
      projectGroups: () => [{ key: 'g', name: 'P', members: [{ id: 'p1' }] }],
      restAccess: (handle: { name: string }) => ({ ids: async () => ['c'], request: async (_method: string, payload: Obj) => { used.push(handle.name); dispatched.push(payload); return {}; }, call: async () => ({}) }),
    } as unknown as T3Client;
    const settleHandle = { name: 'settle', available: true, watch() {}, later: async () => ({}) } as unknown as Native;
    const undoHandle = { name: 'undo', available: true, watch() {}, later: async () => ({}) } as unknown as Native;
    await sidebarCommand(client, settleHandle, {} as never, 'settle', 'a', '');
    await sidebarCommand(client, undoHandle, {} as never, 'undo', '', '');
    expect(dispatched.map(payload => payload.type)).toEqual(['thread.settle', 'thread.unsettle']);
    expect(used).toEqual(['settle', 'undo']);
  });
  test('a hover snooze preset resolves its wake when pressed, not when the menu painted', async () => {
    const { sidebarCommand } = await import('./sidebar-commands');
    const dispatched: Obj[] = [];
    const threads = [{ id: 'a', projectId: 'p1', title: 'A', status: 'idle', createdAt: '2026-10-04T10:00:00.000Z', updatedAt: '2026-10-04T10:00:00.000Z', lineage: {} }];
    const client = {
      shell: { projects: [{ id: 'p1', title: 'P' }], threads }, config: { environment: { capabilities: { threadSnooze: true } }, providers: [], keybindings: [] },
      threadId: '', projectId: 'p1', query: '', writable: true, ready: true, connection: 'connected', environmentId: 'env', presentation: {},
      local: { drafts: {}, snapshotDrafts: {}, deviceSettings: { timestampFormat: 'locale' }, clientSettings: {} },
      projectGroups: () => [{ key: 'g', name: 'P', members: [{ id: 'p1' }] }],
      restAccess: () => ({ ids: async () => ['c'], request: async (_method: string, payload: Obj) => { dispatched.push(payload); return {}; }, call: async () => ({}) }),
    } as unknown as T3Client;
    const before = Date.now();
    await sidebarCommand(client, { available: true, watch() {}, later: async () => ({}) } as unknown as Native, {} as never, 'snooze:hour', 'a', '');
    const until = Date.parse(String(dispatched[0]?.snoozedUntil));
    expect(dispatched[0]?.type).toBe('thread.snooze');
    expect(until - before).toBeGreaterThanOrEqual(3_600_000);
    expect(until - Date.now()).toBeLessThanOrEqual(3_600_000);
  });
});

describe('receding favicons', () => {
  test('grayscale follows the filter luminance per scheme', async () => {
    const { grayIdentity } = await import('./sidebar-view');
    expect(grayIdentity('light-dark(#f54900, #ff8904)')).toEqual({ projectGray: 'light-dark(#686868, #989898)', projectGraySurface: 'light-dark(#68686824, #98989824)' });
  });
});

describe('in-place rename (Sidebar.tsx handleDoubleClick)', () => {
  test('the double-click\'s own presses keep the edit; opening another row drops it', async () => {
    const { sidebarLocal, sidebarSelecting } = await import('./sidebar-commands');
    const threads = [{ id: 'a', projectId: 'p1', title: 'A', lineage: {} }, { id: 'b', projectId: 'p1', title: 'B', lineage: {} }];
    const client = { shell: { projects: [], threads }, config: {}, threadId: 'a', query: '', local: { clientSettings: {} }, projectGroups: () => [],
      restAccess: () => ({ call: async () => ({}) }) } as unknown as T3Client;
    const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
    await sidebarLocal(client, native, 'rename-start', 'a', '');
    expect(await sidebarSelecting(client, native, 'a', 'click')).toBe(false);
    expect(sidebarSession(client).renameId).toBe('a');
    await sidebarSelecting(client, native, 'b', 'click');
    expect(sidebarSession(client).renameId).toBe('');
  });
});

describe('sidebar wall time', () => {
  test('commands carry the window\'s wall time; the runtime clock (virtual under the agent) only measures from it', async () => {
    const { adoptCommandTime, wall, clock } = await import('./sidebar-state');
    const realNow = Date.now;
    try {
      let virtual = 6_100;
      Date.now = () => virtual;
      const client = {} as T3Client;
      expect(wall(client)).toBe(6_100);
      adoptCommandTime(client, 1_791_000_000_000);
      expect(wall(client)).toBe(1_791_000_000_000);
      virtual += 1_000;
      expect(wall(client)).toBe(1_791_000_001_000);
      adoptCommandTime(client, 0);
      expect(wall(client)).toBe(1_791_000_001_000);
      expect(clock()).toBe(7_100);
    } finally { Date.now = realNow; }
  });
});

describe('remembered shelves (useLocalStorage t3code:sidebar:*-expanded)', () => {
  test('shelf expansion and the project scope survive the preferences file; junk decodes to the defaults', async () => {
    const { adoptSidebarPrefs, defaultSidebarPrefs, sidebarPrefs } = await import('./sidebar-state');
    const saved = JSON.parse(JSON.stringify({ sidebar: { ...defaultSidebarPrefs(), settledExpanded: true, workingExpanded: true, scope: 'g2', visited: { a: '2026-10-04T10:00:00.000Z', b: 'nope' } } }));
    const local = {};
    adoptSidebarPrefs(local, saved);
    expect(sidebarPrefs({ local } as unknown as T3Client)).toEqual({ settledExpanded: true, snoozedExpanded: false, workingExpanded: true, scope: 'g2', visited: { a: '2026-10-04T10:00:00.000Z' }, projectExpanded: {}, projectOrder: [] });
    const fresh = {};
    adoptSidebarPrefs(fresh, { sidebar: 'garbage' });
    expect(sidebarPrefs({ local: fresh } as unknown as T3Client)).toEqual(defaultSidebarPrefs());
  });
});

describe('thread search pointer (onHighlight)', () => {
  test('the result under the pointer becomes the highlighted one Enter opens', async () => {
    const { sidebarLocal } = await import('./sidebar-commands');
    const threads = ['a', 'b', 'c'].map(id => ({ id, projectId: 'p1', title: `fixture ${id}`, status: 'idle', createdAt: '2026-10-04T10:00:00.000Z', updatedAt: '2026-10-04T10:00:00.000Z', lineage: {}, archivedAt: null }));
    const client = { shell: { projects: [{ id: 'p1', title: 'P' }], threads }, config: { environment: { capabilities: {} } }, query: 'fixture', threadId: '',
      local: { clientSettings: {}, drafts: {} }, projectGroups: () => [{ key: 'g', name: 'P', members: [{ id: 'p1' }] }] } as unknown as T3Client;
    const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
    await sidebarLocal(client, native, 'search-hover', 'c', '');
    expect(sidebarSession(client).searchIndex).toBe(2);
    await sidebarLocal(client, native, 'search-hover', 'zzz', '');
    expect(sidebarSession(client).searchIndex).toBe(2);
  });
});
