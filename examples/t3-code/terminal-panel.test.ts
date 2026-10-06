import { describe, expect, test } from 'bun:test';
import { activateTerminalPane, openTerminalSurface, removePanelTerminal, splitTerminalSurface } from './terminal-panel';
import { savedPanel, adoptRightPanels } from './r10-device-panels';
import { closePanelSurfaces, registerSurfaceClose } from './right-panel-tabs';
import type { PanelState, Surface } from './r4-surfaces-panel';
const surface = (): Surface => ({ id: 'terminal:1', kind: 'terminal', path: '', line: 0, reveal: 0,
  terminal: { terminalIds: ['1', '2', '3'], activeTerminalId: '2', splitDirection: 'vertical' } });
const panel = (): PanelState => ({ surfaces: [surface(), { id: 'files', kind: 'files', path: '', line: 0, reveal: 0 }], active: 'terminal:1', visible: true, userRevision: 1 });
describe('right-panel terminals', () => {
  test('closing the active split selects the last pane; final pane selects neighbouring surface', () => {
    const state = panel(), terminal = state.surfaces[0]!;
    removePanelTerminal(state, terminal, '2');
    expect(terminal.terminal?.activeTerminalId).toBe('3');
    removePanelTerminal(state, terminal, '3'); removePanelTerminal(state, terminal, '1');
    expect(state.active).toBe('files'); expect(state.visible).toBe(true);
  });
  test('restoration keeps split order, direction and active pane', () => {
    const state = panel(), saved = savedPanel(state);
    const next: { rightPanels?: unknown } = {};
    adoptRightPanels(next, { rightPanels: { 'env:thread': saved } });
    expect(next.rightPanels).toEqual({ 'env:thread': saved });
    expect(saved?.surfaces[0]).toMatchObject({ terminal: surface().terminal });
  });
  test('single close guards the entire surface once; bulk close cleans up without a guard', async () => {
    const owner = {}, state = panel(); let guards = 0; const deleted: string[] = [];
    registerSurfaceClose(owner, 'terminal', { guard: () => { guards++; return false; }, cleanup: entry => { deleted.push(...entry.terminal!.terminalIds); } });
    await closePanelSurfaces(owner, state, 'close', 'terminal:1');
    expect(guards).toBe(1); expect(deleted).toEqual([]);
    await closePanelSurfaces(owner, state, 'close-all', '');
    expect(guards).toBe(1); expect(deleted).toEqual(['1', '2', '3']); expect(state.visible).toBe(false);
  });
});

import { T3Client } from './client';
import { addTerminalSurface, terminalPanelLocal, terminalPanelOps, terminalPanelIds, terminalPanelRetained } from './terminal-panel';
import { focusedTerminal } from './terminal-focus';
import { panelState, surfaceLocal, surfaceStore } from './r4-surfaces-panel';
import { terminalUiStore } from './terminal-ui-state';
import type { Files, Native } from './protocol';
function clientFixture() {
  const client = new T3Client();
  client.environmentId = 'env'; client.threadId = 'thread'; client.projectId = 'project';
  client.shell.projects = [{ id: 'project', workspaceRoot: '/repo' }];
  client.shell.threads = [{ id: 'thread', projectId: 'project', worktreePath: '/repo/tree' }];
  const calls: { method: string; payload: unknown }[] = [];
  client.request = async (_native, method, payload) => { calls.push({ method, payload }); return {}; };
  return { client, calls, native: { available: true } as Native };
}
test('panel splits stop at four; new surfaces and drawer IDs never collide', async () => {
  const { client, calls, native } = clientFixture(), ref = { environmentId: 'env', threadId: 'thread' };
  terminalUiStore(client).getState().ensureTerminal(ref, 'term-1', { open: true });
  await addTerminalSurface(client, native);
  for (let i = 0; i < 4; i++) await terminalPanelLocal(client, native, 'split-vertical', 'env:thread|term-2', '');
  expect(terminalPanelIds(client, ref)).toEqual(['term-2', 'term-3', 'term-4', 'term-5']);
  expect(panelState(client).surfaces[0]?.terminal?.splitDirection).toBe('vertical');
  expect(calls).toHaveLength(4);
  await addTerminalSurface(client, native);
  expect(terminalPanelIds(client, ref)).toEqual(['term-2', 'term-3', 'term-4', 'term-5', 'term-6']);
  expect(terminalPanelRetained(client)).toHaveLength(5);
  expect(calls[0]).toMatchObject({ method: 'terminal.open', payload: { cwd: '/repo/tree', worktreePath: '/repo/tree', env: { T3CODE_PROJECT_ROOT: '/repo', T3CODE_WORKTREE_PATH: '/repo/tree' } } });
});
test('single surface close queues all names once; confirmed and bulk closes delete every history', async () => {
  const { client, calls, native } = clientFixture();
  await addTerminalSurface(client, native);
  await terminalPanelLocal(client, native, 'split', 'env:thread|term-1', '');
  await surfaceLocal(client, native, 'close', 'terminal:term-1', '');
  expect(surfaceStore(client).terminalClose).toMatchObject({ serial: 1, title: 'Close 2 terminals?', body: 'This stops their running processes and clears their histories: "Terminal 1", "Terminal 2".' });
  expect(calls).toHaveLength(2);
  await surfaceLocal(client, native, 'close-confirmed', 'terminal:term-1', '');
  expect(calls.slice(2)).toEqual(['term-1', 'term-2'].map(terminalId => ({ method: 'terminal.close', payload: { threadId: 'thread', terminalId, deleteHistory: true } })));
  expect(panelState(client).visible).toBe(false);
  await addTerminalSurface(client, native);
  await surfaceLocal(client, native, 'close-all', '', '');
  expect(surfaceStore(client).terminalClose.serial).toBe(1);
  expect(calls.at(-1)).toMatchObject({ method: 'terminal.close', payload: { deleteHistory: true } });
});
test('accepting a queued close after switching threads closes only the original sessions', async () => {
  const { client, calls, native } = clientFixture();
  await addTerminalSurface(client, native);
  await surfaceLocal(client, native, 'close', 'terminal:term-1', '');
  const target = surfaceStore(client).terminalClose.target;
  client.threadId = 'other';
  await addTerminalSurface(client, native);
  await surfaceLocal(client, native, 'close-confirmed', target, '');
  expect(panelState(client).surfaces).toHaveLength(1);
  expect(calls.at(-1)).toEqual({ method: 'terminal.close', payload: { threadId: 'thread', terminalId: 'term-1', deleteHistory: true } });
});
test('draft right-panel sessions share reserved identity and survive first send', async () => {
  const { client, native } = clientFixture(); client.threadId = '';
  client.restAccess = () => ({ ids: async () => ['reserved'] }) as ReturnType<T3Client['restAccess']>;
  await addTerminalSurface(client, native);
  expect(terminalPanelIds(client, { environmentId: 'env', threadId: 'reserved' })).toEqual(['term-1']);
  const before = panelState(client);
  client.threadId = 'reserved';
  expect(panelState(client)).toBe(before);
});


test('native pane commands use emitting surface while another surface is active', async () => {
  const { client, calls, native } = clientFixture();
  await addTerminalSurface(client, native); await addTerminalSurface(client, native);
  const result = { message: '', id: '', value: '' };
  const message = { type: 'command', command: 'terminal.split', environmentId: 'env', threadId: 'thread', terminalId: 'term-1', surface: 'terminal:term-1' };
  await terminalPanelOps.call(client, 'terminalpanellocal:message', 'env:thread|term-1', JSON.stringify(message), 0, native, {} as Files, result);
  expect(panelState(client).surfaces[0]?.terminal?.terminalIds).toEqual(['term-1', 'term-3']);
  expect(panelState(client).surfaces[1]?.terminal?.terminalIds).toEqual(['term-2']);
  await terminalPanelOps.call(client, 'terminalpanellocal:message', 'env:thread|term-1', JSON.stringify({ ...message, command: 'terminal.close' }), 0, native, {} as Files, result);
  expect(surfaceStore(client).terminalClose).toMatchObject({ op: 'terminalpanellocal:close', target: 'env:thread|term-1', title: 'Close terminal "Terminal 1"?' });
  expect(calls.filter(call => call.method === 'terminal.close')).toEqual([]);
});


test('panel dedicated focus operations preserve surface identity', async () => {
  const { client, native } = clientFixture();
  await addTerminalSurface(client, native);
  await terminalPanelLocal(client, native, 'focus-in', 'env:thread|term-1', 'terminal:term-1');
  expect(focusedTerminal(client)).toEqual({ environmentId: 'env', threadId: 'thread', terminalId: 'term-1', surface: 'terminal:term-1' });
  await terminalPanelLocal(client, native, 'focus-out', 'env:thread|term-1', 'drawer');
  expect(focusedTerminal(client)?.surface).toBe('terminal:term-1');
  await terminalPanelLocal(client, native, 'focus-out', 'env:thread|term-1', 'terminal:term-1');
  expect(focusedTerminal(client)).toBeNull();
});

// T3 Code 1e2ecbd975 rightPanelStore.test.ts:910-982 over the clone's PanelState (isOpen → visible,
// activeSurfaceId → active, resourceId and the optional splitDirection → the `terminal` record).
describe('rightPanelStore', () => {
  const empty = (): PanelState => ({ surfaces: [], active: '', visible: false, userRevision: 0 });
  test('tracks one surface per terminal session', () => {
    const state = empty();
    openTerminalSurface(state, 'term-1'); openTerminalSurface(state, 'term-2');
    expect(state.surfaces.map(entry => [entry.id, entry.kind, entry.terminal])).toEqual([
      ['terminal:term-1', 'terminal', { terminalIds: ['term-1'], activeTerminalId: 'term-1', splitDirection: 'horizontal' }],
      ['terminal:term-2', 'terminal', { terminalIds: ['term-2'], activeTerminalId: 'term-2', splitDirection: 'horizontal' }]]);
    expect(state.active).toBe('terminal:term-2');
  });
  test('tracks split panes and the active pane within a terminal surface', () => {
    const state = empty();
    openTerminalSurface(state, 'term-1'); splitTerminalSurface(state, 'terminal:term-1', 'term-2');
    expect(state.surfaces[0]?.terminal).toEqual({ terminalIds: ['term-1', 'term-2'], activeTerminalId: 'term-2', splitDirection: 'horizontal' });
    activateTerminalPane(state, 'terminal:term-1', 'term-1'); removePanelTerminal(state, state.surfaces[0]!, 'term-1');
    expect(state.surfaces[0]?.terminal).toEqual({ terminalIds: ['term-2'], activeTerminalId: 'term-2', splitDirection: 'horizontal' });
  });
  test('tracks vertical layout for a terminal surface', () => {
    const state = empty();
    openTerminalSurface(state, 'term-1'); splitTerminalSurface(state, 'terminal:term-1', 'term-2', 'vertical');
    expect(state.surfaces[0]?.terminal).toEqual({ terminalIds: ['term-1', 'term-2'], activeTerminalId: 'term-2', splitDirection: 'vertical' });
    expect(state.active).toBe('terminal:term-1');
  });
  test('closing the final terminal pane removes its surface and closes the panel', () => {
    const state = empty();
    removePanelTerminal(state, openTerminalSurface(state, 'term-1'), 'term-1');
    expect(state).toMatchObject({ visible: false, active: '', surfaces: [] });
  });
});
