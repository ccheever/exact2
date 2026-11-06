// The thread details card's right-click menus (T3 Code v0.1.5 BranchToolbar.tsx showWorkspaceContextMenu, the
// RunContextSelector's onContextMenu in the panel form; ThreadDetailsPanel's items menu on the card): the Workspace row
// offers Copy full path while it has a path, and the rest of the card opens Show in thread details. The path menu is
// menu-shaped, so macOS presents it as its own menu at the pointer. The items menu has an icon box beside each label, so
// macOS draws it as the app does, at the point the card gives it, and a checkbox row's choice keeps it open, as v0.1.6's
// Base UI menu does (exact2 issues/closed/20261031-macos-context-popover-not-menu-shaped.md: such a popover opens).
import { describe, expect, test } from 'bun:test';
import { shellDetails } from '../../src/details/shell-details';
import { shellLocal } from '../../src/shell/shell-commands';
import { toasts } from '../../src/shell/toast';
import { setFanout } from '../../src/composer/composer-controls-fanout';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { contractComponent } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';
import { connected } from '../support/composer-controls-fixture';

const body = contractComponent('shell-details.contract', 'DetailsBody');
const card = appText('src/details/shell-details.contract').split('\n');

/** The view lines of `component name`, each with its depth. */
function view(name: string): { depth: number; text: string }[] {
  const from = card.indexOf(`component ${name}`), to = card.findIndex((line, i) => i > from && line.startsWith('component '));
  const lines = card.slice(card.indexOf('  view', from) + 1, to < 0 ? undefined : to).filter(line => line.trim() !== '' && !line.trim().startsWith('//'));
  return lines.map(line => ({ depth: line.length - line.trimStart().length, text: line.trim() }));
}

describe('the path menu is menu-shaped, so macOS presents it as its own menu at the pointer (LLP 1021 D3)', () => {
  test('PathMenuPopover: each row is text alone, with no heading', () => {
    const lines = view('PathMenuPopover');
    const rows = lines.filter(line => line.text.startsWith('button '));
    expect(rows.length).toBeGreaterThan(0);
    for (const row of rows) {
      const at = lines.indexOf(row);
      const inside = lines.slice(at + 1).filter((line, i, all) => all.slice(0, i + 1).every(entry => entry.depth > row.depth));
      expect(inside.map(line => line.text.split(' ')[0])).toEqual(inside.map(() => 'text'));
    }
    expect(lines.find(line => line.text.startsWith('column id="details-path-menu"'))!.text).not.toContain('aria-label');
  });
});

describe('Show in thread details on the card is drawn, and stays open after a choice (ThreadDetailsItemsMenuItems)', () => {
  const lines = () => view('ItemsMenuPopover').map(line => line.text);
  test('a check icon box beside each label keeps it from being menu-shaped', () => {
    const items = lines();
    expect(items.find(line => line.startsWith('column id="details-items-menu"'))).toContain('aria-label="Show in thread details"');
    expect(items.find(line => line.includes('id=`details-items-${item.id}`'))).toContain('role="menuitemcheckbox" aria-checked=item.checked');
    expect(items).toContain('box width="1rem" height="1rem" flex-shrink=0 display="flex" align-items="center" justify-content="center"');
    expect(items).toContain('ShellIcon(name="check", size=14, color="light-dark(#18181b, #f5f5f5)")');
  });

  test('a checkbox row keeps the menu open; Reset to default closes it', () => {
    const items = lines();
    expect(items.find(line => line.includes('id=`details-items-${item.id}`'))).not.toContain('popovertarget');
    expect(items.find(line => line.startsWith('button id="details-items-reset"'))).toContain('popovertargetaction="hide"');
  });

  test('it opens where the card says: at a right-click, or under Choose what to show', () => {
    const state = { hovered: 'x', menuLeft: '0px', menuTop: '0px' };
    body.call('menuOpened', state, {}, [{ clientX: 1200, clientY: 205 }]);
    expect(state).toEqual({ hovered: '', menuLeft: '1200px', menuTop: '205px' });
    expect(card.find(line => line.includes('scroll width="100%" min-height=0'))).toContain('contextmenu=menuOpened contextPopover="details-items-menu"');
    expect(lines().find(line => line.startsWith('column id="details-items-menu"'))).toContain('margin=0 left=left top=top');
    expect(card.find(line => line.includes('button id="details-choose-items"'))).toContain('press=chooseOpened');
  });
});

describe('the Workspace row\'s Copy full path', () => {
  test('the row has the path of its checkout, and none while a new worktree is still to be made', async () => {
    const { client, native, command } = await connected();
    const details = () => shellDetails(client, native, true, '', false);
    expect((await details()).workspacePath).toBe('/repo');
    await command('cclocal:env-mode', '', 'worktree');
    expect((await details()).workspacePath).toBe('');
    await command('cclocal:env-mode', '', 'local');
    setFanout(client, [{ instanceId: 'codex', model: 'model-a', options: [] }, { instanceId: 'claude', model: 'model-b', options: [] }]);
    expect((await details()).workspacePath).toBe(''); // each model starts in its own worktree
  });

  test('the menu names its popover only while there is a path; without one the right-click reaches the card\'s menu', () => {
    const derive = (workspacePath: string) => body.derives({ hovered: '' }, { details: { workspacePath, items: {}, git: { branch: {} }, servers: {}, automations: {}, usage: {}, lineage: [] }, density: 'full' }).pathMenu;
    expect([derive('/repo'), derive('')]).toEqual(['details-path-menu', '']);
    const select = card.find(line => line.includes('button id="details-workspace"'))!, locked = card.find(line => line.includes('testId="details-folder"'))!;
    for (const row of [select, locked]) expect(row).toContain('contextPopover=pathMenu');
  });

  test('its one item copies the path and toasts it, as the reference\'s does', async () => {
    const item = card.find(line => line.includes('id="details-copy-path"'))!;
    expect(item).toContain('press=act("shelllocal:details-copy-path", "", path)');
    expect(card[card.indexOf(item) + 1]!.trim()).toStartWith('text "Copy full path"');
    const copied: string[] = [];
    const native = { available: true, watch() {}, later: async (request: unknown) => { copied.push(String((request as Obj).text)); return { ok: true, generation: 0, value: { copied: true } }; } } as Native;
    const { client } = await connected();
    await shellLocal(client, native, 'details-copy-path', '', '/repo/worktrees/feature');
    expect(copied).toEqual(['/repo/worktrees/feature']);
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['success', 'Path copied', '/repo/worktrees/feature']]);
    const failing = { ...native, later: async () => ({ ok: false, generation: 0, error: 'denied' }) } as Native;
    await shellLocal(client, failing, 'details-copy-path', '', '/repo');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Failed to copy path' });
  });
});

describe('Show in thread details in Settings › Appearance stays open after a choice (ThreadDetailsItemsRow)', () => {
  const menu = appText('src/appearance/thread-details-items-menu.contract').split('\n').map(line => line.trim());
  test('a checkbox row keeps the menu open; Reset to default closes it', () => {
    expect(menu.find(line => line.startsWith('button id=`${menuId}-${item.id}`'))).not.toContain('popovertarget');
    expect(menu.find(line => line.startsWith('button id=`${menuId}-reset`'))).toContain('popovertargetaction="hide"');
  });
  test('its keys are Base UI Menu\'s: the arrows on the trigger open it at a row, and move among the rows', () => {
    expect(menu.find(line => line.startsWith('button popovertarget=menuId'))).toContain('key=triggerKey');
    expect(menu.find(line => line.startsWith('KeyMenu('))).toBe('KeyMenu(menuId=`${menuId}-keys`, items=keyItems, keyed=0, gap="0px", modal=true)');
  });

});
