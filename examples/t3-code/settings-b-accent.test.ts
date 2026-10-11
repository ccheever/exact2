import { test, expect } from 'bun:test';
import { hexToHsv, hsvToHex, accentHsv, FALLBACK_ACCENT_COLOR } from './settings-b-accent';

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
