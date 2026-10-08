// The theme editor's colour picker (theme-color-picker.contract). Ported from T3 Code
// 1e2ecbd975 (MIT, LICENSE-T3): settings/ThemeColorPicker.tsx (ThemeColorPickerPanel: the
// alpha suffix, RGB parsing, HEX synchronisation), ui/color-picker.tsx (the saturation/
// brightness plane and the hue slider: pointer and key rules) and lib/color.ts (hexToHsv,
// hsvToHex). The panel's HSV lives in the Contract component, which converts as
// hsvToHex does (`contractHsvToHex` mirrors its spelling); every
// commit lands here as a `themelocal:` op, which parses typed text, keeps the alpha suffix
// and applies the family through the draft (settings-appearance-editor.ts).
import { toHex } from './settings-themes';

export type HsvColor = { h: number; s: number; v: number };

/** lib/color.ts hexToHsv: a normalized six-digit sRGB hex to HSV. */
export function hexToHsv(hex: string): HsvColor {
  const numeric = Number.parseInt(hex.slice(1), 16);
  const red = ((numeric >> 16) & 255) / 255, green = ((numeric >> 8) & 255) / 255, blue = (numeric & 255) / 255;
  const max = Math.max(red, green, blue), min = Math.min(red, green, blue), delta = max - min;
  let hue = 0;
  if (delta !== 0) {
    if (max === red) hue = ((green - blue) / delta) % 6;
    else if (max === green) hue = (blue - red) / delta + 2;
    else hue = (red - green) / delta + 4;
    hue *= 60;
    if (hue < 0) hue += 360;
  }
  return { h: hue, s: max === 0 ? 0 : delta / max, v: max };
}

/** lib/color.ts hsvToHex. */
export function hsvToHex(hue: number, saturation: number, value: number): string {
  const normalizedHue = ((hue % 360) + 360) % 360;
  const chroma = value * saturation;
  const x = chroma * (1 - Math.abs(((normalizedHue / 60) % 2) - 1));
  const match = value - chroma;
  const [red, green, blue] = normalizedHue < 60 ? [chroma, x, 0] : normalizedHue < 120 ? [x, chroma, 0] : normalizedHue < 180 ? [0, chroma, x]
    : normalizedHue < 240 ? [0, x, chroma] : normalizedHue < 300 ? [x, 0, chroma] : [chroma, 0, x];
  return `#${[red, green, blue].map(channel => Math.round((channel + match) * 255).toString(16).padStart(2, '0')).join('')}`;
}

/** theme-color-picker.contract's conversion (tcpHex): hsvToHex step for step, as Contract spells it. */
export function contractHsvToHex(hue: number, saturation: number, value: number): string {
  const normalizedHue = ((hue % 360) + 360) % 360, sector = Math.floor(normalizedHue / 60);
  const x = value * saturation * (1 - Math.max(((normalizedHue / 60) % 2) - 1, 1 - ((normalizedHue / 60) % 2)));
  const part = (role: number) => role === 0 ? value * saturation : role === 1 ? x : 0;
  const channel = (roles: number[]) => Math.round((part(roles[sector]!) + (value - value * saturation)) * 255);
  return `#${[channel([0, 1, 2, 2, 1, 0]), channel([1, 0, 0, 1, 2, 2]), channel([2, 2, 1, 0, 0, 1])].map(part => part.toString(16).padStart(2, '0')).join('')}`;
}

/** themePickerAlphaSuffix: the value's alpha byte, '' when opaque, re-attached on commit. */
export function themePickerAlphaSuffix(value: string): string {
  const normalized = toHex(value) ?? '';
  const alpha = normalized.length === 9 ? normalized.slice(7) : '';
  return alpha === 'ff' ? '' : alpha;
}
/** normalizeThemePickerColor: the opaque six-digit hex, black when unparseable. */
export function normalizeThemePickerColor(value: string): string {
  return (toHex(value) ?? '#000000').slice(0, 7);
}
export function themeHexToRgb(hex: string): readonly [number, number, number] {
  const numeric = Number.parseInt(normalizeThemePickerColor(hex).slice(1), 16);
  return [numeric >> 16, (numeric >> 8) & 255, numeric & 255] as const;
}
/** themeRgbToHex: `12, 34, 56`, `12 34 56` or `rgb(12, 34, 56)` to hex; null unless three integers 0–255. */
export function themeRgbToHex(value: string): string | null {
  const normalized = value.trim().replace(/^rgb\(\s*/i, '').replace(/\s*\)$/, '');
  const channels = normalized.split(/[,\s]+/).filter(Boolean).map(Number);
  if (channels.length !== 3 || channels.some(channel => !Number.isInteger(channel) || channel < 0 || channel > 255)) return null;
  return `#${channels.map(channel => channel.toString(16).padStart(2, '0')).join('')}`;
}
export function themeRgbValue(hex: string): string {
  return themeHexToRgb(hex).join(', ');
}

/**
 * What a picker op commits for the row's current value, or null to leave it alone. `color`
 * (the plane, the hue slider and their keys) and `rgb` keep the incoming alpha suffix; `hex`
 * commits a complete six-digit value as typed, lowercased (handleHexChange), so it drops it.
 */
export function pickerCommit(part: string, text: string, current: string): string | null {
  if (part === 'color') return /^#[0-9a-f]{6}$/.test(text) ? text + themePickerAlphaSuffix(current) : null;
  if (part === 'hex') return /^#[0-9a-f]{6}$/i.test(text) ? text.toLowerCase() : null;
  if (part === 'rgb') { const hex = themeRgbToHex(text); return hex ? hex + themePickerAlphaSuffix(current) : null; }
  return null;
}

/** The picker's view of a row's value: what the panel shows before the user touches it. */
export type PickerFields = { hex6: string; rgb: string; h: number; s: number; v: number };
export function pickerFields(value: string): PickerFields {
  const hex6 = normalizeThemePickerColor(value);
  return { hex6, rgb: themeRgbValue(hex6), ...hexToHsv(hex6) };
}

/**
 * theme-color-picker.contract's display and send rules, for tests: the panel shows its own HSV while
 * a drag is on, its op is in flight (`sent` above the row's echoed `seq`) or the row shows its colour;
 * a stamp is strictly above both the last one sent and the row's.
 */
export const pickerOwns = (touched: boolean, dragging: boolean, sent: number, seq: number, ownHex: string, rowHex6: string) =>
  touched && (dragging || sent > seq || ownHex === rowHex6);
export const pickerStamp = (now: number, sent: number, seq: number) => Math.max(now, Math.max(sent, seq) + 0.001);

// ── The controls' rules, as theme-color-picker.contract computes them ─────────
const clamp = (value: number) => Math.min(1, Math.max(0, value));
/** ColorSaturationValuePlane's pointer: x and y from the plane's box of w × h. */
export const planePoint = (x: number, y: number, w: number, h: number) => ({ s: clamp(x / w), v: 1 - clamp(y / h) });
/** ColorHueSlider's pointer. */
export const huePoint = (x: number, w: number) => clamp(x / w) * 360;
/** The plane's keys for one axis; null for a key it leaves alone (Tab, letters). */
export function planeKey(current: number, key: string, shiftKey: boolean): number | null {
  const step = shiftKey ? 0.1 : 0.02;
  if (key === 'ArrowRight' || key === 'ArrowUp') return clamp(current + step);
  if (key === 'ArrowLeft' || key === 'ArrowDown') return clamp(current - step);
  if (key === 'Home') return 0;
  if (key === 'End') return 1;
  return null;
}
/** The hue slider's keys: 1° (10° with Shift), wrapping at 360. */
export function hueKey(current: number, key: string, shiftKey: boolean): number | null {
  if (!['ArrowDown', 'ArrowLeft', 'ArrowRight', 'ArrowUp'].includes(key)) return null;
  const direction = key === 'ArrowRight' || key === 'ArrowUp' ? 1 : -1;
  return (current + direction * (shiftKey ? 10 : 1) + 360) % 360;
}
