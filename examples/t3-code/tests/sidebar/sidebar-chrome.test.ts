// The sidebar's chrome against T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3), read and run from the contracts:
// sidebar/mainAppLocation.ts isSidebarUtilityPage (the footer under Usage), SidebarUpdateCapsule.tsx (the update
// button stays inside the sidebar), ThreadStatusIndicators.tsx (a row's status label is no live region; Woke keeps
// one), SidebarThreadRow.tsx and LegacySidebar.tsx (the ⌘1–⌘9 jump badges are `rounded-full h-5` pills), the drag
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
  test.each([['sidebar.contract', 'Sidebar', { sidebar: { revealPending: '', revealRow: '', tree: [], treeCurrent: '' } }]] as const)(
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

test('the jump badges are pills, as `rounded-full` on their 1.25rem height', () => {
  for (const [file, marker] of [['sidebar-row.contract', 'top=(t.card ? "1.9375rem" : "0.5rem") height="1.25rem"']] as const) {
    const badge = line(file, marker);
    const radius = Number(/border-radius="([\d.]+)rem"/.exec(badge)?.[1]);
    expect(radius).toBeGreaterThanOrEqual(1.25 / 2);
  }
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
