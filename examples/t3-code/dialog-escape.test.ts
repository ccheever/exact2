// Escape over stacked popups. The Mac host gives a key to the first `aria-keyshortcuts` button it finds by node id
// (host/apple ShortcutsMac.swift `nodes()`), so a popup mounted earlier takes Escape from a dialog mounted over it unless
// it gives up its shortcut while that dialog shows. Base UI closes only the topmost popup. Read from the Contract
// sources; the keys are driven on macOS.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from './contract-action-fixture';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
const line = (text: string, needle: string) => text.split('\n').find(candidate => candidate.includes(needle)) ?? '';

describe('the thread details card gives Escape to the dialogs it opens', () => {
  test('a git dialog or the Merge confirmation keeps Escape from the card\'s backdrop', async () => {
    const card = await source('shell-details.contract');
    expect(line(card, 'testId="thread-details-backdrop"')).toContain('aria-keyshortcuts=(details.git.dialog != "" or details.prRows.merge.open ? "" : "Escape")');
    // The dialogs' own Escape buttons, inside them.
    const git = await source('r4-git.contract');
    expect(line(git, 'testId=`${testId}-close`')).toContain('aria-keyshortcuts="Escape"');
    expect(line(git, 'testId=`${testId}-backdrop`')).not.toContain('aria-keyshortcuts');
    expect(line(await source('r6-pr.contract'), 'testId="pr-merge-cancel"')).toContain('aria-keyshortcuts="Escape"');
  });
});

// OpenInPicker's Menu (panel form, MenuPopup align="end" anchored at the row, w-(--anchor-width)): a press anywhere
// outside it or Escape closes it, as Base UI's Menu does; the host light-dismisses a `popover="auto"` (exact2 PR #676).
describe('the "Open in" menu is a popover', () => {
  test('opened by its chevron, under the row at its width, modal while it shows, with no click-away layer', async () => {
    const card = await source('shell-details.contract');
    const menu = line(card, 'column id="details-editors-menu"');
    for (const part of ['popover="auto"', 'position-area="bottom span-left"', 'margin-top="0.25rem"', 'width=width', 'toggle=toggled',
      'data-dialog-return="menu"', 'focusin=dialogMenuEntered("details-editors-menu", "details-choose-editor")']) expect(menu).toContain(part);
    const chevron = line(card, 'button id="details-choose-editor"');
    for (const part of ['popovertarget="details-editors-menu"', 'key=triggerKey', 'aria-haspopup="menu"', 'aria-expanded=open']) expect(chevron).toContain(part);
    expect(chevron).not.toContain('press=');
    expect(line(card, 'KeyMenu(menuId="details-editors-keys"')).toContain('keyed=0, gap="0px", modal=true)');
    expect(line(card, 'button id=`details-editor-${editor.id}`')).toContain('popovertarget="details-editors-menu" popovertargetaction="hide"');
    expect(line(card, 'OpenInRow(')).toContain('menuWidth=((inline ? 262 : 246) * rem / 16)');
    for (const gone of ['details-editors-backdrop', 'editorsOpen', 'closeEditors', 'act("editors"']) expect(card).not.toContain(gone);
    // Nothing above the card keeps the menu's state any more.
    for (const file of ['app.contract', 'app-window.contract', 'app-main.contract', 'app-overlays.contract'])
      expect(await source(file)).not.toMatch(/detailsEditors|editorsOpen|"editors"/);
  });

  test("the chevron's tint and expanded state follow the popover's toggle", () => {
    const row = contractComponent('shell-details.contract', 'OpenInRow');
    const state = row.state();
    expect(state.open).toBe(false);
    row.call('toggled', state, {}, [{ newState: 'open', oldState: 'closed' }]);
    expect(state.open).toBe(true);
    row.call('toggled', state, {}, [{ newState: 'closed', oldState: 'open' }]);
    expect(state.open).toBe(false);
  });
});

// A narrow window's right panel is a sheet whose toggle declares Escape (right-panel-escape.test.ts), mounted before the
// Commit dialog the thread details open over it; the host would give Escape to that toggle first. Base UI's dialogs are
// modal, so the dialog is `aria-modal`: while it shows only its own shortcuts are heard.
describe('a dialog over the right panel sheet takes Escape first', () => {
  test('the git dialogs and the Merge confirmation are modal, their Escape inside them', async () => {
    const git = await source('r4-git.contract');
    const popup = line(git, 'aria-label=label testId=testId id=testId');
    expect(popup).toContain('aria-modal=dialogSeated.includes(testId)');
    expect(line(await source('r6-pr.contract'), 'testId="pr-merge-dialog" id="pr-merge-dialog"')).toContain('aria-modal=dialogSeated.includes("pr-merge-dialog")');
  });
});

// ProjectScriptsControl's Menu (panel): a press outside it or Escape closes it, as Base UI's Menu does.
describe('the project scripts menu is a popover', () => {
  test('anchored at the chevron, light-dismissed, modal while it shows', async () => {
    const polish = await source('r6-polish.contract');
    const menu = line(polish, 'column id="details-scripts-menu"');
    expect(menu).toContain('popover="auto"');
    expect(menu).toContain('position-area="bottom span-left"');
    expect(menu).toContain('width=width');
    expect(line(polish, 'button id="details-script-actions"')).toContain('popovertarget="details-scripts-menu"');
    expect(line(polish, 'KeyMenu(menuId="details-scripts-keys"')).toContain('modal=true)');
    expect(polish).not.toContain('details-scripts-backdrop');
    expect(line(await source('shell-details.contract'), 'R6ScriptRow(')).toContain('menuWidth=((inline ? 262 : 246) * rem / 16)');
  });
});
