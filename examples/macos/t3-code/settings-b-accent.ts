// Lane settings-b: the accent picker's starting point (lib/color.ts hexToHsv),
// FALLBACK_ACCENT_COLOR when the instance has no accent yet.
export const FALLBACK_ACCENT_COLOR = '#2563eb';
export function hexToHsv(hex: string): { h: number; s: number; v: number } {
  const value = /^#[0-9a-f]{6}$/i.test(hex) ? hex : FALLBACK_ACCENT_COLOR;
  const numeric = Number.parseInt(value.slice(1), 16);
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
/** hsvToHex, for tests against the contract's sbChannel/sbByte. */
export function hsvToHex(hue: number, saturation: number, value: number): string {
  const channel = (n: number) => { const k = (n + hue / 60) % 6; return Math.floor((value - value * saturation * Math.max(0, Math.min(k, 4 - k, 1))) * 255 + 0.5); };
  return `#${[channel(5), channel(3), channel(1)].map(part => part.toString(16).padStart(2, '0')).join('')}`;
}
export function accentHsv(accent: string): { accentH: number; accentS: number; accentV: number } {
  const { h, s, v } = hexToHsv(accent);
  return { accentH: h, accentS: s, accentV: v };
}
