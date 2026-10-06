// Appearance → the theme editor (lane settings-a). Reference: ThemeEditorPanel.tsx
// (THEME_EDITOR_ROLE_GROUPS, the simple Background/Accent pair, Advanced, Filter
// colors), themePalette.ts updateThemeColorFamily and packages/shared
// themePalettes.ts (T3_CODE_LIGHT/DARK_THEME_COLORS). The draft lives with the
// client while the panel is open and paints the app (applyThemeColorPreview);
// saving installs or replaces the custom theme.
import type { T3Client } from './client';
import { ClientError } from './protocol';
import { themeIdFromName, toHex, type CustomTheme } from './settings-themes';
import { themeRoles } from './settings-appearance';

type Mode = 'light' | 'dark';
const LIGHT: Record<string, string> = { canvas: '#fcfcfc', chrome: '#fcfcfc', toolbar: '#fcfcfc', toolbarForeground: '#27272a', toolbarBorder: '#e4e4e7', toolbarControl: '#ffffff',
  toolbarControlForeground: '#27272a', toolbarControlHover: '#f4f4f5', surface: '#ffffff', surfaceRaised: '#fcfcfc', surfaceOverlay: '#ffffff', text: '#27272a', textMuted: '#71717b',
  border: '#e4e4e7', input: '#d4d4d8', focus: '#1b4ed8', accent: '#1b4ed8', accentForeground: '#ffffff', secondary: '#fafafa', secondaryForeground: '#27272a', muted: '#fafafa',
  mutedForeground: '#71717b', placeholder: '#71717b', secondaryLabel: '#71717b', iconMuted: '#71717b', error: '#fb2c36', errorForeground: '#c10007', errorSurface: '#fcebec',
  warning: '#fe9a00', warningForeground: '#bb4d00', warningSurface: '#fcf4e8', update: '#1b4ed8', updateForeground: '#1b4ed8', updateSurface: '#e0e6f7', accentSurface: '#f4f4f5',
  accentSurfaceForeground: '#18181b', messageSurface: '#f4f4f5', messageForeground: '#27272a', messageAction: '#1b4ed8', messageActionForeground: '#ffffff', messageActionHover: '#3160db',
  codeBackground: '#ffffff', codeForeground: '#27272a', sidebar: '#fafafa', sidebarForeground: '#27272a', sidebarMutedForeground: '#71717b', sidebarControlSurface: '#f4f4f5',
  sidebarRowHover: '#fcfcfc', sidebarRowActive: '#ffffff', sidebarRowSelected: '#ffffff', sidebarBorder: '#e4e4e7', terminalBackground: '#fcfcfc', terminalForeground: '#27272a',
  terminalCursor: '#26384e', terminalSelection: '#d0d6dd', terminalScrollbar: '#d6d6d6', terminalScrollbarHover: '#bdbdbd' };
const DARK: Record<string, string> = { canvas: '#0a0a0a', chrome: '#0a0a0a', toolbar: '#0a0a0a', toolbarForeground: '#f5f5f5', toolbarBorder: '#191919', toolbarControl: '#111111',
  toolbarControlForeground: '#f5f5f5', toolbarControlHover: '#141414', surface: '#111111', surfaceRaised: '#111111', surfaceOverlay: '#111111', text: '#f5f5f5', textMuted: '#818181',
  border: '#191919', input: '#1e1e1e', focus: '#346bf1', accent: '#346bf1', accentForeground: '#ffffff', secondary: '#111111', secondaryForeground: '#f5f5f5', muted: '#111111',
  mutedForeground: '#818181', placeholder: '#818181', secondaryLabel: '#818181', iconMuted: '#818181', error: '#fb414a', errorForeground: '#ff6467', errorSurface: '#301214',
  warning: '#fe9a00', warningForeground: '#ffb900', warningSurface: '#312108', update: '#346bf1', updateForeground: '#51a2ff', updateSurface: '#121b34', accentSurface: '#141414',
  accentSurfaceForeground: '#f5f5f5', messageSurface: '#141414', messageForeground: '#f5f5f5', messageAction: '#346bf1', messageActionForeground: '#ffffff', messageActionHover: '#3061d9',
  codeBackground: '#111111', codeForeground: '#f5f5f5', sidebar: '#000000', sidebarForeground: '#f1f3f7', sidebarMutedForeground: '#a3a3a3', sidebarControlSurface: '#0a0a0a',
  sidebarRowHover: '#131313', sidebarRowActive: '#1a1b1b', sidebarRowSelected: '#111111', sidebarBorder: '#141414', terminalBackground: '#0a0a0a', terminalForeground: '#f5f5f5',
  terminalCursor: '#b4cbff', terminalSelection: '#343a47', terminalScrollbar: '#222222', terminalScrollbarHover: '#363636' };
export const STANDARD: Record<Mode, Record<string, string>> = { light: LIGHT, dark: DARK };

/** THEME_EDITOR_ROLE_GROUPS: each family shows its primary role and writes its related roles. */
export const ROLE_GROUPS: { id: string; title: string; families: [string, string, string][] }[] = [
  { id: 'foundation', title: 'Foundation', families: [['background', 'Background', 'canvas'], ['surface', 'Surface', 'surface'], ['raised-surface', 'Raised surface', 'surfaceRaised'],
    ['overlay', 'Overlay', 'surfaceOverlay'], ['text', 'Text', 'text'], ['muted-text', 'Muted text', 'mutedForeground'], ['border', 'Border', 'border'], ['input', 'Input', 'input']] },
  { id: 'brand-content', title: 'Brand & content', families: [['subtle-surface', 'Subtle surface', 'secondary'], ['highlight-surface', 'Highlight surface', 'accentSurface'],
    ['accent', 'Accent', 'accent'], ['action', 'Action', 'messageAction'], ['message-surface', 'Message surface', 'messageSurface'], ['code-surface', 'Code surface', 'codeBackground']] },
  { id: 'context', title: 'Context', families: [['sidebar-background', 'Sidebar background', 'sidebar'], ['sidebar-controls', 'Sidebar controls', 'sidebarControlSurface'],
    ['sidebar-selection', 'Sidebar selection', 'sidebarRowSelected'], ['terminal-background', 'Terminal background', 'terminalBackground']] },
  { id: 'status', title: 'Status', families: [['error', 'Error', 'error'], ['warning', 'Warning', 'warning']] },
];
const SIMPLE: [string, string, string][] = [['background', 'Background', 'canvas'], ['accent', 'Accent', 'accent']];

// ── sRGB helpers (the reference solves in OKLCH; these keep the same intent) ──
const rgb = (hex: string): [number, number, number] => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];
const hexOf = (c: number[]) => '#' + c.map(v => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, '0')).join('');
const mixHex = (a: string, b: string, t: number) => { const x = rgb(a), y = rgb(b); return hexOf(x.map((v, i) => v + (y[i]! - v) * t)); };
const luminance = (hex: string) => { const [r, g, b] = rgb(hex).map(v => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!; };
/** readableThemeForeground: the theme's light or dark ink, whichever reads on the background. */
const foregroundOn = (background: string) => luminance(background) < 0.179 ? '#fffaff' : '#241523';
const toneOn = (selected: string, surface: string) => mixHex(selected, foregroundOn(surface) === '#fffaff' ? '#ffffff' : '#000000', 0.25);

/** updateThemeColorFamily, over 6-digit hex. */
export function updateFamily(colors: Record<string, string>, role: string, input: string): Record<string, string> {
  const hex = toHex(input);
  if (!hex) throw new ClientError('Enter a color such as #1b4ed8.');
  const value = hex.slice(0, 7);
  const canvas = colors.canvas ?? '#fcfcfc', dark = luminance(canvas) < 0.179;
  const status = () => { const surface = mixHex(canvas, value, dark ? 0.16 : 0.08); return { surface, foreground: toneOn(value, surface) }; };
  switch (role) {
    case 'canvas': return { ...colors, canvas: value, chrome: value, toolbar: value };
    case 'text': return { ...colors, text: value, toolbarForeground: value, toolbarControlForeground: value };
    case 'mutedForeground': return { ...colors, textMuted: value, mutedForeground: value, placeholder: value, secondaryLabel: value, iconMuted: value, sidebarMutedForeground: value };
    case 'border': return { ...colors, border: value, toolbarBorder: value, sidebarBorder: value };
    case 'secondary': return { ...colors, secondary: value, secondaryForeground: foregroundOn(value), muted: value, toolbarControl: value };
    case 'accentSurface': return { ...colors, accentSurface: value, accentSurfaceForeground: foregroundOn(value), toolbarControlHover: value };
    case 'accent': { const updateSurface = mixHex(canvas, value, dark ? 0.32 : 0.16);
      return { ...colors, accent: value, accentForeground: foregroundOn(value), focus: value, update: value, updateForeground: toneOn(value, updateSurface), updateSurface, terminalCursor: value }; }
    case 'messageAction': { const fg = foregroundOn(value);
      return { ...colors, messageAction: value, messageActionForeground: fg, messageActionHover: mixHex(value, fg === '#fffaff' ? '#000000' : '#ffffff', 0.12) }; }
    case 'messageSurface': return { ...colors, messageSurface: value, messageForeground: foregroundOn(value) };
    case 'codeBackground': return { ...colors, codeBackground: value, codeForeground: foregroundOn(value) };
    case 'sidebar': return { ...colors, sidebar: value, sidebarForeground: foregroundOn(value) };
    case 'sidebarRowSelected': { const sidebar = colors.sidebar ?? canvas;
      return { ...colors, sidebarRowHover: mixHex(sidebar, value, 0.5), sidebarRowActive: mixHex(sidebar, value, 0.8), sidebarRowSelected: value }; }
    case 'terminalBackground': { const fg = foregroundOn(value), terminalDark = luminance(value) < 0.179;
      return { ...colors, terminalBackground: value, terminalForeground: fg, terminalSelection: mixHex(value, colors.accent ?? value, terminalDark ? 0.35 : 0.18),
        terminalScrollbar: mixHex(value, fg, terminalDark ? 0.42 : 0.22), terminalScrollbarHover: mixHex(value, fg, terminalDark ? 0.55 : 0.32) }; }
    case 'error': { const s = status(); return { ...colors, error: value, errorForeground: s.foreground, errorSurface: s.surface }; }
    case 'warning': { const s = status(); return { ...colors, warning: value, warningForeground: s.foreground, warningSurface: s.surface }; }
    default: return { ...colors, [role]: value };
  }
}

// ── The draft ────────────────────────────────────────────────────────────────
export type Draft = { kind: string; subject: string; editingId: string; name: string; appearance: Mode; advanced: boolean; filter: string; colors: Record<Mode, Record<string, string>> };
const drafts = new WeakMap<T3Client, Draft>();
const customOf = (client: T3Client): CustomTheme[] => (client.local as unknown as { customThemes?: CustomTheme[] }).customThemes || [];
/** A theme's full role set for one appearance: its own roles over the standard ones. */
function rolesOf(id: string, mode: Mode, custom: CustomTheme[]): Record<string, string> {
  const own = custom.find(theme => theme.id === id);
  if (own) return { ...STANDARD[mode], ...(own[mode] ?? own[own.appearance] ?? {}) };
  if (id === 't3-code' || !id) return { ...STANDARD[mode] };
  return { ...STANDARD[mode], ...(themeRoles(id, mode, custom) as Record<string, string>) };
}

/** The draft for the dialog the app has open (kind, subject); opened fresh whenever either changes, dropped when closed. */
export function syncDraft(client: T3Client, kind: string, subject: string, activeLight: string, activeDark: string, appearance: Mode): Draft | null {
  if (!['create', 'edit', 'duplicate'].includes(kind)) { drafts.delete(client); return null; }
  const current = drafts.get(client);
  if (current && current.kind === kind && current.subject === subject) return current;
  const custom = customOf(client);
  const own = custom.find(theme => theme.id === subject);
  const seedLight = kind === 'create' ? activeLight : subject, seedDark = kind === 'create' ? activeDark : subject;
  const label = own?.label ?? subject;
  const draft: Draft = { kind, subject, editingId: kind === 'edit' && own ? own.id : '', appearance: kind === 'edit' && own ? own.appearance : appearance,
    name: kind === 'edit' ? label : kind === 'duplicate' ? `${titleOf(subject, custom)} copy` : '', advanced: false, filter: '',
    colors: { light: rolesOf(seedLight, 'light', custom), dark: rolesOf(seedDark, 'dark', custom) } };
  drafts.set(client, draft);
  return draft;
}
const BUILT_IN: Record<string, string> = { 't3-code': 'T3 Code', 't3-chat': 'T3 Chat', grove: 'Grove', ocean: 'Ocean', ember: 'Ember', iris: 'Iris' };
const titleOf = (id: string, custom: CustomTheme[]) => custom.find(theme => theme.id === id)?.label ?? BUILT_IN[id] ?? id;
export const currentDraft = (client: T3Client) => drafts.get(client) ?? null;

/** The draft as a temporary custom theme the palette can paint (applyThemeColorPreview). */
export function previewTheme(client: T3Client): CustomTheme | null {
  const draft = drafts.get(client);
  if (!draft) return null;
  return { id: '__theme-editor-draft', label: draft.name || 'Draft', appearance: draft.appearance, light: draft.colors.light, dark: draft.colors.dark };
}

export type EditorRow = { id: string; label: string; role: string; value: string };
export type EditorGroup = { id: string; title: string; rows: EditorRow[] };
export type EditorView = { open: boolean; title: string; saveLabel: string; name: string; appearance: string; advanced: boolean; filter: string; rows: EditorRow[]; groups: EditorGroup[]; canSave: boolean; session: string; presets: string[] };
/** ThemeColorPicker presets: Tailwind 500 hues and the neutral ends. */
const PRESETS = ['#ef4444', '#f97316', '#f59e0b', '#eab308', '#84cc16', '#22c55e', '#10b981', '#14b8a6', '#06b6d4', '#0ea5e9', '#3b82f6', '#6366f1', '#8b5cf6', '#a855f7', '#d946ef', '#ec4899', '#f43f5e', '#78716c', '#71717b', '#27272a', '#0a0a0a', '#fafafa', '#fcfcfc', '#ffffff'];
export function editorView(draft: Draft | null): EditorView {
  if (!draft) return { open: false, title: '', saveLabel: '', name: '', appearance: 'light', advanced: false, filter: '', rows: [], groups: [], canSave: false, session: '', presets: PRESETS };
  const colors = draft.colors[draft.appearance];
  const row = ([id, label, role]: [string, string, string]): EditorRow => ({ id, label, role, value: colors[role] ?? STANDARD[draft.appearance][role] ?? '#000000' });
  const filter = draft.filter.trim().toLowerCase();
  const groups = ROLE_GROUPS.map(group => ({ id: group.id, title: group.title, rows: group.families.filter(([, label]) => !filter || label.toLowerCase().includes(filter)).map(row) }))
    .filter(group => group.rows.length > 0);
  const name = draft.name.trim();
  return { open: true, title: draft.editingId ? 'Edit theme' : 'Create theme', saveLabel: draft.editingId ? 'Save theme' : 'Create theme', name: draft.name, appearance: draft.appearance,
    advanced: draft.advanced, filter: draft.filter, rows: SIMPLE.map(row), groups, canSave: name.length <= 48, session: `${draft.kind}:${draft.subject}`, presets: PRESETS };
}

/** theme-draft commands: `name`, `appearance`, `advanced`, `filter`, or `color:<role>` with the value. */
export function editDraft(client: T3Client, part: string, value: string): void {
  const draft = drafts.get(client);
  if (!draft) throw new ClientError('Open the theme editor first.');
  if (part === 'name') { if (value.length > 48) throw new ClientError('Use a name of 48 characters or fewer.'); draft.name = value; return; }
  if (part === 'appearance') { if (value !== 'light' && value !== 'dark') throw new ClientError('Choose Light or Dark.'); draft.appearance = value; return; }
  if (part === 'advanced') { draft.advanced = value === 'true'; return; }
  if (part === 'filter') { draft.filter = value.slice(0, 64); return; }
  if (part.startsWith('color:')) {
    const role = part.slice(6);
    if (!ROLE_GROUPS.some(group => group.families.some(([, , familyRole]) => familyRole === role))) throw new ClientError('That color is not part of a theme.');
    draft.colors[draft.appearance] = updateFamily(draft.colors[draft.appearance], role, value);
    return;
  }
  throw new ClientError('Unsupported theme editor change.');
}

/** Save: a new custom theme (create, duplicate) or the edited one replaced in place. */
export function saveDraft(client: T3Client): CustomTheme {
  const draft = drafts.get(client);
  if (!draft) throw new ClientError('Open the theme editor first.');
  const name = draft.name.trim();
  if (!name || name.length > 48) throw new ClientError('Name the theme (48 characters or fewer).');
  const custom = customOf(client);
  const taken = ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', ...custom.filter(theme => theme.id !== draft.editingId).map(theme => theme.id)];
  let id = draft.editingId;
  if (!id) { const base = themeIdFromName(name); id = base; for (let n = 2; taken.includes(id); n++) id = `${base}-${n}`; }
  const theme: CustomTheme = { id, label: name, appearance: draft.appearance, light: { ...draft.colors.light }, dark: { ...draft.colors.dark } };
  const local = client.local as unknown as { customThemes?: CustomTheme[] };
  if (draft.editingId) local.customThemes = custom.map(entry => entry.id === draft.editingId ? theme : entry);
  else {
    if (custom.length >= 100) throw new ClientError('Remove a theme before adding another.');
    local.customThemes = [...custom, theme];
  }
  drafts.delete(client);
  return theme;
}

/** serializeThemeFile: version 1, the base appearance's colours and the other as a variant. */
export function serializeTheme(theme: CustomTheme): string {
  const base = theme.appearance, other: Mode = base === 'light' ? 'dark' : 'light';
  const file = { version: 1, id: theme.id, name: theme.label, appearance: base, colors: theme[base] ?? {}, ...(theme[other] ? { variants: { [other]: theme[other] } } : {}) };
  return `${JSON.stringify(file, null, 2)}\n`;
}

type Prefs = { theme: string; themeLight: string; themeDark: string };
/** settings-core rows theme-editor (draft edits), theme-editor-save and theme-export. */
export async function themeEditorCommand(client: T3Client, native: { available: boolean; later(request: unknown): Promise<unknown> } | null, row: string, part: string, value: string): Promise<string> {
  if (row === 'theme-editor') { editDraft(client, part, value); return ''; }
  if (row === 'theme-editor-save') {
    const draft = drafts.get(client);
    if (draft && value.trim()) draft.name = value;
    const editing = draft?.editingId ?? '';
    const theme = saveDraft(client);
    const local = client.local as unknown as { clientSettings?: Prefs };
    // A created theme becomes the active pair ("It's now active."); an edit keeps the selection.
    if (!editing && local.clientSettings) local.clientSettings = { ...local.clientSettings, theme: theme.id, themeLight: theme.id, themeDark: theme.id };
    return '';
  }
  if (row === 'theme-export') {
    const theme = customOf(client).find(entry => entry.id === value);
    if (!theme) throw new ClientError('That theme is no longer installed.');
    if (!native?.available) throw new ClientError('Exporting needs the macOS app.');
    const reply = await client.restAccess(native as never).call({ op: 'saveText', suggestedName: `${theme.id}.json`, text: serializeTheme(theme) });
    return reply.saved === true ? '' : '';
  }
  throw new ClientError('Unsupported theme action.');
}
