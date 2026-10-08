// The theme editor's colour picker. "picker color conversion" is ported from T3 Code 1e2ecbd975
// lib/color.test.ts with its original names, and "shared color controls in settings" from
// settings/colorPickers.test.tsx (MIT, LICENSE-T3): the React harness becomes the picker's rules
// (theme-color-picker.ts, which mirror theme-color-picker.contract, where the panel's state lives)
// and its draft ops (themeLocal). The first two keep their names; the others are renamed to what they
// check here, with the reference test they come from in a comment. The pointer batching, the second
// pointer and the unmount flush are the Contract component's and are checked by the drives.
import { describe, expect, it, test } from 'bun:test';
import type { T3Client } from './client';
import type { Obj } from './domain';
import { decodeClientPrefs } from './settings-core';
import type { CustomTheme } from './settings-themes';
import { currentDraft, editDraft, editorView, previewTheme, syncDraft, themeEditorCommand, themeLocal, updateFamily } from './settings-appearance-editor';
import { contractHsvToHex, hexToHsv, hsvToHex, huePoint, hueKey, pickerCommit, pickerFields, pickerOwns, pickerStamp, planeKey, planePoint, themePickerAlphaSuffix, themeRgbToHex, type HsvColor } from './theme-color-picker';

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
  // colorPickers.test.tsx "persists independent native range changes through theme batching with alpha"
  it('keeps the alpha through independent saturation and brightness changes', () => {
    const value = '#ff000080', alpha = themePickerAlphaSuffix(value), picker = startAt(pickerFields(value).hex6);
    commitHsv(picker, { ...picker.hsv, s: 0.5 }, alpha);
    commitHsv(picker, { ...picker.hsv, v: 0.5 }, alpha);
    expect(picker.commits.at(-1)).toBe('#80404080');
    key(picker, 'v', 'ArrowDown', true, alpha);
    expect(picker.commits.at(-1)).toBe('#66333380');
    expect(percent(picker.hsv.s)).toBe(50);
  });
  // "batches theme drag updates and flushes the final color with alpha on pointer release"
  it('a hue drag position commits its colour with the alpha', () => {
    const value = '#ff000080', picker = startAt(pickerFields(value).hex6);
    commitHsv(picker, { ...picker.hsv, h: huePoint(25, 100) });
    commitHsv(picker, { ...picker.hsv, h: huePoint(50, 100) });
    expect(pickerCommit('color', picker.commits.at(-1)!, value)).toBe('#00ffff80');
  });
  // "clamps out-of-bounds drags and flushes theme changes on cancellation and unmount"
  it('clamps out-of-bounds plane positions, keeping the alpha', () => {
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
    const echoed = pickerFields('#ffffff'), ownHex = contractHsvToHex(picker.hsv.h, picker.hsv.s, picker.hsv.v);
    expect(pickerOwns(true, false, 5, 5, ownHex, echoed.hex6)).toBe(true);
    expect([Math.round(picker.hsv.h), echoed.h]).toEqual([10, 0]);
  });
});

describe('the panel follows its own colour or the row (theme-color-picker.contract own, push)', () => {
  test('a send waits for its landing, a drag keeps the marker, a change from elsewhere takes over', () => {
    // Nothing touched yet: the row's value shows.
    expect(pickerOwns(false, false, 0, 0, '#000000', '#1b4ed8')).toBe(false);
    // A drag is on and its op is in flight: the panel shows its own colour, the row still the old one.
    expect(pickerOwns(true, true, 2, 1, '#405180', '#1b4ed8')).toBe(true);
    // The op landed but the pointer moved on (row = the sent colour, the panel a newer one): still its own while dragging.
    expect(pickerOwns(true, true, 2, 2, '#364774', '#405180')).toBe(true);
    // Released and landed: the row shows the panel's colour.
    expect(pickerOwns(true, false, 3, 3, '#364774', '#364774')).toBe(true);
    // Landed and the row differs (a typed value, the row's HEX field, Light/Dark switched): the row's value shows.
    expect(pickerOwns(true, false, 3, 3, '#364774', '#0a0a0a')).toBe(false);
  });
  test('stamps rise strictly, above the last sent and the row, whatever the clock says', () => {
    expect(pickerStamp(15000, 0, 0)).toBe(15000);
    expect(pickerStamp(15000, 15000, 0)).toBeCloseTo(15000.001, 6);
    expect(pickerStamp(15000, 15000.001, 14000)).toBeCloseTo(15000.002, 6);
    // A remounted row (its `sent` back to 0) still stamps above what the draft applied.
    expect(pickerStamp(15000, 0, 15000.004)).toBeCloseTo(15000.005, 6);
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
  test('a row starts the picker from its value: opaque hex, RGB and HSV; the alpha stays with the draft', () => {
    expect(pickerFields('#1B4ED8')).toEqual({ hex6: '#1b4ed8', rgb: '27, 78, 216', ...hexToHsv('#1b4ed8') });
    expect(pickerFields('#ff000080')).toMatchObject({ hex6: '#ff0000', rgb: '255, 0, 0', h: 0, s: 1, v: 1 });
    expect([themePickerAlphaSuffix('#ff000080'), themePickerAlphaSuffix('rgba(0, 0, 255, 0.5)'), themePickerAlphaSuffix('#0000ffff')]).toEqual(['80', '80', '']);
    expect(pickerFields('not a colour')).toMatchObject({ hex6: '#000000', rgb: '0, 0, 0' });
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
  /** One picker op for the open session's draft, as the control sends it (`<session>|<role>`). */
  const op = (fake: ReturnType<typeof client>, part: string, role: string, value: string, n: number) =>
    themeLocal(as(fake), part, `session-${currentDraft(as(fake))!.sessionId}|${role}`, value, n);

  test('a picker op changes the family, its preview and the row in step; a late older op is dropped', () => {
    const fake = client();
    syncDraft(as(fake), 'create', '#1', prefs(fake), 'light');
    expect(op(fake, 'color', 'accent', '#00ffff', 20)).toBe('');
    let accent = row(fake, 'create', '#1', 'accent');
    expect([accent.value, accent.hex6, accent.rgb, accent.seq]).toEqual(['#00ffff', '#00ffff', '0, 255, 255', 20]);
    expect(previewTheme(as(fake))!.light!.focus).toBe('#00ffff');
    op(fake, 'color', 'accent', '#ff00ff', 10);
    accent = row(fake, 'create', '#1', 'accent');
    expect([accent.value, accent.seq]).toEqual(['#00ffff', 20]);
    // A partial typed value changes nothing but still answers the op (the control stops waiting).
    op(fake, 'hex', 'accent', '#12', 30);
    op(fake, 'rgb', 'accent', '12, 34', 31);
    accent = row(fake, 'create', '#1', 'accent');
    expect([accent.value, accent.seq]).toEqual(['#00ffff', 31]);
    op(fake, 'rgb', 'accent', '12, 34, 56', 32);
    expect(row(fake, 'create', '#1', 'accent').value).toBe('#0c2238');
    op(fake, 'hex', 'accent', '#ABCDEF', 33);
    expect(row(fake, 'create', '#1', 'accent').value).toBe('#abcdef');
    // An op at or below the applied stamp is dropped.
    op(fake, 'color', 'accent', '#ff00ff', 33);
    expect(row(fake, 'create', '#1', 'accent').value).toBe('#abcdef');
    // Never an error: no draft, a role that is not a family, or another session's op (which changes nothing).
    expect(op(fake, 'color', 'chrome', '#000000', 40)).toBe('');
    expect(themeLocal(as(client()), 'color', 'session-1|accent', '#000000', 40)).toBe('');
    expect(themeLocal(as(fake), 'color', 'session-999|accent', '#000000', 41)).toBe('');
    expect(themeLocal(as(fake), 'color', 'accent', '#000000', 42)).toBe('');
    expect(row(fake, 'create', '#1', 'accent').value).toBe('#abcdef');
  });

  test('switching rows and appearances starts each from its own value; one stamp per role spans both appearances', () => {
    const fake = client();
    syncDraft(as(fake), 'create', '#1', prefs(fake), 'light');
    op(fake, 'color', 'canvas', '#202020', 50);
    expect([row(fake, 'create', '#1', 'canvas').seq, row(fake, 'create', '#1', 'accent').seq]).toEqual([50, 0]);
    expect(row(fake, 'create', '#1', 'accent').hex6).toBe('#1b4ed8');
    editDraft(as(fake), 'appearance', 'dark');
    // The dark row shows its own value with the role's stamp, so an open panel, its op landed and its
    // colour not the row's, follows the dark value instead of carrying the light one across.
    const dark = editorView(syncDraft(as(fake), 'create', '#1', prefs(fake), 'light')).rows.find(entry => entry.role === 'canvas')!;
    expect([dark.value, dark.seq]).toEqual(['#0a0a0a', 50]);
    expect(pickerOwns(true, false, 50, dark.seq, '#202020', dark.hex6)).toBe(false);
    op(fake, 'color', 'canvas', '#303030', 51);
    expect(editorView(syncDraft(as(fake), 'create', '#1', prefs(fake), 'light')).rows[0]!.value).toBe('#303030');
    editDraft(as(fake), 'appearance', 'light');
    expect(row(fake, 'create', '#1', 'canvas').value).toBe('#202020');
  });

  test('a colour with alpha keeps it through the plane, the hue and RGB, and is saved with it', async () => {
    const fake = client();
    (fake.local.customThemes as CustomTheme[]).push({ id: 'glass', label: 'Glass', appearance: 'light', light: { accent: '#ff000080', canvas: '#ffffff' }, dark: null });
    syncDraft(as(fake), 'edit', 'glass#1', prefs(fake), 'light');
    expect(row(fake, 'edit', 'glass#1', 'accent')).toMatchObject({ value: '#ff000080', hex6: '#ff0000', rgb: '255, 0, 0' });
    op(fake, 'color', 'accent', '#00ffff', 1);
    expect(row(fake, 'edit', 'glass#1', 'accent').value).toBe('#00ffff80');
    op(fake, 'rgb', 'accent', '0, 0, 255', 2);
    expect(row(fake, 'edit', 'glass#1', 'accent').value).toBe('#0000ff80');
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', 'Glass');
    const saved = (fake.local.customThemes as CustomTheme[]).find(entry => entry.id === 'glass')!;
    expect([saved.light!.accent, saved.light!.focus, saved.light!.update]).toEqual(['#0000ff80', '#0000ff80', '#0000ff80']);
    // The foreground derived from it reads the colour as it shows over the canvas.
    expect(saved.light!.accentForeground).toBe(updateFamily({ canvas: '#ffffff' }, 'accent', '#8080ff').accentForeground);
    // A typed HEX value replaces it whole, as the reference's handleHexChange does.
    syncDraft(as(fake), 'edit', 'glass#2', prefs(fake), 'light');
    op(fake, 'hex', 'accent', '#00FF00', 3);
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
    op(fake, 'color', advanced ? 'border' : 'accent', '#123456', 1);
    expect(syncDraft(as(fake), '', '', prefs(fake), 'light')).toBeNull();
    expect([JSON.stringify(fake.local.customThemes), previewTheme(as(fake))]).toEqual([before, null]);
    // Save.
    syncDraft(as(fake), kind, `${subject}x`, prefs(fake), 'light');
    if (advanced) editDraft(as(fake), 'advanced', 'true');
    op(fake, 'color', advanced ? 'border' : 'accent', '#123456', 2);
    op(fake, 'rgb', 'canvas', '250, 240, 230', 3);
    await themeEditorCommand(as(fake), null, 'theme-editor-save', '', kind === 'edit' ? 'Dusk' : 'Picked');
    const saved = (fake.local.customThemes as CustomTheme[]).find(entry => entry.label === (kind === 'edit' ? 'Dusk' : 'Picked'))!;
    expect([saved.light![advanced ? 'border' : 'accent'], saved.light!.canvas, saved.light!.chrome]).toEqual(['#123456', '#faf0e6', '#faf0e6']);
  });
});

// popover-escape-parity: Escape in the theme editor, read from the Contract sources (as dialog-focus.test.ts
// reads its handlers); the keys themselves are driven on macOS (tasks/20261008-popover-escape-parity.md).
// Reference: ThemeEditorPanel.tsx is a plain `role="dialog"` div with no Escape of its own (its only
// keydown listener cancels Inspect, not built here, X30), so an Escape in it reaches the page's
// useEscapeToGoBack (Settings' navigateToMainApp) and the editor, above the router, stays; the colour
// popover (Base UI Popover) closes on Escape, prevents it, and gives the focus back to its swatch.
describe('Escape in the theme editor (popover-escape-parity)', () => {
  const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

  test('the editor has no Escape of its own: no shortcut, no key handler, not a modal', async () => {
    const editor = await source('settings-appearance-editor.contract');
    const nodes = editor.split('\n').filter(line => /^\s*[a-z][\w-]*\b/.test(line) && !/^\s*(\/\/|action|state|derive|use|component|fn)\b/.test(line));
    expect(nodes.filter(line => /aria-keyshortcuts=/.test(line))).toEqual([]);
    expect(nodes.filter(line => /\skey=(?!editor\.session|group\.id|row\.id)/.test(line))).toEqual([]);
    expect(nodes.filter(line => /aria-modal=/.test(line))).toEqual([]);
    const close = nodes.find(line => line.includes('testId="theme-editor-close"'))!;
    expect(close).toContain('button press=close aria-label="Close the theme editor" class=SettingsGhost');
  });

  test('Settings\' Back, which an Escape in the editor reaches, blurs the focus before it leaves', async () => {
    const core = await source('settings-core.contract');
    expect(core).toContain('  action leave\n    blur()\n    back()');
    expect(core.split('\n').find(line => line.includes('testId="close-settings"'))).toContain('button press=leave hover=hover("back") aria-label="Back" aria-keyshortcuts=(menuOpen or query != "" ? "" : "Escape")');
  });

  test('Escape in the colour popover closes only the popover and gives the focus to its swatch', async () => {
    const picker = await source('theme-color-picker.contract');
    const popover = picker.split('\n').find(line => line.includes('testId=`theme-color-${row.id}-popover`'))!;
    // The host's light dismiss closes a popover="auto" on Escape; aria-modal keeps Settings' Back from taking it first.
    for (const part of ['popover="auto"', 'key=popKey', 'role="dialog"', 'aria-modal=true']) expect(popover).toContain(part);
    expect(picker).toContain('action popKey(k: string)\n    if k == "Escape"\n      focus(`theme-editor-swatch-${row.id}`)');
    expect(picker).toContain('popovertarget=`theme-color-${row.id}`');
    expect(picker).toContain('id=`theme-editor-swatch-${row.id}`');
  });
});
