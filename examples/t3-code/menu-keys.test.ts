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
      expect(body).toContain('column id=menuId width="100%" tabindex=-1 autofocus=true retainFocus=true key=keys focus=entered');
      // `${menuId}-first`: a keyboard opening hands the focus on to the first item.
      expect(body).toContain('column id=`${menuId}-first` width="100%" gap=gap tabindex=-1 focus=enteredFirst');
      expect(body).toContain('match kmTarget(items, current, k)');
      expect(body).toContain('preventDefault()\n          stopPropagation()\n          current = item.id\n          focus(item.id)');
      // Opened from the keyboard (the trigger's count moved): the first item.
      expect(body).toContain('if keyed != seen\n      seen = keyed\n      match first(items)');
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
  ];
  test.each(menus)('%s wraps its rows in the keyboard menu %s', async (file, menuId) => {
    const text = await source(file);
    const quoted = menuId.includes('${') ? `menuId=\`${menuId}\`` : `menuId="${menuId}"`;
    expect(text).toMatch(new RegExp(`KeyMenu(Watched)?\\(${quoted.replace(/[$(){}`.]/g, ch => `\\${ch}`)}, items=`));
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

describe('#298 bug 4: Custom snooze from the sidebar row by the real pointer and ↓', () => {
  test('the row keeps its hover actions shown while the snooze menu holds the focus (SnoozeMenuButton pins them)', async () => {
    const row = await component('sidebar-row.contract', 'ThreadRow');
    expect(row).toContain('derive shown = hovering or snoozeFocus');
    expect(row).toContain('when shown or swept\n                        SidebarCardActions(');
    expect(row).toContain('max-width=(shown ? "0px" : "15rem")');
    expect(row).toContain('max-width=(shown ? "15rem" : "0px") overflow=(shown ? "visible" : "hidden") opacity=(shown ? 1 : 0) pointer-events=(shown ? "auto" : "none") aria-hidden=(not shown)');
    expect(row).toContain('KeyMenuWatched(menuId=`snooze-${t.id}-keys`, items=snoozeItems, keyed=snoozeKeyed, gap="0px", inside=snoozeInside)');
    expect(row).toContain('[KmItem(id=`snooze-${t.id}-custom`, label="Custom…")]');
    const item = await component('sidebar-row.contract', 'SnoozeMenuItem');
    expect(item).toContain('button id=itemId cursor="pointer" press=press popovertarget=popId popovertargetaction="hide" hover=hover focus=focused(true) blur=focused(false) role="menuitem"');
    expect(item).toContain('background-color=(over or lit ?');
    expect(await component('sidebar-row.contract', 'SidebarCardActions')).toContain('press=hoverCard("", false) key=snoozeKey popovertarget=`snooze-${t.id}`');
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
    expect(menu).toContain('action moreKey(k: string)\n    if kmOpenKey(k)\n      keyed = keyed + 1');
    for (const id of ['pr-more-refresh', 'pr-more-ask', 'pr-more-explain', 'pr-more-fix-findings', 'pr-more-draft', 'pr-more-merge-now', 'pr-more-enable-auto-merge', 'pr-more-method-${item.method}', 'pr-more-open-host', 'pr-more-copy-link', 'pr-more-copy-number', 'pr-more-close', 'pr-more-reopen', 'pr-more-revert'])
      expect(menu).toContain(id);
    expect(menu).toContain('KeyMenu(menuId="pr-more-keys", items=rows, keyed=keyed, gap="0px")');
  });

  test('the confirmation takes the focus at Cancel from "…" (a root task: autofocus waits while a control holds it) and Cancel gives it back', async () => {
    const app = await source('app.contract');
    expect(app).toContain('task prDialogFocus when prDetail.actions.dialogOpen key=prDetail.actions.dialogValue');
    expect(app).toContain('  action focusPrDialog\n    focus("pr-action-dialog-cancel")');
    const dialog = await component('pages-pr-actions.contract', 'PrActionDialog');
    expect(dialog).toContain('button id="pr-action-dialog-cancel" press=dismiss key=fromCancel autofocus=true aria-keyshortcuts="Escape"');
    expect(dialog).toContain('action dismiss\n    local("pr-ui-cancel", ref, "")\n    focus(opener)');
    expect(dialog).toContain('"pull-request-approve-workflows" : (actions.primary != "" and (actions.dialogValue == actions.primary or startsWith(actions.dialogValue, `${actions.primary}:`)) ? "pull-request-primary" : "pull-request-more")');
  });
});
