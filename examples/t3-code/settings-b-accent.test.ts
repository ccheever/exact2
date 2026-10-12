import { describe, expect, it, test } from 'bun:test';
import { hexToHsv, hsvToHex, accentHsv, FALLBACK_ACCENT_COLOR } from './settings-b-accent';
import { hueKey, planeKey } from './theme-color-picker';

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
// the drive in the 20261011-progress-native-bars record on the feat(example)/t3-code branch.
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

test('the accent picker\'s Contract key rules are color-picker.tsx\'s: the stops step 2 % (10 % with Shift), the hue 1° (10°) and wraps', async () => {
  const source = await Bun.file(new URL('./settings-b-accent.contract', import.meta.url)).text();
  expect(source).toContain('fn sbAxisKey(cur: number, k: string, shift: bool): number = k == "Home" ? 0 : (k == "End" ? 1 : min(1, max(0, cur + sbUp(k) * (shift ? 0.1 : 0.02))))');
  expect(source).toContain('let nh = (ch + sbUp(k) * (e.shiftKey ? 10 : 1) + 360) % 360');
  expect(source).toContain('fn sbUp(k: string): number = k == "ArrowRight" or k == "ArrowUp" ? 1 : -1');
  expect(source).toContain('fn sbHex(h: number, s: number, v: number): string = `#${sbByte(sbChannel(h, s, v, 5))}${sbByte(sbChannel(h, s, v, 3))}${sbByte(sbChannel(h, s, v, 1))}`');
});
