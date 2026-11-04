// The narrow window: the breakpoints in CSS px, which View's page zoom shrinks (app.contract mediaWidth), the sidebar
// sheet under the Settings and Usage modals, which still hand it the keyboard, Settings' icon rail under 768 CSS px, and a project's settings that
// close the sheet (Sidebar.tsx openProjectSettings).
import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { contractComponent } from '../support/contract-action-fixture';
import { appPath, appText } from '../support/app-files';

const contracts = (dir: string): string[] => readdirSync(dir).flatMap(name => {
  const path = join(dir, name);
  return statSync(path).isDirectory() ? contracts(path) : name.endsWith('.contract') ? [path] : [];
});

describe('breakpoints at View\'s zoom', () => {
  test('one derive names the window\'s CSS px width, and no contract compares the raw window width', () => {
    const root = appText('app.contract');
    expect(root).toContain('derive mediaWidth = viewport.width\n');
    expect(root).toContain('derive narrowDiff = mediaWidth <= 980');
    expect(root).toContain('derive sidebarSheetMode = mediaWidth < 768');
    expect(root).toContain('derive diffWidth = narrowDiff ? (mediaWidth <= 760 ?');
    const raw = /(viewport\.width|viewportWidth|inlineDocumentViewportWidth|\bvw)\s*(<=|>=|<|>)\s*(640|768|980|1024|1280|760)\b/;
    const found = [appPath('app.contract'), ...contracts(fileURLToPath(new URL('../../src', import.meta.url)))].flatMap(path =>
      readFileSync(path, 'utf8').split('\n').map((line, at) => raw.test(line) ? `${path}:${at + 1}` : '').filter(Boolean));
    expect(found).toEqual([]);
  });
  test('the palette\'s status label shows from md up', () => {
    expect(appText('src/palette/palette.contract')).toMatch(/\n\s+if mediaWidth >= 768\n\s+text row\.leadStatus /);
  });
});

describe('the sheet and the route modals', () => {
  test('Usage is drawn after the sheet, as Settings is: the reference\'s z-50 dialogs are over its z-sheet one', () => {
    const window = appText('src/shell/app-window.contract');
    const sheet = window.indexOf('          SidebarSheet(first="sidebar-sheet-trigger"');
    expect(sheet).toBeGreaterThan(0);
    expect(window.indexOf('        UsageRoute(pageCover=pageCover')).toBeGreaterThan(sheet);
    expect(window.indexOf('          SettingsWindow(settingsOpen=settingsOpen')).toBeGreaterThan(sheet);
    expect(appText('src/shell/app-main.contract').split('component UsageRoute')[0]).not.toContain('AppRouteModal(label="Usage"');
  });
  test('a route modal under the sheet takes no focus as it opens and is hidden from assistive technology', () => {
    const modal = contractComponent('src/shell/app-route-modal.contract', 'AppRouteModal');
    const scope = (under: boolean) => ({ label: 'Settings', modalId: 'settings-dialog', mediaWidth: 1280, canvas: '', border: '', backdrop: '', inert: false, under, first: 'a', last: 'b' });
    expect(modal.attribute('role="dialog"', 'aria-hidden', {}, scope(true))).toBe(true);
    expect(modal.attribute('role="dialog"', 'aria-hidden', {}, scope(false))).toBe(false);
    expect(appText('src/shell/app-route-modal.contract')).toContain('DialogOpenFocus(target=(under ? "" : modalId))');
  });
  test('the sheet over Settings and Usage takes their Escape', () => {
    const settings = appText('src/shell/app-settings.contract');
    expect(settings).toMatch(/derive escapeOwned = [^\n]*\bsheetOver\b/);
    expect(settings).toContain('under=sheetOver,');
    expect(appText('src/shell/app-window.contract')).toContain('sheetOver=sidebarSheetShown');
    expect(appText('src/usage/pages-usage.contract')).toContain('dialog=(page.detail.open or page.prices.open or confirmOpen or under)');
    // The sheet's Escape goes no further: the modal over it closes on the next one.
    expect(appText('src/sidebar/sidebar-sheet.contract')).toMatch(/if k == "Escape" and escape and not e\.isComposing\n\s+preventDefault\(\)\n\s+stopPropagation\(\)\n\s+dismiss\(\)/);
  });
  test('a project\'s settings and a row\'s drop close the sheet where their answers open Settings or the thread', () => {
    const members = appText('src/composer/composer-controls-members.contract');
    expect(members).toMatch(/startsWith\("sidebar:project-settings:"\)\n\s+if sidebarSheetMode[^\n]*\n\s+sidebarSheetOpen = false\n\s+openSettings\(\)/);
    expect(members).toMatch(/startsWith\("open:"\)\n\s+if sidebarSheetMode[^\n]*\n\s+sidebarSheetOpen = false\n\s+openThread/);
  });
});

describe('Settings\' icon rail under 768 CSS px', () => {
  const nav = contractComponent('src/settings/settings-core.contract', 'SettingsNav');
  const state = { navFocused: '', navFocusedRoute: '' };
  const scope = (compact: boolean, compactSearch: boolean, calls: string[] = []) => ({
    nav: { searching: false, items: [] }, route: 'general', navActiveRoute: (route: string) => route, query: '', pal: {}, active: 0,
    compact, compactSearch, compactSet: (open: boolean) => calls.push(`compact ${open}`), onKey: (k: string) => calls.push(`key ${k}`),
  });
  test('w-60 wide, w-14 as a rail, the whole modal over the page while its search is open', () => {
    expect(nav.attribute('testId="settings-navigation"', 'width', state, scope(false, false))).toBe('15rem');
    expect(nav.attribute('testId="settings-navigation"', 'width', state, scope(true, false))).toBe('3.5rem');
    expect(nav.attribute('testId="settings-navigation"', 'width', state, scope(true, true))).toBe('100%');
    expect(nav.attribute('testId="settings-navigation"', 'position', state, scope(true, true))).toBe('absolute');
  });
  test('/ and ⌘F open the rail\'s search before they focus the field; a blur with no query folds it', () => {
    const calls: string[] = [];
    nav.call('searchShortcut', state, scope(true, false, calls), []);
    expect(calls).toEqual(['compact true', 'key /']);
    calls.length = 0;
    nav.call('searchBlur', state, scope(true, true, calls), []);
    expect(calls).toEqual(['compact false']);
    calls.length = 0;
    nav.call('searchShortcut', state, scope(false, false, calls), []);
    expect(calls).toEqual(['key /']);
  });
});
