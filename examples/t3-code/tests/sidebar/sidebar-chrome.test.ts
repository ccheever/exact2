// The sidebar's chrome against T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3), read and run from the contracts:
// sidebar/mainAppLocation.ts isSidebarUtilityPage (the footer under Usage), SidebarUpdateCapsule.tsx (the update
// button stays inside the sidebar), ThreadStatusIndicators.tsx (a row's status label is no live region; Woke keeps
// one), SidebarThreadRow.tsx and LegacySidebar.tsx (the ⌘1–⌘9 jump badges are `rounded-full h-5` pills), and
// showThreadUndoNotice.ts (the undo notice is announced).
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
  test.each([['sidebar.contract', 'Sidebar', { sidebar: { revealPending: '', revealRow: '' } }],
    ['legacy-sidebar.contract', 'LegacySidebar', { legacy: { revealPending: '', revealRow: '' } }]] as const)(
    '%s keeps Settings, Pull requests and Usage under Usage; the pull requests page draws Back instead', (file, name, data) => {
      const sidebar = contractComponent(file, name), state = sidebar.state();
      for (const button of ['testId="connection-settings"', 'testId="open-pull-requests"', 'testId="open-usage"']) {
        expect(sidebar.drawn(button, state, { data, page: 'usage', pullRequests: true })).toBe(true);
        expect(sidebar.drawn(button, state, { data, page: '', pullRequests: true })).toBe(true);
        expect(sidebar.drawn(button, state, { data, page: 'pull-requests', pullRequests: true })).toBe(false);
      }
      expect(sidebar.drawn('text "Back"', state, { data, page: 'usage', pullRequests: true })).toBe(false);
      expect(sidebar.drawn('text "Back"', state, { data, page: 'pull-requests', pullRequests: true })).toBe(true);
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
  for (const [file, marker] of [['sidebar-row.contract', 'top=(t.card ? "1.9375rem" : "0.5rem") height="1.25rem"'], ['legacy-sidebar.contract', 'testId=`thread-jump-']] as const) {
    const badge = line(file, marker);
    const radius = Number(/border-radius="([\d.]+)rem"/.exec(badge)?.[1]);
    expect(radius).toBeGreaterThanOrEqual(1.25 / 2);
  }
});

test('the undo notice is announced: it is a live region until the shell has an announcer', () => {
  expect(line('undo-notice.contract', 'testId="sidebar-undo-notice"')).toContain('role="status"');
});
