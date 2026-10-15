// Focus and Return in four places, run with the Contract action semantics of contract-action-fixture.ts; the keys
// themselves are driven on macOS. Reference 1e2ecbd975: KeybindingsSettings.tsx (captureKeybinding and its Input),
// Sidebar.tsx (SnoozeMenuButton, a Base UI Menu), RightPanelTabs.tsx (the "+" menu and RightPanelEmptyState).
import { describe, expect, test } from 'bun:test';
import { contractComponent, type Env } from './contract-action-fixture';
import { contractFn } from './contract-fn-fixture';

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
  test('a chord ends recording but keeps the recorder mounted; Escape restores the binding there; leaving it unmounts it', () => {
    const state = field.state();
    const { calls, scope } = recorder(['capture'], { original: 'mod+n', value: 'mod+n', label: 'Chat: New' });
    const drawn = (recording: boolean, value: string) => field.drawn('KeyRecorder(', state, { ...scope, recording, value });
    expect(drawn(true, 'mod+n')).toBe(true);
    // The chord: the root sets the draft and ends recording; the field still has the recorder, so the focus stays and
    // Settings' Back (outside the recorder's modal row) does not hear the next Escape.
    field.call('took', state, scope, ['mod+k']);
    expect(drawn(false, 'mod+k')).toBe(true);
    field.call('took', state, scope, ['escape']);
    expect(calls).toEqual(['capture:mod+n,mod+k', 'capture:mod+n,escape']);
    // An unassigned row keeps its field after Escape, still focused, as the reference's Input stays mounted.
    expect(drawn(false, '')).toBe(true);
    field.call('took', state, scope, ['blur']);
    expect(drawn(false, 'mod+k')).toBe(false);
    expect(calls.at(-1)).toBe('capture:mod+n,blur');
  });
  test('the field shows the draft once it stops recording, and the placeholder while it records', async () => {
    const source = await Bun.file(new URL('./settings-keybindings.contract', import.meta.url)).text();
    expect(source).toContain('    if recording or held\n');
    expect(source).toContain('KeyRecorder(prompt=(recording ? "Press shortcut" : "Unassigned"), value=(recording ? "" : value), recording=recording,');
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
  const cases = [
    ['r4-surfaces.contract', 'R4AddItem', ['choose', 'hidePopover:r4-add-surface'], {}],
    ['browser-surface.contract', 'BrowserAddItem', ['choose', 'hidePopover:r4-add-surface'], {}],
    ['sidebar-row.contract', 'SnoozeMenuItem', ['press', 'hidePopover:snooze-thread'], { popId: 'snooze-thread' }],
    // The shared row kits: Settings' menus and selects (SkMenuItem), Settings › Connections' menus and selects, Load
    // balancing's machine rows included (CnMenuItem).
    ['settings-kit.contract', 'SkMenuItem', ['choose', 'hidePopover:settings-menu'], { menuId: 'settings-menu' }],
    ['settings-b-kit.contract', 'CnMenuItem', ['choose', 'hidePopover:load-weight-env-menu'], { menuId: 'load-weight-env-menu', submenu: '', itemId: 'load-weight-env-option-2' }],
  ] as const;
  for (const [file, name, pressed, props] of cases) {
    test(`${name}: Return presses the row and closes its menu; a chord or a repeat does not`, () => {
      const item = contractComponent(file, name);
      const run = (args: unknown[]) => {
        const { calls, scope } = recorder(['choose', 'press', 'hidePopover', 'togglePopover', 'preventDefault', 'stopPropagation'], props);
        item.call('rowKey', item.state(), scope, args);
        return calls;
      };
      expect(run(key('Enter'))).toEqual(['preventDefault', 'stopPropagation', ...pressed]);
      expect(run(key('Enter', { repeat: true }))).toEqual(['preventDefault', 'stopPropagation']);
      expect(run(key('Enter', { metaKey: true }))).toEqual([]);
      expect(run(key(' '))).toEqual([]);
    });
  }
  for (const name of ['CoreMenu', 'TraitsMenu']) {
    test(`${name} (Settings' selects and trait menus): Return chooses the focused option and closes the menu`, () => {
      const menu = contractComponent('settings-rows.contract', name);
      const { calls, scope } = recorder(['choose', 'hidePopover', 'preventDefault', 'stopPropagation'], { menuId: 'time-format-menu' });
      menu.call('rowKey', menu.state(), scope, ['24-hour', ...key('Enter')]);
      expect(calls).toEqual(['preventDefault', 'stopPropagation', 'choose:24-hour', 'hidePopover:time-format-menu']);
    });
  }
  test('a Connections row that opens a submenu toggles it on Return, as its click does', () => {
    const item = contractComponent('settings-b-kit.contract', 'CnMenuItem');
    const { calls, scope } = recorder(['choose', 'hidePopover', 'togglePopover', 'preventDefault', 'stopPropagation'], { menuId: 'machine-menu', submenu: 'icon-menu', itemId: 'icon' });
    item.call('rowKey', item.state(), scope, key('Enter'));
    expect(calls).toEqual(['preventDefault', 'stopPropagation', 'choose', 'togglePopover:icon-menu,{"source":"icon"}']);
  });
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
  test('the + menu rows, the snooze rows and the kits carry the handler', async () => {
    const read = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
    expect(await read('r4-surfaces.contract')).toContain('popovertarget="r4-add-surface" popovertargetaction="hide" key=rowKey');
    expect(await read('browser-surface.contract')).toContain('button id="add-surface-browser" press=choose popovertarget="r4-add-surface" popovertargetaction="hide" key=rowKey');
    expect(await read('sidebar-row.contract')).toContain('popovertarget=popId popovertargetaction="hide" key=rowKey');
    expect(await read('settings-kit.contract')).toContain('button id=itemId press=choose popovertarget=menuId popovertargetaction="hide" key=rowKey');
    expect(await read('settings-b-kit.contract')).toContain('popovertargetaction=(submenu != "" ? "toggle" : "hide") key=rowKey');
    expect((await read('settings-rows.contract')).split('popovertargetaction="hide" key=rowKey(option.value)').length - 1).toBe(2);
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
