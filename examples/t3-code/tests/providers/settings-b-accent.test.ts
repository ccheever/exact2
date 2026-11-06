import { describe, expect, it, test } from 'bun:test';
import { contractComponent, contractFns } from '../support/contract-action-fixture';
import { hexToHsv, hsvToHex, accentHsv, FALLBACK_ACCENT_COLOR } from '../../src/providers/settings-b-accent';
import { hueKey, planeKey } from '../../src/appearance/theme-color-picker';

test('the accent picker starts from the saved color and its HSV formula round-trips hex', () => {
  for (const hex of ['#2563eb', '#ff0000', '#00ff00', '#0000ff', '#ffffff', '#000000', '#7f22fe', '#c800de', '#12ab34']) {
    const { h, s, v } = hexToHsv(hex);
    expect(hsvToHex(h, s, v)).toBe(hex);
  }
  expect(accentHsv('')).toEqual(accentHsv(FALLBACK_ACCENT_COLOR));
  expect(accentHsv('nope').accentH).toBeCloseTo(hexToHsv(FALLBACK_ACCENT_COLOR).h, 6);
  // The plane's corners: top-right is the pure hue, the bottom edge is black, the left edge white→black.
  expect(hsvToHex(220, 1, 1)).toBe('#0055ff');
  expect(hsvToHex(220, 0, 1)).toBe('#ffffff');
  expect(hsvToHex(220, 1, 0)).toBe('#000000');
});

// T3 Code 1e2ecbd975 settings/colorPickers.test.tsx (MIT, LICENSE-T3), its two ProviderAccentColorPicker key tests with
// their names: the React harness becomes the picker's rules as settings-b-accent.contract keeps them (the stops' keys
// `sbAxisKey`, the hue's `hueKey`, both color-picker.tsx's, mirrored by theme-color-picker.ts planeKey and hueKey; each
// key commits `sbHex`, mirrored by hsvToHex here). The Contract component's keys, focus and range values are checked by
// driving the app on macOS.
describe('shared color controls in settings (the provider accent)', () => {
  type Picker = { hsv: { h: number; s: number; v: number }; commits: string[] };
  const startAt = (hex: string): Picker => ({ hsv: hexToHsv(hex), commits: [] });
  const key = (picker: Picker, axis: 'h' | 's' | 'v', name: string, shift = false) => {
    const next = axis === 'h' ? hueKey(picker.hsv.h, name, shift) : planeKey(picker.hsv[axis], name, shift);
    if (next === null) return false;
    picker.hsv = { ...picker.hsv, [axis]: next };
    picker.commits.push(hsvToHex(picker.hsv.h, picker.hsv.s, picker.hsv.v));
    return true; // preventDefault
  };
  // The stops' aria-valuenow (s × 100) and aria-valuetext (round(s × 100) "%"), the hue's round(h).
  const valueNow = (n: number) => Math.round(n * 100);

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
    expect([valueNow(picker.hsv.v), `${valueNow(picker.hsv.v)}%`, valueNow(picker.hsv.s)]).toEqual([90, '90%', 100]);
    key(picker, 's', 'ArrowDown', true);
    expect(picker.commits.at(-1)).toBe('#e61717');
    expect([valueNow(picker.hsv.s), `${valueNow(picker.hsv.s)}%`]).toEqual([90, '90%']);
    key(picker, 's', 'ArrowUp', true);
    expect(picker.commits.at(-1)).toBe('#e60000');
    key(picker, 'v', 'ArrowLeft', true);
    expect(picker.commits.at(-1)).toBe('#cc0000');
    key(picker, 'v', 'ArrowRight', true);
    expect(picker.commits.at(-1)).toBe('#e60000');
    const before = picker.commits.length;
    expect(key(picker, 's', 'Tab')).toBe(false);
    expect(key(picker, 'v', 'Tab')).toBe(false);
    expect(picker.commits.length).toBe(before);
  });

  it('supports Home/End and clamps each axis without changing the other', () => {
    const picker = startAt('#ff0000');
    key(picker, 's', 'Home');
    key(picker, 's', 'ArrowLeft');
    expect(valueNow(picker.hsv.s)).toBe(0);
    expect(picker.commits.at(-1)).toBe('#ffffff');
    key(picker, 'v', 'Home');
    key(picker, 'v', 'ArrowDown');
    expect(valueNow(picker.hsv.v)).toBe(0);
    expect(picker.commits.at(-1)).toBe('#000000');
    key(picker, 's', 'End');
    expect([valueNow(picker.hsv.s), valueNow(picker.hsv.v)]).toEqual([100, 0]);
    key(picker, 'v', 'End');
    key(picker, 'v', 'ArrowUp');
    expect(picker.commits.at(-1)).toBe('#ff0000');
    key(picker, 's', 'ArrowLeft');
    expect(valueNow(picker.hsv.s)).toBe(98);
    expect(picker.commits.at(-1)).toBe('#ff0505');
  });
});

// colorPickers.browser.test.tsx and AddProviderInstanceDialog.environment.browser.test.tsx at T3 Code 2359643cff: with
// no colour set the picker offers "Add color", which saves the colour it shows; nothing is saved by opening it.
test('the accent picker adds the displayed colour while none is set, and the Add provider dialog saves it only after Add color', () => {
  const picker = contractComponent('settings-b-accent.contract', 'ProviderAccentPicker');
  const fallback = accentHsv('');
  const math = { floor: Math.floor, min: Math.min, max: Math.max, round: Math.round };
  const scope = { ...math, ...contractFns('settings-b-accent.contract', math), editorId: 'kilo', title: 'Kilo Code', h0: fallback.accentH, s0: fallback.accentS, v0: fallback.accentV,
    writable: true, busy: false, op: (...args: unknown[]) => args, rem: 16 };
  const state = picker.state();
  expect(picker.drawn('testId="provider-accent-add"', state, { ...scope, accent: '' })).toBe(true);
  expect(picker.drawn('testId="provider-accent-add"', state, { ...scope, accent: '#ff0000' })).toBe(false);
  expect(picker.attribute('testId="provider-accent-add"', 'press', state, { ...scope, accent: '' })).toEqual(['provider-accent', 'kilo', '', FALLBACK_ACCENT_COLOR, true]);
  expect(picker.attribute('testId="provider-accent-add"', 'popovertargetaction', state, { ...scope, accent: '' })).toBe('hide');
  const wizard = contractComponent('providers-wizard.contract', 'AddProviderDialog');
  const form = { accent: '', accentOpen: false };
  wizard.call('toggleAccent', form, {}, []);
  expect(form).toEqual({ accent: '', accentOpen: true });
  wizard.call('addAccent', form, {}, []);
  expect(form).toEqual({ accent: '#2563eb', accentOpen: false });
});
