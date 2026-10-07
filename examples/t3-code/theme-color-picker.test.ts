// The theme editor's colour picker. "picker color conversion" is ported from T3 Code 1e2ecbd975
// lib/color.test.ts and "shared color controls in settings" from settings/colorPickers.test.tsx
// (MIT, LICENSE-T3), original names; the React harness becomes the picker's rules
// (theme-color-picker.ts, mirrored by theme-color-picker.contract) and its draft ops (themeLocal).
import { describe, expect, it, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { decodeClientPrefs } from './settings-core';
import type { CustomTheme } from './settings-themes';
import { editDraft, editorView, previewTheme, syncDraft, themeEditorCommand, themeLocal, updateFamily } from './settings-appearance-editor';
import { contractHsvToHex, hexToHsv, hsvToHex, huePoint, hueKey, pickerCommit, pickerFields, planeKey, planePoint, themePickerAlphaSuffix, themeRgbToHex, type HsvColor } from './theme-color-picker';

describe('picker color conversion', () => {
  it.each(['#000000', '#ffffff', '#808080', '#ff0000', '#00ff00', '#0000ff', '#2563eb'])('round trips %s through HSV without changing the persisted color', hex => {
    const { h, s, v } = hexToHsv(hex);
    expect(hsvToHex(h, s, v)).toBe(hex);
    expect(contractHsvToHex(h, s, v)).toBe(hex);
  });
  it('wraps hue at the slider boundary and across keyboard steps', () => {
    expect(hsvToHex(360, 1, 1)).toBe('#ff0000');
    expect(hsvToHex(720, 1, 1)).toBe('#ff0000');
    expect(hsvToHex(-60, 1, 1)).toBe('#ff00ff');
    expect(contractHsvToHex(360, 1, 1)).toBe('#ff0000');
  });
  it('represents black and greys without undefined saturation', () => {
    expect(hexToHsv('#000000')).toEqual({ h: 0, s: 0, v: 0 });
    expect(hexToHsv('#ffffff')).toEqual({ h: 0, s: 0, v: 1 });
  });
});

describe('the Contract conversion', () => {
  test('tcpHex gives the reference hsvToHex on a grid of the plane and the hue slider', () => {
    let checked = 0;
    for (let h = 0; h <= 360; h += 7.5) for (let s = 0; s <= 1.0001; s += 0.05) for (let v = 0; v <= 1.0001; v += 0.05) {
      expect(contractHsvToHex(h, s, v)).toBe(hsvToHex(h, s, v));
      checked++;
    }
    expect(checked).toBeGreaterThan(20000);
  });
});

// The picker's state as the Contract keeps it: one HSV, each change committed through themeLocal.
type Picker = { hsv: HsvColor; commits: string[] };
const startAt = (hex: string): Picker => ({ hsv: hexToHsv(hex), commits: [] });
const commitHsv = (picker: Picker, next: HsvColor, alpha = '') => { picker.hsv = next; picker.commits.push(contractHsvToHex(next.h, next.s, next.v) + alpha); };
const key = (picker: Picker, axis: 'h' | 's' | 'v', name: string, shift = false, alpha = '') => {
  const next = axis === 'h' ? hueKey(picker.hsv.h, name, shift) : planeKey(picker.hsv[axis], name, shift);
  if (next !== null) commitHsv(picker, { ...picker.hsv, [axis]: next }, alpha);
  return next !== null; // preventDefault
};
const percent = (value: number) => Math.round(value * 100);

describe('shared color controls in settings', () => {
  it('adjusts each provider color axis independently and reports the value being changed', () => {
    const picker = startAt('#ff0000');
    expect(key(picker, 'h', 'ArrowLeft')).toBe(true);
    expect(Math.round(picker.hsv.h)).toBe(359);
    key(picker, 'h', 'ArrowRight');
    expect(picker.commits.at(-1)).toBe('#ff0000');
    key(picker, 's', 'ArrowRight', true);
    key(picker, 'v', 'ArrowUp', true);
    expect(picker.commits.at(-1)).toBe('#ff0000');
    key(picker, 'v', 'ArrowDown', true);
    expect(picker.commits.at(-1)).toBe('#e60000');
    expect([percent(picker.hsv.v), `${percent(picker.hsv.v)}%`, percent(picker.hsv.s)]).toEqual([90, '90%', 100]);
    // Up/Down on saturation must adjust the reported saturation, not brightness.
    key(picker, 's', 'ArrowDown', true);
    expect(picker.commits.at(-1)).toBe('#e61717');
    expect(percent(picker.hsv.s)).toBe(90);
    key(picker, 's', 'ArrowUp', true);
    expect(picker.commits.at(-1)).toBe('#e60000');
    // Left/Right on brightness must likewise leave saturation unchanged.
    key(picker, 'v', 'ArrowLeft', true);
    expect(picker.commits.at(-1)).toBe('#cc0000');
    key(picker, 'v', 'ArrowRight', true);
    expect(picker.commits.at(-1)).toBe('#e60000');
    const before = picker.commits.length;
    expect(key(picker, 's', 'Tab')).toBe(false);
    expect(key(picker, 'v', 'Tab')).toBe(false);
    expect(key(picker, 'h', 'Home')).toBe(false);
    expect(picker.commits.length).toBe(before);
  });
  it('supports Home/End and clamps each axis without changing the other', () => {
    const picker = startAt('#ff0000');
    key(picker, 's', 'Home');
    key(picker, 's', 'ArrowLeft');
    expect(percent(picker.hsv.s)).toBe(0);
    expect(picker.commits.at(-1)).toBe('#ffffff');
    key(picker, 'v', 'Home');
    key(picker, 'v', 'ArrowDown');
    expect(percent(picker.hsv.v)).toBe(0);
    expect(picker.commits.at(-1)).toBe('#000000');
    key(picker, 's', 'End');
    expect([percent(picker.hsv.s), percent(picker.hsv.v)]).toEqual([100, 0]);
    key(picker, 'v', 'End');
    key(picker, 'v', 'ArrowUp');
    expect(picker.commits.at(-1)).toBe('#ff0000');
    key(picker, 's', 'ArrowLeft');
    expect(percent(picker.hsv.s)).toBe(98);
    expect(picker.commits.at(-1)).toBe('#ff0505');
  });
  it('persists independent native range changes through theme batching with alpha', () => {
    const value = '#ff000080', alpha = themePickerAlphaSuffix(value), picker = startAt(pickerFields(value).hex6);
    commitHsv(picker, { ...picker.hsv, s: 0.5 }, alpha);
    commitHsv(picker, { ...picker.hsv, v: 0.5 }, alpha);
    expect(picker.commits.at(-1)).toBe('#80404080');
    key(picker, 'v', 'ArrowDown', true, alpha);
    expect(picker.commits.at(-1)).toBe('#66333380');
    expect(percent(picker.hsv.s)).toBe(50);
  });
  it('batches theme drag updates and flushes the final color with alpha on pointer release', () => {
    const value = '#ff000080', picker = startAt(pickerFields(value).hex6);
    commitHsv(picker, { ...picker.hsv, h: huePoint(25, 100) });
    commitHsv(picker, { ...picker.hsv, h: huePoint(50, 100) });
    expect(pickerCommit('color', picker.commits.at(-1)!, value)).toBe('#00ffff80');
  });
  it('clamps out-of-bounds drags and flushes theme changes on cancellation and unmount', () => {
    const value = '#ff000080', picker = startAt(pickerFields(value).hex6);
    commitHsv(picker, { ...picker.hsv, ...planePoint(-50, -50, 100, 100) });
    expect(pickerCommit('color', picker.commits.at(-1)!, value)).toBe('#ffffff80');
    commitHsv(picker, { ...picker.hsv, ...planePoint(150, 150, 100, 100) });
    expect(pickerCommit('color', picker.commits.at(-1)!, value)).toBe('#00000080');
  });
  it('preserves the selected hue when a grey color is echoed back from theme settings', () => {
    // The panel keeps its own HSV while the row shows its colour (theme-color-picker.contract `own`).
    const picker = startAt('#ff0000');
    key(picker, 'h', 'ArrowRight', true);
    commitHsv(picker, { ...picker.hsv, ...planePoint(0, 0, 100, 100) });
    expect(picker.commits.at(-1)).toBe('#ffffff');
    const echoed = pickerFields('#ffffff');
    expect(contractHsvToHex(picker.hsv.h, picker.hsv.s, picker.hsv.v)).toBe(echoed.hex6);
    expect(Math.round(picker.hsv.h)).toBe(10);
    expect(echoed.h).toBe(0);
  });
});

describe('ThemeColorPickerPanel fields', () => {
  test('RGB parsing accepts three integer channels with commas, spaces or rgb()', () => {
    expect(['12, 34, 56', '12 34 56', 'rgb(12, 34, 56)', ' RGB( 12,34,56 ) ', '12,  34 ,56'].map(themeRgbToHex)).toEqual(Array(5).fill('#0c2238'));
    expect(['12, 34', '12, 34, 56, 78', '256, 0, 0', '-1, 0, 0', '1.5, 2, 3', 'a, b, c', '', 'rgb()'].map(themeRgbToHex)).toEqual(Array(8).fill(null));
  });
  test('HEX commits only a complete six-digit value, lowercased, and drops the alpha; RGB and the controls keep it', () => {
    expect(['#ABCDEF', '#abc', '#abcd', 'abcdef', '#abcdef12', '#abcdeg'].map(text => pickerCommit('hex', text, '#11223380'))).toEqual(['#abcdef', null, null, null, null, null]);
    expect(pickerCommit('rgb', '1, 2, 3', '#11223380')).toBe('#01020380');
    expect(pickerCommit('rgb', '1, 2', '#11223380')).toBeNull();
    expect(pickerCommit('color', '#00ffff', '#112233')).toBe('#00ffff');
    expect(pickerCommit('color', '#00ffff', '#112233ff')).toBe('#00ffff');
    expect(pickerCommit('color', 'nope', '#112233')).toBeNull();
  });
  test('a row starts the picker from its value: opaque hex, alpha, RGB and HSV', () => {
    expect(pickerFields('#1B4ED8')).toEqual({ hex6: '#1b4ed8', alpha: '', rgb: '27, 78, 216', ...hexToHsv('#1b4ed8') });
    expect(pickerFields('#ff000080')).toMatchObject({ hex6: '#ff0000', alpha: '80', rgb: '255, 0, 0', h: 0, s: 1, v: 1 });
    expect(pickerFields('rgba(0, 0, 255, 0.5)')).toMatchObject({ hex6: '#0000ff', alpha: '80' });
    expect(pickerFields('not a colour')).toMatchObject({ hex6: '#000000', alpha: '', rgb: '0, 0, 0' });
  });
});

describe('the picker in the theme editor', () => {
  const client = () => ({ local: { deviceSettings: { appearanceMode: 'light' }, clientSettings: decodeClientPrefs({}), customThemes: [] as CustomTheme[] } as Obj });
  const as = (fake: ReturnType<typeof client>) => fake as unknown as T3Client;
  const prefs = (fake: ReturnType<typeof client>) => fake.local.clientSettings as { theme: string; themeLight: string; themeDark: string };
  const row = (fake: ReturnType<typeof client>, kind: string, subject: string, role: string) => {
    const view = editorView(syncDraft(as(fake), kind, subject, prefs(fake), 'light'));
    return [...view.rows, ...view.groups.flatMap(group => group.rows)].find(entry => entry.role === role)!;
  };

  test('a picker op changes the family, its preview and the row in step; a late older op is dropped', () => {
    const fake = client();
    syncDraft(as(fake), 'create', '#1', prefs(fake), 'light');
    expect(themeLocal(as(fake), 'color', 'accent', '#00ffff', 20)).toBe('');
    let accent = row(fake, 'create', '#1', 'accent');
    expect([accent.value, accent.hex6, accent.rgb, accent.seq]).toEqual(['#00ffff', '#00ffff', '0, 255, 255', 20]);
    expect(previewTheme(as(fake))!.light!.focus).toBe('#00ffff');
    themeLocal(as(fake), 'color', 'accent', '#ff00ff', 10);
    accent = row(fake, 'create', '#1', 'accent');
    expect([accent.value, accent.seq]).toEqual(['#00ffff', 20]);
    // A partial typed value changes nothing but still answers the op (the control stops waiting).
    themeLocal(as(fake), 'hex', 'accent', '#12', 30);
    themeLocal(as(fake), 'rgb', 'accent', '12, 34', 31);
    accent = row(fake, 'create', '#1', 'accent');
    expect([accent.value, accent.seq]).toEqual(['#00ffff', 31]);
    themeLocal(as(fake), 'rgb', 'accent', '12, 34, 56', 32);
    expect(row(fake, 'create', '#1', 'accent').value).toBe('#0c2238');
    themeLocal(as(fake), 'hex', 'accent', '#ABCDEF', 33);
    expect(row(fake, 'create', '#1', 'accent').value).toBe('#abcdef');
    // Never an error: no draft, or a role that is not a family.
    expect(themeLocal(as(fake), 'color', 'chrome', '#000000', 40)).toBe('');
    expect(themeLocal(as(client()), 'color', 'accent', '#000000', 40)).toBe('');
  });

  test('switching rows and appearances starts each from its own value; the stamps are per appearance and role', () => {
    const fake = client();
    syncDraft(as(fake), 'create', '#1', prefs(fake), 'light');
    themeLocal(as(fake), 'color', 'canvas', '#202020', 50);
    expect([row(fake, 'create', '#1', 'canvas').seq, row(fake, 'create', '#1', 'accent').seq]).toEqual([50, 0]);
    expect(row(fake, 'create', '#1', 'accent').hex6).toBe('#1b4ed8');
    editDraft(as(fake), 'appearance', 'dark');
    const dark = editorView(syncDraft(as(fake), 'create', '#1', prefs(fake), 'light')).rows.find(entry => entry.role === 'canvas')!;
    expect([dark.value, dark.seq]).toEqual(['#0a0a0a', 0]);
    themeLocal(as(fake), 'color', 'canvas', '#303030', 5);
    expect(editorView(syncDraft(as(fake), 'create', '#1', prefs(fake), 'light')).rows[0]!.value).toBe('#303030');
  });

  test('a colour with alpha keeps it through the plane, the hue and RGB, and is saved with it', async () => {
    const fake = client();
    (fake.local.customThemes as CustomTheme[]).push({ id: 'glass', label: 'Glass', appearance: 'light', light: { accent: '#ff000080', canvas: '#ffffff' }, dark: null });
    syncDraft(as(fake), 'edit', 'glass#1', prefs(fake), 'light');
    expect(row(fake, 'edit', 'glass#1', 'accent')).toMatchObject({ value: '#ff000080', hex6: '#ff0000', alpha: '80' });
    themeLocal(as(fake), 'color', 'accent', '#00ffff', 1);
    expect(row(fake, 'edit', 'glass#1', 'accent').value).toBe('#00ffff80');
    themeLocal(as(fake), 'rgb', 'accent', '0, 0, 255', 2);
    expect(row(fake, 'edit', 'glass#1', 'accent').value).toBe('#0000ff80');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Glass');
    const saved = (fake.local.customThemes as CustomTheme[]).find(entry => entry.id === 'glass')!;
    expect([saved.light!.accent, saved.light!.focus, saved.light!.update]).toEqual(['#0000ff80', '#0000ff80', '#0000ff80']);
    // The foreground derived from it reads the colour as it shows over the canvas.
    expect(saved.light!.accentForeground).toBe(updateFamily({ canvas: '#ffffff' }, 'accent', '#8080ff').accentForeground);
    // A typed HEX value replaces it whole, as the reference's handleHexChange does.
    syncDraft(as(fake), 'edit', 'glass#2', prefs(fake), 'light');
    themeLocal(as(fake), 'hex', 'accent', '#00FF00', 3);
    expect(row(fake, 'edit', 'glass#2', 'accent').value).toBe('#00ff00');
  });

  test('updateThemeColorFamily keeps the alpha in the role and composites it for the roles it derives', () => {
    const next = updateFamily({ canvas: '#000000', sidebar: '#000000', accent: '#1b4ed8' }, 'sidebarRowSelected', '#ffffff80');
    expect([next.sidebarRowSelected, next.sidebarRowHover, next.sidebarRowActive]).toEqual(['#ffffff80', '#404040', '#666666']);
    expect(updateFamily({ canvas: '#000000' }, 'canvas', '#ff0000ff').canvas).toBe('#ff0000');
    expect(updateFamily({ canvas: '#ffffff' }, 'messageAction', '#00000080').messageActionHover).toBe('#1f1f1f80');
    expect(updateFamily({ canvas: '#ffffff' }, 'sidebar', '#00000010').sidebarForeground).toBe('#241523');
    expect(updateFamily({ canvas: '#ffffff' }, 'sidebar', '#000000').sidebarForeground).toBe('#fffaff');
  });

  test.each([
    ['create', '#1', false], ['create', '#2', true],
    ['duplicate', 'grove#3', false], ['duplicate', 'grove#4', true],
    ['edit', 'dusk#5', false], ['edit', 'dusk#6', true],
  ] as const)('%s (%s, advanced %p): Save keeps the chosen colours, Cancel keeps the library', async (kind, subject, advanced) => {
    const fake = client();
    (fake.local.customThemes as CustomTheme[]).push({ id: 'dusk', label: 'Dusk', appearance: 'light', light: { accent: '#7241b8' }, dark: null });
    const before = JSON.stringify(fake.local.customThemes);
    // Cancel: the editor closes and nothing is saved.
    syncDraft(as(fake), kind, subject, prefs(fake), 'light');
    if (advanced) editDraft(as(fake), 'advanced', 'true');
    themeLocal(as(fake), 'color', advanced ? 'border' : 'accent', '#123456', 1);
    expect(syncDraft(as(fake), '', '', prefs(fake), 'light')).toBeNull();
    expect([JSON.stringify(fake.local.customThemes), previewTheme(as(fake))]).toEqual([before, null]);
    // Save.
    syncDraft(as(fake), kind, `${subject}x`, prefs(fake), 'light');
    if (advanced) editDraft(as(fake), 'advanced', 'true');
    themeLocal(as(fake), 'color', advanced ? 'border' : 'accent', '#123456', 2);
    themeLocal(as(fake), 'rgb', 'canvas', '250, 240, 230', 3);
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', kind === 'edit' ? 'Dusk' : 'Picked');
    const saved = (fake.local.customThemes as CustomTheme[]).find(entry => entry.label === (kind === 'edit' ? 'Dusk' : 'Picked'))!;
    expect([saved.light![advanced ? 'border' : 'accent'], saved.light!.canvas, saved.light!.chrome]).toEqual(['#123456', '#faf0e6', '#faf0e6']);
  });
});
