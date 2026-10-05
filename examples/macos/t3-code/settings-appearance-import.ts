// Appearance → Add a theme (lane settings-a). Reference: ThemeImportDialog.tsx,
// ThemeSearchSection.tsx, openVsxThemes.ts and vscodeThemeImport.ts. Open VSX is
// read through the native module's fetchText (open-vsx.org only); an extension's
// theme files are read from Open VSX's per-file endpoint rather than by unpacking
// the VSIX. Files come from an NSOpenPanel (the desktop shell's file picker).
import type { T3Client } from './client';
import { ClientError, type Native } from './protocol';
import { obj, str, type Obj } from './domain';
import { parseThemeFile, themeIdFromName, toHex, type CustomTheme } from './settings-themes';
import { STANDARD, updateFamily } from './settings-appearance-editor';
import { pushToast } from './toast';

export const SUGGESTED_SEARCHES = ['Dracula', 'Catppuccin', 'Nord', 'Tokyo Night'];
export const SORT_OPTIONS: [string, string][] = [['downloadCount', 'Most downloaded'], ['rating', 'Best rated'], ['timestamp', 'Newest'], ['relevance', 'Most relevant']];
const SUPPORTED_LICENSES = new Set(['0BSD', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'CC0-1.0', 'ISC', 'MIT', 'MPL-2.0', 'Unlicense']);
const MAX_THEMES_PER_EXTENSION = 40;
const FORMAT = (n: number) => n >= 1e6 ? `${(n / 1e6).toFixed(1).replace(/\.0$/, '')}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1).replace(/\.0$/, '')}K` : String(n);

export type Extension = { id: string; namespace: string; name: string; displayName: string; publisher: string; description: string; version: string; downloads: string;
  sourceUrl: string; source: string; iconUrl: string; installed: boolean; installing: boolean; action: string };
type Search = { query: string; sort: string; searching: boolean; error: string; results: Extension[] | null; installing: string; pendingUpdate: string; fileName: string; json: string; conflicts: string };
const searches = new WeakMap<T3Client, Search>();
const blank = (): Search => ({ query: '', sort: 'downloadCount', searching: false, error: '', results: null, installing: '', pendingUpdate: '', fileName: '', json: '', conflicts: '' });
export const searchState = (client: T3Client) => { let state = searches.get(client); if (!state) { state = blank(); searches.set(client, state); } return state; };
export function resetImport(client: T3Client): void { searches.set(client, blank()); }

// ── JSONC (VS Code theme and package files allow comments and trailing commas) ──
export function parseJsonc(source: string): unknown {
  let out = '', inString = false;
  for (let i = 0; i < source.length; i++) {
    const c = source[i]!, next = source[i + 1];
    if (inString) { out += c; if (c === '\\') { out += next ?? ''; i++; } else if (c === '"') inString = false; continue; }
    if (c === '"') { inString = true; out += c; continue; }
    if (c === '/' && next === '/') { while (i < source.length && source[i] !== '\n') i++; out += '\n'; continue; }
    if (c === '/' && next === '*') { i += 2; while (i < source.length && !(source[i] === '*' && source[i + 1] === '/')) i++; i++; continue; }
    out += c;
  }
  return JSON.parse(out.replace(/,(\s*[}\]])/g, '$1'));
}

// ── VS Code theme conversion (vscodeThemeImport.ts parseVsCodeThemeFile) ──────
const rgb = (hex: string) => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));
const lum = (hex: string) => { const [r, g, b] = rgb(hex).map(v => { const c = v / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!; };
const contrast = (a: string, b: string) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
/** A VS Code colour (#rgb[a], #rrggbb[aa]) flattened over its surface. */
function over(value: unknown, base: string): string | null {
  const hex = toHex(value);
  if (!hex) return null;
  if (hex.length === 7) return hex;
  const alpha = parseInt(hex.slice(7, 9), 16) / 255, top = rgb(hex.slice(0, 7)), bottom = rgb(base);
  return '#' + top.map((v, i) => Math.round(bottom[i]! + (v - bottom[i]!) * alpha).toString(16).padStart(2, '0')).join('');
}
export function isVsCodeTheme(value: unknown): boolean {
  const file = obj(value);
  if (file.version === 1) return false;
  return Object.keys(obj(file.colors)).some(key => key.includes('.')) || Array.isArray(file.tokenColors);
}
export function humanizeThemeName(raw: string): string {
  const trimmed = raw.trim();
  if (/\s/.test(trimmed) || !/[-_.]/.test(trimmed)) return trimmed;
  return trimmed.split(/[-_.]+/).filter(Boolean).map(word => word.charAt(0).toUpperCase() + word.slice(1)).join(' ');
}
export function convertVsCodeTheme(value: unknown, taken: string[]): CustomTheme {
  const file = obj(value), colors = obj(file.colors);
  const pick = (base: string, ...keys: string[]) => { for (const key of keys) { const hex = over(colors[key], base); if (hex) return hex; } return null; };
  const canvas = pick('#000000', 'editor.background', 'editorPane.background');
  if (!canvas) throw new ClientError('That VS Code theme has no "editor.background" color, so there is nothing to build a palette from.');
  const type = str(file.type).toLowerCase();
  const appearance: 'light' | 'dark' = type === 'light' || type === 'hc-light' ? 'light' : type === 'dark' || type === 'hc-black' ? 'dark' : lum(canvas) < 0.179 ? 'dark' : 'light';
  // 2b8bb0c: the accent is the first candidate that stands apart (contrast >= 1.1) from the canvas and
  // the raised surface; otherwise the standard accent, then white, then black.
  const raisedCandidate = pick(canvas, 'editorWidget.background', 'dropdown.background');
  const standsApart = (first: string, second: string) => contrast(first, second) >= 1.1;
  const apartFromSurfaces = (hex: string) => standsApart(hex, canvas) && (raisedCandidate === null || standsApart(hex, raisedCandidate));
  const standardAccent = STANDARD[appearance].accent!;
  const accent = ['focusBorder', 'button.background', 'textLink.foreground', 'activityBarBadge.background', 'progressBar.background', 'badge.background']
    .map(key => pick(canvas, key)).find((hex): hex is string => hex !== null && apartFromSurfaces(hex))
    ?? [standardAccent, '#ffffff', '#000000'].find(apartFromSurfaces) ?? standardAccent;
  // The derived floor: the standard palette with the canvas and a muted accent, then the theme's own colours.
  let derived = updateFamily({ ...STANDARD[appearance] }, 'canvas', canvas);
  derived = updateFamily(derived, 'accent', over(`${accent}33`, canvas) ?? accent);
  const surfaceRaised = raisedCandidate ?? derived.surfaceRaised!;
  // The checked switch track is messageAction; the unchecked one (input) must stand apart from it.
  const button = pick(canvas, 'button.background');
  const action = button && standsApart(button, canvas) && standsApart(button, surfaceRaised) ? button : accent;
  const inputApart = (hex: string) => standsApart(hex, canvas) && standsApart(hex, action);
  const input = ['input.background', 'input.border'].map(key => pick(canvas, key)).find((hex): hex is string => hex !== null && inputApart(hex))
    ?? [derived.input!, derived.surfaceRaised!, STANDARD[appearance].input!, '#000000', '#ffffff', '#808080'].find(inputApart) ?? '#808080';
  const readable = (surface: string, fallback: string, ...keys: string[]) => {
    const specified = pick(surface, ...keys);
    if (specified && contrast(specified, surface) >= 4.5) return specified;
    if (contrast(fallback, surface) >= 4.5) return fallback;
    return lum(surface) < 0.179 ? '#ffffff' : '#000000';
  };
  const sidebar = pick(canvas, 'sideBar.background', 'activityBar.background') ?? derived.sidebar!;
  const terminal = pick(canvas, 'terminal.background', 'panel.background') ?? derived.terminalBackground!;
  const roles: Record<string, string> = { ...derived, canvas, chrome: canvas, toolbar: canvas,
    text: readable(canvas, derived.text!, 'editor.foreground', 'foreground'), textMuted: readable(canvas, derived.textMuted!, 'descriptionForeground', 'disabledForeground'),
    surface: pick(canvas, 'editorWidget.background') ?? derived.surface!, surfaceRaised,
    surfaceOverlay: pick(canvas, 'menu.background', 'quickInput.background', 'dropdown.background') ?? derived.surfaceOverlay!,
    border: pick(canvas, 'panel.border', 'editorGroup.border', 'contrastBorder') ?? derived.border!, input,
    placeholder: readable(surfaceRaised, derived.placeholder!, 'input.placeholderForeground'), error: readable(canvas, derived.error!, 'editorError.foreground', 'errorForeground'),
    warning: readable(canvas, derived.warning!, 'editorWarning.foreground'), accentSurface: pick(canvas, 'list.activeSelectionBackground', 'list.hoverBackground') ?? derived.accentSurface!,
    codeBackground: pick(canvas, 'textCodeBlock.background') ?? derived.codeBackground!, sidebar, sidebarForeground: readable(sidebar, derived.sidebarForeground!, 'sideBar.foreground'),
    sidebarBorder: pick(sidebar, 'sideBar.border') ?? derived.sidebarBorder!, sidebarRowHover: pick(sidebar, 'list.hoverBackground') ?? derived.sidebarRowHover!,
    sidebarRowActive: pick(sidebar, 'list.inactiveSelectionBackground', 'list.hoverBackground') ?? derived.sidebarRowActive!,
    sidebarRowSelected: pick(sidebar, 'list.activeSelectionBackground') ?? derived.sidebarRowSelected!, terminalBackground: terminal,
    terminalForeground: readable(terminal, derived.terminalForeground!, 'terminal.foreground'),
    terminalCursor: pick(terminal, 'terminalCursor.foreground', 'editorCursor.foreground') ?? derived.terminalCursor!,
    terminalSelection: pick(terminal, 'terminal.selectionBackground', 'editor.selectionBackground') ?? derived.terminalSelection!,
    terminalScrollbar: pick(terminal, 'scrollbarSlider.background') ?? derived.terminalScrollbar! };
  Object.assign(roles, { accent, focus: accent, messageAction: action, messageActionForeground: readable(action, derived.messageActionForeground!, 'button.foreground'),
    accentForeground: readable(accent, derived.accentForeground!, 'button.foreground') });
  const name = [file.displayName, file.name].map(candidate => typeof candidate === 'string' ? humanizeThemeName(candidate) : '').find(Boolean)?.slice(0, 48) || 'VS Code theme';
  const id = themeIdFromName(name);
  const reserved = ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', 'system', 'light', 'dark', 'default'].includes(id);
  return parseThemeFile(JSON.stringify({ version: 1, ...(reserved ? { id: `${id}-vscode` } : {}), name, appearance, colors: roles }), taken);
}
/** A pasted or picked file: a T3 Code theme (version 1) or a VS Code colour theme. */
export function importThemeText(text: string, taken: string[]): CustomTheme {
  let value: unknown;
  try { value = parseJsonc(text); } catch { throw new ClientError('Theme files must contain a JSON object.'); }
  return isVsCodeTheme(value) ? convertVsCodeTheme(value, taken) : parseThemeFile(JSON.stringify(value), taken);
}

// ── Open VSX ──────────────────────────────────────────────────────────────────
type Bridge = { call(request: Obj): Promise<Obj> };
async function fetchText(bridge: Bridge, url: string): Promise<string> {
  const reply = await bridge.call({ op: 'fetchText', url });
  if (reply.ok === false || typeof reply.text !== 'string') throw new ClientError(str(reply.message, 'Open VSX is unavailable right now.'));
  return reply.text;
}
const contributions = (manifest: Obj) => (Array.isArray(obj(manifest.contributes).themes) ? (obj(manifest.contributes).themes as unknown[]) : []).map(obj).filter(entry => typeof entry.path === 'string');
function licenseOk(manifest: Obj, license: string) { const declared = str(manifest.license); return SUPPORTED_LICENSES.has(license) && (!declared || declared === license); }
// A file inside the published package (the .vsix's extension/ folder); Open VSX's /file/
// route serves only the manifest and listed resources, not the contributed theme files.
const fileUrl = (ext: Pick<Extension, 'namespace' | 'name' | 'version'>, path: string) =>
  `https://open-vsx.org/vscode/unpkg/${encodeURIComponent(ext.namespace)}/${encodeURIComponent(ext.name)}/${encodeURIComponent(ext.version)}/extension/${path.replace(/^\.\//, '').split('/').map(encodeURIComponent).join('/')}`;

export async function searchOpenVsx(client: T3Client, native: Native, query: string, sort: string): Promise<Extension[]> {
  const bridge = client.restAccess(native) as unknown as Bridge;
  const params = `query=${encodeURIComponent(query)}&category=Themes&sortBy=${encodeURIComponent(sort)}&sortOrder=desc&size=16`;
  let value: Obj;
  try { value = obj(JSON.parse(await fetchText(bridge, `https://open-vsx.org/api/-/search?${params}`))); }
  catch (error) { throw error instanceof ClientError ? error : new ClientError('Open VSX returned an unreadable response.'); }
  if (!Array.isArray(value.extensions)) throw new ClientError('Open VSX returned an unreadable search response.');
  const installed = new Set(customThemes(client).map(theme => theme.id.replace(/--.*$/, '')));
  const collections = new Set(customThemes(client).map(theme => theme.collection?.id ?? ''));
  const details = await Promise.all((value.extensions as unknown[]).slice(0, 16).map(async candidate => {
    const entry = obj(candidate), namespace = str(entry.namespace), name = str(entry.name);
    if (!namespace || !name) return null;
    try {
      const detail = obj(JSON.parse(await fetchText(bridge, `https://open-vsx.org/api/${encodeURIComponent(namespace)}/${encodeURIComponent(name)}`)));
      const version = str(detail.version), license = str(detail.license);
      const manifest = obj(parseJsonc(await fetchText(bridge, fileUrl({ namespace, name, version }, 'package.json'))));
      if (!contributions(manifest).length || !licenseOk(manifest, license)) return null;
      const id = `${namespace}.${name}`;
      const repository = str(detail.repository);
      return { id, namespace, name, version, displayName: str(detail.displayName, name), publisher: str(obj(detail.publishedBy).loginName, namespace), description: str(detail.description),
        downloads: `${FORMAT(Number(detail.downloadCount) || 0)} downloads`, sourceUrl: /^https:\/\//.test(repository) ? repository : '',
        // ThemeExtensionIcon: the extension's own icon (Open VSX files.icon), else the palette glyph.
        iconUrl: /^https:\/\/open-vsx\.org\//.test(str(obj(detail.files).icon)) ? str(obj(detail.files).icon) : '',
        source: /gitlab\.com/.test(repository) ? 'gitlab' : /github\.com/.test(repository) ? 'github' : 'link', installed: collections.has(openVsxCollection(id)) || installed.has(themeIdFromName(str(detail.displayName, name))), installing: false, action: 'Install' } as Extension;
    } catch { return null; }
  }));
  return details.flatMap(entry => entry ? [{ ...entry, action: entry.installed ? 'Update' : 'Install' }] : []).slice(0, 8);
}
/** importOpenVsxThemeExtension: each contributed theme file (and its includes), converted. */
export async function installOpenVsx(client: T3Client, native: Native, ext: Extension): Promise<CustomTheme[]> {
  const bridge = client.restAccess(native) as unknown as Bridge;
  const manifest = obj(parseJsonc(await fetchText(bridge, fileUrl(ext, 'package.json'))));
  const themes = contributions(manifest);
  if (!themes.length) throw new ClientError('That extension does not contain color themes.');
  if (themes.length > MAX_THEMES_PER_EXTENSION) throw new ClientError('That extension contains too many color themes to import safely.');
  // An update keeps the collection's ids (the reference's openVsxThemeId is stable per file).
  const collection = { id: openVsxCollection(ext.id), label: ext.displayName.slice(0, 48) };
  const taken = ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', ...customThemes(client).filter(theme => theme.collection?.id !== collection.id).map(theme => theme.id)];
  const out: CustomTheme[] = [];
  for (const contribution of themes) {
    const load = async (path: string, depth: number): Promise<Obj> => {
      if (depth > 8) throw new ClientError('That theme includes too many files.');
      const file = obj(parseJsonc(await fetchText(bridge, fileUrl(ext, path))));
      if (typeof file.include !== 'string') return file;
      const base = path.split('/').slice(0, -1).join('/');
      const parent = await load([base, file.include].filter(Boolean).join('/').replace(/\/\.\//g, '/'), depth + 1);
      return { ...parent, ...file, colors: { ...obj(parent.colors), ...obj(file.colors) } };
    };
    const file = await load(str(contribution.path), 0);
    const theme = convertVsCodeTheme({ ...file, name: str(contribution.label, str(file.name, ext.displayName)), type: str(file.type, str(contribution.uiTheme) === 'vs' ? 'light' : 'dark') }, taken);
    taken.push(theme.id);
    out.push(theme);
  }
  // The extension's themes are one collection (CustomThemeCollectionCard; one theme draws as a plain card).
  return out.map(theme => ({ ...theme, collection }));
}
const openVsxCollection = (extensionId: string) => `open-vsx:${extensionId.toLowerCase()}`;

/** ThemeImportDialog onImportedMany: added (or a collection replaced), not made active; "N themes added". */
export function addManyThemes(client: T3Client, themes: CustomTheme[], collectionId = ''): void {
  const local = client.local as unknown as { customThemes?: CustomTheme[] };
  const kept = customThemes(client).filter(theme => !collectionId || theme.collection?.id !== collectionId);
  const updated = kept.length < customThemes(client).length;
  if (kept.length + themes.length > 100) throw new ClientError('Remove a theme before adding another.');
  local.customThemes = [...kept, ...themes];
  const verb = updated ? 'updated' : 'added';
  pushToast(client, { kind: 'success', title: themes.length === 1 ? `${themes[0]!.label} ${verb}` : `${themes.length} themes ${verb}`, description: themes.map(theme => theme.label).join(', ') });
}

const customThemes = (client: T3Client): CustomTheme[] => (client.local as unknown as { customThemes?: CustomTheme[] }).customThemes || [];
type Prefs = { theme: string; themeLight: string; themeDark: string };
/** Installs themes and makes the first one active for its appearance; the reference's success toast. */
export function addThemes(client: T3Client, themes: CustomTheme[]): void {
  const local = client.local as unknown as { customThemes?: CustomTheme[]; clientSettings?: Prefs };
  const current = customThemes(client);
  if (current.length + themes.length > 100) throw new ClientError('Remove a theme before adding another.');
  local.customThemes = [...current, ...themes];
  const first = themes[0]!;
  const both = first.light && first.dark;
  if (local.clientSettings) local.clientSettings = { ...local.clientSettings, ...(both || first.appearance === 'light' ? { themeLight: first.id } : {}),
    ...(both || first.appearance === 'dark' ? { themeDark: first.id } : {}), ...(both ? { theme: first.id } : {}) };
  pushToast(client, { kind: 'success', title: `${first.label} added`, description: both ? 'It’s now active.' : `It’s now your ${first.appearance} theme.` });
}

export type ImportView = { query: string; sort: string; sortLabel: string; sorts: { id: string; value: string; label: string; detail: string; icon: string; selected: boolean; disabled: boolean }[];
  searching: boolean; error: string; searched: boolean; pendingUpdate: string; pendingUpdateName: string; results: Extension[]; status: string; fileName: string; json: string; popular: string[] };
export function importView(client: T3Client): ImportView {
  const state = searchState(client);
  return { query: state.query, sort: state.sort, sortLabel: SORT_OPTIONS.find(([value]) => value === state.sort)?.[1] ?? 'Most downloaded',
    sorts: SORT_OPTIONS.map(([value, label]) => ({ id: value, value, label, detail: '', icon: '', selected: value === state.sort, disabled: false })),
    searching: state.searching, error: state.error, searched: state.results !== null,
    pendingUpdate: state.pendingUpdate, pendingUpdateName: state.results?.find(entry => entry.id === state.pendingUpdate)?.displayName ?? '',
    results: (state.results ?? []).map(ext => ({ ...ext, installing: state.installing === ext.id, action: state.installing === ext.id ? `${ext.installed ? 'Updating' : 'Installing'}...` : ext.action })),
    status: state.searching ? 'Searching themes...' : state.results ? `${state.results.length} supported ${state.results.length === 1 ? 'theme' : 'themes'} found.` : '',
    fileName: state.fileName, json: state.json, popular: SUGGESTED_SEARCHES };
}

/** settings-core theme-add rows: search, sort, install, choose (files), json, add. */
export async function themeImportCommand(client: T3Client, native: Native | null, part: string, value: string): Promise<string> {
  const state = searchState(client);
  if (part === 'reset') { resetImport(client); return ''; }
  if (part === 'json') { state.json = value.slice(0, 1_048_576); return ''; }
  if (part === 'add') {
    const text = value || state.json;
    if (!text.trim()) throw new ClientError('Paste a theme file first.');
    const theme = importThemeText(text, ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', ...customThemes(client).map(entry => entry.id)]);
    addThemes(client, [theme]);
    resetImport(client);
    return '';
  }
  if (!native?.available) throw new ClientError('Adding themes from Open VSX or files needs the macOS app.');
  if (part === 'choose') {
    const reply = await client.restAccess(native).call({ op: 'openText', types: ['json'], multiple: true });
    const files = Array.isArray(reply.files) ? (reply.files as unknown[]).map(obj) : [];
    // A cancelled picker leaves the dialog open (`toasted:` keeps its inline error empty).
    if (!files.length) throw new ClientError('toasted:');
    const taken = ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', ...customThemes(client).map(entry => entry.id)];
    const themes = files.map(file => { const theme = importThemeText(str(file.text), taken); taken.push(theme.id); return theme; });
    state.fileName = files.map(file => str(file.name)).join(', ');
    addManyThemes(client, themes);
    resetImport(client);
    return '';
  }
  if (part === 'search' || part === 'sort') {
    if (part === 'sort') { if (!SORT_OPTIONS.some(([id]) => id === value)) throw new ClientError('Choose a sort order.'); state.sort = value; }
    else state.query = value.slice(0, 128);
    if (!state.query.trim()) { state.results = null; state.error = ''; return ''; }
    state.searching = true; state.error = '';
    try { state.results = await searchOpenVsx(client, native, state.query.trim(), state.sort); }
    catch (error) { state.error = error instanceof Error ? error.message : 'Open VSX search is unavailable right now.'; state.results = null; }
    finally { state.searching = false; }
    return '';
  }
  if (part === 'update-cancel') { state.pendingUpdate = ''; throw new ClientError('toasted:'); }
  if (part === 'install' || part === 'install-confirm') {
    const ext = state.results?.find(entry => entry.id === value);
    if (!ext) throw new ClientError('That theme is no longer in the results.');
    // ThemeSearchSection: an installed collection asks before it is replaced.
    state.pendingUpdate = '';
    if (part === 'install' && customThemes(client).some(theme => theme.collection?.id === openVsxCollection(ext.id))) { state.pendingUpdate = ext.id; throw new ClientError('toasted:'); }
    state.installing = ext.id;
    try { addManyThemes(client, await installOpenVsx(client, native, ext), openVsxCollection(ext.id)); resetImport(client); }
    catch (error) {
      // The search section shows the failure; the dialog stays open.
      state.installing = ''; state.error = error instanceof Error ? error.message : 'That theme could not be added.';
      throw new ClientError('toasted:');
    }
    return '';
  }
  throw new ClientError('Unsupported theme import action.');
}
