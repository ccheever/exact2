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
import { previewTheme } from './settings-appearance-editor';

export type Look = {
  themed: boolean; mode: string;
  canvas: string; sidebar: string; sidebarBorder: string; sidebarText: string; sidebarMuted: string; rowActive: string;
  surface: string; popover: string; border: string; input: string; text: string; muted: string; accent: string; accentText: string; message: string;
  chatMax: number; artwork: boolean; pill: string; diff: string;
  diffAdd: string; diffDel: string; diffAddSurface: string; diffDelSurface: string; diffAddLine: string; diffDelLine: string;
  fontSans: string; fontSize: number; codeFont: string; codeSize: number; wordWrap: boolean; smoothing: boolean; panelMs: number;
  contextStrip: boolean; contextMeter: boolean; richText: boolean; skillsInSlash: boolean; followUp: string; legacySidebar: boolean;
  confirmUnpin: boolean; confirmArchive: boolean; confirmDelete: boolean;
};

const prefsOf = (client: T3Client): ClientPrefs => (client.local as unknown as { clientSettings?: ClientPrefs }).clientSettings || decodeClientPrefs({});
const customOf = (client: T3Client): CustomTheme[] => (client.local as unknown as { customThemes?: CustomTheme[] }).customThemes || [];
/** --chat-content-max-width: 46rem, 72rem (wide) or the whole column (full). */
export const CHAT_MAX: Record<string, number> = { comfortable: 736, wide: 1152, full: 100000 };
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
  return {
    themed, mode,
    canvas: pal.canvas, sidebar: pal.sidebar, sidebarBorder: pal.sidebarBorder, sidebarText: pal.sidebarText, sidebarMuted: pal.sidebarMuted, rowActive: pal.rowActive,
    message: messageSurface(prefs, custom, mode), surface: pal.surface, popover: pal.popover, border: pal.border, input: pal.input, text: pal.text, muted: pal.muted, accent: pal.accent, accentText: pal.accentText,
    diff: prefs.diffColorScheme === 'blue-orange' ? 'blue-orange' : 'red-green', chatMax: CHAT_MAX[prefs.chatWidth] ?? 736, artwork: prefs.environmentIdentificationMode === 'artwork', pill: prefs.environmentIdentificationMode === 'pill' ? 'Nightly' : '',
    diffAdd: diff.add, diffDel: diff.del, diffAddSurface: diff.addSurface, diffDelSurface: diff.delSurface, diffAddLine: diff.addLine, diffDelLine: diff.delLine,
    fontSans: fontStack(prefs.fontFamilySans, false) ?? 'system-ui', fontSize: prefs.fontSizeInterface, codeFont: fontStack(prefs.fontFamilyCode, true) ?? 'ui-monospace',
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
