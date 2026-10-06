// Custom themes (lane settings-core): the reference theme file (themePalette.ts parseThemeFile,
// THEME_FILE_VERSION 1) and duplicates of a library theme with their own canvas and accent.
// Saved with this device's preferences; missing roles fall back to the appearance's default.
import { ClientError } from './protocol';
import { obj, str, type Obj } from './domain';

export type CustomTheme = { id: string; label: string; appearance: 'light' | 'dark'; light: Record<string, string> | null; dark: Record<string, string> | null;
  /** The Open VSX extension a multi-theme install came from (settings-a-collections.ts). */
  collection?: { id: string; label: string } };
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
    return [{ id: str(theme.id), label: str(theme.label).slice(0, 48), appearance: theme.appearance as 'light' | 'dark', light: variant(theme.light), dark: variant(theme.dark) }];
  }).slice(0, 100);
}
