// Add a theme's conflict step (ThemeImportDialog.tsx at v0.1.6): an installed theme asks Update existing or Keep
// both, and a batch shows that step even when another file failed.
import { describe, expect, test } from 'bun:test';
import { importView, searchState, themeImportCommand } from '../../src/appearance/settings-appearance-import';
import type { CustomTheme } from '../../src/appearance/settings-themes';
import { toasts } from '../../src/shell/toast';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';

const file = (name: string, canvas: string, appearance = 'light') => JSON.stringify({ version: 1, name, appearance, colors: { canvas } });
const installed = (): CustomTheme => ({ id: 'aurora', label: 'Aurora', appearance: 'light', light: { canvas: '#ffffff' }, dark: null, collection: { id: 'mine', label: 'Mine' } });
function setup(picked: { name: string; text: string }[] = []) {
  const client = { local: { customThemes: [installed()], clientSettings: { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' } },
    restAccess: () => ({ call: async () => ({ files: picked.map(entry => ({ ...entry, size: entry.text.length })) }) }) } as unknown as T3Client;
  return { client, native: { available: true } as unknown as Native, themes: () => (client.local as unknown as { customThemes: CustomTheme[] }).customThemes };
}

describe('theme import conflicts', () => {
  test('a pasted theme that is installed asks first; Update existing replaces it in place and keeps its collection', async () => {
    const { client, themes } = setup();
    await expect(themeImportCommand(client, null, 'add', file('Aurora', '#eeeeee'))).rejects.toThrow('toasted:');
    expect(importView(client).conflicts).toBe('Aurora');
    expect(themes().length).toBe(1);
    await themeImportCommand(client, null, 'conflict-update', '');
    expect(themes()).toEqual([{ id: 'aurora', label: 'Aurora', appearance: 'light', light: expect.objectContaining({ canvas: '#eeeeee' }), dark: null, collection: { id: 'mine', label: 'Mine' } }]);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Aurora updated', description: 'Aurora' });
    expect(importView(client).conflicts).toBe('');
  });

  test('Keep both adds a numbered copy, or the picked file\'s own name', async () => {
    const { client, themes } = setup();
    await expect(themeImportCommand(client, null, 'add', file('Aurora', '#dddddd'))).rejects.toThrow('toasted:');
    await themeImportCommand(client, null, 'conflict-copy', '');
    expect(themes().map(theme => [theme.id, theme.label])).toEqual([['aurora', 'Aurora'], ['aurora-1', 'Aurora (1)']]);
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Aurora (1) added' });
    searchState(client).fileName = 'aurora-dawn.json';
    await expect(themeImportCommand(client, null, 'add', file('Aurora', '#cccccc'))).rejects.toThrow('toasted:');
    await themeImportCommand(client, null, 'conflict-copy', '');
    expect(themes().at(-1)).toMatchObject({ id: 'aurora-dawn', label: 'Aurora Dawn' });
  });

  test('a batch installs the new themes and shows the conflicts even when another file failed', async () => {
    const { client, native, themes } = setup([{ name: 'aurora.json', text: file('Aurora', '#eeeeee') }, { name: 'dusk.json', text: file('Dusk', '#222222', 'dark') },
      { name: 'broken.json', text: '{"version":2}' }]);
    await expect(themeImportCommand(client, native, 'choose', '')).rejects.toThrow('broken.json: This theme file uses an unsupported version. Expected 1.');
    expect(themes().map(theme => theme.id)).toEqual(['aurora', 'dusk']);
    expect(importView(client).conflicts).toBe('Aurora');
    // Back leaves the step and keeps the failure showing; resolving keeps it too, so the dialog stays open.
    await expect(themeImportCommand(client, native, 'conflict-back', '')).rejects.toThrow('broken.json:');
    expect(importView(client).conflicts).toBe('');
    // Picked again, both installed themes wait in the step.
    await expect(themeImportCommand(client, native, 'choose', '')).rejects.toThrow('broken.json:');
    expect(importView(client).conflicts).toBe('Aurora, Dusk');
    await expect(themeImportCommand(client, native, 'conflict-update', '')).rejects.toThrow('broken.json:');
    expect(toasts(client).at(-1)).toMatchObject({ title: '2 themes updated', description: 'Aurora, Dusk' });
  });

  test('a batch with only conflicts stays open on the step without an error', async () => {
    const { client, native } = setup([{ name: 'aurora.json', text: file('Aurora', '#eeeeee') }, { name: 'dawn.json', text: file('Dawn', '#fafafa') }]);
    await expect(themeImportCommand(client, native, 'choose', '')).rejects.toThrow('toasted:');
    expect(importView(client).conflicts).toBe('Aurora');
    await themeImportCommand(client, native, 'conflict-update', '');
    expect(importView(client).conflicts).toBe('');
  });
});
