// Lane settings-b: Settings → Project → Project icon (ProjectSettingsPanel.tsx's
// icon row, ProjectIconPickerDialog.tsx, ProjectFaviconPickerDialog.tsx,
// projectIconOptions.ts, projectIconColors.ts, projectIdentity.ts,
// ProjectMonogram.tsx, ProjectFavicon.tsx). An override is a Lucide icon, an
// emoji or a monogram; a favicon is a workspace image. Both are group-shared
// fields, so a change fans out to every member project (updateAllMembers).
import { arr, obj, str, initialShell, applyShell, type Obj } from './domain';
import { ClientError, type Native, type Files } from './protocol';
import { LUCIDE_PATHS } from './settings-b-lucide';
import { fileIconToken } from './timeline-files';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { faviconPickLabel } from './desktop-shell-favicon';
import { letGo } from './let-go';

/** PROJECT_ICON_COLORS: swatch bg-*-500; icon ink text-*-600 / dark:text-*-400. */
export const ICON_COLORS: { value: string; label: string; swatch: string; ink: string; surface: string; inkLight: string; inkDark: string }[] = ([
  ['gray', 'Gray', '#6a7282', '#4a5565', '#99a1af'], ['red', 'Red', '#fb2c36', '#e7000b', '#ff6467'],
  ['orange', 'Orange', '#ff6900', '#f54900', '#ff8904'], ['amber', 'Amber', '#fe9a00', '#e17100', '#ffb900'],
  ['yellow', 'Yellow', '#f0b100', '#d08700', '#fdc700'], ['lime', 'Lime', '#7ccf00', '#5ea500', '#9ae600'],
  ['green', 'Green', '#00c950', '#00a63e', '#05df72'], ['emerald', 'Emerald', '#00bc7d', '#009966', '#00d492'],
  ['teal', 'Teal', '#00bba7', '#009689', '#00d5be'], ['cyan', 'Cyan', '#00b8db', '#0092b8', '#00d3f2'],
  ['sky', 'Sky', '#00a6f4', '#0084d1', '#00bcff'], ['blue', 'Blue', '#2b7fff', '#155dfc', '#51a2ff'],
  ['indigo', 'Indigo', '#615fff', '#4f39f6', '#7c86ff'], ['violet', 'Violet', '#8e51ff', '#7f22fe', '#a684ff'],
  ['purple', 'Purple', '#ad46ff', '#9810fa', '#c27aff'], ['fuchsia', 'Fuchsia', '#e12afb', '#c800de', '#ed6aff'],
  ['pink', 'Pink', '#f6339a', '#e60076', '#fb64b6'], ['rose', 'Rose', '#ff2056', '#ec003f', '#ff637e'],
] as const).map(([value, label, swatch, light, dark]) => ({ value, label, swatch, ink: `light-dark(${light}, ${dark})`, surface: `light-dark(${light}24, ${dark}24)`, inkLight: light, inkDark: dark }));
const COLOR_IDS = ICON_COLORS.map(color => color.value);
export const inkOf = (color: string) => ICON_COLORS.find(entry => entry.value === color)?.ink ?? ICON_COLORS[11]!.ink;
/** The monogram tile: color-mix(currentColor 14%, transparent). */
export const surfaceOf = (color: string) => ICON_COLORS.find(entry => entry.value === color)?.surface ?? ICON_COLORS[11]!.surface;

export const POPULAR_ICONS = ['folder-code', 'code-2', 'terminal', 'globe-2', 'server', 'database', 'bot', 'sparkles', 'smartphone', 'monitor',
  'cloud-cog', 'package', 'book-open', 'flask-conical', 'shield-check', 'rocket', 'gamepad-2', 'music', 'image', 'shopping-bag', 'git-branch',
  'workflow', 'wrench', 'layers-3'];
export const PROJECT_EMOJIS: { emoji: string; label: string }[] = [['💻', 'Computer'], ['🛠️', 'Tools'], ['🚀', 'Rocket'], ['🤖', 'Robot'],
  ['✨', 'Sparkles'], ['⚡', 'Lightning'], ['🌐', 'Web'], ['📱', 'Mobile'], ['🖥️', 'Desktop'], ['⌨️', 'Keyboard'], ['⚙️', 'Gear'],
  ['🗄️', 'Database'], ['☁️', 'Cloud'], ['📦', 'Package'], ['📚', 'Books'], ['🧪', 'Test tube'], ['🔒', 'Lock'], ['🎮', 'Game'],
  ['🎵', 'Music'], ['🎬', 'Movie'], ['🖼️', 'Picture'], ['🛍️', 'Shopping'], ['🔥', 'Fire'], ['💡', 'Idea'], ['🧩', 'Puzzle'],
  ['📊', 'Chart'], ['🧠', 'Brain'], ['🦄', 'Unicorn'], ['🐙', 'Octopus'], ['🌱', 'Seedling']].map(([emoji, label]) => ({ emoji: emoji!, label: label! }));

export const iconLabel = (name: string) => name.split('-').map(part => part.charAt(0).toUpperCase() + part.slice(1)).join(' ');
const ICON_NAMES = Object.keys(LUCIDE_PATHS).sort();
/** filterProjectIconNames: the popular set, else every Lucide name containing the query (60 at most). */
export function filterIconNames(query: string): string[] {
  const normalized = query.trim().toLowerCase().replace(/\s+/g, '-');
  if (!normalized) return POPULAR_ICONS;
  return ICON_NAMES.filter(name => name.includes(normalized)).slice(0, 60);
}
export const iconPath = (name: string) => LUCIDE_PATHS[name] ?? LUCIDE_PATHS['folder-code']!;

// ── firstEmoji (Intl.Segmenter's first grapheme, then the pictographic test) ──
const PICTOGRAPHIC = /\p{Extended_Pictographic}/u;
function graphemes(value: string): string[] {
  const Segmenter = (Intl as unknown as { Segmenter?: new (locale: undefined, options: { granularity: string }) => { segment(text: string): Iterable<{ segment: string }> } }).Segmenter;
  if (Segmenter) return Array.from(new Segmenter(undefined, { granularity: 'grapheme' }).segment(value), part => part.segment);
  // Without Intl.Segmenter: join modifiers, variation selectors, keycaps, ZWJ sequences and flag pairs.
  const out: string[] = [];
  const points = Array.from(value);
  for (let index = 0; index < points.length; index++) {
    let cluster = points[index]!;
    const regional = (point: string) => /\p{Regional_Indicator}/u.test(point);
    if (regional(cluster) && points[index + 1] && regional(points[index + 1]!)) cluster += points[++index]!;
    while (index + 1 < points.length && /[\u{FE0E}\u{FE0F}\u{20E3}\u{1F3FB}-\u{1F3FF}\u{E0020}-\u{E007F}\p{M}]/u.test(points[index + 1]!)) cluster += points[++index]!;
    while (points[index + 1] === '‍' && points[index + 2]) {
      cluster += points[++index]! + points[++index]!;
      while (index + 1 < points.length && /[\u{FE0F}\u{1F3FB}-\u{1F3FF}]/u.test(points[index + 1]!)) cluster += points[++index]!;
    }
    out.push(cluster);
  }
  return out;
}
export function firstEmoji(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return '';
  const segment = graphemes(trimmed)[0] ?? '';
  const isFlag = /^\p{Regional_Indicator}{2}$/u.test(segment), isKeycap = /^[#*0-9]️?⃣$/u.test(segment);
  return segment && (PICTOGRAPHIC.test(segment) || isFlag || isKeycap) ? segment : '';
}

// ── Monogram (ProjectMonogramText; graphemes are counted by the server) ──
const MONOGRAM = /^[\p{L}\p{N}][\p{L}\p{N}\p{M}‌‍]*$/u;
export function monogramOf(letters: string): { text: string; valid: boolean } {
  let text = letters;
  try { text = letters.normalize('NFKC'); } catch { /* engines without normalization keep the raw text */ }
  text = text.trim().toUpperCase();
  return { text, valid: text.length > 0 && text.length <= 32 && MONOGRAM.test(text) };
}
/** The monogram's glyph run: one grapheme is 6 of 16 units wide, two are 12 (textLength). */
export const monogramWidth = (text: string) => graphemes(text).length === 1 ? 6 : 12;

// ── deriveProjectIdentity ──
export function projectIdentity(name: string): { monogram: string; color: string } {
  let normalized = name;
  try { normalized = name.normalize('NFKC'); } catch { /* keep */ }
  normalized = normalized.trim();
  const words = normalized.match(/[\p{L}\p{N}]+/gu) ?? [];
  let monogram = 'PR';
  if (words[0]) {
    const glyphs = Array.from(words[0]), first = glyphs[0] ?? 'P';
    const second = glyphs.slice(1).find(glyph => /\p{N}/u.test(glyph))
      ?? (words.length > 1 ? Array.from(words[words.length - 1] ?? '')[0] : glyphs[glyphs.length - 1]) ?? first;
    monogram = Array.from(`${first}${second}`.toUpperCase()).slice(0, 2).join('');
  }
  let index = 0;
  for (const glyph of normalized.toLocaleLowerCase('en-US') || 'project') index = (index * 31 + (glyph.codePointAt(0) ?? 0)) % COLOR_IDS.length;
  return { monogram, color: COLOR_IDS[index] ?? 'blue' };
}

/** ProjectIconOverride decoded from the wire: a lucide icon carrying monogramText is a monogram. */
export function decodeIcon(value: unknown): { kind: string; name: string; color: string; text: string; emoji: string } {
  const icon = obj(value), kind = str(icon.kind);
  const color = COLOR_IDS.includes(str(icon.color)) ? str(icon.color) : '';
  if (kind === 'lucide') {
    const text = str(icon.monogramText) || str(icon.monogram);
    return text ? { kind: 'monogram', name: '', color, text, emoji: '' } : { kind: 'lucide', name: str(icon.name), color, text: '', emoji: '' };
  }
  if (kind === 'monogram') return { kind, name: '', color, text: str(icon.text), emoji: '' };
  if (kind === 'emoji') return { kind, name: '', color: '', text: '', emoji: str(icon.emoji) };
  return { kind: '', name: '', color: '', text: '', emoji: '' };
}
/** ProjectIconOverride encoded for the wire: older peers only know lucide and emoji. */
export function encodeIcon(input: { kind: string; name: string; color: string; text: string; emoji: string }): Obj {
  if (input.kind === 'monogram') return { kind: 'lucide', name: 'folder-code', color: input.color, monogramText: input.text };
  if (input.kind === 'emoji') return { kind: 'emoji', emoji: input.emoji };
  return { kind: 'lucide', name: input.name, color: input.color };
}

// ── The Project page's icon fields (ProjectFavicon for the representative) ──
export type IconFields = { iconKind: string; iconName: string; iconD: string; iconInk: string; iconSurface: string; iconText: string; iconTextWidth: number;
  iconEmoji: string; iconPickColor: string; iconLetters: string; faviconSrc: string; iconScope: string; iconCwd: string; iconPickExternal: string };
export const blankIconFields: IconFields = { iconKind: '', iconName: '', iconD: '', iconInk: '#00000000', iconSurface: '#00000000', iconText: '', iconTextWidth: 12,
  iconEmoji: '', iconPickColor: 'blue', iconLetters: '', faviconSrc: '', iconScope: '', iconCwd: '', iconPickExternal: '' };

// Data sources have no clock: caches are keyed by the connection generation and bounded.
const faviconCache = new Map<string, string>();
/** The signed asset URL against the server origin (no WHATWG URL in every engine). */
export function assetUrl(origin: string, relative: string): string {
  if (!relative) return '';
  if (/^https?:\/\//i.test(relative)) return relative;
  return `${origin.replace(/\/+$/, '')}${relative.startsWith('/') ? '' : '/'}${relative}`;
}
/** projectFaviconUrlAtom: a signed asset URL, or '' when the server reports the fallback marker. */
async function faviconSource(client: T3Client, native: Native | null | undefined, project: Obj): Promise<string> {
  const cwd = str(project.workspaceRoot);
  if (!native?.available || !cwd || client.connection !== 'connected') return '';
  const key = JSON.stringify([client.generation, client.environmentId, cwd, str(project.faviconPath)]), cached = faviconCache.get(key);
  if (cached !== undefined) return cached;
  try {
    const result = await client.rpc(native, 'assets.createUrl', { resource: { _tag: 'project-favicon', cwd, ...(str(project.faviconPath) ? { path: str(project.faviconPath) } : {}) } });
    const url = assetUrl(client.origin, str(obj(result).relativeUrl));
    const missing = !url || url.split(/[?#]/)[0]!.split('/').pop() === 'project-favicon-missing';
    faviconCache.set(key, missing ? '' : url);
    if (faviconCache.size > 64) faviconCache.delete(faviconCache.keys().next().value!);
    return missing ? '' : url;
  } catch { return ''; }
}

export async function iconFields(client: T3Client, native: Native | null | undefined, name: string, members: Obj[]): Promise<IconFields> {
  const representative = members[0];
  if (!representative) return blankIconFields;
  const identity = projectIdentity(name), icon = decodeIcon(representative.projectIcon);
  const color = icon.kind === 'lucide' || icon.kind === 'monogram' ? icon.color || identity.color : identity.color;
  const shown = icon.kind === 'monogram' ? icon.text : identity.monogram;
  return { iconKind: icon.kind, iconName: icon.name, iconD: icon.kind === 'lucide' ? iconPath(icon.name) : '', iconInk: inkOf(icon.kind ? color : identity.color),
    iconSurface: surfaceOf(icon.kind ? color : identity.color), iconText: shown, iconTextWidth: monogramWidth(shown), iconEmoji: icon.emoji,
    iconPickColor: color, iconLetters: icon.kind === 'monogram' ? icon.text : identity.monogram,
    faviconSrc: icon.kind ? '' : await faviconSource(client, native, representative),
    iconScope: members.map(member => str(member.id)).join(','), iconCwd: str(representative.workspaceRoot),
    iconPickExternal: faviconPickLabel(client.origin, members) }; // desktop-shell-favicon.ts
}

// ── The picker resource (icon grid, emoji, monogram validity, image files) ──
const fileCache = new Map<string, { entries: Obj[]; error: string }>();
export async function iconPicker(client: T3Client, native: Native | null | undefined, editor: string, query: string, letters: string, custom: string, cwd: string, group: string, revision = 0) {
  const monogram = monogramOf(letters);
  const icons = editor === 'project-icon' ? filterIconNames(query).map(name => ({ name, label: iconLabel(name), d: iconPath(name) })) : [];
  let files: { index: number; path: string; name: string; token: string }[] = [], fileEmpty = '';
  if (editor === 'project-favicon') {
    const trimmed = query.trim(), key = `${client.generation}:${revision}:${cwd}:${trimmed}`;
    let cached = fileCache.get(key);
    if (!cached) {
      try {
        if (!native?.available || !cwd) throw new ClientError('Connect to this project\'s environment to choose a file.');
        const result = obj(await client.rpc(native, 'projects.searchEntries', { cwd, query: trimmed, limit: 200, imageOnly: true }));
        cached = { entries: arr(result.entries), error: '' };
      } catch (error) { if (letGo(error)) throw error; cached = { entries: [], error: error instanceof Error ? error.message : 'Could not search image files.' }; }
      fileCache.set(key, cached);
      if (fileCache.size > 40) fileCache.delete(fileCache.keys().next().value!);
    }
    files = cached.entries.filter(entry => entry.kind === 'file' && str(entry.path)).map((entry, index) => {
      const path = str(entry.path);
      return { index, path, name: path.slice(path.lastIndexOf('/') + 1), token: fileIconToken(path) };
    });
    fileEmpty = cached.error || (trimmed ? 'No matching image files.' : 'No image files found.');
  }
  // The editor's starting point: the representative's override, else Folder Code in the automatic color.
  const project = client.shell.projects.find(candidate => str(candidate.workspaceRoot) === cwd);
  const current = decodeIcon(project?.projectIcon), identity = projectIdentity(group);
  const seeds = editor === 'project-icon' && project ? [{ key: `seed:${str(project.id)}`, mode: current.kind === 'emoji' || current.kind === 'monogram' ? current.kind : 'lucide',
    iconName: current.kind === 'lucide' && LUCIDE_PATHS[current.name] ? current.name : 'folder-code',
    color: (current.kind === 'lucide' || current.kind === 'monogram') && current.color ? current.color : identity.color,
    emoji: current.kind === 'emoji' ? current.emoji : '💻', letters: current.kind === 'monogram' ? current.text : identity.monogram }] : [];
  return { seeds, icons, iconsEmpty: editor === 'project-icon' && icons.length === 0, colors: ICON_COLORS, emojis: PROJECT_EMOJIS,
    files, fileGroup: group, fileEmpty, monogram: monogram.text, monogramValid: monogram.valid,
    monogramWidth: monogramWidth(monogram.valid ? monogram.text : letters), pasted: firstEmoji(custom) };
}

// ── Writes (setProjectIcon → updateAllMembers) ──
export const ICON_OPS = ['project-icon-set', 'project-favicon-set'];
export async function runIconOp(client: T3Client, native: Native, storage: Files | undefined, op: string, id: string, value: string): Promise<string> {
  if (!storage) throw new ClientError('Project icons need the local outbox.');
  const input = Object.fromEntries(new URLSearchParams(value));
  const ids = id.split(',').map(entry => entry.trim()).filter(Boolean);
  if (!ids.length) throw new ClientError('Choose a project to change its icon.');
  let change: Obj;
  if (op === 'project-favicon-set') {
    const path = str(input.path).trim();
    if (!path || path.length > 1024 || !/\.(?:avif|gif|ico|jpe?g|png|svg|webp)$/i.test(path)) throw new ClientError('Choose an image file.');
    change = { faviconPath: path, projectIcon: null };
  } else {
    const kind = str(input.kind);
    if (kind === 'lucide') {
      if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(str(input.name)) || !LUCIDE_PATHS[str(input.name)] || !COLOR_IDS.includes(str(input.color))) throw new ClientError('Choose an icon and a color.');
      change = { faviconPath: null, projectIcon: encodeIcon({ kind, name: str(input.name), color: str(input.color), text: '', emoji: '' }) };
    } else if (kind === 'emoji') {
      const emoji = str(input.emoji).trim();
      if (!emoji || emoji.length > 32) throw new ClientError('Choose an emoji.');
      change = { faviconPath: null, projectIcon: encodeIcon({ kind, name: '', color: '', text: '', emoji }) };
    } else if (kind === 'monogram') {
      const monogram = monogramOf(str(input.text));
      if (!monogram.valid || !COLOR_IDS.includes(str(input.color))) throw new ClientError('One or two letters or numbers.');
      change = { faviconPath: null, projectIcon: encodeIcon({ kind, name: '', color: str(input.color), text: monogram.text, emoji: '' }) };
    } else throw new ClientError('Choose an icon, emoji, or monogram.');
  }
  const access = client.restAccess(native);
  try {
    const shell = applyShell(initialShell(), await access.http('/api/orchestration/shell'));
    for (const projectId of ids) {
      if (!shell.projects.some(project => str(project.id) === projectId)) throw new ClientError('Project group membership changed. Reopen its settings.');
      const [commandId] = await access.ids(1);
      await access.write(storage, { method: 'projects.mutate', description: 'Update project icon', threadId: '', text: '', uncertain: false,
        payload: { type: 'project.update', commandId, projectId, ...change } });
    }
    client.shell = applyShell(initialShell(), await access.http('/api/orchestration/shell'));
    faviconCache.clear();
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Failed to update project icon', description: error instanceof Error ? error.message : 'An error occurred.', stacked: true });
    throw error;
  }
  return '';
}
