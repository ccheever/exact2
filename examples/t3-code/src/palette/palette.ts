// Adapted from T3 Code 7c3b3b3605ef (MIT); see LICENSE-T3.
// The command palette (⌘K): reference CommandPalette.tsx, CommandPalette.logic.ts,
// CommandPaletteResults.tsx, CommandPaletteThreadItems.tsx, ThreadSearchMatch.tsx and
// ThreadCommandSubtitle.tsx. One view model for every page the overlay can show; Contract
// (palette.contract) owns the open state, the page, the query and the highlight. The ranking
// is palette-search.ts; the add-project pages live in palette-add.ts, the file picker and
// content search in palette-files.ts.
import type { T3Client } from '../client/client';
import { composerPromptLink, promptLinkRepositoryOf } from '../prompt-links/copy-prompt-link';
import { terminalProcessCount } from '../sidebar/sidebar-view';
import { searchPullRequest } from '../sidebar/search-pull-request';
import { arr, num, obj, str, type Obj } from '../client/domain';
import { projectIdentity } from '../client/presentation';
import { searchSettings, searchContext, breadcrumbLabel, searchTarget } from '../settings/settings-search';
import { themeCards } from '../appearance/settings-appearance';
import { decodeClientPrefs, type ClientPrefs } from '../settings/settings-core';
import { chordWinners, type DispatchContext } from '../shell/keyboard-dispatch';
import { linkMode } from './palette-linkpr';
import { scratchRoot, isScratchProject } from '../pages/pages-home';
import { scratchEnvironmentId, scratchEnvironments } from '../composer/scratch-project';
import { primaryEnvironmentId } from '../connections/local-primary';
import { triageEnvironment } from './palette-triage';
import { pickerProjects } from './palette-projects'; // Project order
import { openPanelPullRequestUrl, threadReferenceTarget } from '../sidebar/thread-reference';
import { addProjectEnvironments, newProjectEnvironments, projectLocation } from './palette-environments';
import { fleet, fleetThreads, parseFleetThreadId, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { sourceControlHosts } from '../shared/source-control-hosts';
import { textDirection } from '../shared/text-direction';
import {
  findSearchTokenRanges, foldSearchQuery, PALETTE_MESSAGE_LIMIT, PALETTE_SECONDARY_LIMIT, PALETTE_THREAD_LIMIT, rankMessageMatches, rankSearchGroups, scoreSearchTerms,
  scoreThreadSearch, splitSearchText, threadRecentUse, threadSearchSubject, type SearchMatchHint, type SearchRange, type ThreadSearchSubject,
} from './palette-search';

export type PalettePart = { id: string; text: string; hit: boolean; cls: string };
export type PaletteRow = {
  key: string; index: number; header: string; headerGap: boolean; kind: string; op: string; arg: string; arg2: string;
  icon: string; prefix: string; title: string; bold: string; description: string; shortcut: string; timestamp: string; trailing: string;
  /** The chords that run this row from the field, space-separated (thread.jump.N on a project pick, a new-thread
   *  row's chat.newLocal or chat.newWithoutProject; aria grammar: `Meta+1`); '' for none. */
  jump: string;
  publishHost: string; publishHostLabel: string; publishHosts: { value: string; label: string; selected: boolean }[];
  terminalCount: number; prNumber: string; prIcon: string; prState: string; prLabel: string;
  badge: string; badgeOp: string; checkbox: boolean; checked: boolean; project: string; projectInk: string; projectSurface: string; env: string;
  /** A remote environment's machine glyph before `env` on a project row ('' for none). */
  envMachine: string;
  /** The second line's label ("You:", "Agent:", or a hint's field, as "Branch:") and its text, the matches marked; the
   *  line replaces the description. */
  matchLabel: string; matchParts: PalettePart[];
  /** ThreadRowContext: a thread row's project and environment beside its title while the second line is a match. */
  context: string;
  /** The title's reading direction (textDirection): 'rtl', else '' for the page's own. */
  titleDir: string;
  branch: string; provider: string; current: boolean; titleParts: PalettePart[]; line: string; orbs: Orb[]; top: number; height: number;
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
  publishHost: '', publishHostLabel: '', publishHosts: [], terminalCount: 0, prNumber: '', prIcon: '', prState: '', prLabel: '', description: '', shortcut: '', jump: '', timestamp: '', trailing: '', badge: '', badgeOp: '', checkbox: false, checked: false, project: '', projectInk: '', projectSurface: '', env: '', envMachine: '',
  branch: '', provider: '', current: false, matchLabel: '', matchParts: [], context: '', titleDir: '', titleParts: [], line: '', orbs: [], top: 0, height: 32 };
export function row(fields: Partial<PaletteRow>): PaletteRow { return { ...blankRow, ...fields }; }

export const closedView: PaletteView = {
  open: false, mode: 'command', page: '', parent: '', back: false, addon: 'search', placeholder: '', label: 'Command palette', testId: 'command-palette', rows: [], count: 0,
  autoHighlight: true, empty: '', enterLabel: '', escapeLabel: 'Close', backHint: false, popOnEmpty: false, enterOp: '', enterArg: '', enterArg2: '', accessory: '', accessoryKey: '',
  accessoryEnabled: false, accessoryOp: '', accessoryArg: '', accessoryArg2: '', footerAction: '', contextLabel: '', contextTitle: '', contextDescription: '', contextIcon: '',
  toggles: false, matchCase: false, wholeWord: false, regex: false, summary: '', panel: 'list', inputPaddingRight: 11, loading: false, listHeight: 0, jumpKeys: [],
};

/**
 * One searchable entry before grouping: the reference CommandPaletteItem. `terms` are its keywords (searchTerms),
 * which its title joins as the name; a thread ranks by its `thread` subject instead. A submenu row's `children` are
 * its options, which a root query lists as Suggestions (buildNestedSearchItems).
 */
export type Item = { row: PaletteRow; terms: string[]; thread?: ThreadSearchSubject; secondary?: boolean; children?: Group[] };
export type Group = { value: string; label: string; items: Item[] };
/**
 * What a query searches besides the active groups: the root's Projects, Settings and Threads; `messages`, threads
 * whose messages match, best first; `nested`, the submenus' options; `root`, the groups a submenu falls back to when
 * nothing in it matches; `now`, the moment use is measured from.
 */
export type SearchExtra = { projects: Item[]; settings: Item[]; threads: Item[]; messages?: Item[]; nested?: Item[]; root?: Group[]; now?: number };

type ItemMatch = { score: number; quality: number; groupQuality?: number; strong: boolean; ranges: ReadonlyArray<SearchRange>; explain?: () => SearchMatchHint | undefined; pickedAt?: number; pickedExactly?: boolean };
function scoreItem(item: Item, normalizedQuery: string, now: number, options: { readonly typos: boolean }): ItemMatch | null {
  // The title names the item; search terms are keywords.
  const match: ItemMatch | null = item.thread ? scoreThreadSearch(normalizedQuery, item.thread, now, options)
    : scoreSearchTerms(normalizedQuery, item.row.title !== item.terms[0] ? [item.row.title, ...item.terms] : item.terms, options);
  // A row listed last in its group (a keybinding setting, the open thread) does not decide where the group sits.
  return match && item.secondary ? { ...match, groupQuality: 0 } : match;
}
/**
 * filterCommandPaletteGroups: '>' limits to actions; a root query adds Suggestions, Threads, Projects and Settings,
 * and groups sort by their best match class, keeping this order on ties, so "new" runs "New thread". Server message
 * matches stay in a final group so late results cannot move earlier rows.
 */
export function filterGroups(active: Group[], query: string, inSubmenu: boolean, extra: SearchExtra): Group[] {
  if (extra.root && inSubmenu) {
    const scoped = filterGroups(active, query, true, { ...extra, root: undefined });
    if (scoped.length > 0 || query.trim().length === 0) return scoped;
    return filterGroups(extra.root, query, false, extra);
  }
  const actionsOnly = query.startsWith('>');
  const normalized = foldSearchQuery(actionsOnly ? query.slice(1) : query);
  if (!normalized) return actionsOnly ? active.filter(group => group.value === 'actions') : [...active];
  let base = [...active];
  if (actionsOnly) base = base.filter(group => group.value === 'actions');
  else if (!inSubmenu) base = base.filter(group => group.value !== 'recent-threads');
  const everything = !inSubmenu && !actionsOnly;
  // Threads, what people search for most, come right after commands.
  if (everything) {
    if (extra.nested?.length) base.push({ value: 'nested-search', label: 'Suggestions', items: extra.nested });
    if (extra.threads.length) base.push({ value: 'threads-search', label: 'Threads', items: extra.threads });
    if (extra.projects.length) base.push({ value: 'projects-search', label: 'Projects', items: extra.projects });
    if (extra.settings.length) base.push({ value: 'settings-search', label: 'Settings', items: extra.settings });
  }
  // Use is measured only for threads, which come with their moment (a data source has no clock of its own).
  const now = extra.now ?? 0;
  const ranked = rankSearchGroups(base.map(group => ({ group, items: group.items })), (item, options) => scoreItem(item, normalized, now, options));
  const threadsMatch = ranked.some(({ group }) => group.value === 'threads-search');
  const groups = ranked.map(({ group, ranked: items }): Group => {
    const limit = group.value === 'threads-search' ? PALETTE_THREAD_LIMIT : group.value === 'nested-search' || (everything && threadsMatch) ? PALETTE_SECONDARY_LIMIT : items.length;
    // Secondary items stay below the rest; the sort is stable, so score order holds. Hints are built only for the rows shown.
    const shown = [...items].sort((left, right) => Number(left.item.secondary ?? false) - Number(right.item.secondary ?? false)).slice(0, limit).map(({ item, match }) => {
      if (match.ranges.length > 0) return item;
      const hint = match.explain?.();
      return hint ? matchHintItem(item, hint) : item;
    });
    return { value: group.value, label: group.label, items: shown };
  });
  if (!everything) return groups;
  const listed = new Set(groups.flatMap(group => group.items.map(item => item.row.key)));
  const messages = (extra.messages ?? []).filter(item => !listed.has(item.row.key));
  return messages.length ? [...groups, { value: 'threads-messages', label: 'In messages', items: messages }] : groups;
}
/** HighlightedSearchText: `text` cut at the matched ranges. */
function searchParts(text: string, ranges: ReadonlyArray<SearchRange>): PalettePart[] {
  return splitSearchText(text, ranges).map(part => ({ id: String(part.start), text: part.text, hit: part.highlighted, cls: '' }));
}
/** ThreadRowContext: the project and environment, less a hint's own field ("Project: …" already names the project). */
function rowContext(thread: ThreadSearchSubject | undefined, hintText = ''): string {
  return thread ? [thread.projectTitle, thread.environmentLabel].filter(part => part && part !== hintText).join(' · ') : '';
}
/** SearchMatchHintLine: why a row matched when its title shows no match, in place of its description. */
function matchHintItem(item: Item, hint: SearchMatchHint): Item {
  return { ...item, row: { ...item.row, matchLabel: `${hint.label}:`, matchParts: searchParts(hint.text, hint.ranges), context: rowContext(item.thread, hint.text) } };
}
/**
 * Every option inside a submenu as one row, as "New thread in t3code" or "Change theme: Dracula", keeping what it
 * does and its shortcut. A query that only names the submenu ("new thread in") wants the submenu row itself.
 */
export function nestedItems(items: Item[], query: string): Item[] {
  const topLevel = new Set(items.map(item => item.row.title));
  const folded = foldSearchQuery(query);
  return items.flatMap(parent => {
    if (parent.row.kind !== 'submenu' || !parent.children) return [];
    const stem = parent.row.title.replace(/(\.\.\.|…)$/u, '');
    if (foldSearchQuery(stem).startsWith(folded)) return [];
    const prefix = stem === parent.row.title ? `${stem}: ` : `${stem.trimEnd()} `;
    return parent.children.flatMap(group => group.items.flatMap(child => {
      const title = `${prefix}${child.row.title}`;
      if (child.row.kind === 'submenu' || topLevel.has(title)) return [];
      // Only the combined title: child keywords like "theme" would list every theme.
      return [{ ...child, terms: [title], row: { ...child.row, title, prefix: '', bold: '' } }];
    }));
  });
}
/** The height a row paints at (ui/command.tsx: py-1.5 rows, 28px group labels). */
export function rowHeight(entry: PaletteRow): number {
  if (entry.kind === 'line') return 28;
  if (entry.kind === 'file-header') return 32;
  // A match line replaces the description, so a match row is as tall as a row with one.
  const meta = entry.matchLabel ? 16 : entry.env && entry.icon !== 'project' ? 18 : entry.description || entry.env || entry.titleParts.length ? 16 : 0;
  return 12 + 20 + meta;
}
/**
 * Flatten groups into rows: every selectable row numbered in display order, each group's label on its first row.
 * `directions`: a plain title reads in its own direction (CommandPaletteTitle); the file picker's names do not.
 */
export function flatten(groups: Group[], directions = true): PaletteRow[] {
  let index = 0, top = 8;
  return groups.flatMap((group, groupIndex) => group.items.map((item, at) => {
    const header = at === 0 ? group.label : '', gap = at === 0 && groupIndex > 0;
    top += (header ? 28 : 0) + (gap ? 6 : 0);
    const titleDir = directions && !item.row.bold && textDirection(item.row.title) === 'rtl' ? 'rtl' : '';
    const entry = { ...item.row, key: `${group.value}:${item.row.key}`, index: item.row.kind === 'disabled' || item.row.kind === 'file-header' ? -1 : index++, header, headerGap: gap, titleDir, top };
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
/** The context the palette's field resolves a pressed chord in (CommandPalette.tsx handleKeyDown: an editable focus). */
const FIELD_CONTEXT: DispatchContext = { ...PALETTE_CONTEXT, editableFocus: true };
/** The chords that resolve to `command` in the palette's field, space-separated (PaletteRow `jump`); '' for none. */
export function fieldChords(client: T3Client, command: string): string {
  const winners = chordWinners(arr(client.config.keybindings), FIELD_CONTEXT);
  return [...winners].filter(([, winner]) => winner === command).map(([chord]) => chord).join(' ');
}
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
/** Two or more environments: only then can a thread's environment be searched (useCommandPaletteThreadItems). */
function severalEnvironments(client: T3Client): boolean {
  return new Set([client.environmentId, ...fleet.catalogIds()].filter(Boolean)).size > 1;
}
/**
 * buildThreadActionItems over useThreadShells: every visible thread (not archived, not a subagent) of the focused
 * environment and of each background one (fleetThreads, under `fleet:<environment>:<id>`), ranked by its search subject;
 * the open one is listed last in its group, as picking it does nothing. `query` is the root query a pull request badge
 * shows for (ThreadSearchPullRequestNumber).
 */
export function threadItems(client: T3Client, now: number, query: string, source: EnvironmentFleet = fleet): Item[] {
  const searchedLabels = severalEnvironments(client);
  const entries = new Map([...source.entries.values()].map(entry => [entry.environmentId, entry]));
  return sortedThreads([...client.shell.threads, ...fleetThreads(source)]).filter(thread => obj(thread.lineage).relationshipToParent !== 'subagent').map(thread => {
    const id = str(thread.id), ref = parseFleetThreadId(id), environmentId = ref?.environmentId ?? client.environmentId;
    // ThreadCommandSubtitle's environmentLabel: the thread's own environment's.
    const environmentLabel = projectLocation(client, environmentId, source).label;
    const projectTitle = ref ? str(thread.fleetProjectTitle) : str(client.shell.projects.find(candidate => candidate.id === thread.projectId)?.title);
    const identity = projectIdentity(projectTitle);
    const providers = arr((ref ? entries.get(environmentId)?.config : client.config)?.providers);
    const instance = providers.find(provider => provider.instanceId === obj(thread.modelSelection).instanceId);
    const stampIso = str(thread.latestUserMessageAt ?? thread.updatedAt ?? thread.createdAt);
    const current = !ref && id === client.threadId;
    const subject = threadSearchSubject(ref ? { ...thread, id: ref.threadId } : thread, projectTitle || null, searchedLabels ? environmentLabel : null, environmentId);
    return { thread: subject, terms: [str(thread.title)], ...(current ? { secondary: true } : {}),
      row: row({ key: id, op: 'thread', arg: id, icon: 'message-square', title: str(thread.title, 'Untitled thread'), timestamp: relativeLabel(stampIso, now),
        project: projectTitle ? identity.projectMark : '', projectInk: identity.projectInk, projectSurface: identity.projectSurface, description: projectTitle, env: projectTitle ? environmentLabel : '',
        terminalCount: terminalProcessCount(client, id), ...palettePullRequest(client, thread, query), branch: str(thread.branch), provider: str(instance?.driver), current }) };
  });
}
/**
 * The threads whose messages match and whose title, branch or pull request do not, best first (rankMessageMatches,
 * PALETTE_MESSAGE_LIMIT): the servers' matches, one per thread keyed by its row's id, every environment's ranked
 * together, as their threads' rows with the snippet in place of the project line. `query` is the one the matches were
 * found for.
 */
export function messageItems(threads: Item[], matches: Map<string, Obj>, query: string, now: number): Item[] {
  const byId = new Map(threads.map(item => [item.row.arg, item]));
  const candidates = [...matches].map(([threadId, match]) => ({ match, threadId, messageCreatedAt: str(match.messageCreatedAt) || null,
    quality: typeof match.quality === 'number' ? match.quality : undefined, hits: typeof match.hits === 'number' ? num(match.hits) : undefined }));
  return rankMessageMatches(candidates, candidate => byId.get(candidate.threadId)?.thread, foldSearchQuery(query), now, { withoutTitleMatches: true }).slice(0, PALETTE_MESSAGE_LIMIT).flatMap(candidate => {
    const item = byId.get(candidate.threadId);
    if (!item) return [];
    const snippet = str(candidate.match.snippet).replace(/\s+/g, ' ').trim();
    return [{ ...item, row: { ...item.row, matchLabel: str(candidate.match.source) === 'user' ? 'You:' : 'Agent:', matchParts: searchParts(snippet, findSearchTokenRanges(snippet, query)),
      context: rowContext(item.thread),
      // When the matching message was written, not when the thread last moved.
      timestamp: candidate.messageCreatedAt ? relativeLabel(candidate.messageCreatedAt, now) : item.row.timestamp } }];
  });
}
/** ThreadSearchPullRequestNumber in a thread row: a settled thread's number stays muted (no row hover tints it here). */
function palettePullRequest(client: T3Client, thread: Obj, query: string): Pick<PaletteRow, 'prNumber' | 'prIcon' | 'prState' | 'prLabel'> {
  const pr = searchPullRequest(client, thread, query);
  return pr ? { prNumber: pr.text, prIcon: pr.icon, prState: thread.settledOverride === 'settled' ? '' : pr.state, prLabel: pr.label } : { prNumber: '', prIcon: '', prState: '', prLabel: '' };
}
/**
 * buildProjectActionItems over pickerProjects: the group's name as title, the target's own title for its mark
 * (ProjectFavicon), and its environment's location (projectEnvironmentLocationById) beside the path. On the
 * Projects search a grouped project names its members' environments instead (ProjectSearchDescription).
 */
export function projectItems(client: T3Client, projects: Obj[], prefix: string, op: string, search = false): Item[] {
  return projects.map(project => {
    const title = str(project.title), identity = projectIdentity(str(project.memberTitle) || title), root = str(project.workspaceRoot);
    const members = Array.isArray(project.memberProjects) ? project.memberProjects.map(obj) : [project];
    const location = projectLocation(client, str(project.environmentId));
    const labels = [...new Set(members.map(member => projectLocation(client, str(member.environmentId)).label))];
    const grouped = search && members.length > 1;
    const terms = [title, str(project.memberTitle) || title, root, ...(search ? members.flatMap(member => [str(member.title), str(member.workspaceRoot), projectLocation(client, str(member.environmentId)).label])
      : [...members.flatMap(member => [str(member.title), str(member.workspaceRoot)]), location.label])];
    return { terms, row: row({ key: `${prefix}:${str(project.id)}`, op, arg: str(project.id), icon: 'project', title,
      project: identity.projectMark, projectInk: identity.projectInk, projectSurface: identity.projectSurface,
      env: grouped ? labels.join(' · ') : location.label, envMachine: !grouped && location.remote ? location.machine : '', description: grouped ? '' : root }) };
  });
}

// ── Root ───────────────────────────────────────────────────────────────────
/** CommandPalette's scratchTargetEnvironmentId: where a thread without a project starts, the primary first (useScratchProject). */
const scratchTarget = (client: T3Client) => scratchEnvironmentId(scratchEnvironments(client), primaryEnvironmentId(), client.environmentId || null);
function capabilities(client: T3Client): Obj { return obj(obj(client.config.environment).capabilities); }
export function rootActions(client: T3Client): Item[] {
  const items: Item[] = [];
  const action = (key: string, terms: string[], fields: Partial<PaletteRow>) => items.push({ terms, row: row({ key, ...fields }) });
  const projects = client.shell.projects;
  const current = projects.find(project => project.id === client.projectId) ?? projects[0];
  const thread = client.threadId ? client.shell.threads.find(candidate => candidate.id === client.threadId) : undefined;
  if (projects.length) {
    if (current) action('new-thread', ['new thread', 'chat', 'create', 'draft'], { op: 'new-thread', arg: str(current.id), icon: 'square-pen', prefix: 'New thread in ', bold: str(current.title), title: `New thread in ${str(current.title)}`, shortcut: commandShortcut(client, 'chat.newLocal'), jump: fieldChords(client, 'chat.newLocal') });
    action('new-thread-in', ['new thread', 'project', 'pick', 'choose', 'select'], { kind: 'submenu', op: 'page', arg: 'new-thread-in', icon: 'square-pen', title: 'New thread in...' });
  }
  if (scratchTarget(client) !== null) action('new-thread-without-project', ['new thread', 'no project', 'without project', 'none', 'chat'],
    { op: 'flow', arg: 'scratch', icon: 'message-square-dashed', title: 'New thread without a project', shortcut: commandShortcut(client, 'chat.newWithoutProject'), jump: fieldChords(client, 'chat.newWithoutProject') });
  if (triageEnvironment(client)) action('triage', ['triage', 'bug', 'issue', 'report', 'broken', 'debug', 'help', 'support'],
    { op: 'flow', arg: 'triage', icon: 'bug', title: 'Triage an issue', description: 'An agent investigates a T3 Code problem and helps file an issue' });
  if (composerPromptLink(client, false)) {
    action('copy-prompt-link', ['copy', 'share', 'prompt', 'link'], { op: 'flow', arg: 'copy-prompt-link', icon: 'link', title: 'Copy prompt link' });
    if (promptLinkRepositoryOf(projects.find(project => project.id === client.projectId))) action('copy-prompt-link-with-context', ['copy', 'share', 'prompt', 'link', 'repository', 'model'], { op: 'flow', arg: 'copy-prompt-link', arg2: 'context', icon: 'link', title: 'Copy prompt link with repo and model' });
  }
  if (thread) {
    const reference = threadReferenceTarget(client, openPanelPullRequestUrl(client)); // Copy PR link or the thread's chip
    if (reference) action('copy-thread-reference', reference.kind === 'pull-request' ? ['copy', 'pull request', 'link', 'reference'] : ['copy', 'thread', 'thread id', 'reference'],
      { op: 'copy-thread', icon: 'link', title: reference.kind === 'pull-request' ? 'Copy PR link' : 'Copy thread', description: reference.description, shortcut: commandShortcut(client, 'thread.copyReference') });
    if (linkMode(client.config) !== 'unsupported') action('link-pull-request', ['link', 'pull request', 'pr', 'attach', 'stack'], { op: 'page', arg: 'link-pr', icon: 'pull-request-link', title: 'Link pull request to thread' });
    if (capabilities(client).threadPullRequests === true) {
      const linked = arr(thread.pullRequests).length > 0;
      action('open-thread-pull-requests', ['pull requests', 'linked', 'stack', 'prs'], { kind: linked ? 'action' : 'disabled', op: 'thread-pull-requests', arg: str(thread.id), icon: 'pull-request-link', title: 'Show linked pull requests' });
    }
    action('restart-agent-session', ['restart', 'reset', 'reload', 'agent', 'session', 'skills', 'plugins', 'mcp'], { op: 'flow', arg: 'restart-session', arg2: str(thread.id), icon: 'rotate-ccw', title: 'Restart agent session' });
  }
  action('open-file-picker', ['go to file', 'open file', 'file picker', 'find file', 'quick open'], { op: 'mode', arg: 'files', icon: 'file-search', title: 'Go to file', shortcut: commandShortcut(client, 'filePicker.toggle') });
  action('search-project-contents', ['search project', 'find in files', 'grep', 'content search', 'text search'], { op: 'mode', arg: 'content', icon: 'text-search', title: 'Search project contents', shortcut: commandShortcut(client, 'projectSearch.toggle') });
  // newProjectEnvironmentOptions: any connected environment that can make a new project, which openNewProjectFlow starts on.
  if (newProjectEnvironments(client).length) action('new-project', ['new project', 'create project', 'empty', 'repository', 'repo', 'git init'], { op: 'page', arg: 'new-project', icon: 'folder-git', title: 'New project' });
  // Each host's picker words ("Forgejo / Gitea"). openAddProjectFlow: with no connected environment, Settings › Connections.
  const hostWords = sourceControlHosts.flatMap(host => host.pickerLabel.toLowerCase().split(/[^a-z0-9]+/u).filter(Boolean));
  action('add-project', ['add project', 'folder', 'directory', 'browse', 'clone', 'remote', 'repository', 'repo', 'git', ...hostWords, 'url', 'environment'],
    addProjectEnvironments(client).length ? { op: 'page', arg: 'add-project', icon: 'folder-plus', title: 'Add project' } : { op: 'settings', arg: 'connections', icon: 'folder-plus', title: 'Add project' });
  action('change-theme', ['change theme', 'appearance', 'colors', 'palette'], { kind: 'submenu', op: 'page', arg: 'theme', icon: 'palette', title: 'Change theme', shortcut: commandShortcut(client, 'theme.select') });
  action('change-appearance', ['change appearance', 'light', 'dark', 'system', 'mode', 'toggle'], { kind: 'submenu', op: 'page', arg: 'appearance', icon: 'monitor', title: 'Change appearance', shortcut: commandShortcut(client, 'appearance.cycle') });
  action('theme-editor', ['theme', 'appearance', 'colors', 'palette', 'customize'], { op: 'theme-editor', icon: 'palette', title: 'Toggle theme editor', shortcut: commandShortcut(client, 'themeEditor.toggle') });
  // Every host's own words for a change request ("merge requests", "mrs") and its name.
  const changeRequestWords = [...new Set(sourceControlHosts.flatMap(host => {
    const shortLabel = host.changeRequest.shortLabel.toLowerCase();
    return [host.changeRequest.plural, `${shortLabel}s`, shortLabel, ...host.label.toLowerCase().split(/[^a-z0-9]+/u)].filter(Boolean);
  }))];
  if (capabilities(client).pullRequests === true) action('pull-requests', [...changeRequestWords, 'review', 'merge', 'branch'], { op: 'pull-requests', icon: 'pull-request', title: 'Open pull requests' });
  action('usage', ['usage', 'use', 'tokens', 'cost', 'spend', 'limits', 'stats', 'analytics'], { op: 'usage', icon: 'chart', title: 'Open usage', shortcut: commandShortcut(client, 'usage.open') });
  // The workspace areas F6 moves between (shell-members.contract workspaceMove), after the palette has closed.
  for (const [direction, title, command] of [['next', 'Focus Next Workspace Area', 'workspace.focusNext'], ['previous', 'Focus Previous Workspace Area', 'workspace.focusPrevious']] as const) {
    action(`workspace-focus-${direction}`, [title, 'accessibility', 'navigation', 'focus', 'F6'], { op: 'workspace-focus', arg: direction, icon: 'monitor', title, shortcut: commandShortcut(client, command) });
  }
  action('settings', ['settings', 'preferences', 'configuration', 'keybindings'], { op: 'settings', arg: 'general', icon: 'settings', title: 'Open settings' });
  if (current) action('project-settings', ['project', 'settings', 'name', 'icon', 'scripts', 'model', 'workspace', 'grouping', 'checkout', 'remove', 't3.json'],
    { op: 'project-settings', arg: str(current.id), icon: 'folder', title: 'Project settings', description: str(current.title) });
  return items;
}

/**
 * The New thread in... page (projectThreadItems): the sidebar's project order with the
 * current project first (buildSidebarProjectPickerEntries). enumerateCommandPaletteItems numbers the
 * first nine projects by thread.jump.N, whose rules hold only `isDesktop`: this is the desktop build,
 * so each shows its ⌘N and the field runs it by that chord (CommandPalette.tsx handleKeyDown).
 * The Scratch project is "No project", right after the current project (first when the current one
 * is the Scratch project), with its own chat.newWithoutProject shortcut, so the projects' numbers hold.
 */
export function newThreadInItems(client: T3Client): Item[] {
  const root = scratchRoot(client.config), picked = pickerProjects(client);
  const ordered = picked.filter(project => !isScratchProject(project, root));
  const items = enumerateJumps(client, projectItems(client, ordered, 'new-thread-in', 'new-thread'));
  if (scratchTarget(client) === null) return items;
  const at = picked[0] !== undefined && isScratchProject(picked[0], root) ? 0 : 1;
  items.splice(at, 0, { terms: ['No project', 'no project', 'without project', 'none'],
    row: row({ key: 'new-thread-in:no-project', op: 'flow', arg: 'scratch', icon: 'message-square-dashed', title: 'No project', shortcut: commandShortcut(client, 'chat.newWithoutProject'), jump: fieldChords(client, 'chat.newWithoutProject') }) });
  return items;
}
/**
 * CommandPalette.tsx handleKeyDown: a chord that resolves to a thread.jump.N command is the field's, whether or
 * not a displayed row carries it (⌘5 with two projects, ⌘1 on the root page): it is prevented and stopped, and
 * it runs the row that has it, if one does. These are those chords, in the palette's context.
 */
export function threadJumpChords(client: T3Client): string[] {
  const winners = chordWinners(arr(client.config.keybindings), FIELD_CONTEXT);
  return [...winners].filter(([, command]) => /^thread\.jump\.[1-9]$/.test(command)).map(([chord]) => chord);
}
/** enumerateCommandPaletteItems: the Nth row (N ≤ 9) takes thread.jump.N's label and chord. */
export function enumerateJumps(client: T3Client, items: Item[]): Item[] {
  return items.map((item, index) => {
    const command = index < 9 ? `thread.jump.${index + 1}` : '';
    return { ...item, row: { ...item.row, shortcut: command ? commandShortcut(client, command) : '', jump: command ? fieldChords(client, command) : '' } };
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
  const mode = str(obj(client.local.deviceSettings).appearanceMode, 'system');
  return [['system', 'System', 'monitor'], ['light', 'Light', 'sun'], ['dark', 'Dark', 'moon']].map(([value, label, icon]) => ({ terms: [label!, 'appearance', 'mode'],
    row: row({ key: `appearance:${value}`, op: 'device', arg: 'appearanceMode', arg2: value!, icon: icon!, title: label!, trailing: mode === value ? 'Current' : '' }) }));
}
/**
 * The element a Settings result opens its page at, by the Settings search's own rule (settings-search.ts searchTarget,
 * upstream's `targetId ?? id`): a keybinding command's row, or with no default key the Keybindings page.
 */
export function settingTarget(item: { target: string }): string {
  return searchTarget(item);
}
/** The settings a query can find, ranked by the palette's own matcher. */
export function settingsItems(client: T3Client, query: string): Item[] {
  const context = searchContext(client.config, client.ready, client.projectId ? 'environment' : 'all');
  return searchSettings(query, context).map(item => ({ ...(item.secondary ? { secondary: true } : {}), terms: [item.title, breadcrumbLabel(item.route), ...item.terms],
    row: row({ key: `setting:${item.id}`, op: 'settings', arg: item.route, arg2: settingTarget(item), icon: 'settings', title: item.title, description: `Settings · ${breadcrumbLabel(item.route)}` }) }));
}

/**
 * `matches` are the servers' message matches, one per thread keyed by its row's id, found for `matchQuery`;
 * `linkedThreads`, a pull request's linked threads, take the thread results' place while the query is its URL.
 * `searching` says message results are still on their way.
 */
export type CommandContext = { page: string; query: string; now: number; scheme: string; matches: Map<string, Obj>; matchQuery: string; searching: boolean; linkedThreads?: Item[] | null };
/** A submenu's page: New thread in..., Change theme, Change appearance. */
function submenuGroups(client: T3Client, page: string, scheme: string): Group[] {
  return page === 'new-thread-in' ? [{ value: 'projects', label: 'Projects', items: newThreadInItems(client) }]
    : page === 'theme' ? [{ value: 'themes', label: 'Change theme', items: themeItems(client, scheme) }]
      : page === 'appearance' ? [{ value: 'appearance', label: 'Change appearance', items: appearanceItems(client) }] : [];
}
/**
 * The root page's groups and what its query searches besides them. `threadQuery` is the one a thread's pull request
 * badge shows for: the root query, not a submenu's or an actions filter's.
 */
function rootSearch(client: T3Client, query: string, threadQuery: string, now: number, scheme: string): { root: Group[]; extra: SearchExtra } {
  const actions = rootActions(client);
  const threads = threadItems(client, now, threadQuery);
  const recent = threads.filter(item => item.row.arg !== client.threadId)
    .sort((left, right) => (right.thread ? threadRecentUse(right.thread) : 0) - (left.thread ? threadRecentUse(left.thread) : 0)).slice(0, RECENT_THREAD_LIMIT);
  const root = [{ value: 'actions', label: 'Actions', items: actions }, { value: 'recent-threads', label: 'Recent Threads', items: recent }].filter(group => group.items.length);
  // A query also searches the submenus' options; an empty one shows the groups as they are.
  const searched = query.trim().length > 0;
  const nested = searched ? nestedItems(actions.map(item => item.row.kind === 'submenu' ? { ...item, children: submenuGroups(client, item.row.arg, scheme) } : item), query) : [];
  return { root, extra: { projects: projectItems(client, pickerProjects(client), 'project', 'open-project', true), settings: settingsItems(client, query), threads, now, ...(searched ? { nested, root } : {}) } };
}
/** What a submenu outside this module (an add-project list) searches when nothing in it matches: the root's results. */
export function rootFallback(client: T3Client, query: string, now: number, scheme: string): SearchExtra {
  return query.trim().length > 0 ? rootSearch(client, query, '', now, scheme).extra : { projects: [], settings: [], threads: [] };
}
/**
 * The command palette's root and its submenus (not the add-project flow). A query also finds the submenus' options,
 * and in a submenu where nothing matches it searches the root. The root's Recent Threads are the ones used last,
 * written in or opened, without the open thread, which would always lead.
 */
export function commandView(client: T3Client, context: CommandContext): PaletteView {
  const page = context.page, query = context.query;
  const submenu = page !== '';
  const { root, extra } = rootSearch(client, query, submenu || query.startsWith('>') ? '' : query, context.now, context.scheme);
  const filtered = filterGroups(submenu ? submenuGroups(client, page, context.scheme) : root, query, submenu, { ...extra, threads: context.linkedThreads ?? extra.threads,
    messages: context.linkedThreads ? [] : messageItems(extra.threads, context.matches, context.matchQuery, context.now) });
  const rows = flatten(filtered);
  const actionsOnly = query.startsWith('>');
  return { ...closedView, open: true, page, parent: '', back: submenu, addon: submenu ? 'back' : 'search', placeholder: submenu ? 'Search...' : 'Search commands, projects, and threads...',
    rows, count: rows.filter(entry => entry.index >= 0).length, backHint: submenu, enterLabel: 'Select',
    empty: rows.length ? '' : context.searching ? 'Searching thread messages…' : actionsOnly ? 'No matching actions.' : 'No matching commands, projects, or threads.' };
}
