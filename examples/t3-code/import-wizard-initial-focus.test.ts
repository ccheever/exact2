// import-wizard-initial-focus: where the browser import wizard and the profile removal confirm put the focus, as the
// reference's Base UI dialogs do (T3 Code 1e2ecbd975 BrowserImportWizard.tsx, IntegrationsSettings.tsx; recorded over CDP
// on the reference with the lane's fixture browsers). As the wizard opens, the popup's first Tab stop takes the focus
// (Configure: the first "From" tile; the quit step: Cancel; Full Disk Access: Allow; a blocked source: Close); a step
// change removes the focused button and leaves the focus on the popup itself, from which Tab goes to the first stop and
// Shift+Tab to the last (the close X). The confirm focuses Cancel, and Cancel gives it back to the row's options button
// (after a removal, Add profile). `wizardTabStops` names each screen's stops; these rows check them against the screens of
// browser-profiles.contract, and the root's tasks that move the focus.
import { describe, expect, test } from 'bun:test';
import { emptyBrowserProfilesView, goTo, wizardFocusKey, wizardOpenFocus, wizardTabStops } from './browser-profiles-settings';
import type { WizardStep } from './browser-import';

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
const facts = (step: string, more: Partial<Parameters<typeof wizardTabStops>[0]> = {}) =>
  ({ step, canClose: step !== 'importing', importDisabled: false, fdaGranted: false, fdaBusy: false, retry: false, from: [], into: [], ...more });

describe('the wizard\'s Tab stops, its focus as it opens and from the popup', () => {
  test('each screen\'s stops in order, the close X last (DialogPopup renders it after the step)', () => {
    expect(wizardTabStops(facts('configure', { from: [1, 2], into: [1] })))
      .toEqual(['browser-import-from-0', 'browser-import-from-1', 'browser-import-into-0', 'browser-import-cancel', 'browser-import-run', 'browser-import-x']);
    expect(wizardTabStops(facts('configure', { from: [1], into: [1], importDisabled: true }))).toEqual(['browser-import-from-0', 'browser-import-into-0', 'browser-import-cancel', 'browser-import-x']);
    expect(wizardTabStops(facts('quit'))).toEqual(['browser-import-cancel', 'browser-import-quit', 'browser-import-x']);
    expect(wizardTabStops(facts('fullDiskAccess'))).toEqual(['browser-import-fda-allow', 'browser-import-cancel', 'browser-import-x']);
    expect(wizardTabStops(facts('fullDiskAccess', { fdaGranted: true }))).toEqual(['browser-import-cancel', 'browser-import-fda-continue', 'browser-import-x']);
    expect(wizardTabStops(facts('fullDiskAccess', { fdaBusy: true }))).toEqual(['browser-import-cancel', 'browser-import-x']);
    expect(wizardTabStops(facts('blocked', { retry: true }))).toEqual(['browser-import-close', 'browser-import-retry', 'browser-import-x']);
    expect(wizardTabStops(facts('done'))).toEqual(['browser-import-done', 'browser-import-x']);
    expect(wizardTabStops(facts('checking'))).toEqual(['browser-import-x']);
    expect(wizardTabStops(facts('importing'))).toEqual([]);
  });
  test('as it opens, the first stop (the reference over CDP: Personal, Cancel; from source: Allow, Close)', () => {
    expect(wizardOpenFocus(facts('configure', { from: [1, 2], into: [1] }))).toBe('browser-import-from-0');
    expect(wizardOpenFocus(facts('quit'))).toBe('browser-import-cancel');
    expect(wizardOpenFocus(facts('fullDiskAccess'))).toBe('browser-import-fda-allow');
    // Granted already: Allow is replaced by "Allowed" after the first check, so the popup keeps the focus.
    expect(wizardOpenFocus(facts('fullDiskAccess', { fdaGranted: true }))).toBe('browser-import-popup');
    expect(wizardOpenFocus(facts('blocked', { retry: true }))).toBe('browser-import-close');
  });
  test('the stops are the focusable elements of each screen in tree order, with their ids', async () => {
    expect(ids(await screen('configure'))).toEqual(['browser-import-from-${i}', 'browser-import-into-${i}', 'browser-import-cancel', 'browser-import-run']);
    expect(ids(await screen('quit'))).toEqual(['browser-import-cancel', 'browser-import-quit']);
    expect(ids(await screen('fullDiskAccess'))).toEqual(['browser-import-fda-allow', 'browser-import-cancel', 'browser-import-fda-continue']);
    expect(ids(await screen('blocked'))).toEqual(['browser-import-close', 'browser-import-retry']);
    expect(ids(await screen('done'))).toEqual(['browser-import-done']);
    const profiles = await source('browser-profiles.contract');
    expect(profiles).toContain('each tile, i in wizard.from key=tile.key\n                  BiTile(tileId=`browser-import-from-${i}`, tile=tile,');
    expect(profiles).toContain('    button id=tileId press=press hover=hover aria-pressed=tile.selected');
    expect(profiles).toContain('    button id="browser-import-x" press=press aria-label="Close"');
    // The X follows every screen.
    expect(profiles.indexOf('        when wizard.canClose\n          BiClose(')).toBeGreaterThan(profiles.indexOf('        when wizard.step == "blocked"'));
  });
  test('the popup takes the focus, and only while it holds it does Tab go to the first stop, Shift+Tab to the last', async () => {
    const profiles = await source('browser-profiles.contract');
    expect(profiles).toContain('      column id="browser-import-popup" tabindex=-1 key=popupKeys focus=hold(true) blur=hold(false) class=BpPopup role="dialog" aria-modal=true');
    expect(profiles).toContain('  action popupKeys(k: string, e: KeyboardEvent)\n    if held and k == "Tab" and wizard.tabFirst != ""\n      preventDefault()\n      focus(e.shiftKey ? wizard.tabLast : wizard.tabFirst)\n');
  });
  test('no button autofocuses: the focus moves only where the reference moves it', async () => {
    const profiles = await source('browser-profiles.contract');
    const dialogs = profiles.slice(profiles.indexOf('component BrowserProfileDialogs')).split('\n').filter(line => !line.trimStart().startsWith('//'));
    expect(dialogs.filter(line => /\bautofocus\b/.test(line))).toEqual([]);
  });
});

describe('what a step change is (the focus key the root follows)', () => {
  test('a step of another kind changes it, also one that comes back before the page is read; the same step does not', () => {
    const wizard: { step: WizardStep; fdaGranted: boolean } = { step: { step: 'configure' }, fdaGranted: false };
    const opened = wizardFocusKey(wizard);
    goTo(wizard, { step: 'configure' }); // a vanished target: Configure again, its screen (and the focus) kept
    expect(wizardFocusKey(wizard)).toBe(opened);
    goTo(wizard, { step: 'importing' });
    const importing = wizardFocusKey(wizard);
    expect(importing).not.toBe(opened);
    goTo(wizard, { step: 'blocked', reason: 'readFailed' });
    const blocked = wizardFocusKey(wizard);
    goTo(wizard, { step: 'importing' }); // Try again, unread
    goTo(wizard, { step: 'blocked', reason: 'readFailed' });
    expect(new Set([opened, importing, blocked, wizardFocusKey(wizard)]).size).toBe(4);
    // Full Disk Access granted: Allow is replaced, so the key changes without a step change.
    const before = wizardFocusKey(wizard);
    wizard.fdaGranted = true;
    expect(wizardFocusKey(wizard)).not.toBe(before);
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
    expect(emptyBrowserProfilesView().wizard).toMatchObject({ open: false, focusKey: '', openFocus: '', tabFirst: '', tabLast: '' });
    expect(emptyBrowserProfilesView()).toMatchObject({ removalOpen: false, removalId: '' });
  });
});
