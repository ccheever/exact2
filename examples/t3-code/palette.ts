// The command palette (⌘K): reference CommandPalette.tsx, CommandPalette.logic.ts,
// CommandPaletteResults.tsx and ThreadCommandSubtitle.tsx. One view model for every
// page the overlay can show; Contract (palette.contract) owns the open state, the
// page, the query and the highlight. The add-project pages live in palette-add.ts,
// the file picker and content search in palette-files.ts.
import type { T3Client } from './client';
import { terminalProcessCount } from './sidebar-view';
import { arr, obj, str, type Obj } from './domain';
import { projectIdentity } from './presentation';
import { searchSettings, searchContext, breadcrumbLabel } from './settings-search';
import { themeCards } from './settings-appearance';
import { decodeClientPrefs, type ClientPrefs } from './settings-core';
import { chordWinners, type DispatchContext } from './keyboard-dispatch';
import { linkMode } from './palette-linkpr';
import { scratchRoot, isScratchProject } from './pages-home';
import { pickerProjects } from './r4-polish-palette-projects'; // r4-polish: Project order
import { openPanelPullRequestUrl, threadReferenceTarget } from './thread-reference';

export type PalettePart = { id: string; text: string; hit: boolean; cls: string };
export type PaletteRow = {
  key: string; index: number; header: string; headerGap: boolean; kind: string; op: string; arg: string; arg2: string;
  icon: string; prefix: string; title: string; bold: string; description: string; shortcut: string; timestamp: string; trailing: string;
  /** The chord that runs this row from the field (thread.jump.N on a project pick, aria grammar: `Meta+1`); '' for none. */
  jump: string;
  terminalCount: number;
  badge: string; badgeOp: string; checkbox: boolean; checked: boolean; project: string; projectInk: string; projectSurface: string; env: string;
  branch: string; provider: string; current: boolean; matchLabel: string; matchParts: PalettePart[]; titleParts: PalettePart[]; line: string; orbs: Orb[]; top: number; height: number;
};
type Orb = ReturnType<typeof themeCards>[number]['orbs'][number];
export type PaletteView = {
  open: boolean; mode: string; page: string; parent: string; back: boolean; addon: string; placeholder: string; label: string; testId: string;
  rows: PaletteRow[]; count: number; autoHighlight: boolean; empty: string; enterLabel: string; escapeLabel: string; backHint: boolean; popOnEmpty: boolean;
  enterOp: string; enterArg: string; enterArg2: string; accessory: string; accessoryKey: string; accessoryEnabled: boolean; accessoryOp: string; accessoryArg: string;
  accessoryArg2: string; footerAction: string; contextLabel: string; contextTitle: string; contextDescription: string; contextIcon: string; toggles: boolean;
  matchCase: boolean; wholeWord: boolean; regex: boolean; summary: string; panel: string; inputPaddingRight: number; loading: boolean; listHeight: number;
  /** The chords that resolve to thread.jump.N here: the command palette's field takes each (threadJumpChords). */
  jumpKeys: string[];
};

export const RECENT_THREAD_LIMIT = 12;
const blankRow: PaletteRow = { key: '', index: -1, header: '', headerGap: false, kind: 'action', op: '', arg: '', arg2: '', icon: '', prefix: '', title: '', bold: '',
  terminalCount: 0, description: '', shortcut: '', jump: '', timestamp: '', trailing: '', badge: '', badgeOp: '', checkbox: false, checked: false, project: '', projectInk: '', projectSurface: '', env: '',
  branch: '', provider: '', current: false, matchLabel: '', matchParts: [], titleParts: [], line: '', orbs: [], top: 0, height: 32 };
export function row(fields: Partial<PaletteRow>): PaletteRow { return { ...blankRow, ...fields }; }

export const closedView: PaletteView = {
  open: false, mode: 'command', page: '', parent: '', back: false, addon: 'search', placeholder: '', label: 'Command palette', testId: 'command-palette', rows: [], count: 0,
  autoHighlight: true, empty: '', enterLabel: '', escapeLabel: 'Close', backHint: false, popOnEmpty: false, enterOp: '', enterArg: '', enterArg2: '', accessory: '', accessoryKey: '',
  accessoryEnabled: false, accessoryOp: '', accessoryArg: '', accessoryArg2: '', footerAction: '', contextLabel: '', contextTitle: '', contextDescription: '', contextIcon: '',
  toggles: false, matchCase: false, wholeWord: false, regex: false, summary: '', panel: 'list', inputPaddingRight: 11, loading: false, listHeight: 0, jumpKeys: [],
};

/** One searchable entry before grouping: the reference CommandPaletteItem. */
export type Item = { row: PaletteRow; terms: string[]; recency?: number; secondary?: boolean };
export type Group = { value: string; label: string; items: Item[] };

export function normalizeSearchText(value: string): string {
  return value.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase().replace(/\s+/g, ' ').trim();
}
function rankField(field: string, query: string, tokens: string[]): number {
  const normalized = normalizeSearchText(field);
  if (!normalized || !tokens.every(token => normalized.includes(token))) return Number.NEGATIVE_INFINITY;
  return normalized === query ? 3 : normalized.startsWith(query) ? 2 : normalized.includes(query) ? 1 : 0;
}
function rankItem(item: Item, query: string, tokens: string[]): number {
  const terms = item.terms.filter(term => term.length > 0);
  for (const [index, field] of terms.entries()) {
    const rank = rankField(field, query, tokens);
    if (rank !== Number.NEGATIVE_INFINITY) return index === 0 && item.recency !== undefined ? 1000 + Number(rank === 3) : 1000 - index * 100 + rank;
  }
  return 0;
}
/** filterCommandPaletteGroups: '>' limits to actions; a root query adds Projects, Settings and Threads. */
export function filterGroups(active: Group[], query: string, inSubmenu: boolean, extra: { projects: Item[]; settings: Item[]; threads: Item[] }): Group[] {
  const actionsOnly = query.startsWith('>');
  const normalized = normalizeSearchText(actionsOnly ? query.slice(1) : query);
  if (!normalized) return actionsOnly ? active.filter(group => group.value === 'actions') : [...active];
  const tokens = normalized.split(' ');
  let base = [...active];
  if (actionsOnly) base = base.filter(group => group.value === 'actions');
  else if (!inSubmenu) base = base.filter(group => group.value !== 'recent-threads');
  if (!inSubmenu && !actionsOnly) {
    if (extra.projects.length) base.push({ value: 'projects-search', label: 'Projects', items: extra.projects });
    if (extra.settings.length) base.push({ value: 'settings-search', label: 'Settings', items: extra.settings });
    if (extra.threads.length) base.push({ value: 'threads-search', label: 'Threads', items: extra.threads });
  }
  return base.flatMap(group => {
    const items = group.items.flatMap((item, index) => {
      const haystack = normalizeSearchText(item.terms.join(' '));
      return tokens.every(token => haystack.includes(token)) ? [{ item, index, rank: rankItem(item, normalized, tokens) }] : [];
    }).sort((a, b) => Number(a.item.secondary ?? false) - Number(b.item.secondary ?? false) || b.rank - a.rank
      || (b.item.recency ?? 0) - (a.item.recency ?? 0) || a.index - b.index).map(entry => entry.item);
    return items.length ? [{ ...group, items }] : [];
  });
}
/** The height a row paints at (ui/command.tsx: py-1.5 rows, 28px group labels). */
export function rowHeight(entry: PaletteRow): number {
  if (entry.kind === 'line') return 28;
  if (entry.kind === 'file-header') return 32;
  const meta = entry.env && entry.icon !== 'project' ? 18 : entry.description || entry.titleParts.length ? 16 : 0;
  return 12 + 20 + meta + (entry.matchLabel ? 16 : 0);
}
/** Flatten groups into rows: every selectable row numbered in display order, each group's label on its first row. */
export function flatten(groups: Group[]): PaletteRow[] {
  let index = 0, top = 8;
  return groups.flatMap((group, groupIndex) => group.items.map((item, at) => {
    const header = at === 0 ? group.label : '', gap = at === 0 && groupIndex > 0;
    top += (header ? 28 : 0) + (gap ? 6 : 0);
    const entry = { ...item.row, key: `${group.value}:${item.row.key}`, index: item.row.kind === 'disabled' || item.row.kind === 'file-header' ? -1 : index++, header, headerGap: gap, top };
    entry.height = rowHeight(entry);
    top += entry.height;
    return entry;
  }));
}

// ── Shortcut labels ────────────────────────────────────────────────────────
const KEY_LABELS: Record<string, string> = { arrowup: '↑', arrowdown: '↓', arrowleft: '←', arrowright: '→', enter: '↵', escape: 'Esc', backspace: '⌫', tab: 'Tab', space: 'Space', ' ': 'Space' };
/** formatShortcutLabel on macOS: ⌃⌥⇧⌘ then the key. */
export function shortcutText(shortcut: Obj): string {
  const key = str(shortcut.key);
  const label = KEY_LABELS[key] ?? (key.length === 1 ? key.toUpperCase() : key.charAt(0).toUpperCase() + key.slice(1));
  return `${shortcut.ctrlKey ? '⌃' : ''}${shortcut.altKey ? '⌥' : ''}${shortcut.shiftKey ? '⇧' : ''}${shortcut.metaKey || shortcut.modKey ? '⌘' : ''}${label}`;
}
/** The context the palette resolves its shortcuts in (CommandPalette.tsx: `modelPickerOpen: false`). */
const PALETTE_CONTEXT: DispatchContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
/** findEffectiveShortcutForCommand: the last binding of the command that still wins its chord here. */
export function commandShortcut(client: T3Client, command: string): string {
  const bindings = arr(client.config.keybindings);
  const winners = chordWinners(bindings, PALETTE_CONTEXT);
  const own = bindings.filter(binding => str(binding.command) === command);
  for (const binding of [...own].reverse()) {
    const shortcut = obj(binding.shortcut);
    const chord = [...(shortcut.modKey || shortcut.metaKey ? ['Meta'] : []), ...(shortcut.ctrlKey ? ['Control'] : []), ...(shortcut.altKey ? ['Alt'] : []), ...(shortcut.shiftKey ? ['Shift'] : [])];
    const winner = [...winners].find(([key]) => key.split('+').slice(0, -1).join('+') === chord.join('+') && key.split('+').pop()!.toLowerCase() === (str(shortcut.key) === '+' ? 'plus' : str(shortcut.key)).toLowerCase());
    if (!winner || winner[1] === command || winner[1] === '') return shortcutText(shortcut);
  }
  return '';
}

// ── Threads and projects ───────────────────────────────────────────────────
export function relativeLabel(iso: string, now: number): string {
  const time = Date.parse(iso);
  if (!Number.isFinite(time) || !Number.isFinite(now) || now <= 0) return '';
  const seconds = Math.floor((now - time) / 1000);
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h ago` : `${Math.floor(hours / 24)}d ago`;
}
/** getThreadSortTimestamp(thread, "updated_at") ordering, newest first, archived threads omitted. */
export function sortedThreads(threads: Obj[]): Obj[] {
  const stamp = (thread: Obj) => Date.parse(str(thread.updatedAt ?? thread.latestUserMessageAt ?? thread.createdAt)) || 0;
  return threads.filter(thread => !thread.archivedAt).slice().sort((a, b) => stamp(b) - stamp(a) || str(a.id).localeCompare(str(b.id)));
}
export function threadItems(client: T3Client, now: number, matches: Map<string, Obj>, query: string): Item[] {
  const providers = arr(client.config.providers);
  return sortedThreads(client.shell.threads).map(thread => {
    const project = client.shell.projects.find(candidate => candidate.id === thread.projectId);
    const projectTitle = str(project?.title);
    const identity = projectIdentity(projectTitle);
    const instance = providers.find(provider => provider.instanceId === obj(thread.modelSelection).instanceId);
    const match = matches.get(str(thread.id));
    const source = str(match?.source);
    const content = match && (source === 'user' || source === 'assistant') ? contentParts(str(match.snippet), query) : [];
    const stampIso = str(thread.latestUserMessageAt ?? thread.updatedAt ?? thread.createdAt);
    return { recency: Date.parse(str(thread.updatedAt ?? thread.createdAt)) || 0,
      terms: [str(thread.title), ...arr(thread.pullRequests).map(pr => `#${str(pr.number)}`), projectTitle, str(thread.branch), str(match?.snippet), str(thread.id)],
      row: row({ key: str(thread.id), op: 'thread', arg: str(thread.id), icon: 'message-square', title: str(thread.title, 'Untitled thread'), timestamp: relativeLabel(stampIso, now),
        project: projectTitle ? identity.projectMark : '', projectInk: identity.projectInk, projectSurface: identity.projectSurface, description: projectTitle, env: projectTitle ? 'Local' : '',
        terminalCount: terminalProcessCount(client, str(thread.id)), branch: str(thread.branch), provider: str(instance?.driver), current: thread.id === client.threadId,
        matchLabel: content.length ? (source === 'user' ? 'You:' : 'Agent:') : '', matchParts: content }) };
  });
}
/** ThreadSearchMatchExcerpt: the snippet with every case-folded occurrence of the query marked. */
export function contentParts(snippet: string, query: string): PalettePart[] {
  const text = snippet.replace(/\s+/g, ' ').trim(), needle = query.trim().toLowerCase();
  if (!text) return [];
  const parts: PalettePart[] = [];
  let cursor = 0;
  while (needle && cursor < text.length) {
    const at = text.toLowerCase().indexOf(needle, cursor);
    if (at < 0) break;
    if (at > cursor) parts.push({ id: String(cursor), text: text.slice(cursor, at), hit: false, cls: '' });
    parts.push({ id: String(at), text: text.slice(at, at + needle.length), hit: true, cls: '' });
    cursor = at + needle.length;
  }
  if (cursor < text.length) parts.push({ id: String(cursor), text: text.slice(cursor), hit: false, cls: '' });
  return parts;
}
export function projectItems(client: Pick<T3Client, 'shell'>, prefix: string, op: string): Item[] {
  return client.shell.projects.map(project => {
    const title = str(project.title), identity = projectIdentity(title), root = str(project.workspaceRoot);
    return { terms: [title, title, root, title, root, 'Local'], row: row({ key: `${prefix}:${str(project.id)}`, op, arg: str(project.id), icon: 'project', title,
      project: identity.projectMark, projectInk: identity.projectInk, projectSurface: identity.projectSurface, env: 'Local', description: root }) };
  });
}

// ── Root ───────────────────────────────────────────────────────────────────
function capabilities(client: T3Client): Obj { return obj(obj(client.config.environment).capabilities); }
export function rootActions(client: T3Client): Item[] {
  const items: Item[] = [];
  const action = (key: string, terms: string[], fields: Partial<PaletteRow>) => items.push({ terms, row: row({ key, ...fields }) });
  const projects = client.shell.projects;
  const current = projects.find(project => project.id === client.projectId) ?? projects[0];
  const thread = client.threadId ? client.shell.threads.find(candidate => candidate.id === client.threadId) : undefined;
  if (projects.length) {
    if (current) action('new-thread', ['new thread', 'chat', 'create', 'draft'], { op: 'new-thread', arg: str(current.id), icon: 'square-pen', prefix: 'New thread in ', bold: str(current.title), title: `New thread in ${str(current.title)}`, shortcut: commandShortcut(client, 'chat.new') });
    action('new-thread-in', ['new thread', 'project', 'pick', 'choose', 'select'], { kind: 'submenu', op: 'page', arg: 'new-thread-in', icon: 'square-pen', title: 'New thread in...' });
  }
  if (str(client.config.scratchWorkspaceRoot)) action('new-thread-without-project', ['new thread', 'no project', 'without project', 'none', 'chat'],
    { op: 'flow', arg: 'scratch', icon: 'message-square-dashed', title: 'New thread without a project', shortcut: commandShortcut(client, 'chat.newWithoutProject') });
  if (thread) {
    const reference = threadReferenceTarget(client, openPanelPullRequestUrl(client)); // thread-commands-and-keys: Copy PR link or thread ID
    if (reference) action('copy-thread-reference', ['copy', 'pull request', 'pr link', 'thread id', 'reference'], { op: 'copy-thread', icon: 'link', title: reference.kind === 'pull-request' ? 'Copy PR link' : 'Copy thread ID', description: reference.value, shortcut: commandShortcut(client, 'thread.copyReference') });
    if (linkMode(client.config) !== 'unsupported') action('link-pull-request', ['link', 'pull request', 'pr', 'attach', 'stack'], { op: 'page', arg: 'link-pr', icon: 'pull-request-link', title: 'Link pull request to thread' });
    if (capabilities(client).threadPullRequests === true) {
      const linked = arr(thread.pullRequests).length > 0;
      action('open-thread-pull-requests', ['pull requests', 'linked', 'stack', 'prs'], { kind: linked ? 'action' : 'disabled', op: 'thread-pull-requests', arg: str(thread.id), icon: 'pull-request-link', title: 'Show linked pull requests' });
    }
    action('restart-agent-session', ['restart', 'reset', 'reload', 'agent', 'session', 'skills', 'plugins', 'mcp'], { op: 'flow', arg: 'restart-session', arg2: str(thread.id), icon: 'rotate-ccw', title: 'Restart agent session' });
  }
  action('open-file-picker', ['go to file', 'open file', 'file picker', 'find file', 'quick open'], { op: 'mode', arg: 'files', icon: 'file-search', title: 'Go to file', shortcut: commandShortcut(client, 'filePicker.toggle') });
  action('search-project-contents', ['search project', 'find in files', 'grep', 'content search', 'text search'], { op: 'mode', arg: 'content', icon: 'text-search', title: 'Search project contents', shortcut: commandShortcut(client, 'projectSearch.toggle') });
  if (str(client.config.newProjectsRoot)) action('new-project', ['new project', 'create project', 'empty', 'repository', 'repo', 'git init'], { op: 'page', arg: 'new-project', icon: 'folder-git', title: 'New project' });
  action('add-project', ['add project', 'folder', 'directory', 'browse', 'clone', 'remote', 'repository', 'repo', 'git', 'github', 'gitlab', 'forgejo', 'bitbucket', 'azure', 'devops', 'url', 'environment'],
    { op: 'page', arg: 'add-project', icon: 'folder-plus', title: 'Add project' });
  action('change-theme', ['change theme', 'appearance', 'colors', 'palette'], { kind: 'submenu', op: 'page', arg: 'theme', icon: 'palette', title: 'Change theme', shortcut: commandShortcut(client, 'theme.select') });
  action('change-appearance', ['change appearance', 'light', 'dark', 'system', 'mode', 'toggle'], { kind: 'submenu', op: 'page', arg: 'appearance', icon: 'monitor', title: 'Change appearance', shortcut: commandShortcut(client, 'appearance.cycle') });
  action('theme-editor', ['theme', 'appearance', 'colors', 'palette', 'customize'], { op: 'theme-editor', icon: 'palette', title: 'Toggle theme editor', shortcut: commandShortcut(client, 'themeEditor.toggle') });
  if (capabilities(client).pullRequests === true) action('pull-requests', ['pull requests', 'prs', 'pr', 'github', 'review', 'merge', 'branch'], { op: 'pull-requests', icon: 'pull-request', title: 'Open pull requests' });
  action('usage', ['usage', 'use', 'tokens', 'cost', 'spend', 'limits', 'stats', 'analytics'], { op: 'usage', icon: 'chart', title: 'Open usage', shortcut: commandShortcut(client, 'usage.open') });
  action('settings', ['settings', 'preferences', 'configuration', 'keybindings'], { op: 'settings', arg: 'general', icon: 'settings', title: 'Open settings' });
  if (current) action('project-settings', ['project', 'settings', 'name', 'icon', 'scripts', 'model', 'workspace', 'grouping', 'checkout', 'remove', 't3.json'],
    { op: 'project-settings', arg: str(current.id), icon: 'folder', title: 'Project settings', description: str(current.title) });
  return items;
}

/**
 * The New thread in... page (projectThreadItems): the sidebar's project order with the
 * current project first (buildSidebarProjectPickerEntries), the Scratch project as a
 * trailing "No project". enumerateCommandPaletteItems numbers the first nine by
 * thread.jump.N, whose rules hold only `isDesktop`: this is the desktop build, so each shows
 * its ⌘N and the field runs it by that chord (CommandPalette.tsx handleKeyDown;
 * shell-sidebar-palette-keys SH-3). "No project" past the ninth row has no shortcut.
 */
export function newThreadInItems(client: T3Client): Item[] {
  const root = scratchRoot(client.config);
  const ordered = pickerProjects(client).filter(project => !isScratchProject(project, root));
  const items = projectItems({ shell: { ...client.shell, projects: ordered } }, 'new-thread-in', 'new-thread');
  if (root) items.push({ terms: ['No project', 'no project', 'without project', 'none'],
    row: row({ key: 'new-thread-in:no-project', op: 'flow', arg: 'scratch', icon: 'message-square-dashed', title: 'No project' }) });
  return enumerateJumps(client, items);
}
/**
 * CommandPalette.tsx handleKeyDown: a chord that resolves to a thread.jump.N command is the field's, whether or
 * not a displayed row carries it (⌘5 with two projects, ⌘1 on the root page): it is prevented and stopped, and
 * it runs the row that has it, if one does. These are those chords, in the palette's context.
 */
export function threadJumpChords(client: T3Client): string[] {
  const winners = chordWinners(arr(client.config.keybindings), PALETTE_CONTEXT);
  return [...winners].filter(([, command]) => /^thread\.jump\.[1-9]$/.test(command)).map(([chord]) => chord);
}
/** enumerateCommandPaletteItems: the Nth row (N ≤ 9) takes thread.jump.N's label and chord. */
export function enumerateJumps(client: T3Client, items: Item[]): Item[] {
  const winners = chordWinners(arr(client.config.keybindings), PALETTE_CONTEXT);
  return items.map((item, index) => {
    const command = index < 9 ? `thread.jump.${index + 1}` : '';
    const chord = command ? [...winners].find(([, winner]) => winner === command)?.[0] ?? '' : '';
    return { ...item, row: { ...item.row, shortcut: command ? commandShortcut(client, command) : '', jump: chord } };
  });
}
function themeItems(client: T3Client, scheme: string): Item[] {
  const local = client.local as unknown as { clientSettings?: ClientPrefs; customThemes?: Parameters<typeof themeCards>[1] };
  const prefs = local.clientSettings || decodeClientPrefs({});
  return themeCards(prefs, local.customThemes ?? []).map(card => {
    const active = (scheme === 'dark' ? prefs.themeDark : prefs.themeLight) === card.id;
    return { terms: [card.label, 'theme', 'appearance'], row: row({ key: `theme:${card.id}`, op: 'device', arg: 'theme', arg2: card.id, icon: 'palette', title: card.label,
      trailing: active ? 'Current' : '', orbs: card.orbs }) };
  });
}
function appearanceItems(client: T3Client): Item[] {
  const mode = str(client.local.deviceSettings.appearanceMode, 'system');
  return [['system', 'System', 'monitor'], ['light', 'Light', 'sun'], ['dark', 'Dark', 'moon']].map(([value, label, icon]) => ({ terms: [label!, 'appearance', 'mode'],
    row: row({ key: `appearance:${value}`, op: 'device', arg: 'appearanceMode', arg2: value!, icon: icon!, title: label!, trailing: mode === value ? 'Current' : '' }) }));
}
export function settingsItems(client: T3Client, query: string): Item[] {
  const context = searchContext(client.config, client.ready, client.projectId ? 'environment' : 'all');
  return searchSettings(query, context).map(item => ({ secondary: item.secondary, terms: [item.title, breadcrumbLabel(item.route), ...item.terms],
    row: row({ key: `setting:${item.id}`, op: 'settings', arg: item.route, arg2: item.route === 'general' || item.route === 'appearance' ? `setting-${item.target}` : '',
      icon: 'settings', title: item.title, description: `Settings · ${breadcrumbLabel(item.route)}` }) }));
}

/** `linkedThreads`: a pull request's linked threads, which take the thread results' place while the query is its URL (pr-links-previews-and-routing). */
export type CommandContext = { page: string; query: string; now: number; scheme: string; matches: Map<string, Obj>; matchQuery: string; searching: boolean; linkedThreads?: Item[] | null };
/** The command palette's root and its submenus (not the add-project flow). */
export function commandView(client: T3Client, context: CommandContext): PaletteView {
  const page = context.page;
  const submenu = page !== '';
  const groups: Group[] = page === 'new-thread-in' ? [{ value: 'projects', label: 'Projects', items: newThreadInItems(client) }]
    : page === 'theme' ? [{ value: 'themes', label: 'Change theme', items: themeItems(client, context.scheme) }]
      : page === 'appearance' ? [{ value: 'appearance', label: 'Change appearance', items: appearanceItems(client) }]
        : [{ value: 'actions', label: 'Actions', items: rootActions(client) },
          { value: 'recent-threads', label: 'Recent Threads', items: threadItems(client, context.now, new Map(), '').slice(0, RECENT_THREAD_LIMIT) }].filter(group => group.items.length);
  const threads = submenu ? [] : context.linkedThreads ?? threadItems(client, context.now, context.matches, context.matchQuery);
  const filtered = filterGroups(groups, context.query, submenu, { projects: submenu ? [] : projectItems({ shell: { ...client.shell, projects: pickerProjects(client) } }, 'project', 'open-project'), settings: submenu ? [] : settingsItems(client, context.query), threads });
  const rows = flatten(filtered);
  const actionsOnly = context.query.startsWith('>');
  return { ...closedView, open: true, page, parent: '', back: submenu, addon: submenu ? 'back' : 'search', placeholder: submenu ? 'Search...' : 'Search commands, projects, and threads...',
    rows, count: rows.filter(entry => entry.index >= 0).length, backHint: submenu, enterLabel: 'Select',
    empty: rows.length ? '' : context.searching ? 'Searching thread messages…' : actionsOnly ? 'No matching actions.' : 'No matching commands, projects, or threads.' };
}
