// Round-3 palette lane: New thread in... (projectThreadItems at f90b77d) and
// pullRequest.copyNumber dispatch from the open pull request detail panel.
import { describe, test, expect } from 'bun:test';
import { T3Client } from './client';
import type { Obj } from './domain';
import { commandView, newThreadInItems } from './palette';
import { keyboardDispatch, keyboardDispatchSource } from './keyboard-dispatch';

const NOW = Date.parse('2026-10-03T12:00:00.000Z');
const binding = (key: string, command: string, extra: Obj = {}, when?: Obj): Obj =>
  ({ command, shortcut: { key, modKey: true, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...extra }, ...(when ? { whenAst: when } : {}) });
const desktop = { type: 'identifier', name: 'isDesktop' };
const notTerminal = { type: 'not', node: { type: 'identifier', name: 'terminalFocus' } };
function client(scratch = ''): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1' });
  c.config = { environment: { label: 'Local', capabilities: {} }, scratchWorkspaceRoot: scratch, settings: {},
    keybindings: [binding('k', 'commandPalette.toggle'), binding('1', 'thread.jump.1', {}, desktop), binding('2', 'thread.jump.2', {}, desktop),
      binding('n', 'chat.newWithoutProject', { altKey: true }), binding('k', 'pullRequest.copyNumber', { shiftKey: true }, notTerminal)] };
  c.shell = { sequence: 1, projects: [
    { id: 'p1', title: 'Parity fixture', workspaceRoot: '/repo', updatedAt: '2026-10-01T00:00:00.000Z' },
    { id: 'p2', title: 'Single checkout two', workspaceRoot: '/two', updatedAt: '2026-10-01T00:00:00.000Z' },
    { id: 'p3', title: 'Scratch', workspaceRoot: '/scratch/', updatedAt: '2026-10-03T00:00:00.000Z' }],
  threads: [{ id: 't1', projectId: 'p1', title: 'old', updatedAt: '2026-10-02T01:00:00.000Z' },
    { id: 't2', projectId: 'p2', title: 'new', updatedAt: '2026-10-03T02:00:00.000Z' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}

describe('New thread in...', () => {
  test('the current project first, then by activity; no thread-jump hints', () => {
    const rows = commandView(client(), { page: 'new-thread-in', query: '', now: NOW, scheme: 'dark', matches: new Map(), matchQuery: '', searching: false }).rows;
    expect(rows.map(entry => entry.title)).toEqual(['Parity fixture', 'Single checkout two', 'Scratch']);
    expect(rows.every(entry => entry.shortcut === '')).toBe(true);
    expect(rows[0]!.header).toBe('Projects');
    expect(rows.map(entry => entry.index)).toEqual([0, 1, 2]);
  });
  test('the Scratch project is the trailing "No project" when the server offers one', () => {
    const items = newThreadInItems(client('/scratch'));
    expect(items.map(item => item.row.title)).toEqual(['Parity fixture', 'Single checkout two', 'No project']);
    expect(items[2]!.row).toMatchObject({ op: 'flow', arg: 'scratch', icon: 'message-square-dashed', shortcut: '' });
    const c = client('/scratch'); c.projectId = 'p2';
    expect(newThreadInItems(c).map(item => item.row.arg)).toEqual(['p2', 'p1', 'scratch']);
  });
});

describe('pullRequest.copyNumber', () => {
  const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
  test('only while a pull request detail panel shows a number', () => {
    const c = client();
    expect(keyboardDispatch(c, [], '', '', context).some(item => item.command === 'pullRequest.copyNumber')).toBe(false);
    const item = keyboardDispatch(c, [], '', '', { ...context, prNumber: '#42' }).find(entry => entry.command === 'pullRequest.copyNumber');
    expect(item).toMatchObject({ kind: 'pr-copy', chord: 'Meta+Shift+k', target: 'PR number', extra: '#42', label: 'Copy PR Number' });
    // The palette open (isCommandPaletteOpen) or a modal leaves it out.
    expect(keyboardDispatch(c, [], '', '', { ...context, prNumber: '#42', paletteOpen: true }).some(entry => entry.command === 'pullRequest.copyNumber')).toBe(false);
    expect(keyboardDispatch(c, [], '', '', { ...context, prNumber: '#42', modalOpen: true }).length).toBe(0);
  });
  test('the resource passes the number as its fourteenth argument', () => {
    const args = [[], '', '', false, false, false, false, false, false, false, 1, false, 'command', '#7'];
    expect(keyboardDispatchSource(client(), args).find(entry => entry.command === 'pullRequest.copyNumber')!.extra).toBe('#7');
    expect(keyboardDispatchSource(client(), args.slice(0, 13)).some(entry => entry.command === 'pullRequest.copyNumber')).toBe(false);
  });
});
