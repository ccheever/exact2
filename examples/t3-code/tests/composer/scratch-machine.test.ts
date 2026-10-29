import { afterEach, describe, expect, test } from 'bun:test';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';
afterEach(resetPrimary);
import type { T3Client } from '../../src/client/client';
import type { Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { EnvironmentFleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { environmentOptions, runOnEnvironment } from '../../src/details/git-env';
import { awaitScratchProject, machineChanging, scratchChoices, SCRATCH_NOT_LOADED } from '../../src/composer/scratch-machine';
import { activityRun, setupTurnStarted } from '../../src/timeline/worktree-card-run';
import { adoptWorktreeSetup, threadWorktreeSetup } from '../../src/timeline/timeline-worktree';
import { applyDeviceSnapshot, resetLaunch, visibleMini } from '../../src/device/floating-player';
import { EnvironmentFleet } from '../../src/connections/settings-b-fleet';
import { chatCanvasView } from '../../src/shell/chat-canvas-view';
import { resolvePreviewMiniPlayerFrame, resolveDeviceMiniPlayerSourceSize } from '../../src/device/previewMiniPlayerLayout';
import { deviceMini, floatMiniDevice } from '../../src/device/device-session';
import { toasts } from '../../src/shell/toast';
import { setLayoutFrames } from '../support/layout-frames';
import { withTimers } from '../support/timers';

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
    primaryAt('http://127.0.0.1:1', 'a'); // the draft's machine is this Mac's embedded server
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
      return { ok: true, value: {}, generation: 1 };
    } } as Native;
    const client = scratchDraft();
    expect(await withTimers(() => runOnEnvironment(client, native, 'c', source))).toEqual({ status: null, generation: -1 });
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Could not switch machine', SCRATCH_NOT_LOADED]]);
    expect(client.local.drafts).toEqual({ 'a:new:sa': 'no project text' });
  });

  test('openScratch waits up to 10 s: rereads every half second, then fails', async () => {
    let reads = 0;
    const found = await withTimers(async () => {
      const start = Date.now();
      await awaitScratchProject(() => reads >= 3, async () => { reads++; });
      return Date.now() - start;
    });
    expect([reads, found]).toEqual([3, 1_500]);
    reads = 0;
    const [error, waited] = await withTimers(async () => {
      const start = Date.now();
      return [await awaitScratchProject(() => false, async () => { reads++; }).catch((failure: unknown) => failure), Date.now() - start] as const;
    });
    expect((error as Error).message).toBe(SCRATCH_NOT_LOADED);
    expect([reads, waited]).toEqual([20, 10_000]);
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
  const apply = (client: T3Client, snapshot: Obj, sheet = false) => applyDeviceSnapshot(client, 'env', snapshot, { sheet, autoShowFloatingPreview: true }, new EnvironmentFleet());
  test('the environment\'s first device state is a baseline; a session opened after it floats over its thread', () => {
    resetLaunch();
    const first = thread();
    apply(first, state([open]));
    expect(deviceMini(first, state([open])).show).toBe(false);
    apply(first, state([]));
    apply(first, state([open]));
    expect(deviceMini(first, state([open]))).toMatchObject({ show: true, name: 'iPhone 18 Pro', deviceId: 'IPHONE' });
  });
  test('a new session floats, a closed one takes the player with it, a sheet layout floats nothing', () => {
    resetLaunch();
    const client = thread();
    apply(client, state([]));
    apply(client, state([open]));
    expect(deviceMini(client, state([open])).show).toBe(true);
    apply(client, state([]));
    expect(deviceMini(client, state([])).show).toBe(false);
    resetLaunch();
    const narrow = thread();
    apply(narrow, state([]), true);
    apply(narrow, state([open]), true);
    expect(deviceMini(narrow, state([open])).show).toBe(false);
  });
  test('the player hides while the panel shows the same device (shouldRenderPreviewMiniPlayer)', () => {
    const client = thread();
    floatMiniDevice(client, 't1', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    const mini = deviceMini(client, state([open]));
    expect(visibleMini(mini, { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: '' }).show).toBe(false);
    expect(visibleMini(mini, { hostId: 'local', deviceId: 'OTHER', platform: 'ios', name: '' }).show).toBe(true);
    expect(visibleMini(mini, undefined).show).toBe(true);
  });
  test('a new player is at least 240 wide at the source aspect and starts beside the composer (resolvePreviewMiniPlayerFrame, chatCanvasLayout)', async () => {
    // previewMiniPlayerLayout.test.ts "floats a phone at the minimum width rather than the default box".
    expect(resolvePreviewMiniPlayerFrame({ width: null, position: null, source: resolveDeviceMiniPlayerSourceSize('ios', null), container: { width: 1_000, height: 700 } })).toMatchObject({ width: 240, height: 520 });
    // chatCanvasLayout.ts "New players start beside the composer": its bottom 12 above the canvas's, 12 from the right edge.
    const client = { ...thread(), draftKey: 'env:t1', local: {}, presentation: {} } as unknown as T3Client;
    setLayoutFrames(client, { chat: [400, 0, 740, 840], overlay: [400, 668, 740, 172] });
    floatMiniDevice(client, 't1', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    const canvas = await chatCanvasView(client, null, { width: 740, viewportHeight: 840, detailsInline: false, chatMax: 736, overlaid: true, gesture: '' });
    // 740 pt leaves no readable lane beside it, so it rises above the composer and overlaps messages instead.
    expect([canvas.mini.width, canvas.mini.height, canvas.mini.x + canvas.mini.width, canvas.overlapsChat]).toEqual([240, 520, 740 - 12, true]);
    expect(canvas.mini.top + canvas.mini.height).toBeLessThanOrEqual(668 - 12);
    // A landscape source at the default box: 320 × 180 at the bottom right of a wide canvas.
    const wide = { ...thread(), draftKey: 'env:t1', local: {}, presentation: { deviceStreams: { 'local\u0000IPHONE': { width: 1920, height: 1080, orientation: 'landscape_left' } } } } as unknown as T3Client;
    setLayoutFrames(wide, { chat: [0, 0, 1600, 840], overlay: [432, 668, 736, 172] });
    floatMiniDevice(wide, 't1', { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' });
    expect((await chatCanvasView(wide, null, { width: 1600, viewportHeight: 840, detailsInline: false, chatMax: 736, overlaid: true, gesture: '' })).mini)
      .toMatchObject({ width: 320, height: 180, x: 1600 - 12 - 320, top: 840 - 12 - 180 });
  });
});

describe('the panel tab strip scrolls when its tabs overflow (RightPanelTabs ScrollArea)', () => {
  const { tabStrip, activeSerial } = require('../../src/panel/tab-strip') as typeof import('../../src/panel/tab-strip');
  test('overflow, the furthest offset and the active tab come from the anchors', () => {
    const anchors = { 'tab-strip-view': [0, 200], 'tab-strip-content': [0, 200], 'tab-chip:diff': [270, 52], 'tab-chip:a': [0, 90] };
    expect(tabStrip({ anchors }, ['a', 'diff'], 'diff', 2)).toEqual({ overflow: true, view: 200, content: 350, max: 150, activeLeft: 270, activeRight: 322, serial: 2 });
    expect(tabStrip({ anchors: { 'tab-strip-view': [0, 200], 'tab-chip:a': [0, 90] } }, ['a'], 'a').overflow).toBe(false);
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
