// The device appearance the whole app reads (lane settings-a): the snapshot's
// `look`. Reference: routes/__root.tsx (data-chat-width, data-diff-color-scheme,
// font and theme variables on :root), index.css (--chat-content-max-width,
// :root[data-diff-color-scheme="blue-orange"]), appearanceContrast.ts, the
// theme palette roles and SidebarChrome's environment identification.
// `themed` is false for the stock T3 Code theme at default contrast and glass,
// so surfaces keep their measured stock colours; otherwise they take the
// palette's light-dark() tokens.
import type { T3Client } from './client';
import { CLIENT_DEFAULTS, decodeClientPrefs, type ClientPrefs } from './settings-core';
import { fontStack, palette, themeRoles } from './settings-appearance';
import type { CustomTheme } from './settings-themes';
import type { DiffState } from './diff';
import { STANDARD, previewTheme } from './settings-appearance-editor';
import { clampInterfaceFontSize } from './appearance-fonts';

export type Look = {
  themed: boolean; mode: string;
  canvas: string; sidebar: string; sidebarBorder: string; sidebarText: string; sidebarMuted: string; rowActive: string;
  surface: string; popover: string; border: string; input: string; text: string; muted: string; accent: string; accentText: string; message: string;
  chatMax: number; artwork: boolean; pill: string; diff: string;
  diffAdd: string; diffDel: string; diffAddSurface: string; diffDelSurface: string; diffAddLine: string; diffDelLine: string;
  terminalLight: string; terminalDark: string; terminalFont: string; terminalSize: number;
  fontSans: string; fontSize: number; codeFont: string; codeSize: number; wordWrap: boolean; smoothing: boolean; panelMs: number;
  contextStrip: boolean; contextMeter: boolean; richText: boolean; skillsInSlash: boolean; followUp: string; legacySidebar: boolean;
  confirmUnpin: boolean; confirmArchive: boolean; confirmDelete: boolean;
};

const prefsOf = (client: T3Client): ClientPrefs => (client.local as unknown as { clientSettings?: ClientPrefs }).clientSettings || decodeClientPrefs({});
const customOf = (client: T3Client): CustomTheme[] => (client.local as unknown as { customThemes?: CustomTheme[] }).customThemes || [];
/** --chat-content-max-width in rem: 46rem, 72rem (wide) or the whole column (full). */
export const CHAT_MAX_REM: Record<string, number> = { comfortable: 46, wide: 72 };
/** The chat width setting in px at the root font size (the Interface font size). */
export function chatMaxWidth(chatWidth: string, rootFontSize: number): number {
  return chatWidth === 'full' ? 100000 : (CHAT_MAX_REM[chatWidth] ?? 46) * rootFontSize;
}
// index.css diff tokens: success/destructive by default, blue/orange for colour-blind users.
const DIFF = {
  'red-green': { add: 'light-dark(#00a63e, #05df72)', del: 'light-dark(#e7000b, #ff6467)', addSurface: 'light-dark(#eef9f5, #0e1713)', delSurface: 'light-dark(#fef0f0, #1c100f)',
    addLine: 'light-dark(#18a46c, #07c480)', delLine: 'light-dark(#d52c36, #ff2e3f)' },
  'blue-orange': { add: 'light-dark(#155dfc, #51a2ff)', del: 'light-dark(#f54900, #ff8904)', addSurface: 'light-dark(#eff6ff, #0e1423)', delSurface: 'light-dark(#fff7ed, #1f130b)',
    addLine: 'light-dark(#155dfc, #51a2ff)', delLine: 'light-dark(#f54900, #ff8904)' },
};

export function look(client: T3Client): Look {
  const preview = previewTheme(client);
  const prefs = preview ? { ...prefsOf(client), [preview.appearance === 'light' ? 'themeLight' : 'themeDark']: preview.id } : prefsOf(client);
  const custom = preview ? [...customOf(client), preview] : customOf(client);
  const mode = client.local.deviceSettings.appearanceMode;
  const pal = palette(prefs, custom, mode);
  const diffState = (client as unknown as { diffState?: DiffState }).diffState;
  if (diffState) seedDiffState(client, diffState);
  const themed = prefs.themeLight !== 't3-code' || prefs.themeDark !== 't3-code' || prefs.appearanceContrast !== 100 || prefs.glassOpacity !== CLIENT_DEFAULTS.glassOpacity;
  const diff = DIFF[prefs.diffColorScheme === 'blue-orange' ? 'blue-orange' : 'red-green'];
  // applyAppearanceFontVariables: the root font size (app.contract `rootFont` sets it), 16 until the preferences load.
  const rootFontSize = clampInterfaceFontSize(prefs.fontSizeInterface);
  return {
    themed, mode,
    terminalLight: terminalTheme(prefs.themeLight, 'light', custom), terminalDark: terminalTheme(prefs.themeDark, 'dark', custom),
    terminalFont: prefs.typographyAdvanced ? prefs.fontFamilyTerminal : prefs.fontFamilyCode,
    terminalSize: prefs.typographyAdvanced ? prefs.fontSizeTerminal : prefs.fontSizeCode,
    canvas: pal.canvas, sidebar: pal.sidebar, sidebarBorder: pal.sidebarBorder, sidebarText: pal.sidebarText, sidebarMuted: pal.sidebarMuted, rowActive: pal.rowActive,
    message: messageSurface(prefs, custom, mode), surface: pal.surface, popover: pal.popover, border: pal.border, input: pal.input, text: pal.text, muted: pal.muted, accent: pal.accent, accentText: pal.accentText,
    diff: prefs.diffColorScheme === 'blue-orange' ? 'blue-orange' : 'red-green', chatMax: chatMaxWidth(prefs.chatWidth, rootFontSize), artwork: prefs.environmentIdentificationMode === 'artwork', pill: prefs.environmentIdentificationMode === 'pill' ? 'Nightly' : '',
    diffAdd: diff.add, diffDel: diff.del, diffAddSurface: diff.addSurface, diffDelSurface: diff.delSurface, diffAddLine: diff.addLine, diffDelLine: diff.delLine,
    fontSans: fontStack(prefs.fontFamilySans, false) ?? 'system-ui', fontSize: rootFontSize, codeFont: fontStack(prefs.fontFamilyCode, true) ?? 'ui-monospace',
    codeSize: prefs.fontSizeCode, wordWrap: prefs.wordWrap, smoothing: prefs.fontSmoothing, panelMs: prefs.panelAnimationDurationMs,
    contextStrip: prefs.persistComposerContextStrip, contextMeter: prefs.contextWindowMeterEnabled, richText: prefs.composerRichTextEnabled,
    skillsInSlash: prefs.showSkillsInSlashMenu, followUp: prefs.followUpBehavior, legacySidebar: prefs.legacySidebarEnabled,
    confirmUnpin: prefs.confirmThreadUnpin, confirmArchive: prefs.confirmThreadArchive, confirmDelete: prefs.confirmThreadDelete,
  };
}

/** bg-message (--message-surface): the theme's messageSurface role; the stock theme keeps --accent. */
function messageSurface(prefs: ClientPrefs, custom: CustomTheme[], mode: string): string {
  const half = (id: string, appearance: 'light' | 'dark') => id === 't3-code' ? (appearance === 'light' ? '#f4f4f5' : '#ffffff0a') : themeRoles(id, appearance, custom).messageSurface;
  const light = half(prefs.themeLight, 'light'), dark = half(prefs.themeDark, 'dark');
  return mode === 'light' ? light : mode === 'dark' ? dark : `light-dark(${light}, ${dark})`;
}

// DiffState follows the General diff defaults: seeded once, and again whenever one of them changes.
const seeded = new WeakMap<DiffState, string>();
export function seedDiffState(client: T3Client, state: DiffState | undefined): void {
  if (!state) return;
  const prefs = prefsOf(client);
  const signature = `${prefs.diffLayout}|${prefs.diffIgnoreWhitespace}|${prefs.wordWrap}|${prefs.diffFilesCollapsed}`;
  if (seeded.get(state) === signature) return;
  seeded.set(state, signature);
  state.layout = prefs.diffLayout === 'split' ? 'split' : 'stacked';
  state.ignoreWhitespace = prefs.diffIgnoreWhitespace;
  state.wrap = prefs.wordWrap;
  (state as DiffState & { defaultExpanded?: boolean }).defaultExpanded = !prefs.diffFilesCollapsed;
}
/** The diff toolbar's layout toggle writes the Diff layout setting back (General → Behavior). */
export function rememberDiffLayout(client: T3Client, layout: string): void {
  if (layout !== 'stacked' && layout !== 'split') return;
  const local = client.local as unknown as { clientSettings?: ClientPrefs };
  local.clientSettings = { ...prefsOf(client), diffLayout: layout };
  const state = (client as unknown as { diffState: DiffState }).diffState;
  seeded.set(state, `${layout}|${local.clientSettings.diffIgnoreWhitespace}|${local.clientSettings.wordWrap}|${local.clientSettings.diffFilesCollapsed}`);
}

// Terminal roles from T3 Code 1e2ecbd975 packages/shared/src/themePalettes.ts, converted
// from OKLCH to sRGB like settings-appearance.ts. Order: background, foreground, cursor, selection.
const TERMINAL_PALETTES: Record<string, readonly [string, string]> = {
  't3-chat': ['#fdf7fd #501854 #db2777 #f1c4e6', '#1f1a24 #f9f8fb #db2777 #362d3d'],
  grove: ['#f3f7f4 #241523 #1b7d50 #cce1d7', '#1b2821 #fffaff #69d69a #36654c'],
  ocean: ['#f5f7f8 #241523 #2672af #d0dfeb', '#17212b #fffaff #70b9ee #36566f'],
  ember: ['#f9f7f5 #241523 #ae552a #ebdad1', '#291e1a #fffaff #f09a64 #6e4934'],
  iris: ['#f8f7f9 #241523 #7253b9 #e0d9ee', '#1d1929 #fffaff #9d7df2 #4a3c70'],
};
/** JSON for the native terminal bridge; custom/editor roles override the standard palette. */
export function terminalTheme(id: string, mode: 'light' | 'dark', custom: CustomTheme[]): string {
  const own = custom.find(theme => theme.id === id);
  const roles = { ...STANDARD[mode], ...(own ? own[mode] ?? own[own.appearance] ?? {} : {}) };
  const builtIn = own ? undefined : TERMINAL_PALETTES[id]?.[mode === 'dark' ? 1 : 0].split(' ');
  const rgb = (hex: string) => ({ r: parseInt(hex.slice(1, 3), 16), g: parseInt(hex.slice(3, 5), 16), b: parseInt(hex.slice(5, 7), 16) });
  return JSON.stringify({ dark: mode === 'dark',
    background: rgb(builtIn?.[0] ?? roles.terminalBackground ?? '#fcfcfc'),
    foreground: rgb(builtIn?.[1] ?? roles.terminalForeground ?? '#27272a'),
    cursor: rgb(builtIn?.[2] ?? roles.terminalCursor ?? '#26384e'),
    selectionBackground: builtIn?.[3] ?? (own ? roles.terminalSelection : mode === 'dark' ? 'rgba(180, 203, 255, 0.25)' : 'rgba(37, 63, 99, 0.2)'),
  });
}
