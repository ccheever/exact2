// Lane settings-a: an imported Open VSX theme collection on the Appearance page.
import { describe, expect, test } from 'bun:test';
import { libraryCards, removalPicks, removeThemes, toggleRemovalPick, useCollectionDefaults, variantLabels } from './settings-a-collections';
import type { CustomTheme } from './settings-themes';
import { importView, installOpenVsx, searchState, themeImportCommand, type Extension } from './settings-appearance-import';
import { toasts } from './toast';
import type { T3Client } from './client';
import type { Native } from './protocol';

const roles = (canvas: string) => ({ canvas, accent: '#3355ff', messageAction: '#ff3355' });
const member = (id: string, label: string, appearance: 'light' | 'dark'): CustomTheme => ({ id, label, appearance, light: appearance === 'light' ? roles('#ffffff') : null,
  dark: appearance === 'dark' ? roles('#101010') : null, collection: { id: 'open-vsx:catppuccin.catppuccin-vsc', label: 'Catppuccin' } });
const custom = [member('latte', 'Catppuccin Latte', 'light'), member('frappe', 'Catppuccin Frappé', 'dark'), member('macchiato', 'Catppuccin Macchiato', 'dark'),
  member('mocha', 'Catppuccin Mocha', 'dark'), { id: 'solo', label: 'Solo', appearance: 'dark', light: null, dark: roles('#000000') } as CustomTheme];

describe('theme collections', () => {
  test('variant labels drop the shared words', () => {
    expect(variantLabels(['Catppuccin Latte', 'Catppuccin Frappé', 'Catppuccin Mocha'])).toEqual(['Latte', 'Frappé', 'Mocha']);
    expect(variantLabels(['One Dark', 'One Dark'])).toEqual(['Dark', 'Dark']);
    expect(variantLabels(['Nord', 'Monokai'])).toEqual(['Nord', 'Monokai']);
  });
  test('one card per collection: a root per appearance, the fan of its variants, a one-appearance theme keeps its own orb', () => {
    const cards = libraryCards({ themeLight: 't3-code', themeDark: 'macchiato' }, custom);
    expect(cards.map(card => card.id)).toEqual(['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', 'collection:open-vsx:catppuccin.catppuccin-vsc', 'solo']);
    const card = cards[6]!;
    expect(card).toMatchObject({ label: 'Catppuccin', collection: true, primary: 'macchiato', primaryLabel: 'Catppuccin Macchiato', picked: 0, pickedIds: '' });
    expect(card.variants.map(v => [v.mode, v.label, v.more, v.active, v.offset])).toEqual([['light', 'Latte', 0, false, -52], ['dark', 'Macchiato', 2, true, 52]]);
    expect(card.variants[0]!.aria).toBe('Use light variant, currently Latte');
    expect(card.variants[1]!.aria).toBe('Choose dark variant, 3 options, currently Macchiato');
    expect(card.variants[0]!.options).toEqual([]);
    expect(card.variants[1]!.options.map(o => [o.label, o.x, o.y, o.delay, o.active])).toEqual([['Frappé', 18, 5, 0, false], ['Macchiato', 52, 0, 35, true], ['Mocha', 86, 5, 70, false]]);
    expect(card.variants[1]!.options[1]!.aria).toBe('Use Macchiato for dark mode, currently active');
    expect(card.removal.map(option => [option.themeId, option.orbs.map(orb => orb.mode), option.checked])).toEqual([['latte', ['light'], false], ['frappe', ['dark'], false], ['macchiato', ['dark'], false], ['mocha', ['dark'], false]]);
    expect(cards[7]!.orbs.map(orb => orb.mode)).toEqual(['dark']);
  });
  test('the title uses the first variants; the removal dialog checks variants and removes them', () => {
    const local = { customThemes: custom, clientSettings: { theme: 't3-code', themeLight: 't3-code', themeDark: 'mocha' } };
    useCollectionDefaults(local, 'open-vsx:catppuccin.catppuccin-vsc');
    expect(local.clientSettings).toEqual({ theme: 't3-code', themeLight: 'latte', themeDark: 'frappe' });
    expect(() => useCollectionDefaults(local, 'gone')).toThrow('That theme collection is no longer installed.');
    const owner = {}, subject = 'collection:open-vsx:catppuccin.catppuccin-vsc';
    toggleRemovalPick(owner, subject, 'latte');
    toggleRemovalPick(owner, subject, 'mocha');
    toggleRemovalPick(owner, subject, 'latte');
    toggleRemovalPick(owner, subject, 'frappe');
    const card = libraryCards(local.clientSettings, local.customThemes, removalPicks(owner, subject))[6]!;
    expect([card.picked, card.pickedIds, card.removal.filter(option => option.checked).map(option => option.themeId)]).toEqual([2, 'mocha,frappe', ['frappe', 'mocha']]);
    // A different dialog subject starts unchecked.
    expect(removalPicks(owner, '').ids).toEqual([]);
    removeThemes(local, card.pickedIds.split(','));
    expect(local.customThemes.map(theme => theme.id)).toEqual(['latte', 'macchiato', 'solo']);
    expect(local.clientSettings).toEqual({ theme: 't3-code', themeLight: 'latte', themeDark: 't3-code' });
    expect(() => removeThemes(local, [])).toThrow('Choose at least one theme to remove.');
  });
  test('an Open VSX install reads the package files and groups several themes as one collection', async () => {
    const urls: string[] = [];
    const files: Record<string, string> = {
      'package.json': JSON.stringify({ contributes: { themes: [{ label: 'Catppuccin Mocha', uiTheme: 'vs-dark', path: './themes/mocha.json' }, { label: 'Catppuccin Latte', uiTheme: 'vs', path: './themes/latte.json' }] } }),
      'themes/mocha.json': JSON.stringify({ type: 'dark', colors: { 'editor.background': '#1e1e2e', 'editor.foreground': '#cdd6f4', 'button.background': '#cba6f7' } }),
      'themes/latte.json': JSON.stringify({ type: 'light', colors: { 'editor.background': '#eff1f5', 'editor.foreground': '#4c4f69', 'button.background': '#8839ef' } }),
    };
    const client = { local: { customThemes: [] }, restAccess: () => ({ call: async (request: { url: string }) => {
      urls.push(request.url);
      const path = request.url.split('/3.19.0/extension/')[1] ?? '';
      return files[path] === undefined ? { ok: false, message: 'Open VSX search is unavailable right now.' } : { ok: true, text: files[path] };
    } }) } as unknown as T3Client;
    const ext = { id: 'Catppuccin.catppuccin-vsc', namespace: 'Catppuccin', name: 'catppuccin-vsc', displayName: 'Catppuccin for VSCode', version: '3.19.0' } as Extension;
    const themes = await installOpenVsx(client, {} as Native, ext);
    expect(urls[0]).toBe('https://open-vsx.org/vscode/unpkg/Catppuccin/catppuccin-vsc/3.19.0/extension/package.json');
    expect(themes.map(theme => [theme.label, theme.appearance, theme.collection])).toEqual([
      ['Catppuccin Mocha', 'dark', { id: 'open-vsx:catppuccin.catppuccin-vsc', label: 'Catppuccin for VSCode' }],
      ['Catppuccin Latte', 'light', { id: 'open-vsx:catppuccin.catppuccin-vsc', label: 'Catppuccin for VSCode' }]]);
    // Installing adds without activating ("2 themes added"); installing again asks, then replaces with the same ids.
    const native = { available: true } as unknown as Native;
    searchState(client).results = [ext];
    await themeImportCommand(client, native, 'install', ext.id);
    const local = client.local as unknown as { customThemes: CustomTheme[] };
    const ids = local.customThemes.map(theme => theme.id);
    expect(ids.length).toBe(2);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: '2 themes added', description: 'Catppuccin Mocha, Catppuccin Latte' });
    searchState(client).results = [ext];
    await expect(themeImportCommand(client, native, 'install', ext.id)).rejects.toThrow('toasted:');
    expect(importView(client)).toMatchObject({ pendingUpdate: 'Catppuccin.catppuccin-vsc', pendingUpdateName: 'Catppuccin for VSCode' });
    await themeImportCommand(client, native, 'install-confirm', ext.id);
    expect(local.customThemes.map(theme => theme.id)).toEqual(ids);
    expect(toasts(client).at(-1)).toMatchObject({ title: '2 themes updated' });
  });
});
