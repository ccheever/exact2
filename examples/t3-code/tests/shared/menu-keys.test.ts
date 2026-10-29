// Run the Contract menu actions and retain the complete menu, row, dialog and state-owned wiring inventory.
// Native focus/dispatch timing is checked by driving the menus on macOS, which a source read cannot do.
import { describe, expect, test } from 'bun:test';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { contractFn } from '../support/contract-fn-fixture';
import { appFiles, appPath } from '../support/app-files';

const sources = new Map<string, Promise<string>>();
const source = (file: string) => {
  if (!sources.has(file)) sources.set(file, Bun.file(appPath(file)).text());
  return sources.get(file)!;
};
const files = appFiles().filter(name => name.endsWith('.contract'));
const event = (extra: Env = {}) => ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, repeat: false, ...extra });
const items = [{ id: 'alpha', label: 'Alpha' }, { id: 'beta', label: 'Beta' }, { id: 'again', label: 'Another' }];
const helpers = (async () => {
  const file = 'menu-keys.contract';
  const first = (rows: unknown[]) => rows[0];
  const kmIndex = await contractFn(file, 'kmIndex', ['items', 'current']);
  const kmAfter = await contractFn(file, 'kmAfter', ['items', 'current'], { kmIndex });
  const kmLetter = await contractFn(file, 'kmLetter', ['k']);
  const kmTarget = await contractFn(file, 'kmTarget', ['items', 'current', 'k'], { kmIndex, kmAfter, kmLetter, first, none: undefined });
  const kmOpenKey = await contractFn(file, 'kmOpenKey', ['k']);
  const kmArrow = await contractFn(file, 'kmArrow', ['k']);
  const kmTriggerKey = await contractFn(file, 'kmTriggerKey', ['k'], { kmOpenKey, kmArrow });
  const kmTriggerTarget = await contractFn(file, 'kmTriggerTarget', ['items', 'k']);
  const kmEnd = await contractFn(file, 'kmEnd', ['items', 'keyed'], { first });
  const kmKeyEnd = await contractFn(file, 'kmKeyEnd', ['k'], { kmOpenKey });
  const kmOpenTarget = await contractFn(file, 'kmOpenTarget', ['menuId', 'end']);
  const kmEndKeyed = await contractFn(file, 'kmEndKeyed', ['end']);
  return { first, kmIndex, kmTarget, kmOpenKey, kmArrow, kmTriggerKey, kmTriggerTarget, kmEnd, kmKeyEnd, kmOpenTarget, kmEndKeyed };
})();
function effects(calls: unknown[][]): Env {
  return Object.fromEntries(['preventDefault', 'stopPropagation', 'focus', 'blur', 'showPopover', 'hidePopover', 'place', 'local', 'control', 'hoverCard', 'openMenu', 'leave']
    .map(name => [name, (...args: unknown[]) => calls.push([name, ...args])]));
}
/** The lines of `component name` in `file`, up to the next top-level declaration. */
async function component(file: string, name: string): Promise<string> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end).join('\n');
}

describe('the shared menu keyboard (Base UI Menu)', () => {
  test('the popup owns its keys and observes row focus without consuming dialog lineage', async () => {
    const body = await component('menu-keys.contract', 'KeyMenu');
    expect(body).toContain('column id=menuId width="100%" tabindex=-1 autofocus=true retainFocus=true aria-modal=modal key=keys focus=entered focusin=rowEntered');
    expect(body).toContain('column id=`${menuId}-last` width="100%" tabindex=-1 focus=enteredLast');
    expect(body).toContain('column id=`${menuId}-first` width="100%" gap=gap tabindex=-1 focus=enteredFirst');
    expect(body).not.toMatch(/\b(hover|pointerdown|pointerup|pointermove)=/);
    const menu = contractComponent('menu-keys.contract', 'KeyMenu'), state = menu.state();
    const calls: unknown[][] = [], scope = { ...await helpers, ...effects(calls), items, keyed: 0 };
    menu.call('entered', state, scope, []);
    expect(state).toEqual({ current: '', seen: 0 });
    expect(calls).toEqual([]);
    menu.call('rowEntered', state, scope, [{ target: 'beta', relatedTarget: 'alpha' }]);
    expect(state.current).toBe('beta');
    expect(calls).toEqual([]); // focusin must keep bubbling to the dialog owner.
    for (const target of ['', 'nested-menu', 'nested-row', 'disabled-row']) {
      menu.call('rowEntered', state, scope, [{ target }]);
      expect(state.current).toBe('beta');
    }
    menu.call('keys', state, scope, ['ArrowDown', event()]);
    expect(state.current).toBe('again');
    expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['focus', 'again']]);
    calls.length = 0;
    menu.call('entered', state, scope, []);
    expect(state.current).toBe('');
    expect(calls).toEqual([]);
  });

  test('arrows wrap, Home/End find endpoints, and typeahead continues from the focused row', async () => {
    const menu = contractComponent('menu-keys.contract', 'KeyMenu');
    const cases = [
      ['', 'ArrowDown', 'alpha'], ['', 'ArrowUp', 'again'], ['again', 'ArrowDown', 'alpha'],
      ['alpha', 'ArrowUp', 'again'], ['beta', 'Home', 'alpha'], ['beta', 'End', 'again'],
      ['alpha', 'a', 'again'], ['again', 'A', 'alpha'], ['alpha', 'b', 'beta'],
    ];
    for (const [current, key, target] of cases) {
      const state = { ...menu.state(), current }, calls: unknown[][] = [];
      menu.call('keys', state, { ...await helpers, ...effects(calls), items }, [key, event()]);
      expect(state.current).toBe(target);
      expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['focus', target]]);
    }
    for (const key of ['z', 'Enter', ' ', 'Space', 'Escape', 'Tab', 'ArrowLeft', 'ArrowRight']) {
      const state = { ...menu.state(), current: 'beta' }, calls: unknown[][] = [];
      menu.call('keys', state, { ...await helpers, ...effects(calls), items }, [key, event()]);
      expect(state.current).toBe('beta');
      expect(calls).toEqual([]);
    }
    for (const modifier of ['metaKey', 'ctrlKey', 'altKey']) {
      const state = menu.state(), calls: unknown[][] = [];
      menu.call('keys', state, { ...await helpers, ...effects(calls), items }, ['ArrowDown', event({ [modifier]: true })]);
      expect(state.current).toBe('');
      expect(calls).toEqual([]);
    }
    const state = menu.state(), calls: unknown[][] = [];
    menu.call('keys', state, { ...await helpers, ...effects(calls), items }, ['A', event({ shiftKey: true })]);
    expect(state.current).toBe('alpha');
    calls.length = 0;
    for (const key of ['ArrowDown', 'ArrowUp', 'Home', 'End', 'a']) {
      menu.call('keys', state, { ...await helpers, ...effects(calls), items: [] }, [key, event()]);
    }
    expect(calls).toEqual([]);
  });

  // Every painted menu of the clone; the native ones (a context menu, the panel tab's) keep AppKit's own keyboard.
  const menus: [string, string][] = [
    ['sidebar-row.contract', 'snooze-${t.id}-keys'], ['pages-pr-actions.contract', 'pr-more-keys'], ['pages-pr-handoffs.contract', 'pr-checkout-keys'],
    ['pages-prs.contract', 'pr-sort-keys'], ['pages-prs.contract', 'pr-host-keys'], ['pages-prs.contract', 'pr-filters-keys'], ['pages-prs.contract', 'pr-filters-sub-keys'],
    ['pages-pr-quick.contract', '${popId}-keys'], ['pages-pr-stack.contract', 'pr-stack-keys'], ['settings-b-kit.contract', '${menuId}-keys'], ['settings-kit.contract', '${menuId}-keys'],
    ['settings-rows.contract', '${menuId}-keys'], ['timeline-plan.contract', 'plan-menu-${message.id}-keys'], ['legacy-sidebar.contract', 'legacy-sidebar-options-keys'],
    ['snapshot.contract', 'snapshot-sound-keys'], ['pages-usage.contract', 'usage-environment-keys'], ['pages-usage-prices.contract', 'usage-prices-apply-keys'],
    ['git.contract', 'details-git-options-keys'], ['panel.contract', 'add-surface-menu-keys'], ['panel.contract', 'linked-pr-menu-keys'],
    ['shell-panels.contract', 'title-menu-keys'], ['shell-panels.contract', 'title-submenu-keys'], ['requests.contract', 'approval-menu-${approval.id}-keys'],
    ['shell-details.contract', 'details-editors-keys'], ['project-scripts.contract', 'details-scripts-keys'], ['diff.contract', 'diff-scope-keys'], ['diff.contract', 'diff-turns-keys'],
    ['files-surface.contract', 'crumb-menu-keys'],
    ['files-surface.contract', 'file-editors-keys'], ['table-menu.contract', 'table-copy-keys'],
    ['pages-pr-code.contract', 'pull-request-code-scope-keys'],
  ];
  test.each(menus)('%s wraps its rows in the keyboard menu %s', async (file, menuId) => {
    const text = await source(file);
    const quoted = menuId.includes('${') ? `menuId=\`${menuId}\`` : `menuId="${menuId}"`;
    expect(text).toMatch(new RegExp(`KeyMenu\\(${quoted.replace(/[$(){}`.]/g, ch => `\\${ch}`)}, items=.*, modal=(true|false)\\)`));
  });

  test('a row the keys move to has an id: every menuitem in a file with a keyboard menu', async () => {
    const files = appFiles().filter(name => name.endsWith('.contract'));
    const missing: string[] = [];
    let rows = 0;
    for (const file of files) {
      const text = await source(file);
      if (!/\bKeyMenu\(/.test(text)) continue;
      text.split('\n').forEach((line, index) => {
        if (!/^\s*(button|link)\b/.test(line) || !/role=(?:"menuitem|\(.*menuitem)/.test(line)) return;
        // The context menus (the sidebar's rows, the legacy sidebar's rows and headers, the Files tree's, a panel tab's, a pull
        // request number's) and the legacy New thread's members menu are AppKit menus on macOS (LLP 1021 §5.1, D2): their
        // keyboard is the system's. A row that is always disabled is never a stop; diff.contract's AddSurfaceItem is mounted nowhere.
        if (/testId=`(thread|draft|tab|legacy-thread|legacy-project|legacy-members|files-tree)-menu-/.test(line) || /testId=`\$\{menuId\}-(copy-link|open-external)`/.test(line) || /\bdisabled=true\b/.test(line) || /popovertarget="diff-add-surface"/.test(line)) return;
        rows++;
        if (!/^\s*(button|link) id=/.test(line)) missing.push(`${file}:${index + 1}`);
      });
    }
    expect(rows).toBeGreaterThanOrEqual(40);
    expect(missing).toEqual([]);
  });
});

describe('public popover triggers execute one ordered command sequence', () => {
  // The complete migrated inventory, plus the Icon submenu's Enter/Space trigger (its arrows belong to its parent).
  const openers: [string, string, string][] = [
    ['legacy-sidebar.contract', 'LegacyOptions', 'triggerKey'], ['timeline-plan.contract', 'PlanCard', 'triggerKey'],
    ['connections.contract', 'ThisMachineSection', 'triggerKey'], ['connections.contract', 'EnvironmentRow', 'triggerKey'],
    ['connections.contract', 'EnvironmentIconMenu', 'triggerKey'], ['settings-b-kit.contract', 'CnSelect', 'triggerKey'],
    ['pages-pr-stack.contract', 'PrdStackControls', 'triggerKey'],
    ['settings-rows.contract', 'ScopeSentence', 'projectKey'], ['settings-rows.contract', 'ScopeSentence', 'environmentKey'],
    ['settings-source-control.contract', 'WriterTraitsPicker', 'triggerKey'],
    ['settings-rows.contract', 'CoreRowView', 'traitsKey'], ['settings-rows.contract', 'CoreSelect', 'triggerKey'],
    ['pages-pr-code.contract', 'PrdCodeToolbar', 'triggerKey'], ['pages-pr-code.contract', 'PrdDiffOptions', 'triggerKey'], ['settings-a-hosts.contract', 'HostRowView', 'triggerKey'],
    ['pages-pr-quick.contract', 'PrRowStack', 'triggerKey'], ['settings-kit.contract', 'SelectControl', 'triggerKey'],
    ['pages-prs.contract', 'PrSortMenu', 'triggerKey'], ['pages-prs.contract', 'PrHostMenu', 'triggerKey'],
    ['pages-prs.contract', 'PrFiltersMenu', 'triggerKey'], ['device-tools.contract', 'ToolsSelect', 'triggerKey'],
    ['pages-usage.contract', 'UsageBreadcrumb', 'triggerKey'], ['pages-usage.contract', 'UsageProviderMenu', 'triggerKey'],
    ['pages-pr-handoffs.contract', 'PrdCheckoutMenu', 'triggerKey'],
    ['browser-surface.contract', 'BrowserMore', 'triggerKey'], ['browser-surface.contract', 'BrowserLauncherProfiles', 'triggerKey'],
    ['settings-keybindings.contract', 'KeybindingRow', 'triggerKey'], ['snapshot.contract', 'SnapshotPanel', 'triggerKey'],
    ['browser-defaults.contract', 'BdViewportSelect', 'triggerKey'], ['panel.contract', 'AddSurfaceMenu', 'triggerKey'],
    ['settings-projects.contract', 'GhostSelect', 'triggerKey'], ['settings-projects.contract', 'ProjectActionsSection', 'triggerKey'],
    ['pages-pr-actions.contract', 'PrdActionsMenu', 'moreKey'], ['browser-profiles.contract', 'BrowserProfilesSetting', 'triggerKey'],
    ['browser-profiles.contract', 'BrowserProfileRow', 'triggerKey'], ['sidebar-row.contract', 'ThreadRow', 'snoozeKey'],
    ['settings-scheduled.contract', 'TaskRow', 'triggerKey'], ['settings-scheduled.contract', 'ScheduledEditor', 'environmentKey'],
    ['providers-setup.contract', 'ProviderAccountRow', 'triggerKey'], ['git.contract', 'GitRows', 'gitMenuKey'],
    ['pages-usage-prices.contract', 'UsagePricesApply', 'triggerKey'], ['project-scripts.contract', 'ScriptRow', 'triggerKey'],
    ['shell-details.contract', 'OpenInRow', 'triggerKey'], ['files-surface.contract', 'FileHeader', 'editorsKey'],
    ['requests.contract', 'ApprovalActions', 'menuKey'], ['diff.contract', 'DiffToolbar', 'scopeKey'], ['device-workspace.contract', 'RailMenuButton', 'triggerKey'],
    ['add-project-machine.contract', 'AddMachineTargetSelect', 'triggerKey'], ['add-project-machine.contract', 'AddMachineSubmenu', 'rowKey'],
  ];
  // Submenu triggers open on Enter, Space (and ArrowRight, Base UI's submenu key); their arrows belong to the parent menu.
  const submenu = (name: string) => name === 'EnvironmentIconMenu' || name === 'AddMachineSubmenu';
  test('the inventory includes every opener and no retired invoker, focus redirect, watcher or counter remains', async () => {
    const found: string[] = [];
    for (const file of files) {
      const text = await source(file);
      expect(text).not.toMatch(/\b(KeyMenuOpen|KmDoor|KeyMenuWatched|kmBump)\b|[-]km\b/);
      for (const owner of text.matchAll(/^component (\w+)\n([\s\S]*?)(?=^component |$(?![\s\S]))/gm)) {
        for (const action of owner[2]!.matchAll(/^  action (\w+)[^\n]*\n([\s\S]*?)(?=^  \S|$(?![\s\S]))/gm)) {
          if (action[2]!.includes('kmTriggerTarget')) found.push(`${file}:${owner[1]}:${action[1]}`);
        }
      }
    }
    expect(found.sort()).toEqual(openers.map(row => row.join(':')).sort());
  });
  test.each(openers)('%s %s.%s uses the real source, handles modifiers/repeat, and safely opens an empty menu', async (file, name, action) => {
    const body = await component(file, name);
    const actionBody = body.match(new RegExp(`^  action ${action}[^\\n]*\\n([\\s\\S]*?)(?=^  \\S|$(?![\\s\\S]))`, 'm'))![1]!;
    const command = actionBody.match(/showPopover\((.+), source=(.+)\)/)!;
    const itemName = actionBody.match(/kmTriggerTarget\((\w+), k\)/)![1]!;
    const owner = contractComponent(file, name), calls: unknown[][] = [];
    const scope: Env = { ...await helpers, ...effects(calls), [itemName]: items, section: 'models', rowId: 'model', rowRef: 'repo:42',
      selectId: 'select', menuId: 'popup', popId: 'row-popup', idPrefix: 'account-', t: { id: 'thread' }, row: { id: 'row' },
      projects: [{ id: 'project' }], environments: [{ id: 'env' }], environment: { environmentId: 'env' }, account: { instanceId: 'instance' }, message: { id: 'message' },
      host: { id: 'host', menuId: 'host-popup' }, item: { id: 'task', environmentId: 'task-env' }, approval: { id: 'approval' }, testId: 'rail-trigger',
      frame: () => ({ x: 0, y: 0, width: 600, height: 28 }) };
    // SelectControl measures its trigger as the keys open it (SelectPopup's min-w-(--anchor-width)).
    const state = (): Env => (name === 'SelectControl' ? { anchorWidth: 0 } : {});
    const popup = value(command[1]!, scope), sourceId = value(command[2]!, scope);
    expect(sourceId).not.toBe(popup);
    const triggerOwner = name === 'ThreadRow' ? await component(file, 'SidebarCardActions') : body;
    const trigger = triggerOwner.split('\n').find(line => /^\s*button /.test(line) && line.includes(` key=${action} `));
    expect(trigger).toBeDefined();
    expect(`${trigger} `).toContain(` id=${command[2]} `);
    expect(trigger).toContain(` popovertarget=${command[1]} `);
    // The same eligible rows drive the public opening and the menu's subsequent navigation.
    expect(body).toContain(`items=${itemName},`);
    // LegacyOptions measures nothing first (its menu is end-aligned by `position-area`, not a measured tail).
    const prep: unknown[][] = name === 'PrFiltersMenu' ? [['openMenu']]
      : name === 'PrRowStack' ? [['control', 'quick-stack', 'repo:42']]
      : name === 'ThreadRow' ? [['hoverCard', '', false]]
      : name === 'GitRows' ? [['local', 'git-open', 'menu', '']] : [];
    const keys = name === 'AddMachineSubmenu' ? ['Enter', ' ', 'Space', 'ArrowRight'] : submenu(name) ? ['Enter', ' ', 'Space'] : ['ArrowDown', 'ArrowUp', 'Enter', ' ', 'Space'];
    for (const key of keys) {
      calls.length = 0;
      owner.call(action, state(), scope, [key, event()]);
      expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ...prep, ['showPopover', popup, { source: sourceId }], ['focus', key === 'ArrowUp' ? 'again' : 'alpha']]);
      calls.length = 0;
      owner.call(action, state(), scope, [key, event({ repeat: true })]);
      expect(calls).toEqual([['preventDefault'], ['stopPropagation']]);
      for (const modifier of ['metaKey', 'ctrlKey', 'altKey', 'shiftKey']) {
        calls.length = 0;
        owner.call(action, state(), scope, [key, event({ [modifier]: true })]);
        expect(calls).toEqual([]);
      }
      owner.call(action, state(), { ...scope, [itemName]: [] }, [key, event()]);
      expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ...prep, ['showPopover', popup, { source: sourceId }]]);
    }
    for (const key of ['Escape', 'Tab', 'Home', 'End', 'a', ...(submenu(name) ? ['ArrowDown', 'ArrowUp'] : [])]) {
      calls.length = 0;
      owner.call(action, state(), scope, [key, event()]);
      expect(calls).toEqual([]);
    }
  });
});

describe('↓ and ↑ on a closed trigger open its menu at the first or the last item (Base UI Menu)', () => {
  test('state-owned menus retain root/first/last entry and mount-time endpoint selection', async () => {
    const h = await helpers, menu = contractComponent('menu-keys.contract', 'KeyMenu');
    for (const [key, end, keyed] of [['ArrowDown', 'first', 1], ['ArrowUp', 'last', -1], ['Enter', 'first', 1], [' ', 'first', 1], ['Space', 'first', 1], ['Escape', '', 0]] as const) {
      expect(h.kmKeyEnd(key)).toBe(end);
      expect(h.kmEndKeyed(end)).toBe(keyed);
      expect(h.kmOpenTarget('menu', end)).toBe(end ? `menu-${end}` : 'menu');
    }
    for (const [action, keyed, expected] of [['entered', 1, 'alpha'], ['entered', -1, 'again'], ['enteredFirst', 0, 'alpha'], ['enteredLast', 0, 'again']] as const) {
      const state = menu.state(), calls: unknown[][] = [], scope = { ...h, ...effects(calls), items, keyed };
      menu.call(action, state, scope, []);
      expect(state.current).toBe(expected);
      expect(calls).toEqual([['focus', expected]]);
      if (action === 'entered') {
        expect(state.seen).toBe(keyed);
        calls.length = 0;
        menu.call(action, state, scope, []);
        expect(state.current).toBe('');
        expect(calls).toEqual([]);
      }
      calls.length = 0;
      menu.call(action, menu.state(), { ...scope, items: [] }, []);
      expect(calls).toEqual([]);
    }
  });
});

describe('#298 bug 4: Custom snooze from the sidebar row by the real pointer and ↓', () => {
  test('the open popup pins its trigger across descendant focus changes and unpins only when closed', async () => {
    const row = await component('sidebar-row.contract', 'ThreadRow');
    expect(row).toContain('if shown or swept\n                        SidebarCardActions(');
    expect(row).toContain('max-width=(shown ? "0px" : "15rem")');
    expect(row).toContain('max-width=(shown ? "15rem" : "0px") overflow=(shown ? "visible" : "hidden") opacity=(shown ? 1 : 0) pointer-events=(shown ? "auto" : "none") aria-hidden=(not shown)');
    expect(row).toContain('KeyMenu(menuId=`snooze-${t.id}-keys`, items=snoozeItems, keyed=0, gap="0px", modal=true)');
    expect(row).toContain('[KmItem(id=`snooze-${t.id}-custom`, label="Custom…")]');
    expect(row).toMatch(/id=`snooze-\$\{t.id\}` popover="auto" toggle=snoozeChanged/);
    const owner = contractComponent('sidebar-row.contract', 'ThreadRow'), state = owner.state();
    const shown = row.match(/^  derive shown = (.+)$/m)![1]!;
    expect(value(shown, { ...state, hovering: false })).toBe(false);
    owner.call('snoozeChanged', state, {}, [{ newState: 'open' }]);
    const menu = contractComponent('menu-keys.contract', 'KeyMenu'), menuState = menu.state();
    for (const target of ['alpha', 'beta', 'again', '']) {
      menu.call('rowEntered', menuState, { ...await helpers, items }, [{ target }]);
      expect(value(shown, { ...state, hovering: false })).toBe(true);
    }
    owner.call('snoozeChanged', state, {}, [{ newState: 'closed' }]);
    expect(value(shown, { ...state, hovering: false })).toBe(false);
    expect(value(shown, { ...state, hovering: true })).toBe(true);
    const item = await component('sidebar-row.contract', 'SnoozeMenuItem');
    expect(item).toContain('button id=itemId cursor="pointer" press=press popovertarget=popId popovertargetaction="hide" hover=hover focus=focused(true) blur=focused(false) role="menuitem"');
    expect(item).toContain('background-color=(lit ?');
    expect(item).not.toMatch(/\b(inside|pointer): action|\b(inside|pointer)=/);
    const actions = await component('sidebar-row.contract', 'SidebarCardActions');
    expect(actions).toContain('id=`snooze-trigger-${t.id}`');
    expect(actions).toContain('press=snoozePress key=snoozeKey popovertarget=`snooze-${t.id}`');
  });
});

describe('Escape from the snooze menu with the pointer outside', () => {
  test('the trigger the focus returns to keeps the actions shown after the closed toggle releases the pin', async () => {
    // AFTER drive 11-snooze-pointer-out-escape: the closed toggle hid the actions (aria-hidden) under the
    // returned focus and the macOS host dropped it. has-[:focus-visible] (Sidebar.tsx) keeps them shown.
    const row = await component('sidebar-row.contract', 'ThreadRow');
    const shown = row.match(/^  derive shown = (.+)$/m)![1]!;
    const t = { id: 'thread' };
    const actionIds = value(row.match(/^  derive actionIds = (.+)$/m)![1]!, { t });
    expect(actionIds).toEqual(['discard-trigger-thread', 'snooze-trigger-thread', 'sweep-thread']);
    const owner = contractComponent('sidebar-row.contract', 'ThreadRow'), state = owner.state(), scope: Env = { t, actionIds };
    const isShown = () => value(shown, { ...state, hovering: false });
    // ↓ on the focused trigger: the menu opens and its first row takes the focus.
    owner.call('actionsIn', state, scope, []);
    owner.call('snoozeChanged', state, scope, [{ newState: 'open' }]);
    owner.call('actionsOut', state, scope, [{ target: 'snooze-trigger-thread', relatedTarget: 'snooze-thread-hour' }]);
    expect(isShown()).toBe(true);
    // Escape, the pointer on the composer: the host focuses the trigger, then the popup's toggle closes.
    owner.call('actionsIn', state, scope, []);
    owner.call('snoozeChanged', state, scope, [{ newState: 'closed' }]);
    expect(state.snoozeOpen).toBe(false);
    expect(isShown()).toBe(true);
    // Moving between the actions keeps them; focus leaving them, or the window, releases them.
    owner.call('actionsOut', state, scope, [{ target: 'snooze-trigger-thread', relatedTarget: 'sweep-thread' }]);
    expect(isShown()).toBe(true);
    owner.call('actionsOut', state, scope, [{ target: 'sweep-thread', relatedTarget: 'composer' }]);
    expect(isShown()).toBe(false);
    owner.call('actionsIn', state, scope, []);
    owner.call('actionsOut', state, scope, [{ target: 'snooze-trigger-thread', relatedTarget: null }]);
    expect(isShown()).toBe(false);
    // Wiring: the actions' own row observes focus-within, and both card instances pass the row's actions.
    const actions = await component('sidebar-row.contract', 'SidebarCardActions');
    expect(actions).toContain('    row align-items="stretch" focusin=focusIn focusout=focusOut\n');
    expect(actions).toContain('button cursor="pointer" id=`discard-trigger-${t.id}` press=run("discard-draft", t.id, "")');
    expect(row.split('snoozeKey=snoozeKey, snoozePress=snoozePress, focusIn=actionsIn, focusOut=actionsOut)').length - 1).toBe(2);
  });
});

// The Pull Requests Filters submenus, as PullRequestListFilters' Base UI Menu draws and keys them.
describe('the Pull Requests Filters submenus', () => {
  test('submenu Escape returns to its row until the pointer leaves; outer close restores Filters', async () => {
    const body = await component('pages-prs.contract', 'PrFiltersMenu');
    const menu = contractComponent('pages-prs.contract', 'PrFiltersMenu');
    for (const left of [false, true]) {
      const state = { ...menu.state(), sub: 'author', subLeft: left }, calls: unknown[][] = [];
      const scope: Env = { ...await helpers, ...effects(calls) };
      scope.closeSub = () => menu.call('closeSub', state, scope, []);
      menu.call('subEscape', state, scope, ['Escape']);
      // Closing the submenu, and then the menu, stops the pickers' host search (`option-kind` none).
      expect(calls).toEqual([['preventDefault'], ['stopPropagation'], left ? ['blur'] : ['focus', 'pr-filter-author'], ['control', 'option-kind', '']]);
      expect(state.sub).toBe('');
      calls.length = 0;
      menu.call('subEscape', state, scope, ['Escape']);
      expect(calls).toEqual([]); // the next Escape is available to the public popup.
      menu.call('menuChanged', state, scope, [{ newState: 'closed' }]);
      expect(calls).toEqual([['focus', 'pull-requests-filters'], ['control', 'option-kind', '']]);
      calls.length = 0;
      menu.call('menuChanged', state, scope, [{ newState: 'open' }]);
      expect(calls).toEqual([]);
    }
    const state = { ...menu.state(), sub: 'author' };
    menu.call('rowLeft', state, {}, ['state']);
    expect(state.subLeft).toBe(false);
    menu.call('rowLeft', state, {}, ['author']);
    expect(state.subLeft).toBe(true);
    const calls: unknown[][] = [], scope = { ...await helpers, ...effects(calls) };
    menu.call('open', state, scope, ['state']);
    expect(state).toMatchObject({ sub: 'state', subPointer: true, subLeft: false });
    menu.call('subKey', state, scope, ['author', 'ArrowRight', event()]);
    expect(state).toMatchObject({ sub: 'author', subPointer: false, subLeft: false });
    expect(calls).toEqual([['control', 'option-kind', ''], ['preventDefault'], ['control', 'option-kind', 'authors'], ['focus', 'pr-filters-sub-keys']]);
    calls.length = 0;
    const filterItems = value(body.match(/^  derive filterItems = (.+)$/m)![1]!, { KmItem: (item: unknown) => item, serverShown: false });
    menu.call('subKey', state, { ...scope, filterItems }, ['author', 'ArrowDown', event()]);
    expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['focus', 'pr-filter-labels']]);
    expect(body).not.toContain('pr-filters-rest');
    expect(body).toContain('popover="auto" toggle=menuChanged');
    expect(body).toContain('row align-items="flex-start" key=subEscape');
    expect(body).toContain('searchEscape=closeSub, control=control)');
    expect(body).toContain('KeyMenu(menuId="pr-filters-keys", items=filterItems, keyed=0, gap="0px", modal=true)');
    for (const row of ['state', 'involvement', 'author', 'labels', 'draft', 'review', 'checks', 'project']) {
      expect(body).toContain(`press=open("${row}"), keys=subKey("${row}"), leave=rowLeft("${row}"),`);
    }
    const searchBody = await component('pages-prs.contract', 'PrFilterSub');
    expect(searchBody).toContain('if k == "Escape"\n      preventDefault()\n      stopPropagation()\n      searchEscape()\n    else if k != "ArrowDown"\n      stopPropagation()');
  });
});

// The behaviour (a hovered row takes the focus, ↓ goes on from it, leaving hands the keys back) is
// menu-one-highlight.test.contract, which the agent runs against the app on a KeyMenu (Sort) and an owner-drawn menu (scope).
describe('one highlight: the pointer and the keys move one focus (Base UI highlightedIndex)', () => {
  // [file, row component, the row's own id]: every painted menu row in a KeyMenu (the title menu keeps ContextMenu's DOM
  // fallback, whose rows highlight on hover or focus).
  const rows: [string, string, string][] = [
    ['pages-prs.contract', 'PrMenuItem', 'testId'], ['pages-prs.contract', 'PrHostItem', '`pr-host-${host.label}`'], ['pages-prs.contract', 'PrSubTrigger', 'testId'],
    ['pages-prs.contract', 'PrAuthorItem', '`pr-author-${person.key}`'], ['pages-prs.contract', 'PrLabelItem', '`pr-label-${label.key}`'],
    ['browser-surface.contract', 'BsMenuItem', 'id'], ['browser-surface.contract', 'BrowserAppearanceItem', '"browser-more-appearance"'],
    ['browser-surface.contract', 'BrowserLauncherProfileItem', '`launcher-browser-profile-${profile.id}`'], ['browser-surface.contract', 'BrowserAddItem', '"add-surface-browser"'],
    ['diff.contract', 'DiffMenuItem', 'testId'], ['legacy-sidebar.contract', 'LegacyRadio', 'testId'], ['pages-pr-actions.contract', 'PaMenuItem', 'testId'],
    ['pages-pr-actions.contract', 'PaRadioItem', 'testId'], ['pages-pr-code.contract', 'PrdScopeCommit', '`pull-request-code-scope-${commit.short}`'],
    ['pages-pr-code.contract', 'PrdScopeMore', '"pull-request-code-scope-more"'], ['pages-pr-handoffs.contract', 'PrdHandoffItem', 'testId'],
    ['pages-pr-links.contract', 'PrdActOnItem', '`${idPrefix}${item.key}`'], ['pages-pr-quick.contract', 'PqStackItem', 'testId'], ['pages-pr-stack.contract', 'PsMenuItem', 'testId'],
    ['pages-usage.contract', 'CheckRow', 'testId'], ['pages-usage.contract', 'UsageMenuItem', 'testId'], ['settings-kit.contract', 'SkMenuItem', 'itemId'],
    ['settings-b-kit.contract', 'CnMenuItem', 'itemId'], ['device-workspace.contract', 'DeviceMenuItem', 'testId'], ['timeline-plan.contract', 'PlanMenuItem', 'itemId'],
    ['sidebar-row.contract', 'SnoozeMenuItem', 'itemId'], ['shell-details.contract', 'EditorOption', '`details-editor-${editor.id}`'],
    ['settings-scheduled.contract', 'TaskMenuItem', 'itemId'], ['settings-projects.contract', 'ImportScriptItem', '`import-script-${script.name}`'],
    ['panel.contract', 'PanelMenuItem', '`linked-pr-menu-${test}`'], ['panel.contract', 'AddSurfaceMenuItem', '`add-surface-${surface.id}`'],
    ['files-surface.contract', 'FilesCrumbItem', 'test'], ['markdown.contract', 'TableMenuItem', 'testId'], ['connections-routes.contract', 'RouteMenuItem', 'itemId'],
    ['connections.contract', 'EnvironmentIconMenu', '`environment-icon-${environment.environmentId}`'],
    ['browser-defaults.contract', 'BdViewportItem', '`browser-default-viewport-option-${option.value}`'], ['git.contract', 'GitMenuRow', '`details-git-menu-${item.id}`'],
  ];
  test.each(rows)('%s %s: pointer focus and arrows share the real row; leaving clears its highlight', async (file, name, id) => {
    const body = await component(file, name);
    expect(body).toContain('    keysId: string');
    expect(body).not.toMatch(/\b(over|hovered|hovering) or lit\b|\blit or (over|hovered|hovering)\b/);
    const row = contractComponent(file, name), state = row.state(), calls: unknown[][] = [];
    const scope: Env = { ...effects(calls), keysId: 'menu-root', itemId: 'real-item', testId: 'real-test', id: 'real-id', test: 'real-test',
      idPrefix: 'prefix-', person: { key: 'person' }, host: { label: 'host', unavailable: '' }, label: { key: 'label' }, profile: { id: 'profile' },
      commit: { short: 'commit' }, editor: { id: 'editor' }, script: { name: 'script' }, item: { key: 'item-key', id: 'item', disabled: false },
      surface: { id: 'surface', available: true }, environment: { environmentId: 'env' }, option: { value: 'viewport' }, view: { hasWebContents: true }, disabled: false };
    const realId = value(id, scope);
    const line = body.split('\n').find(line => /^\s*button /.test(line) && line.includes(` id=${id} `))!;
    expect(line).toBeDefined();
    const hover = line.match(/\bhover=(\w+)/)?.[1] ?? body.match(/^    box .*\bhover=(\w+)/m)?.[1];
    expect(hover).toBeDefined();
    expect(line).toContain('focus=focused(true) blur=focused(false)');
    row.call(hover!, state, scope, [true]);
    // The More menu's rows close its Appearance submenu on entry, and Appearance opens it.
    expect(calls).toEqual(name === 'BsMenuItem' ? [['hidePopover', 'browser-appearance-menu'], ['focus', realId]]
      : name === 'BrowserAppearanceItem' ? [['focus', realId], ['showPopover', 'browser-appearance-menu', { source: realId }]] : [['focus', realId]]);
    row.call('focused', state, scope, [true]);
    expect(state.lit).toBe(true);
    const menu = contractComponent('menu-keys.contract', 'KeyMenu'), menuState = menu.state();
    const menuScope = { ...await helpers, ...effects(calls), items: [{ id: realId, label: 'Current' }, { id: 'next-row', label: 'Next' }] };
    calls.length = 0;
    menu.call('rowEntered', menuState, menuScope, [{ target: realId }]);
    expect(calls).toEqual([]);
    menu.call('keys', menuState, menuScope, ['ArrowDown', event()]);
    expect(calls).toEqual([['preventDefault'], ['stopPropagation'], ['focus', 'next-row']]);
    calls.length = 0;
    row.call(hover!, state, scope, [false]);
    expect(calls).toEqual(name === 'PrSubTrigger' ? [['focus', 'menu-root'], ['leave']] : [['focus', 'menu-root']]);
    row.call('focused', state, scope, [false]);
    expect(state.lit).toBe(false);
    calls.length = 0;
    row.call(hover!, state, scope, [false]);
    expect(calls).toEqual(name === 'PrSubTrigger' ? [['leave']] : []);
    // Every use passes the popup it sits in (the next test checks which).
    for (const f of files) {
      for (const line of (await source(f)).split('\n').filter(l => new RegExp(`^\\s*${name}\\(`).test(l))) expect(line).toContain(`${name}(keysId=`);
    }
  });

  test('every keysId names the KeyMenu popup the row sits in', async () => {
    const files = new Map<string, string[]>();
    for (const f of appFiles().filter(n => n.endsWith('.contract') && !n.endsWith('.test.contract'))) files.set(f, (await source(f)).split('\n'));
    const indent = (line: string) => line.length - line.trimStart().length;
    // The popup a line opens: KeyMenu's own `menuId`, or the `-keys` KeyMenu of a wrapper (SkPopup, CnMenu) by its `menuId`.
    const popupOf = (line: string): string | null => {
      const own = line.match(/^\s*KeyMenu\(menuId=(`[^`]*`|"[^"]*"|[\w.]+)/);
      if (own) return own[1]!;
      // RailMenuButton names its KeyMenu's popup itself (`keysId`).
      const rail = line.match(/^\s*RailMenuButton\(.*\bkeysId=("[^"]*")/)?.[1];
      if (rail) return rail;
      const wrapped = line.match(/^\s*(?:SkPopup|CnMenu)\(menuId=(`[^`]*`|"[^"]*"|[\w.]+)/)?.[1];
      if (!wrapped) return null;
      return /^[`"]/.test(wrapped) ? `${wrapped.slice(0, -1)}-keys${wrapped.slice(-1)}` : `\`\${${wrapped}}-keys\``;
    };
    // The popups around a line: the nearest one among its ancestors in its component, else those around each use of the
    // component. `beside`: no ancestor opens one, but a KeyMenu precedes an ancestor at its own depth (the rows sit beside it).
    type Around = { popup: string; beside: boolean };
    const around = (file: string, at: number, seen: Set<string>): Around[] => {
      const lines = files.get(file)!;
      let depth = indent(lines[at]!), beside: string | null = null;
      for (let i = at - 1; i >= 0; i--) {
        const line = lines[i]!;
        const owner = line.match(/^component (\w+)$/)?.[1];
        if (owner) {
          if (beside) return [{ popup: beside, beside: true }];
          if (seen.has(owner)) return [];
          seen.add(owner);
          return [...files].flatMap(([f, ls]) => ls.flatMap((l, j) => new RegExp(`^\\s+${owner}\\(`).test(l) ? around(f, j, seen) : []));
        }
        if (!line.trim() || line.trimStart().startsWith('//') || indent(line) > depth) continue;
        if (indent(line) === depth) { beside ??= popupOf(line); continue; }
        depth = indent(line);
        const popup = popupOf(line);
        if (popup) return [{ popup, beside: false }];
      }
      return [];
    };
    const wrong: string[] = [], besides = new Set<string>();
    let checked = 0;
    for (const [file, lines] of files) lines.forEach((line, at) => {
      const keys = line.match(/^\s*\w+\(keysId=(`[^`]*`|"[^"]*"|[\w.]+)/)?.[1];
      // A component that hands its own `keysId` on is checked at its uses.
      if (!keys || keys === 'keysId') return;
      const popups = around(file, at, new Set());
      checked++;
      if (popups.length === 0 || popups.some(entry => entry.popup !== keys)) wrong.push(`${file}:${at + 1} keysId=${keys} in ${popups.map(entry => entry.popup).join(', ') || 'no KeyMenu'}`);
      for (const entry of popups) if (entry.beside) besides.add(file);
    });
    expect(wrong).toEqual([]);
    // A floor, so a pattern that stops matching the uses fails here (146 when it was set).
    expect(checked).toBeGreaterThan(140);
    // No menu draws its rows beside its KeyMenu (the device rail's were, so a
    // key at a focused row reached no menu).
    expect([...besides]).toEqual([]);
  });

  test('no painted menu row paints from its own hover', async () => {
    const offenders: string[] = [];
    for (const file of appFiles().filter(name => name.endsWith('.contract'))) {
      const text = await source(file);
      if (!/\bKeyMenu\(/.test(text)) continue;
      text.split('\n').forEach((line, index) => {
        // The rows the keys reach (they carry an id); a list with no keyboard of its own (a branch picker's cursor, a
        // workspace select) has one highlight already.
        if (!/^\s*(button|link) id=/.test(line) || !/role=(?:"(menuitem|option)|\(.*(menuitem|option))/.test(line)) return;
        // The title menu draws ContextMenu's DOM fallback (its own hover or focus); the sidebar's and a panel tab's context
        // menus are AppKit menus on macOS.
        // The "+" menu's profile list opens from its chevron and has no keyboard (issues/closed/20261009-popover-action-commands.md), so it has one highlight already.
        if (/testId=`title-menu-|testId=`(thread|draft|tab)-menu-|testId=`browser-profile-/.test(line)) return;
        if (/background-color=\([^"]*\b(over|hovered|hovering)\b/.test(line)) offenders.push(`${file}:${index + 1}`);
      });
    }
    expect(offenders).toEqual([]);
  });
});

// More › Appearance is MenuSubTrigger + MenuSubPopup (PreviewMoreMenu.tsx, ui/menu.tsx):
// hover opens it beside its row through the public popover command, the pointer can travel into it, and a sibling
// row (or the Zoom row) closes only it, so Zoom − is never covered.
describe('the Browser More menu\'s Appearance submenu', () => {
  const child = 'browser-appearance-menu';
  const run = (component: string, action: string, scope: Env, args: unknown[]) => {
    const calls: unknown[][] = [], row = contractComponent('browser-surface.contract', component), state = row.state();
    row.call(action, state, { ...effects(calls), ...scope }, args);
    return calls;
  };
  test('hovering Appearance opens the submenu from its row; leaving the row toward it does not close it; a disabled row opens nothing', () => {
    const live = { keysId: 'browser-more-keys', view: { hasWebContents: true } };
    expect(run('BrowserAppearanceItem', 'hover', live, [true])).toEqual([['focus', 'browser-more-appearance'], ['showPopover', child, { source: 'browser-more-appearance' }]]);
    const appearance = contractComponent('browser-surface.contract', 'BrowserAppearanceItem'), state = appearance.state(), calls: unknown[][] = [];
    appearance.call('focused', state, { ...effects(calls), ...live }, [true]);
    appearance.call('hover', state, { ...effects(calls), ...live }, [false]); // the pointer leaves the row for the submenu
    expect(calls).toEqual([['focus', 'browser-more-keys']]);
    expect(run('BrowserAppearanceItem', 'hover', { ...live, view: { hasWebContents: false } }, [true])).toEqual([]);
  });
  test('entering a sibling row or the Zoom row closes only the submenu, never the More menu', () => {
    for (const disabled of [false, true]) {
      const calls = run('BsMenuItem', 'hover', { keysId: 'browser-more-keys', id: 'browser-more-hard-reload', disabled }, [true]);
      expect(calls[0]).toEqual(['hidePopover', child]);
      expect(calls.filter(call => call[0] === 'hidePopover')).toEqual([['hidePopover', child]]);
    }
    expect(run('BsMenuItem', 'hover', { keysId: 'browser-more-keys', id: 'browser-more-hard-reload', disabled: false }, [false])).toEqual([]);
    expect(run('BrowserMore', 'zoomRowHover', {}, [true])).toEqual([['hidePopover', child]]);
    expect(run('BrowserMore', 'zoomRowHover', {}, [false])).toEqual([]);
  });
});
