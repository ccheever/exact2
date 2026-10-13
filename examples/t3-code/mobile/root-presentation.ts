import { mobileAccountSettings } from './settings-account';
// Root adapters over mobile presentation and the shared client, upstream365aa87982.
// @ref llp/1109.004-home-projection.decision.md#decision
import { normalizeMobilePreferences, resolveMobileAppearance } from './settings-preferences';
import { mobileSettingsScope, settingsRootView, settingsHeaderConfiguration, decodeSettingsHeaderEvent, type SettingsScopeSelection } from './settings';
import { arr, obj, str } from './shared/domain';
import { fleet } from './shared/settings-b-fleet';
import { mobileClient } from './client';
import { MOBILE_SERVER_ROUTES } from './settings-server';
import { mobileHomeChrome, decodeHomeChromeEvent } from './home-chrome';

export function mobileLayoutFacts(input: string) {
  let value: Record<string, unknown> = {};
  try { value = obj(JSON.parse(input)); } catch { /* Bake has no UIKit geometry. */ }
  return { safeTop: typeof value.safeTop === 'number' && Number.isFinite(value.safeTop) ? Math.max(0, value.safeTop) : 0, safeBottom: typeof value.safeBottom === 'number' && Number.isFinite(value.safeBottom)
    ? Math.max(0, value.safeBottom) : 0, liquidGlass: value.liquidGlass === true, keyboardDocked: value.keyboardDocked === true,
    displayScale: typeof value.displayScale === 'number' && Number.isFinite(value.displayScale) && value.displayScale > 0 ? value.displayScale : 1,
    windowHeight: typeof value.windowHeight === 'number' && Number.isFinite(value.windowHeight) && value.windowHeight > 0 ? value.windowHeight : 0 };
}

export function homeChromeEvent(input: string) {
  return decodeHomeChromeEvent(input) ?? { kind: '', value: '' };
}

export function homeChromeView(args: unknown[]) {
  const [routeKey, query, environmentId, projectKey, scheme, safeBottom, liquidGlass, _revision, rows, groupingMode, layout] = args;
  const environments = arr(rows).map(row => ({ environmentId: str(row.id).split('\n')[1] ?? '', label: str(row.label) }))
    .filter(row => row.environmentId);
  const entries = [...fleet.entries.values()];
  const connectingEnvironments = arr(rows).filter(row => row.retrying === true).map(row => ({ environmentLabel: str(row.label) }));
  return mobileHomeChrome({ routeKey: str(routeKey), query: str(query), environmentId: str(environmentId), projectKey: str(projectKey),
    layout: layout === 'sidebar' ? 'sidebar' : 'compact', groupingMode: str(groupingMode), scheme: str(scheme), safeBottom: Number(safeBottom) || 0, liquidGlassSupported: liquidGlass === true,
    environments, workspace: {
      connectionError: mobileClient.error || null, connectingEnvironments,
      hasConnectingEnvironment: connectingEnvironments.length > 0,
      hasPendingShellSnapshot: mobileClient.connection === 'connected' && !mobileClient.shellLoaded
        || entries.some(entry => entry.phase === 'connected' && entry.synchronized !== entry.generation),
      hasLoadedShellSnapshot: mobileClient.shellLoaded || entries.some(entry => entry.synchronized === entry.generation),
      hasReadyEnvironment: mobileClient.ready || entries.some(entry => entry.phase === 'connected' && entry.synchronized === entry.generation),
    } });
}

// @ref llp/1109.002-design-system-parity.spec.md#user-preference-and-accessibility-scaling
export function settingsRoot(args: unknown[]) {
  const [serialized, systemScheme, rows, safeBottom, routeKey, selection, , projectRouteKey, compactRoot, visitToken] = args;
  const preferences = normalizeMobilePreferences(serialized);
  const appearance = resolveMobileAppearance(preferences, str(systemScheme));
  const scope = settingsScope(rows, selection, preferences.projectGroupingMode);
  return { visitToken: str(visitToken), root: settingsRootView({ ...mobileAccountSettings(), savedEnvironmentCount: arr(rows).length,
    enabledRoutes: ['SettingsThreads', 'SettingsProjectOverview', 'SettingsEnvironments', 'SettingsAppearance', 'SettingsKeyboard', 'SettingsFollowUp', 'SettingsOrganization', 'SettingsArchive', 'SettingsProviderAccounts', 'SettingsScheduledTasks', 'SettingsUsage', 'SettingsAbout', ...Object.keys(MOBILE_SERVER_ROUTES)],
    scope, preferences, scheme: appearance.scheme, themeId: appearance.themeId, safeBottom: Number(safeBottom) || 0 }),
    header: JSON.stringify({ ...obj(JSON.parse(settingsHeaderConfiguration(str(routeKey), true, scope, str(visitToken)))), close: compactRoot === true, compactRoot: compactRoot === true }),
    projectHeader: JSON.stringify({ ...obj(JSON.parse(settingsHeaderConfiguration(str(projectRouteKey), false, scope, str(visitToken)))), back: false }),
    environmentIds: JSON.stringify(scope.selected.map(environment => environment.environmentId)),
    serverScope: JSON.stringify({ environmentIds: scope.scoped.map(environment => environment.environmentId),
      members: scope.selection.projectKey ? scope.scoped.flatMap(environment =>
        (scope.projects.find(project => project.key === scope.selection.projectKey)?.projectKeys ?? [])
          .filter(key => key.startsWith(`${environment.environmentId}:`))
          .map(key => ({ environmentId: environment.environmentId, id: key.slice(environment.environmentId.length + 1) }))) : null,
      projectLabel: scope.projectLabel }) };
}
function settingsScope(rows: unknown, input: unknown, groupingMode: string) {
  const value = obj(input);
  const selection: SettingsScopeSelection = { environmentIds: value.allEnvironments === true ? null
    : Array.isArray(value.environmentIds) ? value.environmentIds.filter((id): id is string => typeof id === 'string') : null,
    projectKey: str(value.projectKey) };
  return mobileSettingsScope(arr(rows).map(row => ({ environmentId: str(row.environmentId), label: str(row.label),
    state: str(row.state), origin: str(row.url), machine: str(row.machine) })), selection, groupingMode);
}
export function settingsScopeEvent(args: unknown[]) {
  const [text, configuration, visitToken, routeKey] = args;
  const event = decodeSettingsHeaderEvent(str(text));
  let header: Record<string, unknown> = {};
  try { header = obj(JSON.parse(str(configuration))); } catch { /* A route without a scope header refuses its events. */ }
  const availableEnvironmentIds = arr(header.environments).map(environment => str(environment.id));
  const permitted = event?.kind === 'all' && event.value === ''
    || event?.kind === 'environment' && availableEnvironmentIds.includes(event.value)
    || event?.kind === 'project' && (event.value === '' || arr(header.projects).some(project => project.id === event.value))
    || event?.kind === 'close' && header.close === true && event.value === ''
    || event?.kind === 'back' && header.back === true && event.value === '';
  if (!event || !permitted || !str(visitToken) || event.visitToken !== visitToken || event.requestRoute !== routeKey
    || header.visitToken !== visitToken || header.routeKey !== routeKey)
    return { kind: '', value: '', visitToken: '', requestRoute: '', availableEnvironmentIds: [] };
  return { ...event, availableEnvironmentIds };
}

export function mobileScheduledHeader(args: unknown[]) {
  const [serialized, rows, routeKey, selection, canCreate, visitToken] = args;
  const preferences = normalizeMobilePreferences(serialized);
  const scope = settingsScope(rows, selection, preferences.projectGroupingMode);
  return { configuration: JSON.stringify({ ...obj(JSON.parse(settingsHeaderConfiguration(str(routeKey), false, scope, str(visitToken)))),
    back: false, addActionID: 'scheduled-new', addEnabled: canCreate === true }) };
}
