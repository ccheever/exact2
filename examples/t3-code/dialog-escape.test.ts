// Escape over stacked popups. The Mac host gives a key to the first `aria-keyshortcuts` button it finds by node id
// (host/apple ShortcutsMac.swift `nodes()`), so a popup mounted earlier takes Escape from a dialog mounted over it unless
// it gives up its shortcut while that dialog shows. Base UI closes only the topmost popup. Read from the Contract
// sources; the keys are driven on macOS.
import { describe, expect, test } from 'bun:test';

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
