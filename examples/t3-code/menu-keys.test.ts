// fix-keyboard-focus: the menus' keyboard (menu-keys.contract) and the real-input fixes of the batch report
// (#298 bugs 4, 5, 6, 13 and 16), read from the Contract sources as dialog-focus.test.ts reads its dialogs.
// These guard the wiring; the behavior is proven by the macOS drives in tasks/20261008-fix-keyboard-focus.md.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const dir = new URL('./', import.meta.url);
const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of `component name` in `file`, up to the next top-level declaration. */
async function component(file: string, name: string): Promise<string> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end).join('\n');
}

describe('the shared menu keyboard (Base UI Menu)', () => {
  test('the popup takes the focus as it opens, keeps it on a click, and hears the keys of its rows', async () => {
    for (const name of ['KeyMenu', 'KeyMenuWatched']) {
      const body = await component('menu-keys.contract', name);
      expect(body).toContain('column id=menuId width="100%" tabindex=-1 autofocus=true retainFocus=true aria-modal=modal key=keys focus=entered');
      // `${menuId}-first` and `${menuId}-last`: an owner that mounts the menu hands the focus on to its first or last item.
      expect(body).toContain('column id=`${menuId}-last` width="100%" tabindex=-1 focus=enteredLast');
      expect(body).toContain('column id=`${menuId}-first` width="100%" gap=gap tabindex=-1 focus=enteredFirst');
      expect(body).toContain('match kmTarget(items, current, k)');
      expect(body).toContain('preventDefault()\n          stopPropagation()\n          current = item.id\n          focus(item.id)');
      // Opened from the keyboard (the trigger's count moved): the first item, or the last when ↑ opened it (a negative count).
      expect(body).toContain('if keyed != seen\n      seen = keyed\n      match kmEnd(items, keyed)');
    }
    expect(await component('menu-keys.contract', 'KeyMenuWatched')).toContain('blur=left');
  });

  test('↓ ↑ wrap from the popup or an item, Home and End, a letter finds the next label', async () => {
    const text = await source('menu-keys.contract');
    const target = text.split('\n').find(line => line.startsWith('fn kmTarget('))!;
    expect(target).toContain('k == "ArrowDown" ? first(kmAfter(items, current))');
    expect(target).toContain('k == "ArrowUp" ? at(items, kmIndex(items, current) < 0 ? -1 : kmIndex(items, current) - 1)');
    expect(target).toContain('k == "Home" ? first(items)');
    expect(target).toContain('k == "End" ? at(items, -1)');
    expect(target).toContain('startsWith(toLowerCase(item.label), toLowerCase(k))');
    expect(text).toContain('fn kmAfter(items: list<KmItem>, current: string): list<KmItem> = concat(slice(items, kmIndex(items, current) + 1), slice(items, 0, kmIndex(items, current) + 1))');
    expect(text).toContain('fn kmOpenKey(k: string): bool = k == "Enter" or k == " " or k == "Space"');
  });

  // Every painted menu of the clone; the native ones (a context menu, the panel tab's) keep AppKit's own keyboard.
  const menus: [string, string][] = [
    ['sidebar-row.contract', 'snooze-${t.id}-keys'], ['pages-pr-actions.contract', 'pr-more-keys'], ['pages-pr-handoffs.contract', 'pr-checkout-keys'],
    ['pages-prs.contract', 'pr-sort-keys'], ['pages-prs.contract', 'pr-host-keys'], ['pages-prs.contract', 'pr-filters-keys'], ['pages-prs.contract', 'pr-filters-sub-keys'],
    ['pages-pr-quick.contract', '${popId}-keys'], ['pages-pr-stack.contract', 'pr-stack-keys'], ['settings-b-kit.contract', '${menuId}-keys'], ['settings-kit.contract', '${menuId}-keys'],
    ['settings-rows.contract', '${menuId}-keys'], ['timeline-plan.contract', 'plan-menu-${message.id}-keys'], ['legacy-sidebar.contract', 'legacy-sidebar-options-keys'],
    ['snapshot.contract', 'snapshot-sound-keys'], ['pages-usage.contract', 'usage-environment-keys'], ['pages-usage-prices.contract', 'usage-prices-apply-keys'],
    ['pages-hero.contract', 'hero-project-keys'], ['r4-git.contract', 'details-git-options-keys'], ['r4-surfaces.contract', 'r4-add-surface-keys'], ['r4-surfaces.contract', 'linked-pr-menu-keys'],
    ['shell-panels.contract', 'title-menu-keys'], ['shell-panels.contract', 'title-submenu-keys'], ['requests.contract', 'approval-menu-${approval.id}-keys'],
    ['shell-details.contract', 'details-editors-keys'], ['r6-polish.contract', 'details-scripts-keys'], ['diff.contract', 'diff-scope-keys'], ['diff.contract', 'diff-turns-keys'],
    ['r6-device.contract', 'device-text-keys'], ['r6-device.contract', 'device-rotate-keys'], ['r6-device.contract', 'device-more-keys'], ['r4-surfaces-files.contract', 'crumb-menu-keys'],
    ['r4-surfaces-files.contract', 'file-editors-keys'], ['r8-keys-table-menu.contract', 'table-copy-keys'],
    ['pages-pr-code.contract', 'pull-request-code-scope-keys'],
  ];
  test.each(menus)('%s wraps its rows in the keyboard menu %s', async (file, menuId) => {
    const text = await source(file);
    const quoted = menuId.includes('${') ? `menuId=\`${menuId}\`` : `menuId="${menuId}"`;
    expect(text).toMatch(new RegExp(`KeyMenu(Watched)?\\(${quoted.replace(/[$(){}`.]/g, ch => `\\${ch}`)}, items=.*, modal=(true|false)\\)`));
  });

  test('a popover menu is modal while it shows, so Escape closes it before a page\'s own Escape shortcut (Usage\'s Back, Settings\')', async () => {
    for (const [file, line] of [['pages-usage.contract', 'KeyMenu(menuId="usage-environment-keys"'], ['pages-pr-actions.contract', 'KeyMenu(menuId="pr-more-keys"'],
      ['settings-kit.contract', 'KeyMenu(menuId=`${menuId}-keys`'], ['settings-b-kit.contract', 'KeyMenu(menuId=`${menuId}-keys`'], ['sidebar-row.contract', 'KeyMenuWatched(menuId=`snooze-${t.id}-keys`']] as const)
      expect((await source(file)).split('\n').find(l => l.includes(line))).toContain('modal=true)');
    // A menu its owner mounts from state keeps its backdrop's Escape (outside the menu): not modal.
    for (const [file, line] of [['shell-panels.contract', 'KeyMenu(menuId="title-menu-keys"'], ['diff.contract', 'KeyMenu(menuId="diff-scope-keys"']] as const)
      expect((await source(file)).split('\n').find(l => l.includes(line))).toContain('modal=false)');
    // The diff scope menu: the right panel's toggle declares Escape too (and wins among non-modal shortcuts), so the
    // menus' box scopes the shortcuts while it shows and closes the menus on Escape itself (the base closed the panel).
    const scope = await component('diff.contract', 'DiffScopeMenu');
    expect(scope).toContain('column aria-modal=true position="absolute"');
    // Its own Escape is a shortcut inside that scope, so it closes the menus even when nothing inside holds the focus.
    expect(scope).toContain('button press=closeMenus aria-keyshortcuts="Escape" aria-label="Close diff scope menu" tabindex=-1 aria-hidden=true');
    expect(scope).toContain('  action closeMenus\n    command("diff-view", "menu", "", 0)\n    focus("diff-scope")');
  });

  test('a row the keys move to has an id: every menuitem in a file with a keyboard menu', async () => {
    const files = readdirSync(dir).filter(name => name.endsWith('.contract'));
    const missing: string[] = [];
    let rows = 0;
    for (const file of files) {
      const text = await source(file);
      if (!/\bKeyMenu(Watched)?\(/.test(text)) continue;
      text.split('\n').forEach((line, index) => {
        if (!/^\s*(button|link)\b/.test(line) || !/role=(?:"menuitem|\(.*menuitem)/.test(line)) return;
        // The context menus (the sidebar's rows, a panel tab's) are AppKit menus on macOS (LLP 1021 §5.1): their keyboard
        // is the system's. A row that is always disabled is never a stop; diff.contract's AddSurfaceItem is mounted nowhere.
        if (/testId=`(thread|draft|tab)-menu-/.test(line) || /\bdisabled=true\b/.test(line) || /popovertarget="diff-add-surface"/.test(line)) return;
        rows++;
        if (!/^\s*(button|link) id=/.test(line)) missing.push(`${file}:${index + 1}`);
      });
    }
    expect(rows).toBeGreaterThanOrEqual(40);
    expect(missing).toEqual([]);
  });

  test('a menu opened by the data module: its trigger lets go of the focus, so the popup can take it', async () => {
    for (const [file, comp, call] of [['diff.contract', 'DiffToolbar', 'command("diff-view", "menu", "scope", 0)'], ['r6-device.contract', 'R6DeviceWorkspaceView', 'local("surface-r6dev-menu", "", menu)'],
      ['r4-surfaces-files.contract', 'R4CrumbView', 'local("surface-files-crumb", crumb.path, "")'], ['r4-surfaces.contract', 'R4LinkedRowView', 'local("surface-pr-menu", row.key, "")'],
      ['markdown.contract', 'TableCopyButton', 'press()']] as const)
      expect(await component(file, comp)).toContain(`    blur()\n    ${call}`);
  });
});

describe('↓ and ↑ on a closed trigger open its menu at the first or the last item (Base UI Menu, coordinator ruling 2026-10-08)', () => {
  test('the helpers: a keyboard opening\'s count carries its end in its sign; a mounted menu\'s owner names the end', async () => {
    const text = await source('menu-keys.contract');
    expect(text).toContain('fn kmArrow(k: string): bool = k == "ArrowDown" or k == "ArrowUp"');
    expect(text).toContain('fn kmBump(keyed: number, last: bool): number = last ? (keyed < 0 ? keyed - 1 : 0 - keyed - 1) : (keyed < 0 ? 1 - keyed : keyed + 1)');
    expect(text).toContain('fn kmEnd(items: list<KmItem>, keyed: number): option<KmItem> = keyed < 0 ? at(items, -1) : first(items)');
    expect(text).toContain('fn kmKeyEnd(k: string): string = k == "ArrowUp" ? "last" : (k == "ArrowDown" or kmOpenKey(k) ? "first" : "")');
    expect(text).toContain('fn kmOpenTarget(menuId: string, end: string): string = end == "" ? menuId : `${menuId}-${end}`');
    expect(text).toContain('fn kmEndKeyed(end: string): number = end == "last" ? -1 : (end == "first" ? 1 : 0)');
    // kmBump, as written: every bump differs from the last, and its sign is the end asked for.
    const bump = (keyed: number, last: boolean) => last ? (keyed < 0 ? keyed - 1 : 0 - keyed - 1) : (keyed < 0 ? 1 - keyed : keyed + 1);
    let keyed = 0;
    for (const last of [false, true, true, false, true, false, false]) {
      const next = bump(keyed, last);
      expect(next).not.toBe(keyed);
      expect(next < 0).toBe(last);
      keyed = next;
    }
  });

  test('a popover menu: two invisible invokers over its trigger show it by ↓ and ↑ while the trigger holds the focus', async () => {
    const open = await component('menu-keys.contract', 'KeyMenuOpen');
    for (const [press, keys, test] of [['opened(false)', 'ArrowDown', 'open-first'], ['opened(true)', 'ArrowUp', 'open-last']])
      expect(open).toContain(`button popovertarget=menuId popovertargetaction="show" press=${press} aria-keyshortcuts=(armed ? "${keys}" : "") aria-hidden=true tabindex=-1 pointer-events="none" position="absolute" left=0 top=0 width="100%" height="100%" padding=0 border-width=0 opacity=0 testId=\`\${menuId}-${test}\``);
    // Every popover menu's trigger that counts its keyboard openings has them, armed by its own focus.
    // Not: the Icon submenu row (a row inside a menu), the colour picker (not a menu).
    const skip = ['environment-icon-', 'theme-color-'];
    const missing: string[] = [];
    let triggers = 0;
    for (const file of readdirSync(dir).filter(name => name.endsWith('.contract'))) {
      const text = await source(file);
      for (const line of text.split('\n')) {
        const m = line.match(/^\s*button .*?popovertarget=("[^"]+"|`[^`]+`|[\w.]+) .*?key=(\w+)/);
        if (!m || /popovertargetaction=/.test(line) || skip.some(s => m[1].includes(s))) continue;
        triggers++;
        if (!new RegExp(`focus=\\w+Arm\\(true\\) blur=\\w+Arm\\(false\\)`).test(line)) missing.push(`${file}: ${m[1]} not armed by its focus`);
        if (!text.includes(`KeyMenuOpen(menuId=${m[1]}, armed=`)) missing.push(`${file}: ${m[1]} has no KeyMenuOpen`);
      }
    }
    expect(missing).toEqual([]);
    expect(triggers).toBeGreaterThanOrEqual(31); // and the snooze clock, whose `key=` comes first (pinned in the bug 4 test)
    // Each owner bumps its count by the end the invoker reports.
    for (const [file, comp, counter] of [['pages-prs.contract', 'PrSortMenu', 'keyed'], ['r4-git.contract', 'R4GitRows', 'gitMenuKeyed'], ['settings-rows.contract', 'ScopeSentence', 'projectOpens'], ['settings-kit.contract', 'SettingsSelect', 'keyed']] as const)
      expect(await component(file, comp)).toMatch(new RegExp(`action \\w+Opened\\(last: bool\\)\\n    ${counter} = kmBump\\(${counter}, last\\)`));
  });

  test('a menu its owner mounts from state: ↓ and ↑ on its trigger open it, at the first or the last item', async () => {
    // An action reads the state as it began, so the end comes from the key itself, not from the state it just set.
    const title = await component('chat.contract', 'ChatHeader');
    expect(title).toContain('action titleKey(k: string, e: KeyboardEvent)\n    titleEnd = kmKeyEnd(k)\n    if kmArrow(k) and not e.metaKey and not e.ctrlKey and not e.altKey\n      preventDefault()\n      title("open", "")\n      focus(kmOpenTarget("title-menu-keys", kmKeyEnd(k)))');
    expect(await source('requests.contract')).toContain('      menuOpen = true\n      focus(kmOpenTarget(`approval-menu-${approval.id}-keys`, kmKeyEnd(k)))');
    const details = await source('shell-details.contract');
    expect(details).toContain('      scriptsOpen = true\n      focus(kmOpenTarget("details-scripts-keys", kmKeyEnd(k)))');
    expect(details).toContain('press=toggleEditors key=editorsKey focus=clearEditorsEnd ');
    expect(details.split('\n').find(l => l.includes('KeyMenu(menuId="details-editors-keys"'))).toContain('keyed=kmEndKeyed(menuEnd)');
    // A press that toggles reads the state as it began: the focus moves in only when the menu opens.
    expect(await source('requests.contract')).toContain('    menuOpen = not menuOpen\n    // An action reads the state as it began: `not menuOpen` is the menu opening.\n    if not menuOpen\n');
    expect(details).toContain('    scriptsOpen = not scriptsOpen\n    // An action reads the state as it began: `not scriptsOpen` is the menu opening.\n    if not scriptsOpen\n');
    // Menus the data module mounts: the trigger's keys ask for the end, its taking the focus clears it (no `pointerdown`: that would
    // keep the press from the window's light-dismiss count), and the menu reads it as it mounts.
    const rail = await component('r6-device.contract', 'R6RailButton');
    expect(rail).toContain('button id=testId press=press key=keyed focus=menuEnd("")');
    expect(rail).toContain('    if menu and kmArrow(k) and not e.metaKey and not e.ctrlKey and not e.altKey\n      preventDefault()\n      press()');
    expect((await source('r6-device.contract')).split('keyed=kmEndKeyed(railEnd)').length - 1).toBe(3);
    const files = await source('r4-surfaces-files.contract');
    for (const id of ['crumb-menu-keys', 'file-editors-keys']) expect(files.split('\n').find(l => l.includes(`KeyMenu(menuId="${id}"`))).toContain('keyed=kmEndKeyed(menuEnd)');
    expect(files).toContain('key=crumbKey focus=menuEnd("")');
    expect(files).toContain('key=editorsKey focus=menuEnd("")');
    expect(await source('r4-surfaces.contract')).toContain('R4LinkedMenu(row=row, local=local, command=command, keyed=kmEndKeyed(menuEnd))');
    const diff = await source('diff.contract');
    expect(diff).toContain('DiffScopeMenu(data=data, command=command, keyed=kmEndKeyed(scopeEnd))');
    expect(diff).toContain('button id="diff-scope" press=openScope key=scopeKey focus=scopeEnd("") ');
    // The table's Copy menu mounts at the window: the request itself carries the end, and the menu reads it as `menu.keyed`.
    const copy = await component('markdown.contract', 'TableCopyButton');
    expect(copy).toContain('button press=openMenu key=menuKey ');
    expect(copy).toContain('      if k == "ArrowUp"\n        pressLast()\n      else\n        pressFirst()');
    expect(await source('markdown.contract')).toContain('pressFirst=local("table-menu", block.id, `keys:first\\t${block.markdown}\\t${block.csv}`), pressLast=local("table-menu", block.id, `keys:last\\t${block.markdown}\\t${block.csv}`)');
    expect(await source('r8-keys-table-menu.contract')).toContain('keyed=menu.keyed, gap="0px", modal=false)');
  });
});

describe('#298 bug 4: Custom snooze from the sidebar row by the real pointer and ↓', () => {
  test('the row keeps its hover actions shown while the snooze menu holds the focus (SnoozeMenuButton pins them)', async () => {
    const row = await component('sidebar-row.contract', 'ThreadRow');
    expect(row).toContain('derive shown = hovering or snoozeFocus or snoozeHover');
    expect(row).toContain('when shown or swept\n                        SidebarCardActions(');
    expect(row).toContain('max-width=(shown ? "0px" : "15rem")');
    expect(row).toContain('max-width=(shown ? "15rem" : "0px") overflow=(shown ? "visible" : "hidden") opacity=(shown ? 1 : 0) pointer-events=(shown ? "auto" : "none") aria-hidden=(not shown)');
    expect(row).toContain('KeyMenuWatched(menuId=`snooze-${t.id}-keys`, items=snoozeItems, keyed=snoozeKeyed, gap="0px", inside=snoozeInside, modal=true)');
    expect(row).toContain('[KmItem(id=`snooze-${t.id}-custom`, label="Custom…")]');
    const item = await component('sidebar-row.contract', 'SnoozeMenuItem');
    expect(item).toContain('button id=itemId cursor="pointer" press=press popovertarget=popId popovertargetaction="hide" hover=hover focus=focused(true) blur=focused(false) role="menuitem"');
    // FX-2: highlighted by its focus alone, which the pointer gives it (menu-keys.contract's doors).
    expect(item).toContain('background-color=(lit ?');
    // A real click's press takes the focus from the popup before its release: the pointer on a row keeps the pin.
    expect(item).toContain('action hover(value: bool)\n    if value\n      focus(`${itemId}-km`)\n    else if lit\n      focus(keysId)\n    pointer(value)');
    expect(row).toContain('inside=snoozeInside, pointer=snoozePointer)');
    const actions = await component('sidebar-row.contract', 'SidebarCardActions');
    expect(actions).toContain('press=hoverCard("", false) key=snoozeKey focus=snoozeArm(true) blur=snoozeArm(false) popovertarget=`snooze-${t.id}`');
    expect(actions).toContain('KeyMenuOpen(menuId=`snooze-${t.id}`, armed=snoozeArmed, opened=snoozeOpened)');
    expect(row).toContain('action snoozeOpened(last: bool)\n    snoozeKeyed = kmBump(snoozeKeyed, last)');
  });
});

describe('#298 bugs 6 and 5: Escape goes to the palette over Custom snooze, and the dialog gets its focus back', () => {
  test('covered by the palette, the dialog declares no Escape, so the palette\'s own takes it first', async () => {
    const dialog = await component('sidebar-overlays.contract', 'SidebarSnoozeDialog');
    expect(dialog).toContain('aria-keyshortcuts=(covered ? "" : "Escape") testId="snooze-cancel"');
    expect(await source('sidebar-overlays.contract')).toContain('SidebarSnoozeDialog(data=data, busy=busy, covered=covered, local=local, run=run)');
    const window = await source('app-window.contract');
    expect(window.split('covered=paletteOpen)').length - 1).toBe(2);
  });

  test('a sidebar dialog takes the focus at its first stop again as the palette over it closes', async () => {
    const app = await source('app.contract');
    expect(app).toContain('task sidebarDialogFocus when data.sidebar.dialog != "" and not paletteOpen key=data.sidebar.dialog');
  });
});

describe('#298 bugs 13 and 16: the pull request More menu and its Close dialog by keyboard', () => {
  test('Enter or Space on "…" counts a keyboard opening; every enabled row, the hand-offs included, is an arrow stop', async () => {
    const menu = await component('pages-pr-actions.contract', 'PrdActionsMenu');
    expect(menu).toContain('button id="pull-request-more" popovertarget="pr-more-menu" hover=hover key=moreKey');
    expect(menu).toContain('action moreKey(k: string)\n    if kmOpenKey(k)\n      keyed = kmBump(keyed, false)');
    // ↓ and ↑ on "…" open it at the first or the last row (Base UI Menu's trigger).
    expect(menu).toContain('KeyMenuOpen(menuId="pr-more-menu", armed=keyArmed, opened=keyOpened)');
    for (const id of ['pr-more-refresh', 'pr-more-ask', 'pr-more-explain', 'pr-more-fix-findings', 'pr-more-draft', 'pr-more-merge-now', 'pr-more-enable-auto-merge', 'pr-more-method-${item.method}', 'pr-more-open-host', 'pr-more-copy-link', 'pr-more-copy-number', 'pr-more-close', 'pr-more-reopen', 'pr-more-revert'])
      expect(menu).toContain(id);
    expect(menu).toContain('KeyMenu(menuId="pr-more-keys", items=rows, keyed=keyed, gap="0px", modal=true)');
  });

  test('the Act on rows of More and of Check out have their own ids, so ↓ reaches the rows of the menu that is open', async () => {
    // Real keys, 2026-10-09 (realinput-1009): More's ↓ stayed on "Fix findings" for each Act on row, because the key
    // menu focused the row by id and the hidden Check out menu's row with that id came first.
    const more = await component('pages-pr-actions.contract', 'PrdActionsMenu');
    expect(more).toContain('KmItem(id=(handing ? `pr-more-act-on-${item.key}` : ""), label=item.label)');
    expect(more).toContain('PrdActOnItems(keysId="pr-more-keys", items=actOn, disabled=(handoffs.pending != ""), act=act, idPrefix="pr-more-act-on-")');
    const checkout = await source('pages-pr-handoffs.contract');
    expect(checkout).toContain('KmItem(id=`pr-act-on-${item.key}`, label=item.label)');
    expect(checkout).toContain('PrdActOnItems(keysId="pr-checkout-keys", items=actOn, disabled=(handoffs.pending != ""), act=act, idPrefix="pr-act-on-")');
    expect(await component('pages-pr-links.contract', 'PrdActOnItem')).toContain('button id=`${idPrefix}${item.key}`');
  });

  test('the confirmation takes the focus at Cancel: the asking control lets go of it first, so Cancel\'s autofocus applies; Cancel gives it back', async () => {
    // `autofocus` waits while a control holds the focus (HTML's rule); "…" held it, given back by the closing menu.
    expect(await component('pages-pr-actions.contract', 'PrdHeaderActions')).toContain('action ask(what: string)\n    blur()\n    local("pr-ui-ask", ref, what)');
    const menu = await component('pages-pr-actions.contract', 'PrdActionsMenu');
    expect(menu).toContain('action ask(what: string)\n    blur()\n    local("pr-ui-ask", ref, what)');
    for (const what of ['merge', 'enable-auto-merge', 'close', 'revert']) expect(menu).toContain(`press=ask("${what}")`);
    expect(menu.split('local("pr-ui-ask"').length - 1).toBe(1); // only inside `ask`
    expect(await component('pages-pr-actions.contract', 'PrdApproveWorkflows')).toContain('action ask\n    blur()\n    local("pr-ui-ask", ref, "approve-workflows")');
    const dialog = await component('pages-pr-actions.contract', 'PrActionDialog');
    expect(dialog).toContain('button id="pr-action-dialog-cancel" press=dismiss key=fromCancel autofocus=true aria-keyshortcuts="Escape"');
    expect(dialog).toContain('action dismiss\n    local("pr-ui-cancel", ref, "")\n    focus(opener)');
    expect(dialog).toContain('"pull-request-approve-workflows" : (actions.primary != "" and (actions.dialogValue == actions.primary or startsWith(actions.dialogValue, `${actions.primary}:`)) ? "pull-request-primary" : "pull-request-more")');
    // No root line for it (app.contract's budget).
    expect(await source('app.contract')).not.toContain('pr-action-dialog-cancel');
  });
});

// audit-wave-followups-3: the Pull Requests Filters submenus, as PullRequestListFilters' Base UI Menu draws and keys them.
describe('the Pull Requests Filters submenus (audit-wave-followups-3)', () => {
  test('FW-3: Escape with a submenu open closes only the submenu and gives the focus back to its row', async () => {
    const menu = await component('pages-prs.contract', 'PrFiltersMenu');
    expect(menu).toContain('action subEscape(k: string)\n    if k == "Escape" and sub != ""\n      preventDefault()\n      closeSub()');
    // The focus goes back to the submenu's row (FloatingFocusManager's return to the trigger), unless the pointer left
    // that row after the submenu opened: the trigger's mouseleave stops the return and the focus drops (BODY on the
    // reference, from a row or from "Search authors").
    expect(menu).toContain('action closeSub\n    if subLeft\n      blur()\n    else\n      focus(`pr-filter-${sub}`)\n    sub = ""');
    expect(menu).toContain('action rowLeft(name: string)\n    if sub == name\n      subLeft = true');
    // Each opening, by a press or by the keyboard, starts with the pointer on (or never off) the row.
    expect(menu).toContain('    sub = sub == name ? "" : name\n    subPointer = true\n    subLeft = false\n');
    expect(menu).toContain('      subPointer = false\n      subLeft = false\n');
    for (const row of ['state', 'involvement', 'author', 'labels', 'draft', 'review', 'checks', 'project']) {
      expect(menu).toContain(`press=open("${row}"), keys=subKey("${row}"), leave=rowLeft("${row}"),`);
    }
    expect(await component('pages-prs.contract', 'PrSubTrigger')).toContain('action hover(value: bool)\n    if value\n      focus(`${testId}-km`)\n    else if lit\n      focus(keysId)\n    if not value\n      leave()');
    // On the row that holds the submenu and the Filters rows, so an Escape from either reaches it; the author search
    // stops its Escape and closes the submenu the same way.
    expect(menu).toContain('row align-items="flex-start" key=subEscape');
    expect(menu).toContain('searchEscape=closeSub, control=control)');
    expect(menu).not.toContain('focus("pr-filters-keys")');
    // The menu's keys then go on from that row (from Author, ↓ is Labels, as on the reference): a Filters row moves the
    // focus from itself, since the popup's KeyMenu knows only the rows its own keys reached.
    expect(menu).toContain('derive filterItems = [KmItem(id="pr-filter-state", label="State"), KmItem(id="pr-filter-involvement", label="Involvement"), KmItem(id="pr-filter-author", label="Author"), KmItem(id="pr-filter-labels", label="Labels"),');
    expect(menu).toContain('KeyMenu(menuId="pr-filters-keys", items=filterItems, keyed=keyed, gap="0px", modal=true)');
    expect(menu).toContain('  action subKey(name: string, k: string, e: KeyboardEvent)\n');
    expect(menu).toContain('    else if not e.metaKey and not e.ctrlKey and not e.altKey\n');
    expect(menu).toContain('      match kmTarget(filterItems, `pr-filter-${name}`, k)\n        case some(item)\n          preventDefault()\n          stopPropagation()\n          focus(item.id)\n        case none');
    expect(await component('pages-prs.contract', 'PrFilterSub')).toContain('if k == "Escape"\n      preventDefault()\n      stopPropagation()\n      searchEscape()\n    else if k != "ArrowDown"\n      stopPropagation()');
  });
  test('FW-4: the Author submenu tints its chosen row and ticks none; the radio submenus keep their tick', async () => {
    const sub = await component('pages-prs.contract', 'PrFilterSub');
    expect(sub).toContain('label="Anyone", detail="", selected=(page.author == ""), tick=false,');
    expect(sub).toContain('label="Open", detail="", selected=(page.state == "open"), tick=true,');
    const author = await component('pages-prs.contract', 'PrAuthorItem');
    expect(author).not.toContain('name="check"');
    expect(author).toContain('person.selected ? "light-dark(#27272a14, #f2f2f214)"');
  });
});

// The behaviour (a hovered row takes the focus, ↓ goes on from it, leaving hands the keys back) is
// menu-one-highlight.test.contract, which the agent runs against the app on a KeyMenu (Sort) and an owner-drawn menu (scope).
describe('one highlight: the pointer and the keys move one focus (audit-wave-followups-4 FX-2; Base UI highlightedIndex)', () => {
  test('a row\'s door makes it the menu\'s current row and hands it the focus, so the keys go on from the row the pointer rests on', async () => {
    for (const name of ['KeyMenu', 'KeyMenuWatched']) {
      const body = await component('menu-keys.contract', name);
      expect(body).toContain('  action pointed(id: string)\n    current = id\n    focus(id)');
      expect(body).toContain('      each item in items key=item.id\n        KmDoor(id=item.id, enter=pointed(item.id))');
      // The popup hears no pointer and no hover of its own: a press inside it still reaches the window's light dismiss
      // (popover-escape-parity), and the macOS host hovers one node at a time (#322).
      expect(body).not.toMatch(/\b(hover|pointerdown|pointerup|pointermove)=/);
    }
    // A node the host focuses has a size (PresenterMac.focusElement); the door is invisible, hidden from VoiceOver and no Tab stop.
    expect(await component('menu-keys.contract', 'KmDoor')).toContain('box id=`${id}-km` tabindex=-1 focus=enter aria-hidden=true position="absolute" left=0 top=0 width=1 height=1 opacity=0 pointer-events="none"');
  });

  const door = (id: string) => id.startsWith('`') || id.startsWith('"') ? `${id.slice(0, -1)}-km${id.slice(-1)}` : `\`\${${id}}-km\``;
  const quote = (text: string) => text.replace(/[\\^$.*+?()[\]{}|]/g, ch => `\\${ch}`);
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
    ['settings-b-kit.contract', 'CnMenuItem', 'itemId'], ['r6-device.contract', 'R6MenuItem', 'testId'], ['timeline-plan.contract', 'PlanMenuItem', 'itemId'],
    ['sidebar-row.contract', 'SnoozeMenuItem', 'itemId'], ['shell-details.contract', 'EditorOption', '`details-editor-${editor.id}`'],
    ['settings-scheduled.contract', 'TaskMenuItem', 'itemId'], ['settings-projects.contract', 'ImportScriptItem', '`import-script-${script.name}`'],
    ['r4-surfaces.contract', 'R4MenuItem', '`linked-pr-menu-${test}`'], ['r4-surfaces.contract', 'R4AddItem', '`add-surface-${surface.id}`'],
    ['r4-surfaces-files.contract', 'R4CrumbItem', 'test'], ['markdown.contract', 'TableMenuItem', 'testId'], ['connections-routes.contract', 'RouteMenuItem', 'itemId'],
    ['connections.contract', 'EnvironmentIconMenu', '`environment-icon-${environment.environmentId}`'],
    ['browser-defaults.contract', 'BdViewportItem', '`browser-default-viewport-option-${option.value}`'], ['r4-git.contract', 'R4GitMenuRow', '`details-git-menu-${item.id}`'],
  ];
  test.each(rows)('%s %s: its focus alone highlights it; the pointer entering focuses its door, leaving hands the focus to the popup', async (file, name, id) => {
    const body = await component(file, name);
    expect(body).toContain('    keysId: string');
    expect(body).toMatch(new RegExp(`    if (value|over|on)\\n      focus\\(${quote(door(id))}\\)\\n    else if lit\\n      focus\\(keysId\\)`));
    expect(body).not.toMatch(/\b(over|hovered|hovering) or lit\b|\blit or (over|hovered|hovering)\b/);
    // Every use passes the popup it sits in (the next test checks which).
    for (const f of readdirSync(dir).filter(n => n.endsWith('.contract'))) {
      for (const line of (await source(f)).split('\n').filter(l => new RegExp(`^\\s*${name}\\(`).test(l))) expect(line).toContain(`${name}(keysId=`);
    }
  });

  test('every keysId names the KeyMenu popup the row sits in', async () => {
    const files = new Map<string, string[]>();
    for (const f of readdirSync(dir).filter(n => n.endsWith('.contract') && !n.endsWith('.test.contract'))) files.set(f, (await source(f)).split('\n'));
    const indent = (line: string) => line.length - line.trimStart().length;
    // The popup a line opens: KeyMenu's own `menuId`, or the `-keys` KeyMenu of a wrapper (SkPopup, CnMenu) by its `menuId`.
    const popupOf = (line: string): string | null => {
      const own = line.match(/^\s*KeyMenu(?:Watched)?\(menuId=(`[^`]*`|"[^"]*"|[\w.]+)/);
      if (own) return own[1]!;
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
    // A floor, so a pattern that stops matching the uses fails here (146 at audit-wave-followups-4).
    expect(checked).toBeGreaterThan(140);
    // Only the device rail draws its rows beside their KeyMenu (a key at a focused row reaches no menu; the record's "Found, not changed").
    expect([...besides]).toEqual(['r6-device.contract']);
  });

  test('the menus whose rows their owner draws: one focused row, the door on entering, the popup on leaving', async () => {
    const cases: [string, string, string[]][] = [
      ['pages-hero.contract', 'HeroMenu', ['action hover(id: string, rowId: string, over: bool)\n    if over\n      focus(`${rowId}-km`)\n    else if lit == id\n      focus("hero-project-keys")', 'background-color=(lit == project.id ?']],
      ['r6-polish.contract', 'R6ScriptMenu', ['action hover(id: string, rowId: string, value: bool)\n    if value\n      focus(`${rowId}-km`)\n    else if lit == id\n      focus("details-scripts-keys")', 'background-color=(lit == script.id ?', 'background-color=(lit == "add" ?']],
      ['settings-rows.contract', 'ScopeMenu', ['    if over\n      focus(`${rowId}-km`)\n    else if lit == id\n      focus(`${menuId}-keys`)', 'background-color=(lit == choice.id or choice.selected ?']],
      ['settings-rows.contract', 'CoreMenu', ['    if over\n      focus(`${rowId}-km`)\n    else if lit == id\n      focus(`${menuId}-keys`)', 'background-color=(lit == option.id or option.selected ?']],
      ['settings-rows.contract', 'TraitsMenu', ['    if over\n      focus(`${rowId}-km`)\n    else if lit == id\n      focus(`${menuId}-keys`)', 'background-color=(option.selected or lit == option.id ?']],
      ['r4-surfaces-files.contract', 'R4FilesSurface', ['  action editorHover(id: string, over: bool)\n    if over\n      focus(`file-editor-${id}-km`)\n    else if editorLit == id\n      focus("file-editors-keys")', 'background-color=(editorLit == editor.id ?']],
      ['snapshot.contract', 'SnapshotPanel', ['  action soundHover(id: string, over: bool)\n    if over\n      focus(`${id}-km`)\n    else if soundLit == id\n      focus("snapshot-sound-keys")', 'background-color=(soundLit == "snapshot-sound-off" ?']],
      ['r4-surfaces.contract', 'R4LinkedMenu', ['    if on\n      focus("linked-pr-menu-open-km")\n    else if openLit\n      focus("linked-pr-menu-keys")', 'background-color=(openLit ?']],
      ['pages-pr-actions.contract', 'PrdActionsMenu', ['    if on\n      focus("pr-more-open-host-km")\n    else if hostLit\n      focus("pr-more-keys")', 'background-color=(hostLit ?']],
      ['requests.contract', 'MenuChoice', ['    if over\n      focus(`${testId}-km`)\n    else if focused\n      focus(keysId)', 'background-color=(focused ?', 'color=(focused ? "light-dark(#18181b, #f5f5f5)"']],
    ];
    for (const [file, name, wanted] of cases) {
      const body = await component(file, name);
      for (const text of wanted) expect(body).toContain(text);
      expect(body).not.toMatch(/hovered == \w+(\.\w+)? or lit|over == "add"|hovering or focused \?/);
    }
  });

  test('no painted menu row paints from its own hover', async () => {
    const offenders: string[] = [];
    for (const file of readdirSync(dir).filter(name => name.endsWith('.contract'))) {
      const text = await source(file);
      if (!/\bKeyMenu(Watched)?\(/.test(text)) continue;
      text.split('\n').forEach((line, index) => {
        // The rows the keys reach (they carry an id); a list with no keyboard of its own (a branch picker's cursor, a
        // workspace select) has one highlight already.
        if (!/^\s*(button|link) id=/.test(line) || !/role=(?:"(menuitem|option)|\(.*(menuitem|option))/.test(line)) return;
        // The title menu draws ContextMenu's DOM fallback (its own hover or focus); the sidebar's and a panel tab's context
        // menus are AppKit menus on macOS.
        // The "+" menu's profile list opens from its chevron and has no keyboard (X66), so it has one highlight already.
        if (/testId=`title-menu-|testId=`(thread|draft|tab)-menu-|testId=`browser-profile-/.test(line)) return;
        if (/background-color=\([^"]*\b(over|hovered|hovering)\b/.test(line)) offenders.push(`${file}:${index + 1}`);
      });
    }
    expect(offenders).toEqual([]);
  });
});
