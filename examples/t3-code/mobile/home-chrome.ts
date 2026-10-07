// @ref llp/1107.002-design-system-parity.spec.md#semantic-colors
// Upstream 365aa87982 HomeHeader, home-list-filter-menu, WorkspaceConnectionTitle,
// ThreadNavigationSidebar and the app-owned react-native-screens patch.
import { mobileHomeProjects, mobileHomeSources, type HomeOptions, type HomeSource } from './home';
import { mobileTheme } from './design';
import { logicalKey } from './shared/r6-polish-groups';

export interface HomeChromeEnvironment { environmentId: string; label: string }
export interface HomeWorkspaceFacts {
  networkStatus?: 'online' | 'offline' | 'unknown';
  connectionError: string | null;
  connectingEnvironments: { environmentLabel: string }[];
  hasConnectingEnvironment: boolean;
  hasPendingShellSnapshot: boolean;
  hasLoadedShellSnapshot: boolean;
  hasReadyEnvironment: boolean;
}
export function homeConnectionTitle(state?: HomeWorkspaceFacts) {
  if (!state || !(state.networkStatus === 'offline' || state.connectionError !== null
    || state.hasConnectingEnvironment || state.hasPendingShellSnapshot
    || (state.hasLoadedShellSnapshot && !state.hasReadyEnvironment))) return { label: '', progress: false };
  const connecting = state.connectingEnvironments;
  const label = state.networkStatus === 'offline' ? 'You are offline'
    : connecting.length === 1 ? `Reconnecting to ${connecting[0]!.environmentLabel}`
    : connecting.length > 1 ? `Reconnecting ${connecting.length} environments`
    : state.connectionError !== null ? state.connectionError
    : state.hasPendingShellSnapshot ? state.hasLoadedShellSnapshot ? 'Syncing threads...' : 'Loading threads...'
    : 'Not connected';
  return { label, progress: state.networkStatus !== 'offline' && state.connectionError === null
    && (connecting.length > 0 || state.hasPendingShellSnapshot) };
}

/** Root owns these values, search RPCs and navigation. No credentials or sample rows. */
export interface HomeChromeInput {
  routeKey: string;
  layout: 'compact' | 'sidebar';
  query: string;
  environmentId?: string;
  projectKey?: string;
  groupingMode?: HomeOptions['groupingMode'];
  scheme?: string;
  palette?: string;
  safeBottom?: number;
  liquidGlassSupported?: boolean;
  environments: HomeChromeEnvironment[];
  workspace?: HomeWorkspaceFacts;
}
export function mobileHomeChrome(input: HomeChromeInput, sources: HomeSource[] = mobileHomeSources()) {
  const environments = input.environments.map(value => ({ ...value })).sort((a, b) => a.label.localeCompare(b.label));
  const environmentId = environments.some(value => value.environmentId === input.environmentId) ? input.environmentId! : '';
  // The menu uses buildHomeProjectScopes insertion order, not the list's activity sort.
  const grouping = { environmentId: '', shell: { projects: [] }, local: {
    groupingMode: input.groupingMode ?? 'repository', groupingOverrides: {} } };
  const order = new Map<string, number>();
  for (const source of sources) {
    if (environmentId && source.environmentId !== environmentId) continue;
    for (const project of source.shell.projects) {
      const key = logicalKey(grouping, source.environmentId, project);
      if (!order.has(key)) order.set(key, order.size);
    }
  }
  const projects = mobileHomeProjects(sources, { environmentId, groupingMode: input.groupingMode })
    .sort((a, b) => (order.get(a.key) ?? 0) - (order.get(b.key) ?? 0))
    .map(project => ({ key: project.key, label: project.title }));
  const projectKey = projects.some(project => project.key === input.projectKey) ? input.projectKey! : '';
  const theme = mobileTheme(input.scheme ?? 'light', input.palette);
  const configuration = { routeKey: input.routeKey, layout: input.layout, query: input.query,
    environmentId, projectKey, environments, projects, status: homeConnectionTitle(input.workspace),
    colors: { foreground: theme.foreground, muted: theme.colors.muted, iconMuted: theme.colors.iconMuted, subtle: theme.colors.subtle } };
  return { configuration: JSON.stringify(configuration), environmentId, projectKey,
    bottomClearance: input.layout === 'compact' ? Math.max(input.safeBottom ?? 0, 24) + 96
      + (input.liquidGlassSupported === false ? 44 : 0) : Math.max(input.safeBottom ?? 0, 16) + 16,
    emptyClearance: input.layout === 'compact' ? Math.max(input.safeBottom ?? 0, 24)
      + (input.liquidGlassSupported === false ? 44 : 0) : Math.max(input.safeBottom ?? 0, 16) + 16,
    toolbarContentInset: input.layout === 'compact' ? 56 : 0 };
}

export type HomeChromeEvent =
  | { kind: 'search' | 'environment' | 'project'; value: string }
  | { kind: 'settings' | 'environments' | 'compose'; value: '' };
/** Decode only known native actions. Scope values are validated again by mobileHomeChrome. */
export function decodeHomeChromeEvent(text: string): HomeChromeEvent | null {
  let value: unknown;
  try { value = JSON.parse(text); } catch { return null; }
  if (!value || typeof value !== 'object') return null;
  const event = value as Record<string, unknown>;
  if (typeof event.value !== 'string') return null;
  if (event.kind === 'search' || event.kind === 'environment' || event.kind === 'project') return { kind: event.kind, value: event.value };
  if (event.kind === 'settings' || event.kind === 'environments' || event.kind === 'compose') return { kind: event.kind, value: '' };
  return null;
}
