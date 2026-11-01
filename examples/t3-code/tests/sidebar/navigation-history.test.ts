// Back and Forward from the sidebar header and a mouse's side buttons (T3 Code 2359643cff, MIT, see LICENSE-T3:
// navigationHistory.ts, sidebar/SidebarChrome.tsx HistoryNavigationButton; desktop window/DesktopWindow.ts).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { navigationHistory } from '../../src/sidebar/sidebar-history';
import { brandMarkWidth } from '../../src/sidebar/sidebar-min-width';
import { contractComponent } from '../support/contract-action-fixture';

const at = '2026-11-01T00:00:00.000Z';
function client(): T3Client {
  const c = new T3Client();
  c.shell = { projects: [{ id: 'p1', title: 'notes', workspaceRoot: '/notes' }], threads: ['a', 'b'].map(id => ({ id, projectId: 'p1', title: id, createdAt: at, updatedAt: at, archivedAt: null })), sequence: 1 };
  c.config = { keybindings: [{ command: 'navigation.back', shortcut: { key: '[', modKey: true } }, { command: 'navigation.forward', shortcut: { key: ']', modKey: true } }] };
  return c;
}
const ways = (c: T3Client, page = '', settings = false) => { const h = navigationHistory(c, page, settings); return [`${h.backKind}:${h.backTarget}`, `${h.forwardKind}:${h.forwardTarget}`]; };

describe('the window history the sidebar buttons step', () => {
  test('Go back and Go forward are disabled at the history\'s ends and step the visits between', () => {
    const c = client();
    c.threadId = 'a';
    expect(ways(c)).toEqual([':', ':']);
    c.threadId = 'b';
    expect(ways(c)).toEqual(['thread:a', ':']);
    // Arriving at the visit before is what Back does; Forward goes to the one it left.
    c.threadId = 'a';
    expect(ways(c)).toEqual([':', 'thread:b']);
    // A page is a visit too, and another visit drops what was ahead.
    expect(ways(c, 'usage')).toEqual(['thread:a', ':']);
  });
  test('an archived thread is no step', () => {
    const c = client();
    c.threadId = 'a'; ways(c);
    c.threadId = 'b';
    c.shell.threads[0]!.archivedAt = at;
    expect(ways(c)).toEqual([':', ':']);
  });
  test('Settings is a location: Back leaves it', () => {
    const c = client();
    c.threadId = 'a';
    expect(ways(c, '', true)).toEqual(['back:', ':']);
  });
  test('the tooltips name the shortcuts, and the buttons follow the brand mark', () => {
    const c = client();
    expect(navigationHistory(c, '', false)).toMatchObject({ backTip: 'Go back (⌘[)', forwardTip: 'Go forward (⌘])', brandWidth: brandMarkWidth(16) });
    c.config = { keybindings: [] };
    expect(navigationHistory(c, '', false)).toMatchObject({ backTip: 'Go back', forwardTip: 'Go forward' });
  });
});

describe('a step runs what ⌘[ and ⌘] run for its target', () => {
  const members = contractComponent('sidebar-members.contract', 'T3Code');
  const run = (direction: string, kind: string, target: string, extra = '') => {
    const calls: unknown[][] = [];
    const record = (name: string) => (...args: unknown[]) => { calls.push([name, ...args]); };
    const navigationHistory = { backKind: kind, backTarget: target, backExtra: extra, forwardKind: kind, forwardTarget: target, forwardExtra: extra };
    members.call('historyGo', {}, { navigationHistory, selectThread: record('selectThread'), closeModal: record('closeModal'), paletteRun: record('paletteRun') }, [direction]);
    return calls;
  };
  test('each kind', () => {
    expect(run('back', 'thread', 'a')).toEqual([['selectThread', 'a']]);
    expect(run('back', 'back', '')).toEqual([['closeModal']]);
    expect(run('forward', 'palette-run', 'settings', 'general')).toEqual([['paletteRun', 'settings', 'general', '']]);
    expect(run('forward', 'new-thread-in', 'p1')).toEqual([['paletteRun', 'new-thread', 'p1', '']]);
    expect(run('forward', '', '')).toEqual([]);
  });
});

describe('a mouse\'s back and forward buttons', () => {
  const window = contractComponent('app-window.contract', 'T3Window');
  const release = (button: number, scope: Record<string, unknown> = {}) => {
    const steps: unknown[] = [];
    window.call('outsidePressUp', { outsidePresses: 0, outsideDown: false }, { paletteOpen: false, modal: false, settingsOpen: false, historyGo: (way: string) => { steps.push(way); }, ...scope }, [{ button, buttons: 0 }]);
    return steps;
  };
  test('step the history at their release', () => {
    expect(release(3)).toEqual(['back']);
    expect(release(4)).toEqual(['forward']);
    expect(release(0)).toEqual([]);
    expect(release(1)).toEqual([]);
  });
  test('not under the palette or a dialog, but in Settings', () => {
    expect(release(3, { paletteOpen: true })).toEqual([]);
    expect(release(3, { modal: true })).toEqual([]);
    expect(release(3, { modal: true, settingsOpen: true })).toEqual(['back']);
  });
});
