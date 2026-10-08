// Lane r8-keys: keyboard and menu fixes from the real-keyboard pass (D1, D2, D4, D6).
import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import { keyboardDispatch, keyboardDispatchSource } from './keyboard-dispatch';
import { keyboardSettings } from './keybinding-settings';
import { claimBoldChord, isBoldChord, richTextComposer } from './r8-keys-chords';
import { placeTableMenu, tableMenuAction, tableMenuSnapshot, TABLE_MENU_HEIGHT } from './r8-keys-table-menu';
import { chatLocal } from './timeline-presentation';
import { sidebarSnapshot } from './sidebar-view';
import { sidebarSession } from './sidebar-state';

const bindings = [
  { command: 'sidebar.toggle', shortcut: { key: 'b', modKey: true } },
  { command: 'diff.toggle', shortcut: { key: 'd', modKey: true } },
  { command: 'composer.effort', shortcut: { key: 'e', modKey: true, shiftKey: true } },
  { command: 'composer.mode', shortcut: { key: 'a', modKey: true, shiftKey: true } },
  { command: 'rightPanel.toggle', shortcut: { key: 'b', modKey: true, altKey: true } },
  { command: 'thread.undo', shortcut: { key: 'z', modKey: true } },
  { command: 'chat.new', shortcut: { key: 'n', modKey: true } },
  { command: 'rightPanel.close', shortcut: { key: 'w', modKey: true } },
  ...Array.from({ length: 9 }, (_, index) => ({ command: `thread.jump.${index + 1}`, shortcut: { key: String(index + 1), modKey: true } })),
];
function client(): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1', scopes: ['orchestration:read', 'orchestration:operate'] });
  c.config = { environment: { label: 'Local', capabilities: {} }, keybindings: bindings, settings: {} };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', title: 'One', updatedAt: '2026-10-03T01:00:00.000Z' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}
const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
class Fake implements Native {
  available = true; calls: Obj[] = [];
  watch() {}
  async later(input: unknown): Promise<unknown> { this.calls.push(obj(input)); return { ok: true, generation: 1, value: {} }; }
}

describe('D1: ⌘B in the rich-text composer bolds', () => {
  test('isRichTextBoldShortcut over chords', () => {
    expect(isBoldChord('Meta+b')).toBe(true);
    expect(isBoldChord('Control+B')).toBe(true);
    expect(isBoldChord('Meta+Shift+b')).toBe(false);
    expect(isBoldChord('Meta+Alt+b')).toBe(false);
    expect(isBoldChord('Meta+n')).toBe(false);
  });
  test('the sidebar toggle gives up Meta+b only while the rich-text composer has the focus', () => {
    const c = client();
    expect(richTextComposer(c)).toBe(true);
    const keys = (focus: boolean, rich = true) => claimBoldChord(keyboardSettings(c.config, focus), focus, rich).keySidebar;
    expect(keys(false)).toBe('Meta+b');
    expect(keys(true)).toBe('');
    expect(keys(true, false)).toBe('Meta+b');
    c.local.clientSettings = { ...obj(c.local.clientSettings as Obj | undefined), composerRichTextEnabled: false } as never;
    expect(richTextComposer(c)).toBe(false);
    // A rebinding to another chord keeps it.
    c.config = { ...c.config, keybindings: [...bindings, { command: 'sidebar.toggle', shortcut: { key: 'b', modKey: true, shiftKey: true } }] };
    expect(claimBoldChord(keyboardSettings(c.config, true), true, true).keySidebar).toBe('Meta+Shift+b');
  });
});

describe('D2/D3/D12: one button per chord, ⌘D toggles the diff', () => {
  test('composer.effort and composer.mode stay on their visible buttons', () => {
    const items = keyboardDispatch(client(), [], '', '', context);
    expect(items.some(item => item.command === 'composer.effort' || item.command === 'composer.mode')).toBe(false);
    const chords = items.map(item => item.chord);
    expect(new Set(chords).size).toBe(chords.length);
  });
  test('diff.toggle opens the diff surface, or closes the panel showing it', () => {
    const c = client();
    expect(keyboardDispatch(c, [], '', '', context).find(item => item.command === 'diff.toggle')).toMatchObject({ chord: 'Meta+d', kind: 'diff-open' });
    c.diffOpen = true;
    expect(keyboardDispatch(c, [], '', '', context).find(item => item.command === 'diff.toggle')).toMatchObject({ kind: 'diff' });
    expect(keyboardDispatch(c, [], '', '', { ...context, draftThreadRoute: true }).find(item => item.command === 'diff.toggle')).toBeUndefined();
    // ⌘W closes a draft's open panel before the window (lane r9-input: its active tab, here the Diff).
    expect(keyboardDispatch(c, [], '', '', { ...context, draftThreadRoute: true, diffOpen: true }).find(item => item.command === 'rightPanel.close')).toMatchObject({ kind: 'close-surface', target: 'diff' });
  });
  test('chat.new is the window\'s, not the sidebar button\'s (it works with the sidebar closed)', () => {
    expect(keyboardDispatch(client(), [], '', '', context).find(item => item.command === 'chat.new')).toMatchObject({ chord: 'Meta+n', kind: 'chat-new' });
    expect(keyboardDispatch(client(), [], '', '', { ...context, modalOpen: true }).find(item => item.command === 'chat.new')).toBeUndefined();
  });
  test('a command with two chords gets a button per chord (each is a menu key equivalent)', () => {
    const c = client();
    c.config = { ...c.config, keybindings: [...bindings, { command: 'chat.new', shortcut: { key: 'o', modKey: true, shiftKey: true } }] };
    const items = keyboardDispatch(c, [], '', '', context).filter(item => item.command === 'chat.new');
    expect(items.map(item => item.chord).sort()).toEqual(['Meta+Shift+o', 'Meta+n']);
    expect(new Set(items.map(item => item.id)).size).toBe(2);
  });
  test('thread jumps follow the painted rows', () => {
    const rows = ['a', 'b', 'c'].map((id, index) => ({ id, section: index === 0 ? 'pinned' : 'active', selected: id === 'b' }));
    const jumps = keyboardDispatch(client(), rows, '', '', context).filter(item => item.command.startsWith('thread.jump.'));
    expect(jumps.map(item => [item.chord, item.target])).toEqual([['Meta+1', 'a'], ['Meta+2', 'b'], ['Meta+3', 'c']]);
  });
});

describe('D4: the undo notice expires', () => {
  test('thread.undo follows the window clock the contract passes', () => {
    const c = client();
    sidebarSession(c).undo = { action: 'Settled', threadIds: ['t1'], at: 1_000 };
    const args = (shown?: boolean) => [[], '', '', false, false, false, false, false, false, false, 0, false, 'command', '', ...(shown === undefined ? [] : [shown])];
    expect(keyboardDispatchSource(c, args(true)).some(item => item.command === 'thread.undo')).toBe(true);
    expect(keyboardDispatchSource(c, args(false)).some(item => item.command === 'thread.undo')).toBe(false);
  });
  test('the notice carries the wall time it ends at', () => {
    const c = client();
    const helpers = { projectIdentity: () => ({ projectMark: '', projectInk: '', projectSurface: '' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
    expect(sidebarSnapshot(c, 0, helpers).sidebar.undoUntil).toBe(0);
    sidebarSession(c).undo = { action: 'Settled', threadIds: ['t1'], at: Date.now() };
    const view = sidebarSnapshot(c, 0, helpers).sidebar;
    expect(view.undoText).toBe('Settled 1 thread,');
    expect(view.undoUntil).toBe(sidebarSession(c).undo!.at + 5000);
  });
});

describe('D6: the table Copy menu is a window-level popup', () => {
  const trigger = { x: 900, y: 200, width: 24, height: 24 };
  test('below the trigger, end-aligned, flipped above when the window has no room', () => {
    expect(placeTableMenu(trigger, 1280, 840)).toEqual({ x: 764, y: 228 });
    expect(placeTableMenu({ ...trigger, y: 790 }, 1280, 840)).toEqual({ x: 764, y: 790 - 4 - TABLE_MENU_HEIGHT });
    expect(placeTableMenu({ ...trigger, x: 40 }, 1280, 840).x).toBe(5);
    expect(placeTableMenu({ ...trigger, x: 1270 }, 1280, 840).x).toBe(1280 - 160 - 5);
  });
  test('↓ or ↑ on the Copy button opens the menu at its first or last item (fix-keyboard-focus)', async () => {
    const c = client(), native = new Fake();
    const table = '| a | b |\n|---|---|\\u0000a,b';
    await chatLocal(c, native, 'table-menu', 'tbl-1', `900|200|24|24|1280|840|keys:first\t${table}`);
    expect(tableMenuSnapshot(c).tableMenu[0]).toMatchObject({ keyed: 1, markdown: '| a | b |\n|---|---|', csv: 'a,b' });
    await chatLocal(c, native, 'table-menu-close', 'tbl-1', '');
    await chatLocal(c, native, 'table-menu', 'tbl-1', `900|200|24|24|1280|840|keys:last\t${table}`);
    expect(tableMenuSnapshot(c).tableMenu[0]).toMatchObject({ keyed: -1, markdown: '| a | b |\n|---|---|' });
    await chatLocal(c, native, 'table-menu-close', 'tbl-1', '');
    // A press asks for no item: the popup takes the focus.
    await chatLocal(c, native, 'table-menu', 'tbl-1', `900|200|24|24|1280|840|${table}`);
    expect(tableMenuSnapshot(c).tableMenu[0]).toMatchObject({ keyed: 0 });
  });
  test('open, toggle, close on copy, and per thread', async () => {
    const c = client(), native = new Fake();
    const value = '900|200|24|24|1280|840|| a | b |\n|---|---|\\u0000a,b'; // the template's raw `\u0000`
    await chatLocal(c, native, 'table-menu', 'tbl-1', value);
    expect(tableMenuSnapshot(c).tableMenu).toEqual([{ id: 'tbl-1', x: 764, y: 228, triggerX: 900, triggerY: 200, markdown: '| a | b |\n|---|---|', csv: 'a,b', keyed: 0 }]);
    await chatLocal(c, native, 'table-menu', 'tbl-1', value);
    expect(tableMenuSnapshot(c).tableMenu).toEqual([]);
    await tableMenuAction(c, 'table-menu', 'tbl-1', value);
    c.threadId = 't2';
    expect(tableMenuSnapshot(c).tableMenu).toEqual([]);
    c.threadId = 't1';
    expect(tableMenuSnapshot(c).tableMenu.length).toBe(1);
    await chatLocal(c, native, 'copy-table', 'tbl-1', 'a,b');
    expect(tableMenuSnapshot(c).tableMenu).toEqual([]);
    expect(native.calls.some(call => call.op === 'copyText' && call.text === 'a,b')).toBe(true);
    await tableMenuAction(c, 'table-menu', 'tbl-1', value);
    await chatLocal(c, native, 'table-menu-close', 'tbl-1', '');
    expect(tableMenuSnapshot(c).tableMenu).toEqual([]);
    await expect(tableMenuAction(c, 'table-menu', 'tbl-1', '0|0|0|0|1280|840|x')).rejects.toThrow('That table is unavailable.');
    // The drawn frame wins over the layout's when the module measures it (a scrolled transcript).
    const measuring: Native = { available: true, watch() {}, async later(input: unknown) { return obj(input).op === 'r8MeasureFrame' ? { ok: true, value: { x: 900, y: 150, width: 24, height: 24, windowWidth: 1280, windowHeight: 840 } } : { ok: true, value: {} }; } } as unknown as Native;
    await chatLocal(c, measuring, 'table-menu', 'tbl-1', value);
    expect(tableMenuSnapshot(c).tableMenu[0]).toMatchObject({ triggerY: 150, y: 178 });
  });
});
