// Ported from T3 Code 1e2ecbd975 themeEditorStore.test.ts and ThemeEditorHost.test.tsx
// (MIT, LICENSE-T3), original names. The host's React harness becomes logic over the
// session and the library: sessionThemes reads each named theme fresh, as useThemeDefinition does.
import { afterEach, describe, expect, it, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { decodeClientPrefs } from './settings-core';
import type { CustomTheme } from './settings-themes';
import { createThemeEditorStore, sessionThemes, toggleThemeEditorForTheme, type ThemeEditorSession } from './theme-editor-session';
import { STORAGE_FAILED, themeSavedNotice } from './theme-editor-notices';
import { editDraft, editorView, syncDraft, themeEditorCommand } from './settings-appearance-editor';
import { toasts } from './toast';

const store = createThemeEditorStore();
afterEach(() => { store.closeThemeEditor(); });

describe('toggleThemeEditorForTheme', () => {
  it('opens a new editor seeded from the theme active for the current appearance', () => {
    toggleThemeEditorForTheme(store, { theme: 't3-chat', themeHalves: { dark: 'ocean' }, initialAppearance: 'dark' });
    expect(store.session).toMatchObject({ editingThemeId: null, seedThemeId: 'ocean', seedName: null, initialAppearance: 'dark' });
  });
  it('closes an open editor', () => {
    toggleThemeEditorForTheme(store, { theme: 't3-chat', themeHalves: null, initialAppearance: 'light' });
    toggleThemeEditorForTheme(store, { theme: 't3-chat', themeHalves: null, initialAppearance: 'light' });
    expect(store.session).toBeNull();
  });
});

const theme = (id: string, accent: string): CustomTheme => ({ id, label: id, appearance: 'dark', dark: { accent } });
const sessionFor = (field: 'editingTheme' | 'seedTheme', id: string, sessionId = 1): ThemeEditorSession =>
  ({ id: sessionId, editingThemeId: field === 'editingTheme' ? id : null, seedThemeId: field === 'seedTheme' ? id : null, seedName: null, initialAppearance: 'dark' });

describe('ThemeEditorHost', () => {
  it.each(['editingTheme', 'seedTheme'] as const)('reopens the same %s with its saved colors', field => {
    let library = [theme('saved-colors', '#1f6e4a')];
    const session = sessionFor(field, 'saved-colors');
    expect(sessionThemes(session, library)[field]?.custom?.dark?.accent).toBe('#1f6e4a');
    library = [theme('saved-colors', '#7241b8')];
    expect(sessionThemes(null, library)[field]).toBeNull();
    expect(sessionThemes({ ...session, id: 2 }, library)[field]?.custom?.dark?.accent).toBe('#7241b8');
  });
  it.each(['editingTheme', 'seedTheme'] as const)('refreshes an open %s when the library changes', field => {
    const session = sessionFor(field, 'updated-theme');
    let library = [theme('updated-theme', '#1f6e4a')];
    expect(sessionThemes(session, library)[field]?.custom?.dark?.accent).toBe('#1f6e4a');
    library = [theme('updated-theme', '#7241b8')];
    expect(sessionThemes(session, library)[field]?.custom?.dark?.accent).toBe('#7241b8');
  });
  it('does not keep editing a theme removed from the library', () => {
    const session = sessionFor('editingTheme', 'removed-theme');
    expect(sessionThemes(session, [theme('removed-theme', '#1f6e4a')]).editingTheme?.id).toBe('removed-theme');
    expect(sessionThemes(session, []).editingTheme).toBeNull();
  });
});

describe('theme editor session and save notices (D16)', () => {
  const client = () => ({ local: { deviceSettings: { appearanceMode: 'dark' }, clientSettings: decodeClientPrefs({ themeLight: 'grove', themeDark: 'ocean' }), customThemes: [] as CustomTheme[] } as Obj });
  const as = (fake: ReturnType<typeof client>) => fake as unknown as T3Client;
  const prefs = (fake: ReturnType<typeof client>) => fake.local.clientSettings as { theme: string; themeLight: string; themeDark: string };
  test('a create session is seeded from the theme active for the appearance and kept until its dialog changes', () => {
    const fake = client();
    const draft = syncDraft(as(fake), 'create', '', prefs(fake), 'dark')!;
    expect([draft.editingId, draft.appearance, draft.name]).toEqual(['', 'dark', '']);
    expect(syncDraft(as(fake), 'create', '', prefs(fake), 'dark')).toBe(draft);
    expect(editorView(syncDraft(as(fake), 'duplicate', 'grove', prefs(fake), 'dark')).name).toBe('Grove copy');
    expect(syncDraft(as(fake), '', '', prefs(fake), 'dark')).toBeNull();
  });
  test('create, edit, merge and a removed theme each say what happened', async () => {
    const fake = client();
    syncDraft(as(fake), 'create', '', prefs(fake), 'dark');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Aurora');
    expect(toasts(as(fake)).at(-1)).toMatchObject({ kind: 'success', title: 'Aurora created', description: 'It’s now active.' });
    expect(prefs(fake).themeDark).toBe('aurora');
    syncDraft(as(fake), 'edit', 'aurora', prefs(fake), 'dark');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Aurora');
    expect(toasts(as(fake)).at(-1)).toMatchObject({ title: 'Aurora saved', description: 'Your changes are now active.' });
    // A light-only theme gains a dark palette when a create takes its name.
    (fake.local.customThemes as CustomTheme[]).push({ id: 'dawn', label: 'Dawn', appearance: 'light', light: { accent: '#ff8800' } });
    syncDraft(as(fake), 'create', '', prefs(fake), 'dark');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Dawn');
    expect(toasts(as(fake)).at(-1)).toMatchObject({ title: 'Dawn updated', description: 'Its dark palette was added.' });
    expect((fake.local.customThemes as CustomTheme[]).find(entry => entry.id === 'dawn')!.dark).toBeDefined();
    // Editing a theme that is removed while the editor is open saves a new one.
    syncDraft(as(fake), 'edit', 'aurora', prefs(fake), 'dark');
    editDraft(as(fake), 'name', 'Aurora');
    fake.local.customThemes = (fake.local.customThemes as CustomTheme[]).filter(entry => entry.id !== 'aurora');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Aurora');
    expect(toasts(as(fake)).at(-1)).toMatchObject({ title: 'Aurora created' });
  });
  test('an edit of an inactive theme is saved without activating it; a failed activation reports storage', () => {
    const saved = { id: 'dusk', label: 'Dusk' };
    expect(themeSavedNotice(saved, { created: false }, { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' }, () => true))
      .toEqual({ kind: 'success', title: 'Dusk saved', description: 'Your changes are saved.' });
    expect(themeSavedNotice(saved, { created: true }, { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' }, () => false)).toEqual(STORAGE_FAILED);
  });
});
