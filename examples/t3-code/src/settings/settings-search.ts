// Settings navigation and search. Reference: SettingsSidebarNav.tsx and settingsSearch.ts (7c3b3b3605ef): the nav
// lists SETTINGS_NAV_GROUPS in order; a query replaces the list with ranked catalog results whose availability and
// owning scope match this client.
import { CATALOG } from './settings-catalog';
import { commandLabel } from './keybinding-view'; // settingsSearch.ts imports KeybindingsSettings.logic's
import { keybindingDefaults } from './keybinding-settings';
import { arr, obj, str, type Obj } from '../client/domain';
import { canManageLocalBackend, primary } from '../connections/local-primary';

type Page = readonly [route: string, label: string, icon: string];
/** SETTINGS_NAV_GROUPS: a group without a heading is set apart by space alone. */
export const NAV_GROUPS: readonly { heading: string; pages: readonly Page[] }[] = [
  { heading: '', pages: [['general', 'General', 'settings-2'], ['appearance', 'Appearance', 'palette'], ['chat', 'Chat', 'message-square-text'],
    ['diffs', 'Diffs & pull requests', 'file-diff'], ['keybindings', 'Keybindings', 'keyboard'], ['snap-shot', 'SnapShots', 'snap-shot']] },
  { heading: 'Agents', pages: [['providers', 'Providers', 'bot'], ['threads', 'Threads', 'messages-square'], ['browser', 'Browser', 'globe'],
    ['devices', 'Devices', 'smartphone'], ['scheduled-tasks', 'Scheduled tasks', 'calendar-clock']] },
  { heading: 'Code', pages: [['projects', 'Projects', 'panels-top-left'], ['source-control', 'Source control', 'git-branch'], ['worktrees', 'Worktrees', 'folder-tree']] },
  { heading: 'Connections', pages: [['connections', 'Environments', 'server'], ['access', 'Sharing & access', 'shield-check']] },
  { heading: '', pages: [['maintenance', 'Maintenance', 'wrench'], ['labs', 'Labs', 'flask-conical'], ['about', 'About', 'info'], ['archived', 'Archive', 'archive']] },
];
export const SECTION_LABELS: readonly Page[] = NAV_GROUPS.flatMap(group => group.pages);
/** The pages drawn from settings-core's sections (General's rows before the split, and Appearance); Worktrees' and
 *  Maintenance's are drawn by the storage panel among its own. */
export const CORE_ROUTES: ReadonlySet<string> = new Set(['general', 'appearance', 'chat', 'diffs', 'threads', 'worktrees', 'maintenance', 'about', 'labs']);
const BREADCRUMB: Record<string, string> = { ...Object.fromEntries(SECTION_LABELS.map(([route, label]) => [route, label])), diagnostics: 'Diagnostics', 'open-source-licenses': 'Open source licenses' };
export const breadcrumbLabel = (route: string) => BREADCRUMB[route] ?? 'Settings';
const CATEGORY_SCOPE: Record<string, string> = { diagnostics: 'environment', projects: 'project', 'source-control': 'environment-defaults', worktrees: 'project-defaults', connections: 'connections', access: 'connections', archived: 'project-defaults' };
/**
 * Every search target is its row's or section's own id, the catalog's `targetId ?? id` (upstream's `hash`), and a
 * keybinding result's is its command's row, `keybinding-<command>`, or with no default key the Keybindings section: the
 * page puts each on the row it names. ⌘K jumps by the same targets (palette.ts settingTarget). These targets sit
 * under an Advanced fold (AdvancedSettings `targetIds`), by the fold's section id: a jump to one opens the fold first
 * (settings-advanced.contract), and lands on its stop while the row is not drawn yet. A source control host's
 * settings sit in its row's details, which open the same way (DiscoveryItemRow's `targetsThisRow`).
 */
export const FOLDED_TARGETS: Readonly<Record<string, string>> = {
  'skills-in-slash-menu': 'composer', 'response-streaming': 'text-generation', 'thread-title-instructions': 'text-generation',
  'worktree-submodules': 'worktree-creation', 'storage-worktrees': 'storage-worktrees', 'storage-worktree-keep-when': 'storage-worktrees',
  'follow-change-request-templates': 'source-control-text-generation', 'source-control-writer-model': 'source-control-text-generation',
  'browser-default-viewport': 'browser', 'browser-default-zoom': 'browser', 'browser-default-appearance': 'browser',
  'panel-animations': 'appearance-interface',
  'composer-context': 'appearance-interface', 'separate-fonts': 'typography', 'font-smoothing': 'typography',
};
/** The reveal stop of the fold a jump target sits under, '' for one under none. */
export const foldStopOf = (target: string) => FOLDED_TARGETS[target] ? `settings-advanced-${FOLDED_TARGETS[target]}-reveal`
  : target.startsWith('source-control-host-') ? `${target}-reveal` : '';
/** A result's jump target: the id of the row or section it names. */
export const searchTarget = (item: { target: string }): string => item.target;

export function normalizeSearchText(text: string) {
  return text.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase().replace(/\s+/g, ' ').trim();
}
/** isSettingsSearchScopeAvailable: where a row can be edited, given the selected scope's kind. */
export function scopeAvailable(required: string, kind: string) {
  switch (required) {
    case '': case 'connections': return true;
    case 'environment': case 'checkout': return required === kind;
    case 'project': return kind === 'project' || kind === 'checkout';
    case 'environment-defaults': return kind === 'environment' || kind === 'all';
    case 'project-defaults': return ['environment', 'all', 'project', 'checkout'].includes(kind);
    default: return true;
  }
}
export { commandLabel };

/** `localBackend`: canManageLocalBackend; `localEnvironment`: the Local environment switch is on. */
export type SearchContext = { connected: boolean; autoSettle: boolean; scopeKind: string; keybindings?: Obj[]; localBackend?: boolean; localEnvironment?: boolean };
export type SettingsItem = { id: string; title: string; route: string; target: string; terms: string[]; scope: string; flags: string[]; secondary: boolean };
/** settingsForThisPlatform over the catalog: the settings this client offers, each with its jump target. ⌘K ranks these too. */
export function settingsCatalog(context: SearchContext): SettingsItem[] {
  const items: SettingsItem[] = CATALOG.map(([id, title, route, targetId, terms, scope, flags]) => ({ id, title, route, target: targetId || id, terms: terms ? [terms] : [], scope, flags: flags ? flags.split(',') : [], secondary: false }));
  // KEYBINDING_SEARCH_ITEMS: one per command the server binds, alphabetical, default keys searchable; a command with no
  // default key lands on the Keybindings section.
  const commands = new Map<string, string[]>();
  for (const binding of arr(context.keybindings)) { const command = str(binding.command); if (command) commands.set(command, [...(commands.get(command) || []), str(binding.key)].filter(Boolean)); }
  const keybindings = [...commands].map(([command, keys]) => ({ id: `keybinding-${command}`, title: commandLabel(command), route: 'keybindings',
    target: keybindingDefaults.some(binding => binding.command === command) ? `keybinding-${command}` : 'keybindings', terms: [command, ...keys], scope: '', flags: [], secondary: true }))
    .sort((a, b) => a.title.localeCompare(b.title));
  // filterAvailableSettingsSearchItems on the macOS desktop: desktop and mac rows always, Windows, WSL and T3 Connect
  // (cloud) rows never; local-backend rows while this machine can be managed, local-environment rows while it is on.
  return [...items, ...keybindings].filter(item => item.flags.every(flag => flag === 'mac' || flag === 'desktop' || (flag === 'environment' || flag === 'providerSettings' || flag === 'macProviderSettings' ? context.connected
    : flag === 'autoSettle' ? context.autoSettle : flag === 'localBackend' ? context.localBackend === true : flag === 'localEnvironment' ? context.localEnvironment !== false : false)));
}
/** searchSettings: every token in some field, ranked exact > prefix > substring > all tokens in title > phrase. */
export function searchSettings(query: string, context: SearchContext) {
  const normalized = normalizeSearchText(query);
  if (!normalized) return [];
  const tokens = normalized.split(' ');
  return settingsCatalog(context).flatMap((item, index) => {
    const title = normalizeSearchText(item.title);
    const fields = [title, normalizeSearchText(BREADCRUMB[item.route] ?? ''), ...item.terms.map(normalizeSearchText)];
    if (!tokens.every(token => fields.some(field => field.includes(token)))) return [];
    const phrase = fields.some(field => field.includes(normalized));
    const rank = title === normalized ? 5 : title.startsWith(normalized) ? 4 : title.includes(normalized) ? 3 : tokens.every(token => title.includes(token)) ? 2 : phrase ? 1 : 0;
    return [{ item, index, rank }];
  }).sort((a, b) => Number(a.item.secondary) - Number(b.item.secondary) || b.rank - a.rank || a.index - b.index).map(({ item }) => item);
}
export function searchTargetScope(id: string) {
  const item = CATALOG.find(entry => entry[0] === id) ?? CATALOG.find(entry => entry[3] === id);
  return item ? { title: item[1], scope: item[5] || CATEGORY_SCOPE[item[2]] || '' } : null;
}

export function settingsNavigation(query: string, context: SearchContext, activeIndex = 0) {
  const searching = normalizeSearchText(query) !== '';
  const results = searchSettings(query, context);
  const active = results.length ? ((activeIndex % results.length) + results.length) % results.length : 0;
  const items = searching
    ? results.map((item, index) => ({ id: `search-result-${item.id}`, route: item.route, target: searchTarget(item),
        title: item.title, section: BREADCRUMB[item.route] ?? '', icon: SECTION_LABELS.find(([route]) => route === item.route)?.[2] ?? 'settings-2', scope: item.scope || CATEGORY_SCOPE[item.route] || '', active: index === active,
        heading: '', groupStart: false }))
    : NAV_GROUPS.flatMap((group, at) => group.pages.map(([route, label, icon], index) => ({ id: route, route, target: '', title: label, section: '', icon, scope: '', active: false,
        heading: index === 0 ? group.heading : '', groupStart: index === 0 && at > 0 })));
  const chosen = searching ? items[active] : undefined;
  return { items, searching, count: items.length, firstRoute: chosen?.route || '', firstTarget: chosen?.target || '', firstId: chosen ? chosen.id : '' };
}
export const searchContext = (config: Obj, connected: boolean, scopeKind: string): SearchContext =>
  ({ connected, scopeKind, autoSettle: obj(obj(config.environment).capabilities).threadAutoSettlement === true, keybindings: arr(config.keybindings),
    localBackend: canManageLocalBackend(), localEnvironment: !primary.disabled });
