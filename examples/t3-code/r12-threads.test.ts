import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { environmentOptions, runOnEnvironment } from './r4-git-env';
import { awaitScratchProject, machineChanging, scratchChoices, SCRATCH_NOT_LOADED } from './r12-threads-scratch';
import { activityRun, setupTurnStarted } from './r12-threads-worktree';
import { adoptWorktreeSetup, threadWorktreeSetup } from './timeline-worktree';
import { autoShowDevices, placeMini, resetLaunch, visibleMini } from './r12-threads-device';
import { r6DeviceMini, floatMiniDevice } from './r6-media-device';
import { toasts } from './toast';

const scratchRoot = (machine: string) => `/Users/${machine}/.t3/scratch`;
const entry = (id: string, extra: Partial<FleetEntry> = {}): FleetEntry => ({
  key: `https://${id}.example.invalid\n${id}`, origin: `https://${id}.example.invalid`, environmentId: id, phase: 'connected', message: '', traceId: '',
  generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: { environment: { label: `Box ${id}` }, scratchWorkspaceRoot: scratchRoot(id) },
  shell: { projects: [], threads: [], sequence: 0 } as unknown as FleetEntry['shell'], scopes: [], error: '', requested: true, ...extra });

function scratchDraft(threadId = ''): T3Client {
  return { environmentId: 'a', origin: 'http://127.0.0.1:1', connection: 'connected', projectId: 'sa', threadId, draftKey: threadId ? `a:${threadId}` : 'a:new:sa',
    config: { environment: { label: 'Local' }, scratchWorkspaceRoot: scratchRoot('a') },
    shell: { projects: [{ id: 'sa', title: 'Scratch', workspaceRoot: scratchRoot('a') }], threads: [] },
    local: { groupingMode: 'repository', groupingOverrides: {}, drafts: { 'a:new:sa': 'no project text' }, selections: {}, composerControls: { contexts: {} } } } as unknown as T3Client;
}

describe('No project drafts switch machine (c47f4263f9, 845ddd9354)', () => {
  test('every connected machine with a "No project" folder is a choice; one not created yet has no project', () => {
    const source = new EnvironmentFleet();
    const b = entry('b', { shell: { projects: [{ id: 'sb', workspaceRoot: `${scratchRoot('b')}/` }], threads: [], sequence: 0 } as unknown as FleetEntry['shell'] });
    const c = entry('c'), d = entry('d', { config: { environment: { label: 'No scratch' } } }), e = entry('e', { phase: 'error' });
    for (const item of [b, c, d, e]) source.entries.set(item.key, item);
    expect(scratchChoices(scratchDraft(), source.entries.values())).toEqual([{ environmentId: 'a', projectId: 'sa' }, { environmentId: 'b', projectId: 'sb' }, { environmentId: 'c', projectId: '' }]);
    expect(environmentOptions(scratchDraft(), source).map(option => [option.id, option.projectId])).toEqual([['a', 'sa'], ['b', 'sb'], ['c', '']]);
    // A started No project thread keeps its machine, and the logical-project rule never groups Scratch folders.
    expect(environmentOptions(scratchDraft('t1'), source).map(option => option.id)).toEqual(['a']);
  });

  test('picking a machine without the folder creates it there, waits for it, then moves the draft', async () => {
    const source = new EnvironmentFleet(), c = entry('c');
    source.entries.set(c.key, c);
    const calls: Obj[] = [];
    let shellReads = 0;
    const native = { available: true, watch() {}, later: async (request: unknown) => {
      const call = request as Obj; calls.push(call);
      if (call.op === 'request' && call.method === 'projects.ensureScratch') return { ok: true, value: { projectId: 'sc' }, generation: 1 };
      if (call.op === 'http') { shellReads++; return { ok: true, value: shellReads < 2 ? { snapshotSequence: 1, projects: [], threads: [] } : { snapshotSequence: 2, projects: [{ id: 'sc', workspaceRoot: scratchRoot('c') }], threads: [] }, generation: 1 }; }
      if (call.op === 'connect') return { ok: true, value: { state: 'connecting', environmentId: 'c' }, generation: 7 };
      return { ok: true, value: {}, generation: 1 };
    } } as Native;
    const client = scratchDraft();
    const moved = await runOnEnvironment(client, native, 'c', source);
    expect(moved.generation).toBe(7);
    expect(calls.filter(call => call.fleet === c.key).map(call => call.op === 'request' ? call.method : call.op)).toContain('projects.ensureScratch');
    expect(shellReads).toBe(2);
    expect(client.local.drafts).toEqual({ 'c:new:sc': 'no project text' });
    expect(client.local.selections.c).toEqual({ projectId: 'sc', threadId: '' });
    expect(machineChanging(client)).toBe(false);
  });

  test('a folder that never reaches this device: "Could not switch machine", the draft stays', async () => {
    const source = new EnvironmentFleet(), c = entry('c');
    source.entries.set(c.key, c);
    const native = { available: true, watch() {}, later: async (request: unknown) => {
      const call = request as Obj;
      if (call.op === 'request') return { ok: true, value: { projectId: 'sc' }, generation: 1 };
      if (call.op === 'http') return { ok: true, value: { snapshotSequence: 1, projects: [], threads: [] }, generation: 1 };
      return { ok: true, value: {}, generation: 1 }; // timelineSleep: no wall-clock wait in the test
    } } as Native;
    const client = scratchDraft();
    expect(await runOnEnvironment(client, native, 'c', source)).toEqual({ status: null, generation: -1 });
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Could not switch machine', SCRATCH_NOT_LOADED]]);
    expect(client.local.drafts).toEqual({ 'a:new:sa': 'no project text' });
  });

  test('openScratch waits up to 10 s: rereads every half second, then fails', async () => {
    const slept: number[] = [];
    const native = { available: true, watch() {}, later: async (request: unknown) => { slept.push(Number((request as Obj).ms)); return { ok: true, value: {}, generation: 0 }; } } as Native;
    let reads = 0;
    await awaitScratchProject(native, () => reads >= 3, async () => { reads++; });
    expect(slept).toEqual([500, 500, 500]);
    slept.length = 0;
    await expect(awaitScratchProject(native, () => false, async () => {})).rejects.toThrow(SCRATCH_NOT_LOADED);
    expect(slept.length).toBe(20);
    expect(slept.reduce((sum, ms) => sum + ms, 0)).toBe(10_000);
  });
});

describe('the worktree setup card leaves a settled thread (resolveVisibleWorktreeSetup turnStarted)', () => {
  const done = { threadId: 't1', phase: 'done', sequence: 4, stages: [{ id: 'agent', status: 'done' }] };
  const client = (runs: Obj[]) => {
    const projection = { thread: { id: 't1', worktreePath: '/w' }, runs, visibleTurnItems: [{ item: { type: 'user_message' } }] };
    return { threadId: 't1', thread: { projection }, projection, ready: true } as unknown as T3Client;
  };
  test('a completed run (a retried preparation included) counts as started: the clean "Worktree ready" card hides', () => {
    const retried = client([{ id: 'r1', ordinal: 1, status: 'completed', startedAt: '2026-10-05T01:00:02Z' }]);
    adoptWorktreeSetup(retried, done);
    expect(threadWorktreeSetup(retried).snapshot).toBeNull();
    expect(setupTurnStarted(retried.projection)).toBe(true);
  });
  test('before the turn is live the card stays; a held queued run is never the activity run', () => {
    const preparing = client([{ id: 'r1', ordinal: 1, status: 'failed', startedAt: null }]);
    adoptWorktreeSetup(preparing, done);
    expect(threadWorktreeSetup(preparing).snapshot).toBe(done);
    const runs = [{ id: 'r1', ordinal: 1, status: 'completed', startedAt: 'x' }, { id: 'r2', ordinal: 2, status: 'queued', queueHeld: true }];
    expect(activityRun({ runs })?.id).toBe('r1');
    expect(activityRun({ runs: [...runs, { id: 'r3', ordinal: 3, status: 'running', startedAt: 'y' }, { id: 'r4', ordinal: 4, status: 'completed' }] })?.id).toBe('r3');
  });
});

describe('a thread\'s device session floats (ChatView autoShowFloatingPreview)', () => {
  const iphone = { hostId: 'local', id: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro', version: 'iOS 26.0' };
  const state = (sessions: Obj[]) => ({ hosts: [], devices: [iphone], sessions });
  const open = { threadId: 't1', hostId: 'local', deviceId: 'IPHONE' };
  const thread = (threadId = 't1') => ({ environmentId: 'env', threadId, generation: 1, presentation: {} }) as unknown as T3Client;
  test('the thread open before the first device state floats its session; a later visit takes a baseline', () => {
    resetLaunch();
    const first = thread();
    autoShowDevices(first, null, false); // before the first chunk: EMPTY_DEVICE_STATE, an empty baseline
    autoShowDevices(first, state([open]), false);
    expect(r6DeviceMini(first, state([open]))).toMatchObject({ show: true, name: 'iPhone 18 Pro', deviceId: 'IPHONE' });
    const later = thread('t2');
    autoShowDevices(later, state([]), false); // the launch thread is elsewhere
    (later as unknown as { threadId: string }).threadId = 't1';
    autoShowDevices(later, state([open]), false);
    expect(r6DeviceMini(later, state([open])).show).toBe(false);
  });
  test('a new session floats, a closed one takes the player with it, a sheet layout floats nothing', () => {
    resetLaunch();
    const client = thread();
    autoShowDevices(client, state([]), false);
    autoShowDevices(client, state([open]), false);
    expect(r6DeviceMini(client, state([open])).show).toBe(true);
    autoShowDevices(client, state([]), false);
    expect(r6DeviceMini(client, state([])).show).toBe(false);
    resetLaunch();
    const narrow = thread();
    autoShowDevices(narrow, state([open]), true);
    expect(r6DeviceMini(narrow, state([open])).show).toBe(false);
  });
  test('the player hides while the panel shows the same device (shouldRenderPreviewMiniPlayer)', () => {
    const client = thread();
    floatMiniDevice(client, 't1', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    const mini = r6DeviceMini(client, state([open]));
    expect(visibleMini(mini, { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: '' }).show).toBe(false);
    expect(visibleMini(mini, { hostId: 'local', deviceId: 'OTHER', platform: 'ios', name: '' }).show).toBe(true);
    expect(visibleMini(mini, undefined).show).toBe(true);
  });
  test('a new player is at least 240 wide at the source aspect, its bottom 12 above the composer (chatCanvasLayout)', () => {
    expect(placeMini(9 / 19.5, { chat: [400, 0, 740, 840], overlay: [400, 668, 740, 172] })).toEqual({ width: 240, height: 520, top: 128 });
    // A short canvas fits the height; the player never rises above 12 under the header.
    expect(placeMini(9 / 19.5, { chat: [0, 0, 600, 500], overlay: [0, 408, 600, 100] })).toEqual({ width: 150, height: 324, top: 64 });
    expect(placeMini(16 / 9, { chat: [0, 0, 900, 840], overlay: [0, 668, 900, 172] })).toEqual({ width: 320, height: 180, top: 468 });
    expect(placeMini(9 / 19.5, {})).toEqual({ width: 240, height: 520, top: 64 });
  });
});

describe('the panel tab strip scrolls when its tabs overflow (RightPanelTabs ScrollArea)', () => {
  const { tabStrip, activeSerial } = require('./r12-threads-tabs') as typeof import('./r12-threads-tabs');
  test('overflow, the furthest offset and the active tab come from the anchors', () => {
    const anchors = { 'r12-tabs-view': [0, 200], 'r12-tabs-content': [0, 200], 'r12-tab:diff': [270, 52], 'r12-tab:a': [0, 90] };
    expect(tabStrip({ anchors }, ['a', 'diff'], 'diff', 2)).toEqual({ overflow: true, view: 200, content: 350, max: 150, activeLeft: 270, activeRight: 322, serial: 2 });
    expect(tabStrip({ anchors: { 'r12-tabs-view': [0, 200], 'r12-tab:a': [0, 90] } }, ['a'], 'a').overflow).toBe(false);
    expect(tabStrip({}, ['a'], 'a')).toMatchObject({ overflow: false, max: 0 });
  });
  test('an active change on the same panel counts; a launch or another panel does not', () => {
    const owner = {};
    expect(activeSerial(owner, 'p1', '')).toBe(0);
    expect(activeSerial(owner, 'p1', 'diff')).toBe(0); // restored: the first active is not a change
    expect(activeSerial(owner, 'p1', 'diff')).toBe(0);
    expect(activeSerial(owner, 'p1', 'device')).toBe(1);
    expect(activeSerial(owner, 'p2', 'files')).toBe(1);
    expect(activeSerial(owner, 'p2', 'diff')).toBe(2);
  });
});
