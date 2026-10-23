// Focus and Return in four places, run with the Contract action semantics of contract-action-fixture.ts; the keys
// themselves are driven on macOS. Reference 1e2ecbd975: KeybindingsSettings.tsx (captureKeybinding and its Input),
// Sidebar.tsx (SnoozeMenuButton, a Base UI Menu), RightPanelTabs.tsx (the "+" menu and RightPanelEmptyState).
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { contractFn } from '../support/contract-fn-fixture';

const key = (k: string, extra: Env = {}) => [k, { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false, ...extra }];
/** A scope whose host commands and action props record their calls. */
const kmReturn = await contractFn('menu-keys.contract', 'kmReturn', ['k', 'e']);
function recorder(names: string[], extra: Env = {}) {
  const calls: string[] = [];
  const scope: Env = { kmReturn, ...extra };
  for (const name of names) scope[name] = (...args: unknown[]) => calls.push(args.length ? `${name}:${args.map(arg => typeof arg === 'object' ? JSON.stringify(arg) : arg).join(',')}` : name);
  return { calls, scope };
}

describe('Settings › Keybindings: the capture field keeps the focus after a chord', () => {
  const field = contractComponent('settings-keybindings.contract', 'CaptureField');
  test('a chord ends recording and the field keeps the focus; Escape restores the binding there; leaving it keeps the field', () => {
    const state = field.state();
    const { calls, scope } = recorder(['capture', 'held'], { original: 'mod+n', value: 'mod+n', label: 'Chat: New', rowId: 'chat-new', recording: true });
    const modal = () => field.drawn('aria-modal=true', state, scope);
    expect(modal()).toBe(true);
    // The chord: the root sets the draft and ends recording; the field still takes keys, so the row stays the window's
    // modal scope and Settings' Back (outside it) does not hear the next Escape.
    field.call('took', state, scope, ['mod+k']);
    scope.recording = false;
    expect(modal()).toBe(true);
    field.call('took', state, scope, ['escape']);
    expect(calls).toEqual(['capture:mod+n,mod+k', 'capture:mod+n,escape']);
    // KeybindingsSettings.tsx's Input stays mounted, unfocused, once the focus leaves it.
    field.call('took', state, { ...scope, recording: true }, ['blur']);
    expect(field.drawn('testId=`keybinding-field-${rowId}`', state, scope)).toBe(true);
    expect(modal()).toBe(false);
    expect(calls.slice(-2)).toEqual(['capture:mod+n,blur', 'held:keybinding-recorder-chat-new,false']);
  });
});

describe('Snooze: Escape returns the focus to the trigger', () => {
  const row = contractComponent('sidebar-row.contract', 'ThreadRow');
  const t = { id: 'thread' };
  test('a pointer press focuses the trigger before the popover opens, then the popup as it shows', () => {
    const state = row.state();
    const { calls, scope } = recorder(['hoverCard', 'focus'], { t });
    row.call('snoozePress', state, scope, []);
    // The trigger holds the focus as the host opens the popover: it is the focus the popover's Escape restores.
    expect(calls).toEqual(['hoverCard:,false', 'focus:snooze-trigger-thread']);
    row.call('snoozeChanged', state, scope, [{ newState: 'open' }]);
    expect(calls.at(-1)).toBe('focus:snooze-thread-keys');
    expect(state.snoozeOpen).toBe(true);
    row.call('snoozeChanged', state, scope, [{ newState: 'closed' }]);
    expect(calls).toHaveLength(3);
  });
  test('a keyboard opening keeps the row its key focused: the toggle moves no focus', () => {
    const state = row.state();
    const { calls, scope } = recorder(['hoverCard', 'focus'], { t });
    row.call('snoozeChanged', state, scope, [{ newState: 'open' }]);
    expect(calls).toEqual([]);
  });
});

describe('Return on a focused menu or launcher row presses it', () => {
  // The Mac host presses a focused menuitem, menuitemradio or option on Return as on Space (its popovertarget
  // included), so these rows take no key of their own: a handler that pressed too would press twice.

  test('the composer\'s Run on and Workspace rows: Return on the focused row sends its press', () => {
    const menus = contractComponent('composer-controls.contract', 'ComposerMenus');
    const run = (current: string, args: unknown[]) => {
      const { calls, scope } = recorder(['command', 'preventDefault', 'stopPropagation', 'focus'], {
        which: 'workspace', workspaceCurrent: current, kmTarget: () => undefined,
        workspaceItems: [{ id: 'run-on-env-b', label: 'Studio' }, { id: 'workspace-local', label: 'Current checkout' }, { id: 'workspace-worktree', label: 'New worktree' }],
      });
      menus.call('move', menus.state(), scope, [-1, ...args]);
      return calls;
    };
    expect(run('run-on-env-b', key('Enter'))).toEqual(['preventDefault', 'stopPropagation', 'command:environment-run-on,,env-b,0']);
    expect(run('workspace-worktree', key('Enter'))).toEqual(['preventDefault', 'stopPropagation', 'command:cclocal:env-mode,,worktree,0']);
    // The popup itself (no row focused) and a chord press nothing.
    expect(run('composer-menu', key('Enter'))).toEqual([]);
    expect(run('workspace-local', key('Enter', { metaKey: true }))).toEqual([]);
  });
  test('a launcher row: Return opens its surface and stays out of the launcher\'s own Enter', () => {
    const row = contractComponent('shell-panels.contract', 'SurfaceRow');
    const run = (args: unknown[]) => {
      const { calls, scope } = recorder(['choose', 'preventDefault', 'stopPropagation'], { surface: { id: 'files' } });
      row.call('rowKey', row.state(), scope, args);
      return calls;
    };
    expect(run(key('Enter'))).toEqual(['stopPropagation', 'preventDefault', 'choose:files']);
    expect(run(key('Enter', { repeat: true }))).toEqual(['stopPropagation', 'preventDefault']);
    expect(run(key('Enter', { altKey: true }))).toEqual(['stopPropagation']);
  });
});

describe('the surface launcher takes the focus each time it mounts', () => {
  const launcher = contractComponent('shell-panels.contract', 'SurfaceLauncher');
  test('its first laid-out size focuses it once; a hidden (0 by 0) layout does not', () => {
    const state = launcher.state();
    const { calls, scope } = recorder(['focus']);
    launcher.call('laidOut', state, scope, [0, 0]);
    expect(calls).toEqual([]);
    launcher.call('laidOut', state, scope, [540, 700]);
    launcher.call('laidOut', state, scope, [600, 700]);
    expect(calls).toEqual(['focus:surface-chooser']);
    // A launcher mounted again starts from its initial state, so it focuses itself again.
    const again = launcher.state(), second = recorder(['focus']);
    launcher.call('laidOut', again, second.scope, [540, 700]);
    expect(second.calls).toEqual(['focus:surface-chooser']);
  });
});
