// Go to file (⌘P) and Search project contents (⇧⌘F): reference files/ProjectFilePicker.tsx,
// ProjectFilePicker.logic.ts and search/ProjectContentSearchDialog.tsx, backed by the
// server's projects.searchEntries and projects.searchContents over the active workspace.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { closedView, flatten, row, type PalettePart, type PaletteRow, type PaletteView } from './palette';
import { highlight } from './timeline-highlight';
import { letGo } from './let-go';

export const FILE_RESULT_LIMIT = 200;
export const CONTENT_RESULT_LIMIT = 500;

/** useActiveProjectTarget: the open thread's worktree, else its project; a draft uses its project. */
export function activeTarget(client: T3Client): { cwd: string; projectName: string; threadId: string } | null {
  const thread = client.threadId ? client.shell.threads.find(candidate => candidate.id === client.threadId) : undefined;
  const projectId = str(thread?.projectId, client.projectId);
  const project = client.shell.projects.find(candidate => candidate.id === projectId);
  if (!project) return null;
  const cwd = str(thread?.worktreePath) || str(project.workspaceRoot);
  return cwd ? { cwd, projectName: str(project.title, 'Project'), threadId: client.threadId } : null;
}

/** First ordered subsequence of the query inside the value, as highlight indices (null when absent). */
export function matchIndices(value: string, query: string): number[] | null {
  if (!query) return [];
  const lower = value.toLowerCase(), indices: number[] = [];
  let at = 0;
  for (let index = 0; index < lower.length; index++) {
    if (lower[index] !== query[at]) continue;
    indices.push(index);
    if (++at === query.length) return indices;
  }
  return null;
}
/** HighlightedFuzzyText: matched letters strong, the rest muted; plain when nothing is searched. */
export function fuzzyParts(value: string, indices: number[]): PalettePart[] {
  const parts: PalettePart[] = [];
  let start = 0;
  for (const index of indices) {
    if (start < index) parts.push({ id: `t${start}`, text: value.slice(start, index), hit: false, cls: '' });
    parts.push({ id: `h${index}`, text: value[index]!, hit: true, cls: '' });
    start = index + 1;
  }
  if (start < value.length) parts.push({ id: `t${start}`, text: value.slice(start), hit: false, cls: '' });
  return parts;
}
/** PierreEntryIcon's glyph for a path: Markdown files have their own. */
export function fileIcon(path: string): string { return /\.(md|mdx|markdown)$/i.test(path) ? 'file-md' : 'file'; }
export function normalizeFileQuery(raw: string): string {
  return raw.trim().toLowerCase().replace(/^[@./]+/, '').replace(/\s/g, '');
}
export function fileRows(entries: Obj[], rawQuery: string): PaletteRow[] {
  const query = normalizeFileQuery(rawQuery), searched = /\S/.test(rawQuery);
  const files = entries.filter(entry => str(entry.kind) === 'file').slice(0, FILE_RESULT_LIMIT);
  return files.map(entry => {
    const path = str(entry.path), name = path.slice(path.lastIndexOf('/') + 1);
    return row({ key: `file:${path}`, op: 'open-file', arg: path, icon: fileIcon(path), title: name, description: path,
      titleParts: searched ? fuzzyParts(name, matchIndices(name, query) ?? []) : [], matchParts: searched ? fuzzyParts(path, matchIndices(path, query) ?? []) : [] });
  });
}

type Cache = { key: string; value: Obj; error: string };
const fileCache = new WeakMap<T3Client, Cache>();
const contentCache = new WeakMap<T3Client, Cache>();
async function cached(store: WeakMap<T3Client, Cache>, client: T3Client, native: Native, key: string, method: string, payload: Obj): Promise<Cache> {
  const hit = store.get(client);
  if (hit && hit.key === key) return hit;
  let entry: Cache;
  try { entry = { key, value: await client.restAccess(native).request(method, payload), error: '' }; }
  catch (failure) { if (letGo(failure)) throw failure; entry = { key, value: {}, error: failure instanceof Error ? failure.message : 'Search failed.' }; }
  store.set(client, entry);
  return entry;
}

export async function filePickerView(client: T3Client, native: Native | null | undefined, query: string): Promise<PaletteView> {
  const base: PaletteView = { ...closedView, open: true, mode: 'files', label: 'File picker', testId: 'project-file-picker', placeholder: 'Search files…', enterLabel: 'Open file',
    escapeLabel: 'Back', panel: 'tall' };
  const target = activeTarget(client);
  if (!target || !native?.available || !client.ready) return { ...base, empty: 'Open a project to search its files.', placeholder: 'Search files…', accessoryEnabled: false };
  const trimmed = query.trim().slice(0, 256);
  const result = await cached(fileCache, client, native, `${client.generation}:${target.cwd}:${trimmed}`, 'projects.searchEntries', { cwd: target.cwd, query: trimmed, limit: FILE_RESULT_LIMIT });
  const rows = flatten([{ value: 'project-files', label: target.projectName, items: fileRows(arr(result.value.entries), result.error ? '' : trimmed).map(entry => ({ row: entry, terms: [] })) }]);
  const empty = result.error ? result.error : trimmed ? 'No matching files.' : 'No files found.';
  return { ...base, rows, count: rows.length, empty: rows.length ? '' : empty };
}

/** HighlightedSearchLine: the line's syntax tokens (timeline-highlight.ts) cut at the server's match ranges. */
export function lineParts(line: string, ranges: Obj[], path = ''): PalettePart[] {
  const marks = ranges.map(range => [num(range.start), num(range.end)] as const).filter(([start, end]) => end > start).sort((a, b) => a[0] - b[0]);
  const hit = (at: number) => marks.some(([start, end]) => at >= start && at < end);
  const tokens = path ? highlight(line, path) : [{ text: line, cls: '' }];
  const parts: PalettePart[] = [];
  let offset = 0;
  for (const token of tokens.length ? tokens : [{ text: line, cls: '' }]) {
    // Split each token wherever the match state changes.
    let start = 0;
    for (let index = 1; index <= token.text.length; index++) {
      if (index < token.text.length && hit(offset + index) === hit(offset + start)) continue;
      const last = parts[parts.length - 1], text = token.text.slice(start, index), marked = hit(offset + start);
      if (last && last.hit === marked && last.cls === token.cls) last.text += text;
      else parts.push({ id: `p${offset + start}`, text, hit: marked, cls: token.cls });
      start = index;
    }
    offset += token.text.length;
  }
  return parts;
}
export function contentRows(matches: Obj[]): PaletteRow[] {
  const groups = new Map<string, Obj[]>();
  for (const match of matches) groups.set(str(match.path), [...(groups.get(str(match.path)) ?? []), match]);
  const rows: PaletteRow[] = [];
  // py-2 around the list; each file a section (32px header, 28px lines, pb-2).
  let index = 0, top = 8;
  for (const [path, list] of groups) {
    const slash = path.lastIndexOf('/');
    rows.push(row({ key: `file:${path}`, kind: 'file-header', headerGap: rows.length > 0, icon: fileIcon(path), title: slash < 0 ? path : path.slice(slash + 1), description: slash < 0 ? '' : path.slice(0, slash), badge: String(list.length), top, height: 32 }));
    top += 32;
    for (const match of list) {
      rows.push(row({ key: `line:${path}:${num(match.lineNumber)}:${index}`, index: index++, kind: 'line', op: 'open-file', arg: path, arg2: String(num(match.lineNumber)),
        line: String(num(match.lineNumber)), matchParts: lineParts(str(match.lineContent), arr(match.matchRanges), path), top, height: 28 }));
      top += 28;
    }
    top += 8;
  }
  return rows;
}
export type ContentFlags = { caseSensitive: boolean; wholeWord: boolean; useRegex: boolean };
export function parseFlags(flags: string): ContentFlags {
  const set = new Set(flags.split(/[\s,]+/));
  return { caseSensitive: set.has('case'), wholeWord: set.has('word'), useRegex: set.has('regex') };
}
export async function contentSearchView(client: T3Client, native: Native | null | undefined, query: string, flagText: string): Promise<PaletteView> {
  const flags = parseFlags(flagText);
  const base: PaletteView = { ...closedView, open: true, mode: 'content', label: 'Search project contents', testId: 'project-content-search', enterLabel: 'Open file', escapeLabel: 'Back',
    panel: 'fill', toggles: true, matchCase: flags.caseSensitive, wholeWord: flags.wholeWord, regex: flags.useRegex, inputPaddingRight: 120, autoHighlight: true };
  const target = activeTarget(client);
  if (!target || !native?.available || !client.ready) return { ...base, toggles: false, placeholder: 'Search project contents…', empty: 'Open a project to search its files.' };
  const placeholder = `Search in ${target.projectName}`;
  if (!query) return { ...base, placeholder, empty: 'Type to search across your project.' };
  const payload = { cwd: target.cwd, query: query.slice(0, 256), limit: CONTENT_RESULT_LIMIT, ...flags };
  const result = await cached(contentCache, client, native, `${client.generation}:${JSON.stringify(payload)}`, 'projects.searchContents', payload);
  const matches = arr(result.value.matches);
  const rows = contentRows(matches);
  const files = new Set(matches.map(match => str(match.path))).size;
  const invalid = !result.error && flags.useRegex && str(obj(result.value).regexFallbackError) !== '';
  const summary = result.error ? result.error : invalid ? 'Invalid regular expression'
    : `${matches.length.toLocaleString('en-US')}${result.value.truncated === true ? '+' : ''} results in ${files.toLocaleString('en-US')} files`;
  return { ...base, placeholder, label: `Search file contents in ${target.projectName}`, rows, count: matches.length, summary,
    empty: rows.length ? '' : result.error ? 'Type to search across your project.' : 'No results found.' };
}
