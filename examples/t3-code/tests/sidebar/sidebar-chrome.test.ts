// The sidebar's chrome against T3 Code 5d3d78ab93 (MIT, see LICENSE-T3), read and run from the contracts:
// sidebar/mainAppLocation.ts isSidebarUtilityPage (the footer under Usage), SidebarUpdateCapsule.tsx (the update
// button stays inside the sidebar), ThreadStatusIndicators.tsx (a row's status label is no live region; Woke keeps
// one), SidebarThreadRow.tsx (the ⌘1–⌘9 jump hints are a Kbd in the status slot), the drag
// ghost's count (ThreadContextDragGhost.tsx `rounded-full`), SidebarThreadUndoNotice.tsx (a group: the notice is
// announced, sidebar.test.ts), the rename field's `dir="auto"` and the Working shelf's grace timers (threadInbox.ts).
import { describe, expect, test } from 'bun:test';
import { contractComponent, value } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

const line = (file: string, marker: string): string => {
  const found = appText(file).split('\n').filter(text => text.includes(marker));
  if (found.length !== 1) throw new Error(`${file}: ${found.length} lines have ${marker}`);
  return found[0]!;
};
/** An attribute's value as written on a view line: a quoted literal, or a parenthesised expression. */
const attribute = (text: string, name: string): string => {
  const at = text.indexOf(` ${name}=`);
  if (at < 0) throw new Error(`no ${name} on ${text.trim().slice(0, 60)}`);
  const rest = text.slice(at + name.length + 2);
  if (rest[0] === '"') return rest.slice(0, rest.indexOf('"', 1) + 1);
  let depth = 0;
  for (let i = 0; i < rest.length; i++) {
    if (rest[i] === '(') depth++;
    if (rest[i] === ')' && --depth === 0) return rest.slice(0, i + 1);
  }
  throw new Error(`unbalanced ${name}`);
};

describe('the footer under a page', () => {
  test.each([['sidebar.contract', 'Sidebar', { sidebar: { revealPending: '', revealRow: '', tree: [], treeCurrent: '', listTop: 0 } }]] as const)(
    '%s keeps Settings, Pull requests and Usage under Usage; the pull requests page draws Back instead', (file, name, data) => {
      const sidebar = contractComponent(file, name), state = sidebar.state();
      for (const button of ['testId="connection-settings"', 'testId="open-pull-requests"', 'testId="open-usage"']) {
        expect(sidebar.drawn(button, state, { data, min: Math.min, max: Math.max, page: 'usage', pullRequests: true })).toBe(true);
        expect(sidebar.drawn(button, state, { data, min: Math.min, max: Math.max, page: '', pullRequests: true })).toBe(true);
        expect(sidebar.drawn(button, state, { data, min: Math.min, max: Math.max, page: 'pull-requests', pullRequests: true })).toBe(false);
      }
      expect(sidebar.drawn('text "Back"', state, { data, min: Math.min, max: Math.max, page: 'usage', pullRequests: true })).toBe(false);
      expect(sidebar.drawn('text "Back"', state, { data, min: Math.min, max: Math.max, page: 'pull-requests', pullRequests: true })).toBe(true);
    });
});

test('the update button keeps its own width unless an update runs, so it stays inside the sidebar', () => {
  const flex = attribute(line('update-everything.contract', 'column margin-left="auto"'), 'flex');
  // Upstream: `shrink-0` while idle, `flex min-w-0 flex-1` while it runs. `flex: 0` (a zero basis) left the 2rem
  // button hanging past the border.
  expect(value(flex, { v: { running: false } })).toBe('none');
  expect(value(flex, { v: { running: true } })).toBe('1');
});

test('a row\'s status label is no live region; Woke keeps one', () => {
  expect(line('sidebar-row.contract', 'text t.status font-size=')).not.toContain('role="status"');
  expect(line('sidebar-row.contract', 'text "Woke"')).toContain('role="status"');
});

test('a held jump modifier draws the shortcut as a Kbd in the status slot, hiding the rest, but not over an open snooze menu', () => {
  const row = contractComponent('sidebar-row.contract', 'ThreadRow');
  const t = { id: 'a', jump: '⌘1', card: true, renaming: false, hoverKey: 'a', section: 'active', woke: false };
  const scope = { t, presets: [], KmItem: (item: object) => item, threadJumpHints: true, hoverId: '', dragId: '', dragVerb: '', sweepKeys: '', sweepVerb: '', page: '' };
  const at = (snoozeOpen: boolean) => ({ ...row.state(), snoozeOpen });
  expect(row.drawn('SidebarJumpHint(label=t.jump)', at(false), scope)).toBe(true);
  expect(row.attribute('visibility=(jumpHint', 'visibility', at(false), scope)).toBe('hidden');
  expect(row.drawn('SidebarJumpHint(label=t.jump)', at(true), scope)).toBe(false);
  expect(row.attribute('visibility=(jumpHint', 'visibility', at(true), scope)).toBe('visible');
  expect(row.drawn('SidebarJumpHint(label=t.jump)', at(false), { ...scope, threadJumpHints: false })).toBe(false);
  // ui/kbd.tsx: h-5, rounded (0.25rem), not a pill.
  const kbd = line('sidebar-row.contract', 'testId="sidebar-jump-hint"');
  expect(kbd).toContain('height="1.25rem"');
  expect(kbd).toContain('border-radius="0.25rem"');
});

test('the undo notice may shrink to nothing, its label giving way to the undo icon', () => {
  expect(line('undo-notice.contract', 'testId="sidebar-undo-notice"')).toContain('min-width=0');
  expect(line('undo-notice.contract', 'testId="sidebar-undo"')).toContain('min-width=0');
  expect(line('undo-notice.contract', 'testId="sidebar-undo"')).not.toContain('flex-shrink=0');
  // The capsule's label adds nothing to its minimum width.
  expect(line('update-everything.contract', 'text v.label flex=1')).toContain('width=0');
});

test('the project filter\'s later changes scroll the thread list to its top; the first, at mount, does not', () => {
  const sidebar = contractComponent('sidebar.contract', 'Sidebar');
  const scrolls: unknown[] = [], scope = { scrollIntoView: (...args: unknown[]) => scrolls.push(args[0]) };
  const state: Record<string, unknown> = { listTopAt: -1 };
  sidebar.call('listTopDue', state, scope, [3, 1, 0]);
  expect(scrolls).toEqual([]);
  sidebar.call('listTopDue', state, scope, [4, 1, 0]);
  expect(scrolls).toEqual(['thread-list-start']);
  expect(state.listTopAt).toBe(4);
});

test('the undo notice is a footer item, not a live region: the shell\'s announcer speaks it', () => {
  expect(line('undo-notice.contract', 'testId="sidebar-undo-notice"')).toContain('role="listitem"');
  expect(line('undo-notice.contract', 'testId="sidebar-undo-notice"')).not.toContain('aria-live');
});

test('the drag ghost\'s count is a pill', () => {
  const count = line('sidebar-overlays.contract', 'text `+${count - 1}`');
  expect(Number(/border-radius="([\d.]+)rem"/.exec(count)?.[1])).toBeGreaterThanOrEqual(Number(/line-height="([\d.]+)rem"/.exec(count)?.[1]) / 2);
});

test('the rename field hands each edit to the data module, which reads its direction', () => {
  const field = contractComponent('sidebar-row.contract', 'SidebarRenameField'), state = field.state({ t: { title: 'Fix the build' } }), calls: unknown[][] = [];
  field.call('edit', state, { t: { id: 'a', title: 'Fix the build' }, releaseUndoClaim: () => { calls.push(['release']); }, local: (...args: unknown[]) => { calls.push(args); } }, ['مرحبا']);
  expect(calls).toEqual([['release'], ['rename-input', 'a', 'مرحبا']]);
  expect(state.title).toBe('مرحبا');
  // The field aligns to the start of its direction: macOS's field reads `start` with `direction`, and aligns left without it.
  expect(line('sidebar-row.contract', 'input id=`sidebar-rename-')).toContain('text-align="start" direction=t.renameDir');
});

test('the Working shelf\'s grace runs on two root timers, with the sidebar open or not', () => {
  const app = appText('app.contract').split('\n');
  const task = (name: string) => { const at = app.findIndex(text => text.trim().startsWith(`task ${name} `)); return [app[at]?.trim(), app[at + 1]?.trim()]; };
  // An owed stamp reads the window's clock at once; the soonest grace ends 2 s after it started.
  expect(task('workingGraceOwed')).toEqual(['task workingGraceOwed when data.sidebar.graceOwed > 0 key=data.sidebar.graceOwed', 'after(1, workingGraceClock)']);
  expect(task('workingGraceEnds')).toEqual(['task workingGraceEnds when data.sidebar.graceUntil > 0 key=data.sidebar.graceUntil', 'after(2000, workingGraceClock)']);
  const root = contractComponent('app.contract', 'T3Code'), sent: unknown[][] = [];
  root.call('workingGraceClock', {}, { wallTime: { epochAtZero: 1_790_000_000_000 }, performanceNow: () => 2_500, command: (...args: unknown[]) => { sent.push(args); } }, []);
  expect(sent).toEqual([['sidebarlocal:working-grace', '', '1790000002500', 0]]);
});

// SidebarUpdatePill's "Check for updates" tooltip goes with the idle pill. Once a server has an update, the update pill
// replaces it under a resting pointer, which gets no leave; the oracle's TooltipPopup unmounts with its trigger.
describe('the idle update pill\'s tip goes with the pill', () => {
  const when = /^ {2}task sidebarUpdateTipGone when (.+)$/m.exec(appText('app.contract'))![1]!;
  const due = (hover: string, available: boolean, running = false) => value(when, { sidebarHoverId: hover, data: { updateEverything: { available, running } } });
  test('the task is due once the update pill takes the idle pill\'s place under its tip', () => {
    expect(due('sidebar-update', false)).toBe(false);
    expect(due('sidebar-update', true)).toBe(true);
    expect(due('sidebar-update', false, true)).toBe(true);
    expect(due('sidebar-settings', true)).toBe(false);
  });
  test('it ends that tip and no other', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const state = { sidebarHoverId: 'sidebar-update' };
    root.call('sidebarUpdateTipEnd', state, {}, []);
    expect(state.sidebarHoverId).toBe('');
    const other = { sidebarHoverId: 'sidebar-usage' };
    root.call('sidebarUpdateTipEnd', other, {}, []);
    expect(other.sidebarHoverId).toBe('sidebar-usage');
  });
});

test('the project filter gives the focus back to its trigger after a keyboard close, not after a pointer pick', () => {
  const menu = contractComponent('sidebar-overlays.contract', 'SidebarScopeMenu');
  const calls: unknown[][] = [];
  const scope = { data: { sidebar: { scopes: [{ key: 'g1', canPin: false }], scopeHighlight: 'g1', scopePinChord: '' } }, pinChordPressed: () => false,
    local: (...args: unknown[]) => calls.push(['local', ...args]), focus: (id: string) => calls.push(['focus', id]), preventDefault: () => {}, stopPropagation: () => {} };
  const key = { shiftKey: false, metaKey: false, ctrlKey: false, altKey: false };
  menu.call('submitScope', menu.state(), scope, []);
  menu.call('searchKey', menu.state(), scope, ['Escape', key]);
  menu.call('pick', menu.state(), scope, ['g1', key]);
  expect(calls).toEqual([['local', 'scope', '', 'g1'], ['focus', 'sidebar-scope-trigger'], ['local', 'scope-key', '', 'Escape'], ['focus', 'sidebar-scope-trigger'], ['local', 'scope', '', 'g1']]);
  expect(line('sidebar.contract', 'id="sidebar-scope-trigger"')).toContain('press=local("scope-open", "", "")');
});
