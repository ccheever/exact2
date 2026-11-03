// The app menu's Settings… (⌘,) is there whatever is open, as the reference's menu item is (DesktopApplicationMenu.ts),
// and opens Settings over the palette, the welcome wizard or a dialog; with Settings open it runs nothing, since the
// reference navigates only from outside the settings route (AppSidebarLayout registerMenuAction("open-settings")).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { keyboardDispatch, type DispatchContext } from '../../src/shell/keyboard-dispatch';
import { keybindingDefaults } from '../../src/settings/keybinding-settings';
import { parseWhen } from '../../src/settings/keybinding-view';
import { appText } from '../support/app-files';
import { contractComponent } from '../support/contract-action-fixture';

function resolved(rule: { key: string; command: string; when?: string }) {
  const tokens = rule.key.split('+'), key = tokens.pop() || '+';
  return { command: rule.command, shortcut: { key, modKey: tokens.includes('mod'), shiftKey: tokens.includes('shift'), altKey: tokens.includes('alt'), ctrlKey: tokens.includes('ctrl'), metaKey: false }, ...(rule.when ? { whenAst: parseWhen(rule.when) } : {}) };
}
function client(): T3Client {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env1', origin: 'http://127.0.0.1:1' });
  c.config = { environment: { label: 'Local', capabilities: {} }, keybindings: keybindingDefaults.map(resolved), settings: {} };
  c.shell = { sequence: 1, projects: [{ id: 'p1', title: 'Widgets', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', title: 'One' }] };
  c.projectId = 'p1'; c.threadId = 't1';
  return c;
}
const rest: DispatchContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
const settingsRow = (context: DispatchContext) => keyboardDispatch(client(), [], '', '', context).filter(row => row.command === 'settings.open');

describe('Settings… in the app menu', () => {
  test('opens Settings with the palette, the welcome wizard or a dialog open, as at rest', () => {
    for (const context of [rest, { ...rest, modalOpen: true, paletteOpen: true }, { ...rest, modalOpen: true }])
      expect(settingsRow(context)).toEqual([expect.objectContaining({ kind: 'settings', chord: 'Meta+,', target: 'general' })]);
  });

  test('runs nothing with Settings open, a palette over it included', () => {
    for (const context of [{ ...rest, modalOpen: true, settingsOpen: true }, { ...rest, modalOpen: true, settingsOpen: true, paletteOpen: true }])
      expect(settingsRow(context)).toEqual([expect.objectContaining({ kind: 'refuse', chord: 'Meta+,' })]);
  });

  test('Settings covers an open palette and the wizard until it closes; a palette opened over Settings is above it', () => {
    const palette = appText('src/palette/palette-members.contract'), settings = appText('src/settings/settings-members.contract');
    expect(palette).toContain('derive paletteShown = paletteOpen and not (settingsOpen and paletteCovered)');
    expect(palette).toMatch(/action paletteShow[\s\S]*?paletteCovered = false/);
    expect(settings).toMatch(/action openSettings[\s\S]*?paletteCovered = paletteOpen/);
    // Leaving Settings (closeModal, its Close and Back) shows a palette it covered again, and closes one it did not.
    const members = contractComponent('src/settings/settings-members.contract', 'T3Code');
    for (const [covered, open] of [[true, true], [false, false]] as const) {
      const state: Record<string, unknown> = { pendingModal: '', connecting: false, projectsOpen: false, paletteOpen: true, settingsOpen: true, paletteCovered: covered, keybindingOpen: false,
        keybindingPrevious: '', keybindingRecording: false, keybindingDrafts: [], providerDialog: '', connectionRemove: '', modelsOpen: false, origin: '', originEdited: false };
      members.call('closeModal', state, {}, []);
      expect(state).toMatchObject({ paletteOpen: open, settingsOpen: false });
    }
    expect(appText('app.contract')).toContain('paletteOpen=paletteShown,');
    expect(appText('src/shell/app-window.contract')).toContain('WelcomeLayer(welcome=welcome, covered=settingsOpen,');
    const layer = contractComponent('src/shell/app-overlays.contract', 'WelcomeLayer');
    for (const covered of [false, true]) expect(layer.drawn('WelcomeWizard(', {}, { welcome: { show: true }, covered })).toBe(!covered);
  });
});
