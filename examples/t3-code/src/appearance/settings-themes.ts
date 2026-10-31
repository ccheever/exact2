// Custom themes: the reference theme file (themePalette.ts parseThemeFile,
// THEME_FILE_VERSION 1), duplicates of a library theme with their own canvas and accent, and
// readableThemeColorOn (themePalette.ts at 7c3b3b3605ef).
// Saved with this device's preferences; missing roles fall back to the appearance's default.
import { ClientError } from '../client/protocol';
import { obj, str, type Obj } from '../client/domain';

export type CustomTheme = { id: string; label: string; appearance: 'light' | 'dark'; light: Record<string, string> | null; dark: Record<string, string> | null;
  /** The Open VSX extension a multi-theme install came from (settings-a-collections.ts). */
  collection?: { id: string; label: string };
  /** ThemeDefinition.managed: generated from the guided editor's canvas and accent; anything else opens the editor in Advanced. */
  managed?: boolean };
const RESERVED = new Set(['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', 'system', 'light', 'dark', 'default']);
const ID = /^[a-z0-9](?:[a-z0-9-]{0,46}[a-z0-9])?$/;

/** #rgb, #rrggbb, #rrggbbaa, rgb()/rgba() and oklch() to opaque or alpha hex; null when unparseable. */
export function toHex(value: unknown): string | null {
  const text = str(value).trim().toLowerCase();
  let match = /^#([0-9a-f]{3})$/.exec(text);
  if (match) return '#' + [...match[1]!].map(c => c + c).join('');
  if (/^#[0-9a-f]{6}([0-9a-f]{2})?$/.test(text)) return text;
  match = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:[\s,/]+([\d.]+%?))?\s*\)$/.exec(text);
  if (match) {
    const channels = [match[1], match[2], match[3]].map(v => Math.max(0, Math.min(255, Math.round(Number(v)))));
    const a = match[4] === undefined ? 1 : match[4].endsWith('%') ? Number(match[4].slice(0, -1)) / 100 : Number(match[4]);
    if (channels.some(Number.isNaN) || Number.isNaN(a)) return null;
    return '#' + channels.map(v => v.toString(16).padStart(2, '0')).join('') + (a >= 1 ? '' : Math.round(Math.max(0, a) * 255).toString(16).padStart(2, '0'));
  }
  match = /^oklch\(\s*([\d.]+%?)\s+([\d.]+)\s+([\d.]+)(?:deg)?\s*(?:\/\s*([\d.]+%?))?\s*\)$/.exec(text);
  if (!match) return null;
  const L = match[1]!.endsWith('%') ? Number(match[1]!.slice(0, -1)) / 100 : Number(match[1]), C = Number(match[2]), h = Number(match[3]) * Math.PI / 180;
  const a = C * Math.cos(h), b = C * Math.sin(h);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3, s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3;
  const rgb = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s];
  const alphaValue = match[4] === undefined ? 1 : match[4].endsWith('%') ? Number(match[4].slice(0, -1)) / 100 : Number(match[4]);
  return '#' + rgb.map(v => { const c = Math.max(0, Math.min(1, v)); const g = c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055; return Math.round(g * 255).toString(16).padStart(2, '0'); }).join('')
    + (alphaValue >= 1 ? '' : Math.round(Math.max(0, alphaValue) * 255).toString(16).padStart(2, '0'));
}
export const themeIdFromName = (name: string) => name.normalize('NFKD').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 48) || 'theme';
function colors(value: unknown, where: string): Record<string, string> {
  const entries = Object.entries(obj(value));
  const result: Record<string, string> = {};
  for (const [role, color] of entries) {
    const hex = toHex(color);
    if (!hex) throw new ClientError(`${where}: ${role} is not a valid color.`);
    result[role] = hex;
  }
  return result;
}
/** parseThemeFile: version 1, a name of at most 48 characters, light or dark, colors and optional variants. */
export function parseThemeFile(text: string, taken: string[]): CustomTheme {
  let value: unknown;
  try { value = JSON.parse(text); } catch { throw new ClientError('Theme files must contain a JSON object.'); }
  const file = obj(value);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new ClientError('Theme files must contain a JSON object.');
  if (file.version !== 1) throw new ClientError('This theme file uses an unsupported version. Expected 1.');
  const name = str(file.name).trim();
  if (!name || name.length > 48) throw new ClientError('Theme files need a name (48 characters or fewer).');
  if (file.appearance !== 'light' && file.appearance !== 'dark') throw new ClientError('Theme files need an appearance of "light" or "dark".');
  if (!file.colors || typeof file.colors !== 'object') throw new ClientError('Theme files need a colors object.');
  const id = file.id === undefined ? themeIdFromName(name) : str(file.id);
  if (!ID.test(id)) throw new ClientError('Theme ids may only contain lowercase letters, numbers, and hyphens.');
  if (RESERVED.has(id)) throw new ClientError(`The theme id "${id}" is reserved.`);
  const appearance = file.appearance;
  const theme: CustomTheme = { id: uniqueId(id, taken), label: name, appearance, light: null, dark: null };
  theme[appearance] = colors(file.colors, name);
  for (const [variant, variantColors] of Object.entries(obj(file.variants))) {
    if (variant !== 'light' && variant !== 'dark') throw new ClientError('Theme variants may only be named "light" or "dark".');
    if (variant === appearance) throw new ClientError(`Theme variants must not repeat the base appearance "${appearance}".`);
    theme[variant] = colors(variantColors, `${name} ${variant}`);
  }
  if (file.managed === true) theme.managed = true;
  return theme;
}
function uniqueId(id: string, taken: string[]) {
  if (!taken.includes(id)) return id;
  for (let n = 2; ; n++) if (!taken.includes(`${id}-${n}`)) return `${id}-${n}`;
}
/** A duplicate keeps every source role and takes the editor's name, canvas and accent per appearance. */
export function duplicateTheme(input: Obj, source: (mode: 'light' | 'dark') => Record<string, string>, taken: string[]): CustomTheme {
  const name = str(input.name).trim();
  if (!name || name.length > 48) throw new ClientError('Name the theme (48 characters or fewer).');
  const variant = (mode: 'light' | 'dark') => {
    const entry = obj(input[mode]);
    const canvas = toHex(entry.canvas), accent = toHex(entry.accent);
    if (!canvas || !accent) throw new ClientError(`Choose a valid ${mode} canvas and accent color.`);
    const roles = { ...source(mode) };
    for (const role of ['canvas', 'chrome', 'toolbar', 'terminalBackground']) roles[role] = canvas;
    for (const role of ['accent', 'focus', 'messageAction', 'update']) roles[role] = accent;
    return roles;
  };
  return { id: uniqueId(themeIdFromName(name), taken), label: name, appearance: 'light', light: variant('light'), dark: variant('dark') };
}
export function decodeCustomThemes(value: unknown): CustomTheme[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap(entry => {
    const theme = obj(entry);
    const ok = ID.test(str(theme.id)) && !RESERVED.has(str(theme.id)) && str(theme.label).length > 0 && (theme.appearance === 'light' || theme.appearance === 'dark');
    if (!ok) return [];
    const variant = (v: unknown) => v && typeof v === 'object' ? Object.fromEntries(Object.entries(obj(v)).filter(([, c]) => toHex(c)).map(([r, c]) => [r, toHex(c)!])) : null;
    return [{ id: str(theme.id), label: str(theme.label).slice(0, 48), appearance: theme.appearance as 'light' | 'dark', light: variant(theme.light), dark: variant(theme.dark),
      ...(theme.managed === true ? { managed: true } : {}) }];
  }).slice(0, 100);
}

// ── readableThemeColorOn (themePalette.ts): lightness moved in OKLCH until a colour reads on a surface ──────────
type Rgb = { r: number; g: number; b: number };
type Oklch = { L: number; C: number; h: number };
const toRgb = (hex: string): Rgb => ({ r: parseInt(hex.slice(1, 3), 16), g: parseInt(hex.slice(3, 5), 16), b: parseInt(hex.slice(5, 7), 16) });
const toLinear = (channel: number) => { const c = channel / 255; return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
const fromLinear = (channel: number) => Math.round(Math.min(1, Math.max(0, channel <= 0.0031308 ? channel * 12.92 : 1.055 * channel ** (1 / 2.4) - 0.055)) * 255);
function rgbToOklch(color: Rgb): Oklch {
  const r = toLinear(color.r), g = toLinear(color.g), b = toLinear(color.b);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return { L, C: Math.hypot(a, bb), h: (Math.atan2(bb, a) * 180) / Math.PI };
}
function oklchToLinear({ L, C, h }: Oklch): Rgb {
  const a = C * Math.cos((h * Math.PI) / 180), bb = C * Math.sin((h * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * bb) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * bb) ** 3, s = (L - 0.0894841775 * a - 1.291485548 * bb) ** 3;
  return { r: 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, g: -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, b: -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s };
}
/** mapThemeOklchToSrgbGamut: the chroma bisected down until the colour fits sRGB. */
function inGamut(color: Oklch): Oklch {
  const fits = (C: number) => { const linear = oklchToLinear({ ...color, C }); return [linear.r, linear.g, linear.b].every(channel => channel >= -0.0001 && channel <= 1.0001); };
  if (fits(color.C)) return color;
  let low = 0, high = color.C;
  const steps = Math.max(1, Math.ceil(Math.log2(Math.max(color.C, 0.000001)) - Math.log2(0.000001)));
  for (let step = 0; step < steps; step += 1) { const mid = (low + high) / 2; if (fits(mid)) low = mid; else high = mid; }
  return { ...color, C: low };
}
function oklchToRgb(color: Oklch): Rgb {
  const linear = oklchToLinear(inGamut(color));
  return { r: fromLinear(linear.r), g: fromLinear(linear.g), b: fromLinear(linear.b) };
}
function luminance(color: Rgb): number {
  const channel = (value: number) => { const c = value / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
  return 0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b);
}
const contrastRatio = (first: Rgb, second: Rgb) => { const a = luminance(first), b = luminance(second); return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05); };
/** solveOklchLightness: 18 bisection steps toward white (`lighter`) or black. */
function solveLightness(base: Oklch, against: Rgb, minContrast: number, direction: 'lighter' | 'darker'): Oklch {
  let low = direction === 'lighter' ? base.L : 0, high = direction === 'lighter' ? 1 : base.L;
  if (contrastRatio(oklchToRgb(base), against) >= minContrast) return base;
  for (let step = 0; step < 18; step += 1) {
    const mid = (low + high) / 2, passes = contrastRatio(oklchToRgb({ ...base, L: mid }), against) >= minContrast;
    if (passes === (direction === 'lighter')) high = mid; else low = mid;
  }
  return { ...base, L: direction === 'lighter' ? high : low };
}
/**
 * Moves only the lightness, so a colour too dim on `surface` keeps its hue instead of collapsing to black or white;
 * the 8-bit colour the solver measured, as hex. A colour that already reads is returned as given.
 */
export function readableThemeColorOn(color: string, surface: string, minContrast: number): string {
  const colorHex = toHex(color), surfaceHex = toHex(surface);
  if (!colorHex || !surfaceHex) return color;
  const surfaceRgb = oklchToRgb(rgbToOklch(toRgb(surfaceHex))), base = rgbToOklch(toRgb(colorHex));
  if (contrastRatio(oklchToRgb(base), surfaceRgb) >= minContrast) return color;
  const solved = oklchToRgb(solveLightness(base, surfaceRgb, minContrast, luminance(surfaceRgb) < 0.179 ? 'lighter' : 'darker'));
  return '#' + [solved.r, solved.g, solved.b].map(channel => channel.toString(16).padStart(2, '0')).join('');
}
