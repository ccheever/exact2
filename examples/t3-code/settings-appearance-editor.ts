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
import { builtInThemeColors } from './settings-theme-library';
import { sessionInputFor, sessionThemes, sourceAppearance, themeEditorStore } from './theme-editor-session';
import { themeSavedNotice, type ThemeSaveContext } from './theme-editor-notices';
import { pushToast } from './toast';
import { pickerCommit, pickerFields, type PickerFields } from './theme-color-picker';

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

/**
 * updateThemeColorFamily, over hex. The chosen colour keeps its alpha in its own roles (the
 * reference stores it); the roles derived from it read it composited over the canvas, or the
 * sidebar for the selection, as the reference's `selectedOn` does.
 */
export function updateFamily(colors: Record<string, string>, role: string, input: string): Record<string, string> {
  const hex = toHex(input);
  if (!hex) throw new ClientError('Enter a color such as #1b4ed8.');
  const value = hex.length === 9 && hex.endsWith('ff') ? hex.slice(0, 7) : hex;
  const selected = value.slice(0, 7), alphaHex = value.slice(7), alpha = alphaHex ? parseInt(alphaHex, 16) / 255 : 1;
  const on = (background: string) => alpha < 1 ? mixHex(background, selected, alpha) : selected;
  const canvas = colors.canvas ?? '#fcfcfc', dark = luminance(canvas) < 0.179, onCanvas = on(canvas);
  const status = () => { const surface = mixHex(canvas, onCanvas, dark ? 0.16 : 0.08); return { surface, foreground: toneOn(selected, surface) }; };
  switch (role) {
    case 'canvas': return { ...colors, canvas: value, chrome: value, toolbar: value };
    case 'text': return { ...colors, text: value, toolbarForeground: value, toolbarControlForeground: value };
    case 'mutedForeground': return { ...colors, textMuted: value, mutedForeground: value, placeholder: value, secondaryLabel: value, iconMuted: value, sidebarMutedForeground: value };
    case 'border': return { ...colors, border: value, toolbarBorder: value, sidebarBorder: value };
    case 'secondary': return { ...colors, secondary: value, secondaryForeground: foregroundOn(onCanvas), muted: value, toolbarControl: value };
    case 'accentSurface': return { ...colors, accentSurface: value, accentSurfaceForeground: foregroundOn(onCanvas), toolbarControlHover: value };
    case 'accent': { const updateSurface = mixHex(canvas, onCanvas, dark ? 0.32 : 0.16);
      return { ...colors, accent: value, accentForeground: foregroundOn(onCanvas), focus: value, update: value, updateForeground: toneOn(selected, updateSurface), updateSurface, terminalCursor: value }; }
    case 'messageAction': { const fg = foregroundOn(onCanvas);
      return { ...colors, messageAction: value, messageActionForeground: fg, messageActionHover: mixHex(selected, fg === '#fffaff' ? '#000000' : '#ffffff', 0.12) + alphaHex }; }
    case 'messageSurface': return { ...colors, messageSurface: value, messageForeground: foregroundOn(onCanvas) };
    case 'codeBackground': return { ...colors, codeBackground: value, codeForeground: foregroundOn(onCanvas) };
    case 'sidebar': return { ...colors, sidebar: value, sidebarForeground: foregroundOn(onCanvas) };
    case 'sidebarRowSelected': { const sidebar = colors.sidebar ?? canvas, onSidebar = on(sidebar);
      return { ...colors, sidebarRowHover: mixHex(sidebar, onSidebar, 0.5), sidebarRowActive: mixHex(sidebar, onSidebar, 0.8), sidebarRowSelected: value }; }
    case 'terminalBackground': { const fg = foregroundOn(onCanvas), terminalDark = luminance(onCanvas) < 0.179;
      return { ...colors, terminalBackground: value, terminalForeground: fg, terminalSelection: mixHex(onCanvas, colors.accent ?? selected, terminalDark ? 0.35 : 0.18),
        terminalScrollbar: mixHex(onCanvas, fg, terminalDark ? 0.42 : 0.22), terminalScrollbarHover: mixHex(onCanvas, fg, terminalDark ? 0.55 : 0.32) }; }
    case 'error': { const s = status(); return { ...colors, error: value, errorForeground: s.foreground, errorSurface: s.surface }; }
    case 'warning': { const s = status(); return { ...colors, warning: value, warningForeground: s.foreground, warningSurface: s.surface }; }
    default: return { ...colors, [role]: value };
  }
}

// ── The draft ────────────────────────────────────────────────────────────────
// The draft belongs to the client's theme editor session (theme-editor-session.ts), which
// the window's root state opens and closes; it outlives Settings (D16).
export type Draft = { kind: string; subject: string; sessionId: number; editingId: string; name: string; appearance: Mode; advanced: boolean; filter: string; colors: Record<Mode, Record<string, string>>;
  /** The stamp of the last colour-picker op applied, per role (themeLocal). */
  pickerSeq: Record<string, number> };
const drafts = new WeakMap<T3Client, Draft>();
const customOf = (client: T3Client): CustomTheme[] => (client.local as unknown as { customThemes?: CustomTheme[] }).customThemes || [];
/**
 * A theme's full role set for one appearance: its own roles over the standard ones. A palette the theme lacks is the
 * standard one (ThemeEditorPanel.tsx:385-390 seeds getThemeEditorColorsByAppearance, then the theme's own palettes).
 */
function rolesOf(id: string, mode: Mode, custom: CustomTheme[]): Record<string, string> {
  const own = custom.find(theme => theme.id === id);
  if (own) return { ...STANDARD[mode], ...(own[mode] ?? {}) };
  if (id === 't3-code' || !id) return { ...STANDARD[mode] };
  // A built-in source seeds its own definition, every role (ThemeEditorPanel's sourceTheme colours).
  return { ...STANDARD[mode], ...(builtInThemeColors(id, mode) ?? themeRoles(id, mode, custom) as Record<string, string>) };
}

/**
 * ThemeEditorPanel's open (1e2ecbd975 :395-399): Advanced for any source theme the guided editor did
 * not make (a built-in, an import, an older or Advanced save), so guided regeneration cannot discard
 * hand-tuned colours; simple for a guided (managed) save and for no source at all. T3 Code's stock look
 * is no theme definition there (getThemeDefinition), so a theme seeded from it opens simple.
 */
export function opensAdvanced(source: { id: string; custom: CustomTheme | null } | null): boolean {
  if (!source || source.id === 't3-code') return false;
  return source.custom?.managed !== true;
}

/**
 * The draft for the editor session the window names (kind, subject): a new session, seeded
 * from the library as it is now, whenever either changes; closed and dropped for any other kind.
 */
export function syncDraft(client: T3Client, kind: string, subject: string, prefs: { theme: string; themeLight: string; themeDark: string }, appearance: Mode): Draft | null {
  const store = themeEditorStore(client);
  const custom = customOf(client);
  const input = sessionInputFor(kind, subject, prefs, appearance, custom);
  if (!input) { store.closeThemeEditor(); drafts.delete(client); return null; }
  const current = drafts.get(client);
  if (current && store.session && current.sessionId === store.session.id && current.kind === kind && current.subject === subject) return current;
  store.openThemeEditor(input);
  const session = store.session!;
  const { editingTheme, seedTheme } = sessionThemes(session, custom);
  const source = editingTheme?.id ?? seedTheme?.id ?? '';
  const draft: Draft = { kind, subject, sessionId: session.id, editingId: editingTheme?.custom ? editingTheme.id : '', appearance: sourceAppearance(editingTheme ?? seedTheme, session.initialAppearance),
    name: editingTheme ? editingTheme.label : session.seedName ?? '', advanced: opensAdvanced(editingTheme ?? seedTheme), filter: '',
    colors: { light: rolesOf(source, 'light', custom), dark: rolesOf(source, 'dark', custom) }, pickerSeq: {} };
  drafts.set(client, draft);
  return draft;
}
export const currentDraft = (client: T3Client) => drafts.get(client) ?? null;

/** The draft as a temporary custom theme the palette can paint (applyThemeColorPreview). */
export function previewTheme(client: T3Client): CustomTheme | null {
  const draft = drafts.get(client);
  if (!draft) return null;
  return { id: '__theme-editor-draft', label: draft.name || 'Draft', appearance: draft.appearance, light: draft.colors.light, dark: draft.colors.dark };
}

const modesOf = (theme: CustomTheme): Mode[] => (['light', 'dark'] as Mode[]).filter(mode => theme[mode] != null || theme.appearance === mode);
/**
 * ThemeEditorPanel's mergeTarget (:421-430): another installed theme the draft's name already names, by its derived id
 * or its label. A create adds the draft's palette to it; an edit renamed onto it folds its palettes in.
 */
export function mergeTargetOf(draft: Pick<Draft, 'name' | 'editingId'>, custom: CustomTheme[]): CustomTheme | null {
  const name = draft.name.trim().toLowerCase();
  if (!name) return null;
  const id = themeIdFromName(draft.name.trim());
  return custom.find(theme => theme.id !== draft.editingId && (theme.id === id || theme.label.trim().toLowerCase() === name)) ?? null;
}
/** The submit button (ThemeEditorPanel.tsx:1252-1266): its label and icon. */
export function submitLabel(draft: Pick<Draft, 'name' | 'editingId' | 'appearance'>, custom: CustomTheme[]): { label: string; icon: string } {
  const target = mergeTargetOf(draft, custom);
  if (draft.editingId) return { label: target ? `Merge into “${target.label}”` : 'Save changes', icon: '' };
  return target ? { label: `Add ${draft.appearance} palette`, icon: 'plus' } : { label: 'Create theme', icon: 'paintbrush' };
}
/**
 * The name changed (ThemeEditorPanel.tsx:449-462): a create named like a theme that has only the selected appearance
 * flips the draft to the free one, so the merge works without a manual toggle.
 */
function followName(draft: Draft, custom: CustomTheme[]): void {
  const target = draft.editingId ? null : mergeTargetOf(draft, custom);
  const taken = target ? modesOf(target) : [];
  if (taken.length === 1 && taken[0] === draft.appearance) draft.appearance = draft.appearance === 'light' ? 'dark' : 'light';
}

/** A family row: its value, the picker's starting point for it, and the last picker op applied (`seq`). */
export type EditorRow = { id: string; label: string; role: string; value: string; seq: number } & PickerFields;
export type EditorGroup = { id: string; title: string; rows: EditorRow[] };
export type EditorView = { open: boolean; title: string; saveLabel: string; saveIcon: string; name: string; appearance: string; advanced: boolean; filter: string; rows: EditorRow[]; groups: EditorGroup[]; canSave: boolean; session: string };
export function editorView(draft: Draft | null, custom: CustomTheme[] = []): EditorView {
  if (!draft) return { open: false, title: '', saveLabel: '', saveIcon: '', name: '', appearance: 'light', advanced: false, filter: '', rows: [], groups: [], canSave: false, session: '' };
  const colors = draft.colors[draft.appearance];
  const row = ([id, label, role]: [string, string, string]): EditorRow => {
    const value = colors[role] ?? STANDARD[draft.appearance][role] ?? '#000000';
    return { id, label, role, value, seq: draft.pickerSeq[role] ?? 0, ...pickerFields(value) };
  };
  const filter = draft.filter.trim().toLowerCase();
  const groups = ROLE_GROUPS.map(group => ({ id: group.id, title: group.title, rows: group.families.filter(([, label]) => !filter || label.toLowerCase().includes(filter)).map(row) }))
    .filter(group => group.rows.length > 0);
  const name = draft.name.trim();
  const submit = submitLabel(draft, custom);
  return { open: true, title: draft.editingId ? 'Edit theme' : 'Create theme', saveLabel: submit.label, saveIcon: submit.icon, name: draft.name, appearance: draft.appearance,
    advanced: draft.advanced, filter: draft.filter, rows: SIMPLE.map(row), groups, canSave: name.length <= 48, session: `session-${draft.sessionId}` };
}
const isFamilyRole = (role: string) => ROLE_GROUPS.some(group => group.families.some(([, , familyRole]) => familyRole === role));

/**
 * `themelocal:<part>` (theme-color-picker.contract) with the id `<session>|<role>`: the picker's `color`
 * (the plane, the hue slider and their keys), `hex` and `rgb` (the typed fields, each keystroke) for
 * one family of the session's draft. The control stamps each op, strictly increasing from the runner
 * time, so a late older op never overwrites a newer one (and an op from a closed session reaches no
 * other), and the row echoes the stamp back (`seq`) so the control knows its op has landed. Never an
 * error: an incomplete field changes nothing, as the reference ignores it until it parses.
 */
export function themeLocal(client: T3Client, part: string, id: string, value: string, n: number): string {
  const draft = drafts.get(client);
  const [session, role = ''] = id.split('|');
  // audit-wave-followups-2 FV-4: the name as it is typed, so the submit label follows it (`<session>|name`).
  if (draft && part === 'name' && role === 'name' && session === `session-${draft.sessionId}` && n > (draft.pickerSeq.name ?? -Infinity)) {
    draft.pickerSeq.name = n; draft.name = value; followName(draft, customOf(client));
    return '';
  }
  if (!draft || session !== `session-${draft.sessionId}` || !isFamilyRole(role)) return '';
  if (n <= (draft.pickerSeq[role] ?? -Infinity)) return '';
  draft.pickerSeq[role] = n;
  const colors = draft.colors[draft.appearance];
  const next = pickerCommit(part, value, colors[role] ?? STANDARD[draft.appearance][role] ?? '#000000');
  if (next) draft.colors[draft.appearance] = updateFamily(colors, role, next);
  return '';
}

/** theme-draft commands: `name`, `appearance`, `advanced`, `filter`, or `color:<role>` with the value. */
export function editDraft(client: T3Client, part: string, value: string): void {
  const draft = drafts.get(client);
  if (!draft) throw new ClientError('Open the theme editor first.');
  if (part === 'name') { if (value.length > 48) throw new ClientError('Use a name of 48 characters or fewer.'); draft.name = value; followName(draft, customOf(client)); return; }
  if (part === 'appearance') { if (value !== 'light' && value !== 'dark') throw new ClientError('Choose Light or Dark.'); draft.appearance = value; return; }
  if (part === 'advanced') { draft.advanced = value === 'true'; return; }
  if (part === 'filter') { draft.filter = value.slice(0, 64); return; }
  if (part.startsWith('color:')) {
    const role = part.slice(6);
    if (!isFamilyRole(role)) throw new ClientError('That color is not part of a theme.');
    draft.colors[draft.appearance] = updateFamily(draft.colors[draft.appearance], role, value);
    return;
  }
  throw new ClientError('Unsupported theme editor change.');
}

/**
 * Save: the edited theme replaced in place, or a new custom theme (create, duplicate). The
 * edited theme is read fresh: one removed while its editor was open saves as a create. A
 * create named like an installed theme that lacks this appearance adds the palette to it.
 */
export function saveDraft(client: T3Client): { theme: CustomTheme; context: ThemeSaveContext } {
  const draft = drafts.get(client);
  if (!draft) throw new ClientError('Open the theme editor first.');
  const name = draft.name.trim();
  if (!name || name.length > 48) throw new ClientError('Name the theme (48 characters or fewer).');
  const custom = customOf(client);
  const local = client.local as unknown as { customThemes?: CustomTheme[] };
  const editing = draft.editingId ? custom.find(theme => theme.id === draft.editingId) ?? null : null;
  const mergeTarget = mergeTargetOf({ name, editingId: editing?.id ?? '' }, custom);
  let theme: CustomTheme, context: ThemeSaveContext;
  if (editing && mergeTarget) {
    // Renamed onto another installed theme (ThemeEditorPanel.tsx:788-828): this theme's palettes fold into it and the
    // edited entry retires; colliding palettes cannot merge.
    const edited = modesOf(editing), collision = edited.find(mode => modesOf(mergeTarget).includes(mode));
    if (collision) throw new ClientError(`“${mergeTarget.label}” already has a ${collision} palette. Pick another name.`);
    const { managed: _managed, ...target } = mergeTarget;
    theme = { ...target, ...Object.fromEntries(edited.map(mode => [mode, { ...draft.colors[mode] }])), ...(mergeTarget.managed === true && !draft.advanced ? { managed: true } : {}) };
    local.customThemes = custom.filter(entry => entry.id !== editing.id).map(entry => entry.id === mergeTarget.id ? theme : entry);
    context = { created: false, mergedAppearance: edited[0] };
  } else if (mergeTarget) {
    if (modesOf(mergeTarget).includes(draft.appearance)) throw new ClientError(`“${mergeTarget.label}” already has light and dark palettes. Pick another name.`);
    // The guided (managed) flag survives only when every palette in the theme came from the guided editor.
    const { managed: _managed, ...target } = mergeTarget;
    theme = { ...target, [draft.appearance]: { ...draft.colors[draft.appearance] }, ...(mergeTarget.managed === true && !draft.advanced ? { managed: true } : {}) };
    local.customThemes = custom.map(entry => entry.id === mergeTarget.id ? theme : entry);
    context = { created: false, mergedAppearance: draft.appearance };
  } else {
    const taken = ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', ...custom.filter(entry => entry.id !== editing?.id).map(entry => entry.id)];
    let id = editing?.id ?? '';
    if (!id) { const base = themeIdFromName(name); id = base; for (let n = 2; taken.includes(id); n++) id = `${base}-${n}`; }
    // handleSubmit: an edit keeps the theme's own palettes (its base appearance and, when it has one, its
    // variant); a new theme installs only the palette of the appearance being edited.
    const modes: Mode[] = editing ? (['light', 'dark'] as Mode[]).filter(mode => mode === editing.appearance || editing[mode] != null) : [draft.appearance];
    // A save from simple mode is the guided editor's (managed); an edit keeps its collection.
    theme = { id, label: name, appearance: editing?.appearance ?? draft.appearance, light: modes.includes('light') ? { ...draft.colors.light } : null, dark: modes.includes('dark') ? { ...draft.colors.dark } : null,
      ...(editing?.collection ? { collection: editing.collection } : {}), ...(draft.advanced ? {} : { managed: true }) };
    if (editing) local.customThemes = custom.map(entry => entry.id === editing.id ? theme : entry);
    else {
      if (custom.length >= 100) throw new ClientError('Remove a theme before adding another.');
      local.customThemes = [...custom, theme];
    }
    context = { created: !editing };
  }
  drafts.delete(client);
  themeEditorStore(client).closeThemeEditor();
  return { theme, context };
}

/** serializeThemeFile: version 1, the base appearance's colours and the other as a variant. */
export function serializeTheme(theme: CustomTheme): string {
  const base = theme.appearance, other: Mode = base === 'light' ? 'dark' : 'light';
  const file = { version: 1, id: theme.id, name: theme.label, appearance: base, colors: theme[base] ?? {}, ...(theme[other] ? { variants: { [other]: theme[other] } } : {}), ...(theme.managed ? { managed: true } : {}) };
  return `${JSON.stringify(file, null, 2)}\n`;
}

type Prefs = { theme: string; themeLight: string; themeDark: string };
/** settings-core rows theme-editor (draft edits), theme-editor-save and theme-export. */
export async function themeEditorCommand(client: T3Client, native: { available: boolean; later(request: unknown): Promise<unknown> } | null, row: string, part: string, value: string): Promise<string> {
  if (row === 'theme-editor') { editDraft(client, part, value); return ''; }
  if (row === 'theme-editor-save') {
    const draft = drafts.get(client);
    if (draft && value.trim()) draft.name = value;
    const { theme, context } = saveDraft(client);
    const local = client.local as unknown as { clientSettings?: Prefs };
    const prefs = local.clientSettings ?? { theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code' };
    // handleSaved: a create or a merge becomes the active pair; an edit keeps the selection.
    const notice = themeSavedNotice(theme, context, prefs, () => {
      if (!local.clientSettings) return false;
      local.clientSettings = { ...local.clientSettings, theme: theme.id, themeLight: theme.id, themeDark: theme.id };
      return true;
    });
    pushToast(client, { ...notice, stacked: true });
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
