// settings-appearance-and-skill-chip (desktop audit 2026-10-09, S1-5 and S1-6), against T3 Code 1e2ecbd975
// (MIT, LICENSE-T3): ThemeEditorPanel.tsx:395-399 opens Advanced for any source theme the guided editor
// did not make, and openVsxThemes.ts:195 names an Open VSX result's publisher by its namespace.
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';
import { decodeClientPrefs } from './settings-core';
import { decodeCustomThemes, parseThemeFile, type CustomTheme } from './settings-themes';
import { editDraft, editorView, opensAdvanced, serializeTheme, syncDraft, themeEditorCommand } from './settings-appearance-editor';
import { builtInThemeColors } from './settings-theme-library';
import { searchOpenVsx } from './settings-appearance-import';

const client = (themeLight = 't3-code') => ({ local: { deviceSettings: { appearanceMode: 'light' }, clientSettings: decodeClientPrefs({ theme: themeLight, themeLight, themeDark: 't3-code' }), customThemes: [] as CustomTheme[] } as Obj });
const as = (fake: ReturnType<typeof client>) => fake as unknown as T3Client;
const prefs = (fake: ReturnType<typeof client>) => fake.local.clientSettings as { theme: string; themeLight: string; themeDark: string };
const library = (fake: ReturnType<typeof client>) => fake.local.customThemes as CustomTheme[];

describe('the theme editor opens Advanced for a theme the guided editor did not make (S1-5)', () => {
  test('Duplicate T3 Chat: "T3 Chat copy", Advanced, all 20 roles in their four groups with T3 Chat\'s colours', () => {
    const fake = client();
    const view = editorView(syncDraft(as(fake), 'duplicate', 't3-chat#1', prefs(fake), 'light'));
    expect([view.title, view.name, view.advanced]).toEqual(['Create theme', 'T3 Chat copy', true]);
    expect(view.groups.map(group => [group.title, group.rows.length])).toEqual([['Foundation', 8], ['Brand & content', 6], ['Context', 4], ['Status', 2]]);
    // The reference's editor, read from its hex fields (Electron, 1e2ecbd975, 2026-10-09).
    expect(view.groups.flatMap(group => group.rows).map(row => `${row.label} ${row.value}`)).toEqual([
      'Background #fdf7fd', 'Surface #faf3fb', 'Raised surface #fdfafd', 'Overlay #ffffff', 'Text #501854', 'Muted text #8d1255', 'Border #eee1ed', 'Input #e7c1dc',
      'Subtle surface #f1c4e6', 'Highlight surface #f3e6f5', 'Accent #db2777', 'Action #db2777', 'Message surface #f7def2', 'Code surface #f5ecf9',
      'Sidebar background #f2e1f4', 'Sidebar controls #f8f8f7', 'Sidebar selection #f8f8f7', 'Terminal background #fdf7fd', 'Error #f7086c', 'Warning #f59e0b']);
    // Every built-in's 57 roles in both appearances; the dark copy is its variant.
    for (const id of ['t3-chat', 'grove', 'ocean', 'ember', 'iris']) for (const mode of ['light', 'dark'] as const) {
      const colors = builtInThemeColors(id, mode)!;
      expect(Object.keys(colors).length).toBe(57);
      expect(Object.values(colors).every(value => /^#[0-9a-f]{6}([0-9a-f]{2})?$/.test(value))).toBe(true);
    }
    expect(builtInThemeColors('t3-code', 'light')).toBeNull();
    expect(editorView(syncDraft(as(fake), 'duplicate', 't3-chat#2', prefs(fake), 'dark')).groups[0]!.rows[0]!.value).toBe(builtInThemeColors('t3-chat', 'dark')!.canvas!);
  });

  test('every built-in source and an unmanaged custom theme open Advanced; T3 Code\'s stock look and a guided save open simple', () => {
    const fake = client();
    for (const id of ['t3-chat', 'grove', 'ocean', 'ember', 'iris']) expect(editorView(syncDraft(as(fake), 'duplicate', `${id}#d`, prefs(fake), 'light')).advanced).toBe(true);
    expect(editorView(syncDraft(as(fake), 'duplicate', 't3-code#d', prefs(fake), 'light'))).toMatchObject({ name: 'T3 Code copy', advanced: false });
    expect(editorView(syncDraft(as(fake), 'create', '#c', prefs(fake), 'light')).advanced).toBe(false);
    // Create theme seeds from the active theme: T3 Chat is unmanaged.
    const chat = client('t3-chat');
    expect(editorView(syncDraft(as(chat), 'create', '#c', prefs(chat), 'light'))).toMatchObject({ title: 'Create theme', advanced: true });
    expect(opensAdvanced(null)).toBe(false);
    expect(opensAdvanced({ id: 'old', custom: { id: 'old', label: 'Old', appearance: 'light', light: {}, dark: null } })).toBe(true);
    expect(opensAdvanced({ id: 'guided', custom: { id: 'guided', label: 'Guided', appearance: 'light', light: {}, dark: null, managed: true } })).toBe(false);
  });

  test('a save from Advanced stays unmanaged and its Edit opens Advanced; a save from simple is managed and its Edit opens simple', async () => {
    const fake = client();
    syncDraft(as(fake), 'duplicate', 't3-chat#1', prefs(fake), 'light');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', '');
    const copy = library(fake).find(theme => theme.label === 'T3 Chat copy')!;
    expect(copy.managed).toBeUndefined();
    expect(editorView(syncDraft(as(fake), 'edit', `${copy.id}#2`, prefs(fake), 'light'))).toMatchObject({ title: 'Edit theme', name: 'T3 Chat copy', advanced: true });
    // Advanced off, then Save: the guided editor's theme, which opens simple again.
    editDraft(as(fake), 'advanced', 'false');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', '');
    expect(library(fake).find(theme => theme.id === copy.id)!.managed).toBe(true);
    expect(editorView(syncDraft(as(fake), 'edit', `${copy.id}#3`, prefs(fake), 'light')).advanced).toBe(false);
    // A new theme made in simple mode is managed; one made in Advanced is not.
    syncDraft(as(fake), 'create', '#4', prefs(fake), 'light');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Simple one');
    syncDraft(as(fake), 'create', '#5', prefs(fake), 'light');
    editDraft(as(fake), 'advanced', 'true');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Hand tuned');
    expect(library(fake).map(theme => [theme.label, theme.managed === true])).toEqual([['T3 Chat copy', true], ['Simple one', true], ['Hand tuned', false]]);
  });

  test('a palette added to a guided theme keeps the flag only from simple mode; an edit keeps its collection', async () => {
    const fake = client();
    library(fake).push({ id: 'pair', label: 'Pair', appearance: 'light', light: { canvas: '#ffffff' }, dark: null, managed: true },
      { id: 'mocha', label: 'Mocha', appearance: 'dark', light: null, dark: { canvas: '#1e1e2e' }, collection: { id: 'open-vsx:catppuccin.catppuccin-vsc', label: 'Catppuccin' } });
    syncDraft(as(fake), 'create', '#1', prefs(fake), 'dark');
    editDraft(as(fake), 'appearance', 'dark');
    editDraft(as(fake), 'advanced', 'true');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Pair');
    const pair = library(fake).find(theme => theme.id === 'pair')!;
    expect([pair.light?.canvas, pair.dark?.canvas, pair.managed]).toEqual(['#ffffff', '#0a0a0a', undefined]);
    expect(editorView(syncDraft(as(fake), 'edit', 'mocha#2', prefs(fake), 'dark')).advanced).toBe(true);
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', '');
    expect(library(fake).find(theme => theme.id === 'mocha')!.collection).toEqual({ id: 'open-vsx:catppuccin.catppuccin-vsc', label: 'Catppuccin' });
  });

  test('the flag survives the saved preferences, the theme file and an import', () => {
    const guided: CustomTheme = { id: 'guided', label: 'Guided', appearance: 'light', light: { canvas: '#ffffff' }, dark: null, managed: true };
    expect(decodeCustomThemes([guided, { ...guided, id: 'plain', managed: false }]).map(theme => theme.managed)).toEqual([true, undefined]);
    const file = serializeTheme(guided);
    expect(JSON.parse(file).managed).toBe(true);
    expect(parseThemeFile(file, []).managed).toBe(true);
    expect(JSON.parse(serializeTheme({ ...guided, managed: undefined })).managed).toBeUndefined();
    expect(parseThemeFile(serializeTheme({ ...guided, managed: undefined }), []).managed).toBeUndefined();
  });
});

describe('Open VSX results name the extension\'s namespace as the publisher (S1-6)', () => {
  test('the namespace, not the uploader\'s login (the audit\'s Dracula results)', async () => {
    const extensions = [['dracula-theme', 'theme-dracula', 'open-vsx', 431500], ['Dracula-2', 'dracula-2', 'TimDeen', 104000], ['bceskavich', 'dracula-pro', 'open-vsx', 18000], ['MateuszDrewniak', 'dracula-dark', 'Verseth', 16900]] as const;
    const fake = { local: { customThemes: [] }, restAccess: () => ({ call: async (request: { url: string }) => {
      const url = request.url;
      if (url.startsWith('https://open-vsx.org/api/-/search?')) return { ok: true, text: JSON.stringify({ extensions: extensions.map(([namespace, name]) => ({ namespace, name })) }) };
      const entry = extensions.find(([namespace, name]) => url === `https://open-vsx.org/api/${namespace}/${name}`);
      if (entry) return { ok: true, text: JSON.stringify({ namespace: entry[0], name: entry[1], displayName: entry[1], version: '1.0.0', license: 'MIT', downloadCount: entry[3], publishedBy: { loginName: entry[2] } }) };
      if (url.endsWith('/extension/package.json')) return { ok: true, text: JSON.stringify({ license: 'MIT', contributes: { themes: [{ label: 'Dracula', uiTheme: 'vs-dark', path: './theme.json' }] } }) };
      return { ok: false, message: 'missing' };
    } }) } as unknown as T3Client;
    const results = await searchOpenVsx(fake, {} as Native, 'Dracula', 'downloadCount');
    expect(results.map(result => `${result.publisher} · ${result.downloads}`)).toEqual([
      'dracula-theme · 431.5K downloads', 'Dracula-2 · 104K downloads', 'bceskavich · 18K downloads', 'MateuszDrewniak · 16.9K downloads']);
  });
});
