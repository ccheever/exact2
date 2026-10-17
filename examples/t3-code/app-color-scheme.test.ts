// Every surface draws in the app's appearance. Settings › Appearance decides it as T3 Code's
// useTheme resolvedTheme does (System follows macOS; Light and Dark override it), and the window shows the same
// (T3WindowChrome.setAppearance). On macOS `viewport.prefersColorScheme` is the system's appearance beneath the
// window's own, so a view that picked its palette from it drew the other palette whenever the two differed.
// The root resolves one `scheme` and hands it down; these checks read the
// Contract sources. The palettes themselves are checked by driving the app on macOS: a source read cannot see a colour.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { decodeClientPrefs } from './settings-core';
import { settingsCore } from './settings-core-view';
import { editorView, syncDraft } from './settings-appearance-editor';
import type { CustomTheme } from './settings-themes';

const dir = new URL('./', import.meta.url);
const source = (file: string) => Bun.file(new URL(file, dir)).text();
const contracts = readdirSync(dir).filter(file => file.endsWith('.contract')).sort();
const DERIVE = '  derive scheme = ';

/** The resolved scheme's expression as a JavaScript function of the mode and the system's scheme. */
async function resolved(): Promise<(mode: string, system: string) => string> {
  const line = (await source('app.contract')).split('\n').find(text => text.startsWith(DERIVE));
  if (!line) throw new Error('app.contract: no derive scheme');
  const body = line.slice(DERIVE.length).replaceAll(' or ', ' || ').replaceAll(' and ', ' && ')
    .replaceAll('data.look.mode', 'mode').replaceAll('viewport.prefersColorScheme', 'system');
  return new Function('mode', 'system', `return ${body};`) as (mode: string, system: string) => string;
}

describe('one resolved colour scheme', () => {
  test('viewport.prefersColorScheme is read once, by the root\'s resolved scheme; no view picks a palette from it', async () => {
    const sites: string[] = [];
    for (const file of contracts) {
      (await source(file)).split('\n').forEach((line, index) => {
        if (line.includes('viewport.prefersColorScheme') && !line.trimStart().startsWith('//')) sites.push(`${file}:${index + 1}`);
      });
    }
    const app = (await source('app.contract')).split('\n');
    expect(sites).toEqual([`app.contract:${app.findIndex(line => line.startsWith(DERIVE)) + 1}`]);
    // The views that resolved the mode themselves (the terminal drawer, Settings › Providers, the provider wizard) take it too.
    for (const file of contracts) expect(`${file}: ${(await source(file)).includes('data.look.mode == "system"')}`).toBe(`${file}: false`);
  });

  test('System follows the system appearance; Light and Dark override it', async () => {
    const scheme = await resolved();
    expect(scheme('system', 'dark')).toBe('dark');
    expect(scheme('system', 'light')).toBe('light');
    expect(scheme('light', 'dark')).toBe('light');
    expect(scheme('dark', 'light')).toBe('dark');
    expect(scheme('light', 'light')).toBe('light');
    expect(scheme('dark', 'dark')).toBe('dark');
    // Before the device settings are read the window follows the system (setAppearance ignores another mode).
    expect(scheme('', 'dark')).toBe('dark');
    expect(scheme('', 'light')).toBe('light');
  });

  // The theme editor's first appearance is useTheme's resolvedTheme (SettingsPanels.tsx:1189 ThemeLibrary
  // initialAppearance, CommandPalette.tsx:557 themeEditor.toggle), not the mode: System on a dark Mac opens Dark.
  test('the theme editor opens on the resolved scheme', async () => {
    expect(await source('app.contract')).toContain('settingsThemeDialog, settingsThemeSubject, delivery.stream, delivery.staged, scheme) as shape SettingsCore');
    expect(await source('app.ts')).toContain("String(args[11] || 'embedded'), args[12] === true, String(args[13] || 'light'));");
    const native = { available: true, later: async () => ({ ok: true, generation: 0, value: { fonts: [] } }) } as unknown as Native;
    const opened = async (mode: string, scheme: string, kind: string, subject: string) => {
      const client = { local: { deviceSettings: { appearanceMode: mode }, clientSettings: decodeClientPrefs({}), customThemes: [] }, ready: true, environmentId: 'env1', revision: 0,
        config: { environment: { environmentId: 'env1', label: 'Studio' }, providers: [], settings: {} }, projectGroups: () => [] } as unknown as T3Client;
      return (await settingsCore(client, native, '', '', '', '', 'appearance', '', true, kind, subject, 'embedded', false, scheme)).editor.appearance;
    };
    for (const kind of ['create', 'duplicate']) {
      expect(`${kind} system/dark: ${await opened('system', 'dark', kind, 't3-chat#1')}`).toBe(`${kind} system/dark: dark`);
      expect(`${kind} system/light: ${await opened('system', 'light', kind, 't3-chat#1')}`).toBe(`${kind} system/light: light`);
      expect(`${kind} dark/dark: ${await opened('dark', 'dark', kind, 't3-chat#1')}`).toBe(`${kind} dark/dark: dark`);
      expect(`${kind} light/light: ${await opened('light', 'light', kind, 't3-chat#1')}`).toBe(`${kind} light/light: light`);
    }
  });

  // ThemeSettings.tsx hands an edit the same resolved appearance, and ThemeEditorPanel.tsx:379-383 keeps it when the
  // source theme has colours for it, else takes the theme's own appearance.
  test('an edit opens on the resolved scheme too; a one-appearance theme opens on its own', () => {
    const roles = { canvas: '#101820', accent: '#44cc88' };
    const themes: CustomTheme[] = [{ id: 'both', label: 'Both', appearance: 'light', light: roles, dark: roles },
      { id: 'night', label: 'Night', appearance: 'dark', light: null, dark: roles }, { id: 'day', label: 'Day', appearance: 'light', light: roles, dark: null }];
    const client = { local: { deviceSettings: { appearanceMode: 'system' }, clientSettings: decodeClientPrefs({}), customThemes: themes } } as unknown as T3Client;
    const prefs = { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' };
    const open = (kind: string, subject: string, scheme: 'light' | 'dark') => editorView(syncDraft(client, kind, subject, prefs, scheme)).appearance;
    expect(open('edit', 'both#1', 'dark')).toBe('dark'); // was the theme's own (light)
    expect(open('edit', 'both#2', 'light')).toBe('light');
    expect(open('edit', 'night#3', 'light')).toBe('dark');
    expect(open('edit', 'day#4', 'dark')).toBe('light');
    expect(open('duplicate', 'day#5', 'dark')).toBe('light'); // was dark, with no colours of its own to show
    expect(open('duplicate', 'both#6', 'dark')).toBe('dark');
    expect(open('duplicate', 'grove#7', 'dark')).toBe('dark');
  });
});
