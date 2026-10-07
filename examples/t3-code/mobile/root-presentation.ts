// Root adapters over mobile presentation and the shared client, upstream365aa87982.
// @ref llp/1106.004-home-projection.decision.md#decision
import { normalizeMobilePreferences, resolveMobileAppearance } from './settings-preferences';
import { mobileSettingsScope, settingsRootView, settingsHeaderConfiguration, decodeSettingsHeaderEvent, toggleSettingsEnvironment, type SettingsScopeSelection } from './settings';
import { arr, obj, str } from './shared/domain';
import { fleet } from './shared/settings-b-fleet';
import { mobileClient } from './client';
import { mobileHomeChrome, decodeHomeChromeEvent } from './home-chrome';

export function mobileLayoutFacts(input: string) {
  let value: Record<string, unknown> = {};
  try { value = obj(JSON.parse(input)); } catch { /* Bake has no UIKit geometry. */ }
  return { safeBottom: typeof value.safeBottom === 'number' && Number.isFinite(value.safeBottom)
    ? Math.max(0, value.safeBottom) : 0, liquidGlass: value.liquidGlass === true };
}

export function homeChromeEvent(input: string) {
  return decodeHomeChromeEvent(input) ?? { kind: '', value: '' };
}

export function homeChromeView(args: unknown[]) {
  const [routeKey, query, environmentId, projectKey, scheme, safeBottom, liquidGlass, _revision, rows, groupingMode] = args;
  const environments = arr(rows).map(row => ({ environmentId: str(row.id).split('\n')[1] ?? '', label: str(row.label) }))
    .filter(row => row.environmentId);
  const entries = [...fleet.entries.values()];
  const connectingEnvironments = arr(rows).filter(row => row.retrying === true).map(row => ({ environmentLabel: str(row.label) }));
  return mobileHomeChrome({ routeKey: str(routeKey), query: str(query), environmentId: str(environmentId), projectKey: str(projectKey),
    layout: 'compact', groupingMode: str(groupingMode), scheme: str(scheme), safeBottom: Number(safeBottom) || 0, liquidGlassSupported: liquidGlass === true,
    environments, workspace: {
      connectionError: mobileClient.error || null, connectingEnvironments,
      hasConnectingEnvironment: connectingEnvironments.length > 0,
      hasPendingShellSnapshot: mobileClient.connection === 'connected' && !mobileClient.shellLoaded
        || entries.some(entry => entry.phase === 'connected' && entry.synchronized !== entry.generation),
      hasLoadedShellSnapshot: mobileClient.shellLoaded || entries.some(entry => entry.synchronized === entry.generation),
      hasReadyEnvironment: mobileClient.ready || entries.some(entry => entry.phase === 'connected' && entry.synchronized === entry.generation),
    } });
}

// @ref llp/1106.002-design-system-parity.spec.md#user-preference-and-accessibility-scaling
export function settingsRoot(args: unknown[]) {
  const [serialized, systemScheme, rows, safeBottom, routeKey, selectionJSON] = args;
  const preferences = normalizeMobilePreferences(serialized);
  const appearance = resolveMobileAppearance(preferences, str(systemScheme));
  const scope = settingsScope(rows, selectionJSON, preferences.projectGroupingMode);
  return { root: settingsRootView({ savedEnvironmentCount: arr(rows).length,
    enabledRoutes: ['SettingsEnvironments', 'SettingsAppearance', 'SettingsKeyboard', 'SettingsFollowUp', 'SettingsOrganization', 'SettingsArchive'],
    scope, preferences, scheme: appearance.scheme, themeId: appearance.themeId, safeBottom: Number(safeBottom) || 0 }),
    header: settingsHeaderConfiguration(str(routeKey), true, scope) };
}
function settingsScope(rows: unknown, selectionJSON: unknown, groupingMode: string) {
  let selection: SettingsScopeSelection = { environmentIds: null, projectKey: '' };
  try { const value = obj(JSON.parse(str(selectionJSON)));
    selection = { environmentIds: Array.isArray(value.environmentIds) ? value.environmentIds.filter((id): id is string => typeof id === 'string') : null,
      projectKey: str(value.projectKey) };
  } catch { /* Initial scope selects all connected environments. */ }
  return mobileSettingsScope(arr(rows).map(row => ({ environmentId: str(row.environmentId), label: str(row.label),
    state: str(row.state), origin: str(row.url), machine: str(row.machine) })), selection, groupingMode);
}
export function settingsScopeEvent(args: unknown[]) {
  const [text, serialized, selectionJSON, rows] = args;
  const event = decodeSettingsHeaderEvent(str(text));
  const scope = settingsScope(rows, selectionJSON, normalizeMobilePreferences(serialized).projectGroupingMode);
  let selection = scope.selection;
  if (event?.kind === 'all') selection = { ...selection, environmentIds: null };
  else if (event?.kind === 'environment') selection = toggleSettingsEnvironment(selection, scope.available, event.value);
  else if (event?.kind === 'project') selection = { ...selection, projectKey: event.value };
  return { selection: JSON.stringify(selection), close: event?.kind === 'close' || event?.kind === 'back' };
}
