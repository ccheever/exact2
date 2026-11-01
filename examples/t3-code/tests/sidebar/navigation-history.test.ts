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
  test('each visit has a location of its own for the router: the first is home\'s, Settings is one over its visit', () => {
    const c = client();
    const hrefs = (page = '', settings = false) => { const h = navigationHistory(c, page, settings); return [h.here, h.backHref, h.forwardHref]; };
    expect(hrefs()).toEqual(['/', '', '']);
    c.threadId = 'a';
    expect(hrefs()).toEqual(['/', '', '']);
    c.threadId = 'b';
    expect(hrefs()).toEqual(['/visit/1', '/', '']);
    c.threadId = 'a';
    expect(hrefs()).toEqual(['/', '', '/visit/1']);
    // A new visit is a new location, even where a visit it dropped was.
    expect(hrefs('usage')).toEqual(['/visit/2', '/', '']);
    expect(hrefs('usage', true)).toEqual(['/visit/settings', '/visit/2', '']);
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
  // The router as far as these actions use it: the selected tab's stack of locations. As the runner's, a push lands on
  // the selected tab whatever route it names, and `open` selects the location's own tab.
  type Router = { tab: string; stack: string[] };
  const tabOf = (location: string) => location.startsWith('/open') ? 'incoming' : location.startsWith('/notification') ? 'notification' : 'home';
  const go = (r: Router, location: string): Router => !location ? r : r.stack.includes(location) ? { ...r, stack: r.stack.slice(0, r.stack.indexOf(location) + 1) } : { ...r, stack: [...r.stack, location] };
  const router = { go, open: (r: Router, location: string): Router => ({ tab: tabOf(location), stack: [location] }), top: (r: Router) => ({ url: r.stack.at(-1), tab: r.tab }),
    stack: (r: Router) => r.stack.map(url => ({ url })) };
  const shut = { paletteOpen: false, modal: false, settingsOpen: false };
  const run = (direction: string, kind: string, target: string, extra = '', hrefs: Record<string, string> = {}, scope: Record<string, unknown> = {}) => {
    const calls: unknown[][] = [];
    const record = (name: string) => (...args: unknown[]) => { calls.push([name, ...args]); };
    const navigationHistory = { backKind: kind, backTarget: target, backExtra: extra, forwardKind: kind, forwardTarget: target, forwardExtra: extra, backHref: '', forwardHref: '', ...hrefs };
    const state = { promptLinks: { tab: 'home', stack: ['/', '/visit/1', '/visit/2'] } as Router };
    members.call('historyGo', state, { ...router, ...shut, navigationHistory, selectThread: record('selectThread'), closeModal: record('closeModal'), paletteRun: record('paletteRun'), ...scope }, [direction]);
    return { calls, router: state.promptLinks };
  };
  test('each kind', () => {
    expect(run('back', 'thread', 'a').calls).toEqual([['selectThread', 'a']]);
    expect(run('back', 'back', '').calls).toEqual([['closeModal']]);
    expect(run('forward', 'palette-run', 'settings', 'general').calls).toEqual([['paletteRun', 'settings', 'general', '']]);
    expect(run('forward', 'new-thread-in', 'p1').calls).toEqual([['paletteRun', 'new-thread', 'p1', '']]);
    expect(run('forward', '', '').calls).toEqual([]);
  });
  test('a step moves the router: Back pops to the visit beneath, Forward pushes the one ahead', () => {
    expect(run('back', 'thread', 'a', '', { backHref: '/visit/1' }).router.stack).toEqual(['/', '/visit/1']);
    expect(run('forward', 'thread', 'b', '', { forwardHref: '/visit/3' }).router.stack).toEqual(['/', '/visit/1', '/visit/2', '/visit/3']);
  });
  test('the router\'s navigate: a step\'s location takes that step, a link opens its tab, any other location nothing', () => {
    expect(run('/visit/1', 'thread', 'a', '', { backHref: '/visit/1' })).toEqual({ calls: [['selectThread', 'a']], router: { tab: 'home', stack: ['/', '/visit/1'] } });
    expect(run('/visit/3', 'thread', 'b', '', { backHref: '/visit/1', forwardHref: '/visit/3' }).calls).toEqual([['selectThread', 'b']]);
    // A visit the history cannot show (an archived thread's: no step) leaves the router where it is.
    expect(run('/visit/1', 'thread', 'a')).toEqual({ calls: [], router: { tab: 'home', stack: ['/', '/visit/1', '/visit/2'] } });
    // A link opens its own tab, also while home has visits, and leaves them (open selects; home's stack is not its).
    expect(run('/open?u=x', 'thread', 'a', '', { backHref: '/visit/1' })).toEqual({ calls: [], router: { tab: 'incoming', stack: ['/open?u=x'] } });
    expect(run('/notification?t=a', '', '').router.tab).toBe('notification');
  });
  test('not under the palette or a dialog other than Settings, as ⌘[ and ⌘]; a link still opens', () => {
    expect(run('back', 'thread', 'a', '', { backHref: '/visit/1' }, { paletteOpen: true, modal: true }).calls).toEqual([]);
    expect(run('/visit/1', 'thread', 'a', '', { backHref: '/visit/1' }, { modal: true }).router.stack).toEqual(['/', '/visit/1', '/visit/2']);
    expect(run('back', 'back', '', '', { backHref: '/visit/1' }, { modal: true, settingsOpen: true }).calls).toEqual([['closeModal']]);
    expect(run('/open?u=x', '', '', '', {}, { paletteOpen: true, modal: true }).router.tab).toBe('incoming');
  });
  test('each visit is mirrored onto the router, a new one over the visit beneath it in the history', () => {
    const state = { promptLinks: { tab: 'home', stack: ['/', '/visit/1', '/visit/2'] } as Router };
    const record = (here: string, backHref = '') => { members.call('recordVisit', state, { ...router, navigationHistory: { here, backHref } }, []); return state.promptLinks.stack; };
    expect(record('/visit/1', '/')).toEqual(['/', '/visit/1']);
    expect(record('/visit/4', '/visit/1')).toEqual(['/', '/visit/1', '/visit/4']);
    expect(record('/visit/settings', '/visit/4')).toEqual(['/', '/visit/1', '/visit/4', '/visit/settings']);
    // Settings closed and a new visit made in one turn: Settings is not left beneath it.
    expect(record('/visit/5', '/visit/4')).toEqual(['/', '/visit/1', '/visit/4', '/visit/5']);
  });
});

describe('a mouse\'s back and forward buttons', () => {
  const window = contractComponent('app-window.contract', 'T3Window');
  const release = (button: number, scope: Record<string, unknown> = {}) => {
    const steps: unknown[] = [];
    window.call('outsidePressUp', { outsidePresses: 0, outsideDown: false }, { paletteOpen: false, modal: false, settingsOpen: false, historyGo: (way: string) => { steps.push(way); }, preventDefault: () => { steps.push('prevented'); }, ...scope }, [{ button, buttons: 0 }]);
    return steps;
  };
  test('step the history at their release', () => {
    // The host would step the router for them too: the release's preventDefault leaves that to the window.
    expect(release(3)).toEqual(['prevented', 'back']);
    expect(release(4)).toEqual(['prevented', 'forward']);
    expect(release(0)).toEqual([]);
    expect(release(1)).toEqual([]);
  });
});
