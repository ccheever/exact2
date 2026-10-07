// Appearance route data and the theme palette (lane settings-core).
// Reference: apps/web/src/components/settings/{ThemeSettings,ThemePreviewCircles,ThemeWireframe}.tsx,
// SettingsPanels.tsx AppearanceSettingsPanel/TypographySection, appearanceContrast.ts, index.css tokens,
// packages/shared/src/{themePalettes,themePreview}.ts. Palette roles were converted from the
// reference OKLCH tokens to sRGB hex (scratch gen-palette.ts); T3 Code keeps the stock index.css tokens.
import { coreRow, CLIENT_DEFAULTS, type ClientPrefs, type CoreRow, type CoreSection } from './settings-core';
import type { CustomTheme } from './settings-themes';

const ROLES = ['canvas', 'surface', 'text', 'textMuted', 'border', 'input', 'accent', 'accentForeground', 'muted', 'mutedForeground', 'sidebar',
  'sidebarForeground', 'sidebarMutedForeground', 'sidebarRowActive', 'sidebarBorder', 'surfaceOverlay', 'accentSurface', 'messageSurface', 'messageAction'] as const;
type Role = (typeof ROLES)[number];
const THEMES: Record<string, [string, string, string]> = {
  't3-code': ['T3 Code', '#fcfcfc #ffffff #27272a #71717b #e4e4e7 #d4d4d8 #1b4ed8 #ffffff #fafafa #71717b #fafafa #27272a #71717b #ffffff #e4e4e7 #ffffff #f4f4f5 #e4e4e7 #4f46e5',
    '#0a0a0a #111111 #f5f5f5 #818181 #191919 #1e1e1e #346bf1 #ffffff #111111 #818181 #000000 #f1f3f7 #a3a3a3 #1a1b1b #141414 #111111 #1c1c1f #27272a #8b9cff'],
  't3-chat': ['T3 Chat', '#fdf7fd #faf3fb #501854 #ac1668 #eee1ed #e7c1dc #db2777 #ffffff #eaa7cb #8d1255 #f2e1f4 #454554 #ac1668 #f8f8f7 #eceae9 #ffffff #f3e6f5 #f7def2 #db2777',
    '#1f1a24 #29232d #f9f8fb #e7d0dd #27242c #302029 #a3004c #fbd0e8 #423a45 #e7d0dd #171018 #f4f4f5 #e7d0dd #261922 #322028 #100a0e #463753 #2b2431 #a3004c'],
  grove: ['Grove', '#f3f7f4 #f3f7f4 #241523 #746c73 #cbd5d1 #becbc5 #1b7d50 #fffaff #e6f0ea #6e696f #e2ede7 #241523 #6b666c #bad7c9 #cbd3d0 #e7e9e8 #d5e6dd #cce1d7 #8f6410',
    '#1b2821 #1b2821 #fffaff #919595 #415f4f #4f725f #69d69a #241523 #253e31 #9da5a2 #21362b #fffaff #9da3a2 #2f5641 #6f7a75 #444d49 #325c46 #37664d #e3b34e'],
  ocean: ['Ocean', '#f5f7f8 #f5f7f8 #241523 #746c75 #cdd4dc #c0c9d4 #2672af #fffaff #e8eff4 #6f6873 #e4ecf2 #241523 #6c6570 #bed4e5 #cdd2d9 #e8e9eb #d8e4ee #d0dfeb #0a6f75',
    '#17212b #17212b #fffaff #8d8f97 #405567 #4f677b #70b9ee #241523 #233544 #969ca6 #1e2d3b #fffaff #989ca5 #2f495f #6d757f #414851 #324e66 #375871 #5bd0d6'],
  ember: ['Ember', '#f9f7f5 #f9f7f5 #241523 #766c74 #ddd2ce #d4c6c1 #ae552a #fffaff #f4ede9 #74686f #f3eae5 #241523 #71646b #e5ccc0 #dad0ce #ece9e9 #eee0d9 #ebdad1 #b23535',
    '#291e1a #291e1a #fffaff #968e8f #664c3f #7a5d4d #f09a64 #241523 #432e23 #a59996 #39281f #fffaff #a49998 #5d3f2d #7e716e #4f4543 #644330 #704b34 #f78a7a'],
  iris: ['Iris', '#f8f7f9 #f8f7f9 #241523 #766c76 #d6d1de #ccc5d6 #7253b9 #fffaff #f0edf6 #726874 #edeaf4 #241523 #6f6471 #d4cce8 #d5d0db #ebe9ed #e5e0f0 #e0d9ee #a82c87',
    '#1d1929 #1d1929 #fffaff #8e8a95 #4d4366 #5d527b #9d7df2 #241523 #2d2643 #9690a1 #272139 #fffaff #9792a0 #3f345e #736d7e #454250 #433765 #4b3d72 #f099d8'],
};
export const THEME_IDS = Object.keys(THEMES);
export function themeRoles(id: string, mode: 'light' | 'dark', custom: CustomTheme[] = []): Record<Role, string> {
  const own = custom.find(theme => theme.id === id);
  const entry = THEMES[own ? 't3-code' : id] ?? THEMES['t3-code']!;
  const values = (mode === 'light' ? entry[1] : entry[2]).split(' ');
  const base = Object.fromEntries(ROLES.map((role, index) => [role, values[index]!])) as Record<Role, string>;
  return own ? { ...base, ...(own[mode] ?? own[own.appearance] ?? {}) } as Record<Role, string> : base;
}

// ── Colour arithmetic (sRGB, as color-mix in srgb) ─────────────────────────
const parse = (hex: string): [number, number, number, number] => {
  const h = hex.replace('#', '');
  return [0, 2, 4].map(i => parseInt(h.slice(i, i + 2), 16)).concat(h.length === 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1) as [number, number, number, number];
};
const hex = ([r, g, b, a]: number[]) => '#' + [r, g, b].map(v => Math.round(Math.max(0, Math.min(255, v!))).toString(16).padStart(2, '0')).join('')
  + (a! >= 0.999 ? '' : Math.round(Math.max(0, a!) * 255).toString(16).padStart(2, '0'));
/** color-mix(in srgb, a weight, b) with premultiplied alpha. */
export function mix(a: string, weight: number, b: string): string {
  const x = parse(a), y = parse(b), alpha = x[3] * weight + y[3] * (1 - weight);
  if (alpha <= 0) return '#00000000';
  return hex([0, 1, 2].map(i => (x[i]! * x[3] * weight + y[i]! * y[3] * (1 - weight)) / alpha).concat(alpha));
}
export const alpha = (color: string, amount: number) => { const [r, g, b, a] = parse(color); return hex([r, g, b, a * amount]); };

/** The tokens this lane paints, in the stock index.css vocabulary, per appearance. */
type Tokens = Record<'canvas' | 'sidebar' | 'sidebarBorder' | 'rowActive' | 'sidebarText' | 'sidebarMuted' | 'sidebarIcon' | 'text' | 'muted' | 'heading' | 'description'
  | 'card' | 'cardBorder' | 'divider' | 'input' | 'field' | 'accent' | 'accentText' | 'switchOff' | 'thumb' | 'popover' | 'popoverBorder' | 'highlight' | 'kbd' | 'output' | 'line' | 'rowHover' | 'sectionMuted' | 'tileActive'
  | 'border' | 'backdrop' | 'footer' | 'surface', string>;
function tokens(themeId: string, mode: 'light' | 'dark', contrast: number, custom: CustomTheme[], glass = 80): Tokens {
  const stock = themeId === 't3-code' || !(themeId in THEMES || custom.some(theme => theme.id === themeId));
  const r = themeRoles(themeId, mode, custom);
  const dark = mode === 'dark';
  // appearanceContrast.ts: base fades text toward the surface, boost pushes it to the extreme.
  const base = Math.min(contrast, 100) / 100, boost = Math.max(contrast - 100, 0) / 100, borderBoost = boost / 4;
  const target = dark ? '#ffffff' : '#000000';
  const ink = (fg: string, bg: string) => mix(mix(fg, base, bg), 1 - boost, target);
  const edge = (border: string, fg: string) => mix(alpha(border, base), 1 - borderBoost, fg);
  const text = stock ? (dark ? '#f5f5f5' : '#27272a') : r.text;
  const canvas = stock ? (dark ? '#0a0a0a' : '#fcfcfc') : r.canvas;
  const sidebar = stock ? (dark ? '#000000' : '#fafafa') : r.sidebar;
  const sidebarText = stock ? (dark ? '#f1f3f7' : '#27272a') : r.sidebarForeground;
  const sidebarMuted = stock ? (dark ? '#a3a3a3' : '#71717b') : r.sidebarMutedForeground;
  const mutedText = stock ? (dark ? '#818181' : '#71717b') : r.textMuted;
  const border = stock ? (dark ? '#ffffff0f' : '#e4e4e7') : r.border;
  const input = stock ? (dark ? '#ffffff14' : '#d4d4d8') : r.input;
  const card = stock ? (dark ? '#111111' : '#ffffff') : r.surface;
  return {
    canvas, sidebar, text: ink(text, canvas), muted: ink(mutedText, canvas), heading: alpha(ink(text, canvas), 0.7), description: alpha(ink(mutedText, canvas), 0.8),
    sidebarBorder: edge(stock ? (dark ? '#ffffff14' : '#e4e4e7') : r.sidebarBorder, sidebarText), rowActive: stock ? (dark ? '#f1f1f112' : '#ffffff') : r.sidebarRowActive,
    sidebarText: ink(sidebarText, sidebar), sidebarMuted: alpha(ink(sidebarMuted, sidebar), 0.8), sidebarIcon: mix(ink(sidebarMuted, sidebar), 0.6, sidebar),
    card: alpha(card, 0.4), cardBorder: edge(alpha(border, 0.6), text), divider: edge(alpha(border, 0.5), text), input: edge(input, text),
    field: stock ? (dark ? '#ffffff07' : '#fcfcfc') : canvas, accent: stock ? (dark ? '#346bf1' : '#1b4ed8') : r.accent, accentText: r.accentForeground,
    switchOff: edge(input, text), thumb: canvas, // The reference blurs what is behind a glass surface; the native top layer has no backdrop
    // filter, so the glass mix is composited over the canvas it would mostly show.
    popover: mix(stock ? (dark ? '#111111' : '#ffffff') : r.surfaceOverlay, 0.18 + 0.82 * glass / 100, canvas),
    popoverBorder: dark ? '#ffffff1a' : alpha(text, 0.1), highlight: alpha(text, dark ? 0.06 : 0.08), kbd: dark ? (stock ? '#0a0a0a' : r.sidebar) : sidebar,
    output: stock ? (dark ? '#ffffff08' : '#fafafa') : r.muted, line: '#7f7f7f40',
    rowHover: stock ? (dark ? '#ffffff0a' : '#fcfcfc') : mix(r.sidebarRowActive, 0.5, sidebar), sectionMuted: alpha(ink(sidebarMuted, sidebar), 0.75),
    tileActive: alpha(stock ? (dark ? '#ffffff0a' : '#f4f4f5') : r.accentSurface, 0.3),
    // Dialogs (ui/dialog.tsx): full-strength border, the blurred backdrop, the muted footer and the bg-card panel.
    border: edge(border, text), backdrop: alpha(canvas, dark ? 0.64 : 0.6), footer: stock ? (dark ? '#ffffff06' : '#fafafab8') : alpha(r.muted, 0.72), surface: card,
  };
}
export type Palette = Tokens;
/** Each token as `light-dark(light, dark)`: the light half wears themeLight, the dark half themeDark. */
export function palette(prefs: Pick<ClientPrefs, 'themeLight' | 'themeDark' | 'appearanceContrast'> & { glassOpacity?: number }, custom: CustomTheme[] = [], mode = 'system'): Palette {
  const glass = prefs.glassOpacity ?? 80;
  const light = tokens(prefs.themeLight, 'light', prefs.appearanceContrast, custom, glass), dark = tokens(prefs.themeDark, 'dark', prefs.appearanceContrast, custom, glass);
  // An explicit appearance paints its own half directly: the host re-resolves light-dark()
  // on views but not inside an SVG scene, so a fixed colour keeps icons in step.
  return Object.fromEntries(Object.keys(light).map(key => [key, mode === 'light' ? light[key as keyof Tokens] : mode === 'dark' ? dark[key as keyof Tokens]
    : `light-dark(${light[key as keyof Tokens]}, ${dark[key as keyof Tokens]})`])) as Palette;
}

// ── Theme library ──────────────────────────────────────────────────────────
export type ThemeOrb = { mode: string; base: string; accent: string; accentMid: string; action: string; picked: boolean; ax: number; ay: number; px: number; py: number; ar: number; pr: number };
export type ThemeCard = { id: string; label: string; selected: boolean; custom: boolean; canvasLight: string; accentLight: string; canvasDark: string; accentDark: string; orbs: ThemeOrb[] };
export type ModeTile = { id: string; label: string; aria: string; selected: boolean; split: boolean; left: Wire; right: Wire };
type Wire = { canvas: string; sidebar: string; surface: string; accentSurface: string; message: string; action: string };
const PREVIEW = { light: { target: '#ffffff', accent: [0.72, 0.22], middle: 0.72, action: [0.18, 0.82] }, dark: { target: '#09090b', accent: [0.28, 0.78], middle: 0.62, action: [0.82, 0.18] } } as const;
const STANDARD_PREVIEW = { light: { sidebar: '#fafafa', canvas: '#fcfcfc', surface: '#ffffff', accentSurface: '#f4f4f5', accent: '#f4f4f5', messageSurface: '#e4e4e7', messageAction: '#4f46e5' },
  dark: { sidebar: '#0f0f10', canvas: '#0a0a0a', surface: '#121212', accentSurface: '#27272a', accent: '#1c1c1f', messageSurface: '#27272a', messageAction: '#8b9cff' } };
function previewColors(id: string, mode: 'light' | 'dark', custom: CustomTheme[] = []) {
  if (id === 't3-code' || !(id in THEMES || custom.some(theme => theme.id === id))) return STANDARD_PREVIEW[mode];
  const r = themeRoles(id, mode, custom);
  return { sidebar: r.sidebar, canvas: r.canvas, surface: r.surface, accentSurface: r.accentSurface, accent: r.accent, messageSurface: r.messageSurface, messageAction: r.messageAction };
}
const farthest = (x: number, y: number) => Math.max(...[[0, 0], [1, 0], [0, 1], [1, 1]].map(([cx, cy]) => Math.hypot(cx! - x, cy! - y))) * 56;
export function themeCards(prefs: Pick<ClientPrefs, 'themeLight' | 'themeDark'>, custom: CustomTheme[] = []): ThemeCard[] {
  return [...THEME_IDS.map(id => [id, THEMES[id]![0], false] as const), ...custom.map(theme => [theme.id, theme.label, true] as const)].map(([id, label, own]) => ({ id, label, custom: own,
    selected: prefs.themeLight === id && prefs.themeDark === id,
    canvasLight: themeRoles(id, 'light', custom).canvas, accentLight: themeRoles(id, 'light', custom).accent, canvasDark: themeRoles(id, 'dark', custom).canvas, accentDark: themeRoles(id, 'dark', custom).accent,
    orbs: (['light', 'dark'] as const).map(mode => themeOrb(id, mode, (mode === 'light' ? prefs.themeLight : prefs.themeDark) === id, custom)) }));
}
/** ThemePreviewCircle: one appearance's orb (a canvas-tinted base, the accent glow, a soft action tint). */
export function themeOrb(id: string, mode: 'light' | 'dark', picked: boolean, custom: CustomTheme[] = []): ThemeOrb {
  const colors = previewColors(id, mode, custom), spec = PREVIEW[mode];
  return { mode, base: mix(colors.canvas, 0.8, spec.target), accent: colors.accent, accentMid: alpha(colors.accent, spec.middle), action: alpha(colors.messageAction, 0.45),
    picked, ax: spec.accent[0] * 56, ay: spec.accent[1] * 56, px: spec.action[0] * 56, py: spec.action[1] * 56,
    ar: farthest(spec.accent[0], spec.accent[1]), pr: farthest(spec.action[0], spec.action[1]) };
}
export function modeTiles(mode: string, prefs: Pick<ClientPrefs, 'themeLight' | 'themeDark'>, custom: CustomTheme[] = []): ModeTile[] {
  const wire = (appearance: 'light' | 'dark'): Wire => {
    const colors = previewColors(appearance === 'light' ? prefs.themeLight : prefs.themeDark, appearance, custom);
    return { canvas: colors.canvas, sidebar: colors.sidebar, surface: colors.surface, accentSurface: colors.accentSurface, message: colors.messageSurface, action: colors.messageAction };
  };
  return [['system', 'System', 'Follow the system appearance'], ['light', 'Light', 'Use light mode'], ['dark', 'Dark', 'Use dark mode']].map(([id, label, aria]) => ({
    id: id!, label: label!, aria: aria!, selected: mode === id, split: id === 'system', left: wire(id === 'dark' ? 'dark' : 'light'), right: wire(id === 'light' ? 'light' : 'dark') }));
}

// ── Fonts ──────────────────────────────────────────────────────────────────
// Contract resolves font families at compile time to generic stacks or bundled faces
// (LLP 1019), so a typed family maps to the macOS stack it names; anything else
// keeps the default and says so instead of pretending to apply.
const SANS_STACKS: Record<string, string> = { '': 'system-ui', 'sf pro': 'system-ui', 'sf pro text': 'system-ui', 'system-ui': 'system-ui', '-apple-system': 'system-ui',
  'new york': 'ui-serif', 'ui-serif': 'ui-serif', 'sf pro rounded': 'ui-rounded', 'ui-rounded': 'ui-rounded', times: 'serif', 'times new roman': 'serif', serif: 'serif',
  helvetica: 'sans-serif', 'helvetica neue': 'sans-serif', arial: 'sans-serif', 'sans-serif': 'sans-serif', 'sf mono': 'ui-monospace', menlo: 'monospace', monospace: 'monospace' };
const MONO_STACKS: Record<string, string> = { '': 'ui-monospace', 'sf mono': 'ui-monospace', 'ui-monospace': 'ui-monospace', menlo: 'monospace', monaco: 'monospace', courier: 'monospace',
  'courier new': 'monospace', monospace: 'monospace' };
/** FontFamilyPicker's families, limited to the distinct faces this app can draw (the host's
 * generic stacks, LLP 1019: system → SF Pro, ui-rounded → SF Pro Rounded, serif → New York,
 * monospace → SF Mono): the default first under its own name with a "default" tag, each in its face. */
const SANS_FAMILIES = ['SF Pro Rounded', 'New York', 'SF Mono'];
const MONO_FAMILIES: string[] = [];
export function fontFamilies(selected: string, mono: boolean, defaultLabel: string) {
  const current = selected.trim();
  const named = (mono ? MONO_FAMILIES : SANS_FAMILIES).map(family => ({ id: family, value: family, label: family, detail: fontStack(family, mono) ?? (mono ? 'ui-monospace' : 'system-ui'),
    icon: `${family} ${family.toLowerCase()} ${family.toUpperCase()}`, selected: current.toLowerCase() === family.toLowerCase(), disabled: false }));
  return [{ id: 'default', value: '', label: defaultLabel, detail: mono ? 'ui-monospace' : 'system-ui', icon: 'default', selected: current === '', disabled: false }, ...named];
}
export function fontStack(family: string, mono: boolean): string | undefined {
  const key = family.trim().replace(/^["']|["']$/g, '').toLowerCase();
  return (mono ? MONO_STACKS : SANS_STACKS)[key];
}

// ── Appearance rows ────────────────────────────────────────────────────────
const option = (value: string, label: string, selected: boolean) => ({ id: value, value, label, detail: '', icon: '', selected, disabled: false });
export function appearanceSections(prefs: ClientPrefs, artworkVisible: boolean): CoreSection[] {
  const row = (key: keyof ClientPrefs, id: string, title: string, description: string, kind: string, extra: Partial<CoreRow> = {}) =>
    coreRow(id, title, description, kind, { checked: prefs[key] === true, value: String(prefs[key]), resettable: prefs[key] !== CLIENT_DEFAULTS[key], target: key, ...extra });
  const slider = (key: 'appearanceContrast' | 'glassOpacity' | 'panelAnimationDurationMs', min: number, max: number, step: number, suffix: string) =>
    ({ min, max, step, suffix, label: `${prefs[key]}${suffix}`, width: 208, amount: prefs[key] });
  const select = (key: keyof ClientPrefs, choices: [string, string][], selectedLabel?: string) =>
    ({ value: String(prefs[key]), label: selectedLabel ?? choices.find(([value]) => value === prefs[key])?.[1] ?? String(prefs[key]), options: choices.map(([value, label]) => option(value, label, value === prefs[key])) });
  const sizes = (min: number, max: number) => Array.from({ length: max - min + 1 }, (_, index) => [String(min + index), `${min + index} px`] as [string, string]);
  const interfaceRows: CoreRow[] = [
    row('appearanceContrast', 'setting-appearance-contrast', 'Contrast', 'Adjust the contrast of colors and borders across the interface.', 'slider', slider('appearanceContrast', 50, 200, 5, '%')),
    row('glassOpacity', 'setting-glass-opacity', 'Glass opacity', 'Higher values make menus, dialogs, and the composer more solid.', 'slider', slider('glassOpacity', 40, 100, 5, '%')),
    ...(artworkVisible ? [row('environmentIdentificationMode', 'environment-identification', 'Environment identification', 'Choose how Dev and Nightly environments are identified.', 'select',
      select('environmentIdentificationMode', [['artwork', 'Artwork'], ['pill', 'Version pill'], ['none', 'None']]))] : []),
    row('diffColorScheme', 'diff-color-scheme', 'Diff colors', 'Choose colors for additions and deletions, including change counts.', 'select',
      { ...select('diffColorScheme', [['red-green', 'Red & green (default)'], ['blue-orange', 'Blue & orange']], prefs.diffColorScheme === 'blue-orange' ? 'Blue & orange' : 'Red & green'),
        icon: prefs.diffColorScheme === 'blue-orange' ? 'dots-blue-orange' : 'dots-red-green' }),
    row('persistComposerContextStrip', 'composer-context', 'Composer context', 'Keep branch and worktree controls below the composer after a thread starts.', 'switch'),
    row('chatWidth', 'chat-width', 'Chat width', 'Set how wide messages and the composer can grow on large screens.', 'select',
      select('chatWidth', [['comfortable', 'Comfortable (default)'], ['wide', 'Wide'], ['full', 'Full']], { comfortable: 'Comfortable', wide: 'Wide', full: 'Full' }[prefs.chatWidth])),
  ];
  const motion = [row('panelAnimationDurationMs', 'panel-animations', 'Panel animations', 'Set how fast panels open and close.', 'slider', { ...slider('panelAnimationDurationMs', 0, 400, 25, ' ms'), info: 'preview' })];
  const font = (familyKey: 'fontFamilySans' | 'fontFamilyCode' | 'fontFamilyComposer' | 'fontFamilyTerminal', sizeKey: 'fontSizeInterface' | 'fontSizeCode' | 'fontSizePrompt' | 'fontSizeTerminal',
    id: string, title: string, description: string, placeholder: string, min: number, max: number) =>
    row(familyKey, id, title, description, 'font', { value: prefs[familyKey], placeholder, amount: prefs[sizeKey], previewSize: familyKey === 'fontFamilySans' || familyKey === 'fontFamilyComposer' ? prefs.fontSizePrompt : prefs[sizeKey], value2: String(prefs[sizeKey]), label2: `${prefs[sizeKey]} px`, target: familyKey, suffix: sizeKey,
      driver: fontStack(prefs[familyKey], familyKey === 'fontFamilyCode' || familyKey === 'fontFamilyTerminal') ?? (familyKey === 'fontFamilyCode' || familyKey === 'fontFamilyTerminal' ? 'ui-monospace' : 'system-ui'),
      status: fontStack(prefs[familyKey], familyKey === 'fontFamilyCode' || familyKey === 'fontFamilyTerminal') ? '' : `${prefs[familyKey]} is not available in this app; the default font is used.`,
      info: { 'interface-font': prefs.typographyAdvanced ? '' : 'preview-prompt', 'prompt-font': 'preview-prompt', 'code-font': prefs.typographyAdvanced ? 'preview-code' : 'preview-code-terminal', 'terminal-font': 'preview-terminal' }[id] ?? '',
      options: fontFamilies(prefs[familyKey], familyKey === 'fontFamilyCode' || familyKey === 'fontFamilyTerminal', placeholder), label: prefs[familyKey].trim() || placeholder,
      resettable: prefs[familyKey] !== '' || prefs[sizeKey] !== CLIENT_DEFAULTS[sizeKey], options2: sizes(min, max).map(([value, label]) => option(value, label, value === String(prefs[sizeKey]))) });
  const typography = [
    font('fontFamilySans', 'fontSizeInterface', 'interface-font', 'Interface font', 'Everything outside code blocks and the terminal.', 'SF Pro', 12, 20),
    ...(prefs.typographyAdvanced ? [font('fontFamilyComposer', 'fontSizePrompt', 'prompt-font', 'Prompt font', 'Only the box you write prompts in. Mono works well here.', prefs.fontFamilySans || 'SF Pro', 12, 20)] : []),
    prefs.typographyAdvanced ? font('fontFamilyCode', 'fontSizeCode', 'code-font', 'Code font', 'Code blocks, diffs, and file previews.', 'SF Mono', 10, 18)
      : font('fontFamilyCode', 'fontSizeCode', 'code-font', 'Monospace font', 'Code blocks, diffs, file previews, and the terminal.', 'SF Mono', 10, 18),
    ...(prefs.typographyAdvanced ? [font('fontFamilyTerminal', 'fontSizeTerminal', 'terminal-font', 'Terminal font', 'Terminal output, independent from code blocks and diffs.', prefs.fontFamilyCode || 'SF Mono', 8, 20),
      row('fontSmoothing', 'font-smoothing', 'Font smoothing', 'Use thinner grayscale text smoothing instead of the macOS default.', 'switch')] : []),
    row('wordWrap', 'word-wrap', 'Word wrap', 'Wrap long lines in code blocks, tables, diffs, and file previews by default.', 'switch'),
  ];
  return [{ id: 'appearance-interface', title: 'Interface', collapsible: false, rows: interfaceRows, toggle: false }, { id: 'motion', title: 'Motion', collapsible: false, rows: motion, toggle: false },
    { id: 'typography', title: 'Typography', collapsible: false, rows: typography, toggle: prefs.typographyAdvanced }];
}
