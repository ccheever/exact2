// Settings navigation and search. Reference: SettingsSidebarNav.tsx and settingsSearch.ts (7c3b3b3605ef): the nav
// lists SETTINGS_NAV_GROUPS in order; a query replaces the list with ranked catalog results whose availability and
// owning scope match this client.
import { CATALOG } from './settings-catalog';
import { commandLabel } from './keybinding-view'; // settingsSearch.ts imports KeybindingsSettings.logic's
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
/** The pages drawn from settings-core's sections (General's rows before the split, and Appearance); on Worktrees and
 *  Maintenance the storage panel's rows (`storage-…`) are its own, with no `setting-` target. */
export const CORE_ROUTES: ReadonlySet<string> = new Set(['general', 'appearance', 'chat', 'diffs', 'threads', 'worktrees', 'maintenance', 'about', 'labs']);
const BREADCRUMB: Record<string, string> = { ...Object.fromEntries(SECTION_LABELS.map(([route, label]) => [route, label])), diagnostics: 'Diagnostics', 'open-source-licenses': 'Open source licenses' };
export const breadcrumbLabel = (route: string) => BREADCRUMB[route] ?? 'Settings';
const CATEGORY_SCOPE: Record<string, string> = { diagnostics: 'environment', projects: 'project', 'source-control': 'environment-defaults', worktrees: 'project-defaults', connections: 'connections', access: 'connections', archived: 'project-defaults' };
// Rows in other lanes' routes that a result can focus; everything else lands on its route.
const NATIVE_TARGETS: Record<string, string> = {
  archive: 'refresh-archive', 'storage-worktrees': 'storage-toggle-worktreeOnDelete', 'storage-artifacts': 'storage-toggle-browserArtifactsAfterDays',
  'storage-worktree-keep-when': 'storage-worktree-keep-when', 'worktree-branch-naming': 'scoped-branchNamingMode',
  'browser-default-viewport': 'browser-default-viewport', 'browser-default-zoom': 'browser-default-zoom', 'browser-default-appearance': 'browser-default-appearance',
  'follow-change-request-templates': 'scoped-sourceControlWritingStyle.followChangeRequestTemplates', 'source-control-writer-model': 'setting-source-control-writer-model',
  'remote-environments': 'manage-connection', 'connections-environment': 'manage-connection', keybindings: 'keybindings-filter',
};
/**
 * The results whose rows sit under an Advanced fold (AdvancedSettings `targetIds`), by the fold's section id. Their
 * target is "<the fold's reveal stop> <the row's own target>": the jump lands on the row while it is drawn, else on
 * the stop, whose focus opens the fold (settings-advanced.contract, settings-members.contract focusSettingsTarget).
 */
export const FOLDED_TARGETS: Readonly<Record<string, string>> = {
  'skills-in-slash-menu': 'composer', 'response-streaming': 'text-generation', 'thread-title-instructions': 'text-generation',
  'worktree-submodules': 'worktree-creation', 'storage-worktree-keep-when': 'storage-worktrees',
  'follow-change-request-templates': 'source-control-text-generation', 'source-control-writer-model': 'source-control-text-generation',
  'browser-default-viewport': 'browser', 'browser-default-zoom': 'browser', 'browser-default-appearance': 'browser',
  'panel-animations': 'appearance-interface', 'environment-identification': 'appearance-interface', 'composer-context': 'appearance-interface',
  'separate-fonts': 'typography', 'font-smoothing': 'typography',
};
const rowTarget = (item: { id: string; route: string; target: string }) =>
  NATIVE_TARGETS[item.id] ?? (CORE_ROUTES.has(item.route) && !item.id.startsWith('storage-') ? `setting-${item.target}` : '');
/** A result's jump target: its row's, behind its fold's reveal stop when it has one. */
export function searchTarget(item: { id: string; route: string; target: string }): string {
  const fold = FOLDED_TARGETS[item.id], target = rowTarget(item);
  return fold ? [`settings-advanced-${fold}-reveal`, target].filter(Boolean).join(' ') : target;
}

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
type Item = { id: string; title: string; route: string; target: string; terms: string[]; scope: string; flags: string[]; secondary: boolean };
function catalog(context: SearchContext): Item[] {
  const items: Item[] = CATALOG.map(([id, title, route, targetId, terms, scope, flags]) => ({ id, title, route, target: targetId || id, terms: terms ? [terms] : [], scope, flags: flags ? flags.split(',') : [], secondary: false }));
  // KEYBINDING_SEARCH_ITEMS: one per command the server binds, alphabetical, default keys searchable.
  const commands = new Map<string, string[]>();
  for (const binding of arr(context.keybindings)) { const command = str(binding.command); if (command) commands.set(command, [...(commands.get(command) || []), str(binding.key)].filter(Boolean)); }
  const keybindings = [...commands].map(([command, keys]) => ({ id: `keybinding-${command}`, title: commandLabel(command), route: 'keybindings', target: 'keybindings', terms: [command, ...keys], scope: '', flags: [], secondary: true }))
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
  return catalog(context).flatMap((item, index) => {
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
