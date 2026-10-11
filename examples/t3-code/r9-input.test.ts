// Lane r9-input: ⌘W closes the active surface tab, moved sidebar rows get fresh hover keys.
import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import { keyboardDispatch } from './keyboard-dispatch';
import { closeChordTarget, surfaceToClose } from './r9-input-panel';
import { panelState, surfaceLocal } from './r4-surfaces-panel';
import { rowHoverKey } from './r9-input-hover';
import { sidebarSnapshot } from './sidebar-view';
import { sidebarPrefs } from './sidebar-state';

const bindings = [
  { command: 'rightPanel.close', shortcut: { key: 'w', modKey: true } },
  { command: 'diff.toggle', shortcut: { key: 'd', modKey: true } },
];
function client(): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1', scopes: ['orchestration:read', 'orchestration:operate'] });
  c.config = { environment: { label: 'Local', capabilities: { threadSettlement: true } }, keybindings: bindings, settings: {} };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', title: 'One', updatedAt: '2026-10-03T01:00:00.000Z' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}
const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
class Fake implements Native {
  available = true; calls: Obj[] = [];
  watch() {}
  async later(input: unknown): Promise<unknown> { this.calls.push(obj(input)); return { ok: true, generation: 1, value: { entries: [], truncated: false } }; }
}
const closeRow = (c: T3Client, panelOpen: boolean, prNumber = '') =>
  keyboardDispatch(c, [], '', '', { ...context, diffOpen: panelOpen, prNumber }).find(item => item.command === 'rightPanel.close');

describe('⌘W: rightPanel.close → closeRightPanelSurface(activeRightPanelSurface)', () => {
  test('no open panel: no row, so Close Window keeps the chord', () => {
    const c = client();
    expect(closeRow(c, false)).toBeUndefined();
    expect(closeChordTarget(c, false, '')).toBeNull();
  });
  test('the active tab closes; its neighbour becomes active; the last tab closes the panel', async () => {
    const c = client(), native = new Fake();
    const state = panelState(c);
    state.surfaces = [{ id: 'files', kind: 'files', path: '', line: 0, reveal: 0 }, { id: 'file:a.ts', kind: 'file', path: 'a.ts', line: 0, reveal: 1 }, { id: 'pull-requests', kind: 'pull-requests', path: '', line: 0, reveal: 0 }];
    state.active = 'file:a.ts'; state.visible = true;
    expect(closeRow(c, true)).toMatchObject({ chord: 'Meta+w', kind: 'close-surface', target: 'file:a.ts', label: 'Close Right Panel' });
    // What the row presses: panelUi("tab-close", id) → shelllocal:surface-close.
    await surfaceLocal(c, native, 'close', 'file:a.ts', '');
    expect(state.surfaces.map(surface => surface.id)).toEqual(['files', 'pull-requests']);
    expect(state.active).toBe('pull-requests');
    expect(closeRow(c, true)).toMatchObject({ kind: 'close-surface', target: 'pull-requests' });
    await surfaceLocal(c, native, 'close', 'pull-requests', '');
    expect(surfaceToClose(c, true)).toBe('files');
    await surfaceLocal(c, native, 'close', 'files', '');
    expect(state.visible).toBe(false);
    expect(closeRow(c, false)).toBeUndefined();
  });
  test('the Diff is a tab like the others', async () => {
    const c = client();
    c.diffOpen = true;
    expect(closeRow(c, true)).toMatchObject({ kind: 'close-surface', target: 'diff' });
    await surfaceLocal(c, new Fake(), 'close', 'diff', '');
    expect(c.diffOpen).toBe(false);
    expect(panelState(c).visible).toBe(false);
  });
  test('the Pull Requests page closes its open pull request first', () => {
    const c = client();
    c.diffOpen = true;
    expect(closeRow(c, true, '#12')).toMatchObject({ kind: 'pr-close' });
  });
});

describe('a moved sidebar row never matches a stale hover', () => {
  test('the key gains a move count on each change of shelf', () => {
    const c = client();
    expect(rowHoverKey(c, 'a', 'active')).toBe('a~active');
    expect(rowHoverKey(c, 'a', 'active')).toBe('a~active');
    expect(rowHoverKey(c, 'a', 'settled')).toBe('a~settled~1');
    // ⌘Z puts it back: the hover left on "a~active#settle" stays stale.
    expect(rowHoverKey(c, 'a', 'active')).toBe('a~active~2');
    expect(rowHoverKey(c, 'b', 'active')).toBe('b~active');
  });
  test('the snapshot rows carry it', () => {
    const c = client();
    const helpers = { projectIdentity: () => ({ projectMark: 'W', projectInk: '#000', projectSurface: '#fff' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
    const now = Date.parse('2026-10-03T02:00:00.000Z');
    const first = sidebarSnapshot(c, now, helpers).threads.find(row => row.id === 't1')!;
    expect(first.hoverKey).toBe(`t1~${first.section}`);
    // Settled with the Settled shelf collapsed: the row is not painted, its move still counts.
    c.shell = { ...c.shell, threads: [{ ...c.shell.threads[0]!, settledOverride: 'settled' }] };
    c.threadId = '';
    expect(sidebarSnapshot(c, now, helpers).threads.find(row => row.id === 't1')).toBeUndefined();
    // ⌘Z: back on its shelf, under a fresh key.
    c.shell = { ...c.shell, threads: [{ ...c.shell.threads[0]!, settledOverride: null }] };
    const back = sidebarSnapshot(c, now, helpers).threads.find(row => row.id === 't1')!;
    expect(back.section).toBe(first.section);
    expect(back.hoverKey).toBe(`t1~${first.section}~2`);
    sidebarPrefs(c).settledExpanded = true;
  });
});
