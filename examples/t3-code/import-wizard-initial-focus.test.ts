// import-wizard-initial-focus: where the browser import wizard and the profile removal confirm put the focus, as the
// reference's Base UI dialogs do (T3 Code 1e2ecbd975 BrowserImportWizard.tsx, IntegrationsSettings.tsx; recorded over CDP
// on the reference with the lane's fixture browsers). As the wizard opens, the popup's first tabbable element takes the
// focus (Configure: the first "From" tile; the quit step: Cancel; Full Disk Access: Allow; a blocked source: Close); a step
// change removes the focused button and leaves the focus on the popup itself. The confirm focuses Cancel, and Cancel gives
// it back to the row's options button (after a removal, Add profile). `wizardOpenFocus` names the element per step; these
// rows check that each name is that step's first tabbable element in browser-profiles.contract, and the root's tasks that
// move the focus.
import { describe, expect, test } from 'bun:test';
import { emptyBrowserProfilesView, wizardOpenFocus } from './browser-profiles-settings';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of one `when wizard.step == "<step>"` screen of BrowserImportWizard. */
async function screen(step: string): Promise<string> {
  const profiles = await source('browser-profiles.contract');
  const wizard = profiles.slice(profiles.indexOf('component BrowserImportWizard'), profiles.indexOf('component BiHeader'));
  const start = wizard.indexOf(`        when wizard.step == "${step}"`);
  expect(start).toBeGreaterThan(0);
  const rest = wizard.slice(start + 1);
  const end = rest.search(/\n {8}when /);
  return end < 0 ? rest : rest.slice(0, end);
}
/** The ids of a screen's focusable elements, in tree order (its buttons, Allow and the tiles). */
const ids = (text: string) => [...text.matchAll(/BiButton\(buttonId="([^"]+)"|button id="([^"]+)"|BiTile\(tileId=`([^`]+)`/g)].map(match => match[1] ?? match[2] ?? match[3]);

describe('the wizard\'s focus as it opens (Base UI Dialog initialFocus: the first tabbable element)', () => {
  test('each step it can open on names its first tabbable element; a grant on Full Disk Access leaves the popup', async () => {
    expect(wizardOpenFocus('configure', false)).toBe('browser-import-from-0');
    expect(wizardOpenFocus('quit', false)).toBe('browser-import-cancel');
    expect(wizardOpenFocus('fullDiskAccess', false)).toBe('browser-import-fda-allow');
    expect(wizardOpenFocus('fullDiskAccess', true)).toBe('browser-import-popup');
    expect(wizardOpenFocus('blocked', false)).toBe('browser-import-close');
    for (const step of ['importing', 'checking', 'done']) expect(wizardOpenFocus(step, false)).toBe('browser-import-popup');
  });
  test('the names are the first focusable element of each screen, before the close X', async () => {
    expect(ids(await screen('configure'))).toEqual(['browser-import-from-${i}', 'browser-import-into-${i}', 'browser-import-cancel', 'browser-import-run']);
    expect(ids(await screen('quit'))).toEqual(['browser-import-cancel', 'browser-import-quit']);
    expect(ids(await screen('fullDiskAccess'))).toEqual(['browser-import-fda-allow', 'browser-import-cancel', 'browser-import-fda-continue']);
    expect(ids(await screen('blocked'))).toEqual(['browser-import-close', 'browser-import-retry']);
    const profiles = await source('browser-profiles.contract');
    // The tiles carry their ids; index 0 of "From" is the first tile.
    expect(profiles).toContain('each tile, i in wizard.from key=tile.key\n                  BiTile(tileId=`browser-import-from-${i}`, tile=tile,');
    expect(profiles).toContain('    button id=tileId press=press hover=hover aria-pressed=tile.selected');
    // The popup is focusable (restoreFocus "popup"), and the X follows every step's screen.
    expect(profiles).toContain('      column id="browser-import-popup" tabindex=-1 class=BpPopup role="dialog" aria-modal=true');
    expect(profiles.indexOf('        when wizard.canClose\n          BiClose(')).toBeGreaterThan(profiles.indexOf('        when wizard.step == "blocked"'));
  });
  test('no button autofocuses: the focus moves only where the reference moves it', async () => {
    const profiles = await source('browser-profiles.contract');
    const dialogs = profiles.slice(profiles.indexOf('component BrowserProfileDialogs')).split('\n').filter(line => !line.trimStart().startsWith('//'));
    expect(dialogs.filter(line => /\bautofocus\b/.test(line))).toEqual([]);
  });
});

describe('the root moves the focus', () => {
  test('as the wizard opens, after a step change or a grant, and for the removal confirm', async () => {
    const app = await source('app.contract');
    expect(app).toContain('  task importWizardStepFocus when integrations.browserProfiles.wizard.focusKey != importWizardAt\n    after(1, importWizardStep)\n  action importWizardStep\n    if importWizardAt == "" and integrations.browserProfiles.wizard.focusKey != ""\n      focus(integrations.browserProfiles.wizard.openFocus)\n    else if integrations.browserProfiles.wizard.focusKey != ""\n      focus("browser-import-popup")\n    importWizardAt = integrations.browserProfiles.wizard.focusKey\n');
    // The confirm: Cancel as it opens; then the row's options button, or Add profile once the row is gone (removed).
    expect(app).toContain('  task removalFocus when integrations.browserProfiles.removalId != removalAt\n    after(1, removalFlip)\n  action removalFlip\n    if removalAt == ""\n      focus("browser-profile-remove-cancel")\n    else if includes(map(integrations.browserProfiles.rows, (row) => row.id), removalAt)\n      focus(`browser-profile-options-${removalAt}`)\n    else\n      focus("browser-profiles-add")\n    removalAt = integrations.browserProfiles.removalId\n');
    // The ids the confirm's focus names: its Cancel, and each row's options button.
    const profiles = await source('browser-profiles.contract');
    expect(profiles).toContain('BiButton(buttonId="browser-profile-remove-cancel", label="Cancel"');
    expect(profiles).toContain('        button id=`browser-profile-options-${row.id}` popovertarget=`browser-profile-menu-${row.id}`');
  });
  test('a closed wizard has no focus key, so its next opening counts as one', () => {
    expect(emptyBrowserProfilesView().wizard).toMatchObject({ open: false, focusKey: '', openFocus: '' });
    expect(emptyBrowserProfilesView()).toMatchObject({ removalOpen: false, removalId: '' });
  });
});
