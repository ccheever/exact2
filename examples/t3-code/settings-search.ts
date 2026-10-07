// Settings navigation and search (lane settings-core). Reference: SettingsSidebarNav.tsx and
// settingsSearch.ts — the sidebar lists SETTINGS_SECTION_LABELS in order (Project only for a
// project/checkout scope); a query replaces the list with ranked catalog results whose
// availability and owning scope match this client.
import { CATALOG } from './settings-catalog';
import { arr, obj, str, type Obj } from './domain';
import { canManageLocalBackend, primary } from './local-primary';

export const SECTION_LABELS: readonly (readonly [string, string, string])[] = [
  ['projects', 'Project', 'panels-top-left'], ['general', 'General', 'settings-2'], ['appearance', 'Appearance', 'palette'], ['keybindings', 'Keybindings', 'keyboard'],
  ['snap-shot', 'SnapShots', 'snap-shot'], ['providers', 'Providers', 'bot'], ['integrations', 'Integrations', 'blocks'], ['scheduled-tasks', 'Scheduled Tasks', 'calendar-clock'],
  ['source-control', 'Source Control', 'git-branch'], ['storage', 'Storage', 'hard-drive'], ['connections', 'Connections', 'link-2'], ['archived', 'Archive', 'archive'],
];
const BREADCRUMB: Record<string, string> = { ...Object.fromEntries(SECTION_LABELS.map(([route, label]) => [route, label])), diagnostics: 'Diagnostics', 'open-source-licenses': 'Open source licenses' };
export const breadcrumbLabel = (route: string) => BREADCRUMB[route] ?? 'Settings';
const CATEGORY_SCOPE: Record<string, string> = { projects: 'project', 'source-control': 'environment-defaults', storage: 'project-defaults', connections: 'connections', archived: 'project-defaults' };
// Rows in other lanes' routes that a result can focus; everything else lands on its route.
const NATIVE_TARGETS: Record<string, string> = {
  archive: 'refresh-archive', 'storage-worktrees': 'storage-toggle-worktreeOnDelete', 'storage-artifacts': 'storage-toggle-browserArtifactsAfterDays',
  'remote-environments': 'manage-connection', 'connections-environment': 'manage-connection', keybindings: 'keybindings-filter',
};

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
const segment = (part: string) => part.replace(/([a-z0-9])([A-Z])/g, '$1 $2').split(/[-_\s]+/).filter(Boolean).map(word => word[0]!.toUpperCase() + word.slice(1)).join(' ');
export function commandLabel(command: string) {
  const special: Record<string, string> = { 'composer.sendAlternate': 'Composer: Opposite Queue or Steer Action', 'composer.sendBackground': 'Composer: Start in Background', 'composer.sendAndNewThread': 'Composer: Send and Start New Thread',
    'thread.steerQueuedMessage': 'Queue: Send First Queued Message as Steer', 'thread.editQueuedMessage': 'Queue: Edit Last Queued Message', 'thread.copyReference': 'Pull Request: Copy Link or Thread ID' };
  if (special[command]) return special[command]!;
  if (command.startsWith('script.') && command.endsWith('.run')) return `Run Script: ${segment(command.slice(7, -4))}`;
  return command.split('.').map(segment).join(': ');
}

/** `localBackend`: canManageLocalBackend; `localEnvironment`: the Local environment switch is on (20261005-local-primary-environment). */
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
    ? results.map((item, index) => ({ id: `search-result-${item.id}`, route: item.route, target: NATIVE_TARGETS[item.id] ?? (item.route === 'general' || item.route === 'appearance' ? `setting-${item.target}` : ''),
        title: item.title, section: BREADCRUMB[item.route] ?? '', icon: SECTION_LABELS.find(([route]) => route === item.route)?.[2] ?? 'settings-2', scope: item.scope || CATEGORY_SCOPE[item.route] || '', active: index === active }))
    : SECTION_LABELS.filter(([route]) => route !== 'projects' || context.scopeKind === 'project' || context.scopeKind === 'checkout')
        .map(([route, label, icon]) => ({ id: route, route, target: '', title: label, section: '', icon, scope: '', active: false }));
  const chosen = searching ? items[active] : undefined;
  return { items, searching, count: items.length, firstRoute: chosen?.route || '', firstTarget: chosen?.target || '', firstId: chosen ? chosen.id : '' };
}
export const searchContext = (config: Obj, connected: boolean, scopeKind: string): SearchContext =>
  ({ connected, scopeKind, autoSettle: obj(obj(config.environment).capabilities).threadAutoSettlement === true, keybindings: arr(config.keybindings),
    localBackend: canManageLocalBackend(), localEnvironment: !primary.disabled });
