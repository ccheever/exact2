// settings-escape-and-nav (2026-10-09 desktop audit S1-1..S1-4, S2-1, S2-5), read from the Contract sources as
// theme-color-picker.test.ts and dialog-focus.test.ts read theirs; the keys and the drag are driven on macOS
// (tasks/20261009-settings-escape-and-nav.md). Reference 1e2ecbd975: useEscapeToGoBack (hooks/useNavigateBack.ts)
// leaves Settings only for an Escape nothing prevented; KeybindingsSettings.tsx's searches, When popover and
// capture input, FontFamilyPicker's Combobox, ProviderModelsSection's slug field and every Base UI Dialog consume
// it first. The app's shortcut listeners skip a key from `[data-keybinding-capture]` (AppSidebarLayout.tsx), and
// Settings renders inside the app Sidebar, whose SidebarRail resizes the width both navs share.
import { describe, expect, test } from 'bun:test';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
/** The lines of one component, from its `component` line to the next top-level declaration. */
async function component(file: string, name: string): Promise<string> {
  const text = await source(file);
  const start = text.indexOf(`\ncomponent ${name}\n`);
  expect(start).toBeGreaterThanOrEqual(0);
  const rest = text.slice(start + 1);
  const end = rest.slice(1).search(/\n(component|shape|style|fn|use) /);
  return end < 0 ? rest : rest.slice(0, end + 1);
}
const line = (text: string, needle: string) => text.split('\n').find(candidate => candidate.includes(needle)) ?? '';

describe('Escape belongs to its owner, then to Settings\' Back', () => {
  test('one escapeOwned value names every owner, and Back gives up its Escape while it holds', async () => {
    const settings = await component('app-settings.contract', 'SettingsWindow');
    const owned = line(settings, 'derive escapeOwned =');
    for (const owner of ['settingsMenu != ""', 'modelsOpen', 'settingsRestoreOpen', 'settingsEscapeHeld', 'snapshotSetupOpen',
      '(settingsRoute == "keybindings" and (keybindingSearch or keybindingRecording))',
      '(settingsRoute == "open-source-licenses" and licenseSearchOpen)',
      '(settingsRoute == "providers" and (providerPage.escapeOwned or modelAdding != ""))']) expect(owned).toContain(owner);
    expect(line(settings, 'SettingsNav(')).toContain('menuOpen=escapeOwned,');
    const core = await source('settings-core.contract');
    expect(line(core, 'testId="close-settings"')).toContain('aria-keyshortcuts=(menuOpen or query != "" ? "" : "Escape")');
  });

  test('the keybinding and licence searches clear and close on Escape, preventing it (S1-1)', async () => {
    const settings = await component('app-settings.contract', 'SettingsWindow');
    expect(settings).toContain('  action keybindingSearchKey(name: string)\n    if name == "Escape"\n      preventDefault()\n      keybindingSearch = false\n      editKeybindingQuery("")');
    expect(settings).toContain('  action licenseSearchKey(name: string)\n    if name == "Escape"\n      preventDefault()\n      licenseSearchOpen = false');
    const panel = await component('settings-keybindings.contract', 'KeybindingsPanel');
    // InputGroupInput autoFocus: the field opens with the focus, so its Escape is the field's.
    expect(line(panel, 'testId="keybindings-filter"')).toContain('key=searchKey blur=searchToggle(false) autofocus=true');
  });

  test('the When editor and the font list are modal scopes while they show, so the host\'s light dismiss gets Escape (S1-1)', async () => {
    const when = await component('settings-keybindings.contract', 'WhenEditor');
    for (const part of ['id="keybinding-when-editor"', 'popover="auto"', 'aria-modal=true']) expect(line(when, 'testId="keybinding-when-editor"')).toContain(part);
    const fonts = await component('settings-a-fonts.contract', 'FontFamilyPicker');
    for (const part of ['popover="auto"', 'aria-modal=true', 'role="dialog"']) expect(line(fonts, 'testId=`font-picker-${rowId}`')).toContain(part);
    // Neither popover has an Escape shortcut of its own: the host closes a popover="auto" on Escape.
    expect(when.split('\n').filter(text => text.includes('aria-keyshortcuts'))).toEqual([]);
    expect(fonts.split('\n').filter(text => text.includes('aria-keyshortcuts'))).toEqual([]);
  });

  test('"Add custom model" is the window\'s state, so its open field owns Escape and its Cancel takes it (S1-2)', async () => {
    const models = await component('providers.contract', 'ProviderModels');
    expect(models).not.toContain('state adding');
    expect(models).toContain('    adding: bool\n');
    expect(models).toContain('  action startAdd\n    slug = ""\n    ui("model-adding", editor.id)');
    expect(models).toContain('  action cancelAdd\n    slug = ""\n    ui("model-adding", "")');
    expect(line(models, 'testId="custom-model-cancel"')).toContain('press=cancelAdd');
    expect(line(models, 'testId="custom-model-cancel"')).toContain('aria-keyshortcuts="Escape"');
    expect(line(models, 'testId="custom-model-slug"')).toContain('autofocus=true');
    const editor = await component('providers.contract', 'ProviderEditor');
    expect(line(editor, 'ProviderModels(')).toContain('ui=ui, adding=(modelAdding == editor.id))');
    const settings = await component('app-settings.contract', 'SettingsWindow');
    expect(line(settings, 'ProvidersPanel(')).toContain('ui=providerUiWindow, modelAdding=modelAdding)');
    expect(settings).toContain('  action providerUiWindow(what: string, value: string)\n    if what == "model-adding"\n      modelAdding = value\n    else\n      if what == "select"\n        modelAdding = ""\n      providerUi(what, value)');
    // A route change unmounts the section (isAdding resets), so the window forgets the open field.
    expect(settings).toContain('  action coreNavigate(route: string)\n    settingsEscapeHeld = false\n    modelAdding = ""');
  });

  test('a Settings dialog makes the page inert, so its own Escape closes only it (S2-1)', async () => {
    const settings = await component('app-settings.contract', 'SettingsWindow');
    expect(line(settings, 'derive dialogShown =')).toContain('restEditor != "" or settingsCore.saOpen or settingsCore.archiveConfirm.id != ""');
    expect(line(settings, 'testId="settings-dialog"')).toContain('inert=(connecting or dialogShown or ');
    // New task's Close and Add device host's Cancel declare Escape; they live outside the inert page.
    expect(line(await source('settings-scheduled.contract'), 'testId="close-scheduled-task"')).toContain('aria-keyshortcuts="Escape"');
    expect(line(await source('settings-a-hosts.contract'), 'testId="device-host-cancel"')).toContain('aria-keyshortcuts="Escape"');
    const window = await source('app-window.contract');
    expect(window.indexOf('        ProjectDialogs(')).toBeGreaterThan(window.indexOf('        SettingsWindow('));
  });
});

describe('the keybinding recorder owns every key while it records (S1-3, S1-4)', () => {
  test('the recording field is the window\'s modal scope, and leaving it stops recording', async () => {
    const capture = await component('settings-keybindings.contract', 'CaptureField');
    expect(line(capture, 'testId="keybinding-recording"')).toContain('aria-modal=true');
    const recorder = line(capture, 't3-key-recorder');
    expect(recorder).toContain('change=capture(original) blur=capture(original, "blur")');
    const app = await source('app.contract');
    expect(app).toContain('    keybindingKey = value == "escape" ? original : (value == "blur" ? keybindingKey : value)\n    keybindingRecording = false');
  });

  test('the app\'s shortcut buttons (and their menu items) are off while it records', async () => {
    const window = await source('app-window.contract');
    expect(line(window, 'ShortcutDispatch(')).toContain('ShortcutDispatch(items=(keybindingRecording and settingsOpen and settingsRoute == "keybindings" ? [] : shortcuts), ');
  });

  test('a conflict warning carries the reference\'s tooltip sentence', async () => {
    const panel = await source('settings-keybindings.contract');
    const warning = await component('settings-keybindings.contract', 'Warning');
    expect(warning).toContain('Tip(label=detail, side="top", align="center", offset=24)');
    expect(panel).toContain('Warning(label=data.draftConflict, detail=`${data.draftConflict} The most recent matching binding wins when both conditions can apply.`)');
    expect(panel).toContain('Warning(label=conflict, detail=`${conflict} The most recent matching binding wins when both conditions can apply.`)');
    expect(panel).toContain('Warning(label=view.unknown, detail="T3 Code does not recognize this condition yet. It can still be saved, but it may not match unless the runtime provides it.")');
  });
});

describe('the Settings nav is the app sidebar: its width and its rail (S2-5)', () => {
  test('the nav takes the shared sidebar width, clamped as the thread sidebar\'s, not a fifth of the window', async () => {
    const settings = await component('app-settings.contract', 'SettingsWindow');
    expect(settings).not.toContain('viewport.width / 5');
    expect(line(settings, 'SettingsNav(')).toContain('width=sidebarWidth,');
    for (const panel of ['ConnectionsPanel(', 'ProvidersPanel(']) expect(line(settings, panel)).toContain('(data.sidebarOpen ? sidebarWidth : 0)');
    const app = await source('app.contract');
    expect(line(app, 'derive sidebarWidth =')).toContain('viewport.width - 640'); // THREAD_MAIN_CONTENT_MIN_WIDTH
  });

  test('the rail and its tooltip sit over the nav\'s edge, resizing and resetting the shared width', async () => {
    const settings = await component('app-settings.contract', 'SettingsWindow');
    expect(line(settings, 'SidebarWindowRail(')).toContain('SidebarWindowRail(width=sidebarWidth, hovered=railHover,');
    expect(line(settings, 'SidebarWindowRail(')).toContain('resize=resizeSidebar, finishResize=finishResize, resetWidth=resetSidebarWidth, hoverCard=hoverRail)');
    expect(settings).toContain('TipCard(label="Drag to resize sidebar", shown=railHover)');
    const rail = await component('r5-shell.contract', 'SidebarWindowRail');
    expect(rail).toContain('aria-label="Resize Sidebar" testId="sidebar-resize"');
    const window = await source('app-window.contract');
    expect(line(window, '        SettingsWindow(')).toContain('resizeSidebar=resizeSidebar, finishResize=finishResize, resetSidebarWidth=resetSidebarWidth)');
  });
});
