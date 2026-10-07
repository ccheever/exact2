// @ref llp/1107.004-home-projection.decision.md#decision
// Pinned 365aa87982 Stack.tsx, SettingsRouteScreen.tsx and settings-environment-filter.
import { mobileHomeProjects, mobileHomeSources, type HomeSource } from './home';
import { mobileClient } from './client';
import { environmentSources } from './shared/connections';
import { fleet } from './shared/settings-b-fleet';
import { logicalKey } from './shared/r6-polish-groups';
import { mobileTextRoles, type MobilePreferences } from './settings-preferences';
import { settingsColors } from './settings-appearance';

/** Inventory only. A prepared component is not a completed, driven route. */
export const SETTINGS_ROUTE_INVENTORY = [
  {
    "name": "Settings",
    "path": "settings/",
    "title": "Settings",
    "source": "`SettingsRouteScreen` in `apps/mobile/src/features/settings/SettingsRouteScreen.tsx`",
    "preparedComponent": "SettingsRoot"
  },
  {
    "name": "SettingsEnvironments",
    "path": "settings/environments",
    "title": "Environments",
    "source": "`SettingsEnvironmentsRouteScreen` in `apps/mobile/src/features/settings/SettingsEnvironmentsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsEnvironmentDetail",
    "path": "settings/environments/:environmentId",
    "title": "Environment",
    "source": "`SettingsEnvironmentDetailRouteScreen` in `apps/mobile/src/features/settings/SettingsEnvironmentDetailRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsEnvironmentNewThreads",
    "path": "settings/new-threads",
    "title": "New threads",
    "source": "`SettingsEnvironmentNewThreadsRouteScreen` in `apps/mobile/src/features/settings/SettingsServerControlsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsEnvironmentSourceControl",
    "path": "settings/source-control",
    "title": "Source control",
    "source": "`SettingsEnvironmentSourceControlRouteScreen` in `apps/mobile/src/features/settings/SettingsServerControlsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsEnvironmentAgentBehavior",
    "path": "settings/agent-behavior",
    "title": "Agent behavior",
    "source": "`SettingsEnvironmentAgentBehaviorRouteScreen` in `apps/mobile/src/features/settings/SettingsServerControlsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsProviderAccounts",
    "path": "settings/provider-accounts",
    "title": "Provider accounts",
    "source": "`SettingsProviderAccountsRouteScreen` in `apps/mobile/src/features/settings/SettingsProviderAccountsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsEnvironmentMaintenance",
    "path": "settings/maintenance",
    "title": "Maintenance",
    "source": "`SettingsEnvironmentMaintenanceRouteScreen` in `apps/mobile/src/features/settings/SettingsServerControlsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsNotifications",
    "path": "settings/notifications",
    "title": "Notifications",
    "source": "`SettingsNotificationsRouteScreen` in `apps/mobile/src/features/settings/SettingsNotificationsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsThreads",
    "path": "settings/thread-preferences",
    "title": "Thread behavior",
    "source": "`SettingsThreadsRouteScreen` in `apps/mobile/src/features/settings/SettingsThreadsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsAbout",
    "path": "settings/about",
    "title": "About T3 Code",
    "source": "`SettingsAboutRouteScreen` in `apps/mobile/src/features/settings/SettingsAboutRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsEnvironmentNew",
    "path": "settings/environment-new",
    "title": "Add Environment",
    "source": "`ConnectionsNewRouteScreen` in `apps/mobile/src/features/connection/ConnectionsNewRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsArchive",
    "path": "settings/archive",
    "title": "Archived Threads",
    "source": "`ArchivedThreadsRouteScreen` in `apps/mobile/src/features/archive/ArchivedThreadsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsAppearance",
    "path": "settings/appearance",
    "title": "Appearance",
    "source": "`SettingsAppearanceRouteScreen` in `apps/mobile/src/features/settings/SettingsAppearanceRouteScreen.tsx`",
    "preparedComponent": "SettingsAppearance"
  },
  {
    "name": "SettingsProjectGrouping",
    "path": "settings/project-grouping",
    "title": "Organization",
    "source": "`SettingsProjectGroupingRouteScreen` in `apps/mobile/src/features/settings/SettingsProjectGroupingRouteScreen.tsx`",
    "preparedComponent": "SettingsChoices"
  },
  {
    "name": "SettingsOrganization",
    "path": "settings/organization",
    "title": "Organization",
    "source": "`SettingsProjectGroupingRouteScreen` in `apps/mobile/src/features/settings/SettingsProjectGroupingRouteScreen.tsx`",
    "preparedComponent": "SettingsChoices"
  },
  {
    "name": "SettingsProjectOverview",
    "path": "settings/project",
    "title": "Project overview",
    "source": "`SettingsProjectOverviewRouteScreen` in `apps/mobile/src/features/settings/SettingsProjectOverviewRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsKeyboard",
    "path": "settings/keyboard",
    "title": "Keyboard",
    "source": "`SettingsKeyboardRouteScreen` in `apps/mobile/src/features/settings/SettingsKeyboardRouteScreen.tsx`",
    "preparedComponent": "SettingsChoices"
  },
  {
    "name": "SettingsFollowUp",
    "path": "settings/follow-ups",
    "title": "Follow-ups",
    "source": "`SettingsFollowUpRouteScreen` in `apps/mobile/src/features/settings/SettingsFollowUpRouteScreen.tsx`",
    "preparedComponent": "SettingsChoices"
  },
  {
    "name": "SettingsScheduledTasks",
    "path": "settings/scheduled-tasks",
    "title": "Scheduled Tasks",
    "source": "`SettingsScheduledTasksRouteScreen` in `apps/mobile/src/features/settings/SettingsScheduledTasksRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsScheduledTaskNew",
    "path": "settings/scheduled-tasks/new",
    "title": "New scheduled task",
    "source": "`SettingsScheduledTaskNewRouteScreen` in `apps/mobile/src/features/settings/SettingsScheduledTasksRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsScheduledTaskEdit",
    "path": "",
    "title": "Edit scheduled task",
    "source": "`SettingsScheduledTaskEditRouteScreen` in `apps/mobile/src/features/settings/SettingsScheduledTasksRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsScheduledTaskBranch",
    "path": "",
    "title": "Base branch",
    "source": "`ScheduledTaskBranchPickerRouteScreen` in `apps/mobile/src/features/settings/ScheduledTaskPickerScreens.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsScheduledTaskModel",
    "path": "",
    "title": "",
    "source": "`ScheduledTaskModelPickerRouteScreen` in `apps/mobile/src/features/settings/ScheduledTaskPickerScreens.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsClientStorage",
    "path": "settings/client-storage",
    "title": "Client Storage",
    "source": "`SettingsClientStorageRouteScreen` in `apps/mobile/src/features/settings/SettingsClientStorageRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsDiagnostics",
    "path": "settings/diagnostics",
    "title": "Diagnostics",
    "source": "`SettingsDiagnosticsRouteScreen` in `apps/mobile/src/features/diagnostics/SettingsDiagnosticsRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsUsageAccount",
    "path": "",
    "title": "Account",
    "source": "`UsageLimitAccountScreen` in `apps/mobile/src/features/usage/UsageLimitsPooled.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsOpenSourceLicenses",
    "path": "settings/open-source-licenses",
    "title": "Open source licenses",
    "source": "`SettingsOpenSourceLicensesRouteScreen` in `apps/mobile/src/features/settings/SettingsOpenSourceLicensesRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsOpenSourceLicense",
    "path": "settings/open-source-licenses/:entryKey",
    "title": "License notice",
    "source": "`SettingsOpenSourceLicenseRouteScreen` in `apps/mobile/src/features/settings/SettingsOpenSourceLicensesRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsUsage",
    "path": "settings/usage",
    "title": "Usage",
    "source": "`UsageRouteScreen` in `apps/mobile/src/features/usage/UsageRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsContent",
    "path": "settings/",
    "title": "",
    "source": "`SettingsContentStack` in `Stack.tsx local SettingsContentStack`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsAuth",
    "path": "settings/auth",
    "title": "",
    "source": "`SettingsAuthRouteScreen` in `apps/mobile/src/features/settings/SettingsAuthRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsWaitlist",
    "path": "settings/waitlist",
    "title": "",
    "source": "`SettingsAuthRouteScreen` in `apps/mobile/src/features/settings/SettingsAuthRouteScreen.tsx`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsSheet",
    "path": "settings",
    "title": "",
    "source": "`SettingsSheetStack` in `Stack.tsx local SettingsSheetStack`",
    "preparedComponent": ""
  },
  {
    "name": "SettingsLegal",
    "path": "settings/legal",
    "title": "Legal",
    "source": "`SettingsLegalRouteScreen` in `apps/mobile/src/features/settings/SettingsLegalRouteScreen.tsx`",
    "preparedComponent": ""
  }
] as const;
export interface SettingsEnvironment {
  environmentId: string; label: string; state: string; origin: string; machine: string;
}
export interface SettingsScopeSelection { environmentIds: string[] | null; projectKey: string }
export function mobileSettingsScope(environments: SettingsEnvironment[], selection: SettingsScopeSelection,
  groupingMode = 'repository', sources: HomeSource[] = mobileHomeSources(),
  configuredIds = new Set(environmentSources(mobileClient, fleet.saved, fleet.entries)
    .filter(source => Object.keys(source.config).length > 0).map(source => source.environmentId))) {
  // Match upstream connectedSettingsTargets without waiting for a shell snapshot.
  const available = environments.filter(environment => environment.state === 'connected' && configuredIds.has(environment.environmentId));
  const selected = available.filter(environment => selection.environmentIds === null || selection.environmentIds.includes(environment.environmentId));
  const grouping = { environmentId: '', shell: { projects: [] }, local: { groupingMode, groupingOverrides: {} } };
  const order = new Map<string, number>();
  for (const source of sources) for (const project of source.shell.projects) {
    const key = logicalKey(grouping, source.environmentId, project);
    if (!order.has(key)) order.set(key, order.size);
  }
  const projects = mobileHomeProjects(sources, { groupingMode }).sort((a, b) => (order.get(a.key) ?? 0) - (order.get(b.key) ?? 0));
  const selectable = projects.filter(project => selected.some(environment => project.projectKeys.some(key => key.startsWith(`${environment.environmentId}:`))));
  const project = projects.find(project => project.key === selection.projectKey);
  const scoped = selection.projectKey ? selected.filter(environment => project?.projectKeys.some(key => key.startsWith(`${environment.environmentId}:`))) : selected;
  return { available, selected, scoped, projects: selectable, projectLabel: project?.title ?? 'Unavailable project',
    selection, filtered: selection.environmentIds !== null || selection.projectKey !== '' };
}
export function toggleSettingsEnvironment(selection: SettingsScopeSelection, available: SettingsEnvironment[], environmentId: string): SettingsScopeSelection {
  const ids = available.map(environment => environment.environmentId);
  const next = new Set(ids.filter(id => selection.environmentIds === null || selection.environmentIds.includes(id)));
  if (ids.includes(environmentId)) { if (next.has(environmentId)) next.delete(environmentId); else next.add(environmentId); }
  return { ...selection, environmentIds: ids.every(id => next.has(id)) ? null : [...next] };
}
export interface SettingsRootInput {
  savedEnvironmentCount: number;
  enabledRoutes: readonly string[];
  scope: ReturnType<typeof mobileSettingsScope>;
  preferences: MobilePreferences;
  scheme: string; themeId: string; safeBottom?: number;
  cloudConfigured?: boolean;
  accountLoaded?: boolean; signedIn?: boolean; accountLabel?: string;
}
export function settingsRootView(input: SettingsRootInput) {
  const enabled = new Set(input.enabledRoutes);
  const row = (target: string, icon: string, label: string, value = '', disabled = false) =>
    ({ target, icon, label, value, disabled: disabled || !enabled.has(target) });
  const noTargets = input.scope.selected.length === 0;
  const connections = [
    ...(input.cloudConfigured ? [row('SettingsAuth', 'person.crop.circle', 'T3 Account', !input.accountLoaded ? 'Checking'
      : input.signedIn ? input.accountLabel || 'Signed in' : 'Sign in', !input.accountLoaded)] : []),
    row('SettingsEnvironments', 'desktopcomputer', 'Environments', `${input.savedEnvironmentCount}`),
    ...(input.cloudConfigured ? [row('SettingsNotifications', 'bell.badge', 'Notifications')] : []),
  ];
  return { bottom: Math.max(input.safeBottom ?? 0, 18) + 18,
    colors: settingsColors(input.scheme, input.themeId), roles: mobileTextRoles(input.preferences.baseFontSize),
    sections: [
      { title: 'Connections', rows: connections },
      { title: 'Interface', rows: [row('SettingsAppearance', 'paintbrush', 'Appearance'), row('SettingsKeyboard', 'keyboard', 'Keyboard')] },
      { title: 'Automations', rows: [row('SettingsScheduledTasks', 'clock', 'Scheduled tasks')] },
      { title: 'Projects & threads', rows: [
        ...(input.scope.selection.projectKey ? [row('SettingsProjectOverview', 'folder', 'Overview', input.scope.projectLabel)] : []),
        row('SettingsOrganization', 'folder', 'Organization'), row('SettingsThreads', 'text.bubble', 'Thread behavior'),
        row('SettingsFollowUp', 'arrow.turn.left.up', 'Follow-ups'), row('SettingsArchive', 'archivebox', 'Archived Threads')] },
      { title: 'Server settings', rows: [row('SettingsProviderAccounts', 'person.crop.circle', 'Provider accounts', '', noTargets),
        row('SettingsEnvironmentNewThreads', 'text.bubble', 'New threads', '', noTargets),
        row('SettingsEnvironmentSourceControl', 'arrow.triangle.branch', 'Source control', '', noTargets),
        row('SettingsEnvironmentAgentBehavior', 'text.alignleft', 'Agent behavior', '', noTargets),
        row('SettingsEnvironmentMaintenance', 'arrow.clockwise', 'Maintenance', '', noTargets)] },
      { title: 'App', rows: [row('SettingsUsage', 'chart.bar.xaxis', 'Usage'), row('SettingsAbout', 'info.circle', 'About T3 Code')] },
    ],
  };
}
export function settingsHeaderConfiguration(routeKey: string, compact: boolean, scope: ReturnType<typeof mobileSettingsScope>) {
  const symbols: Record<string, string> = { server: 'server.rack', cloud: 'cloud', linux: 'terminal', desktop: 'desktopcomputer', laptop: 'laptopcomputer', 'mac-mini': 'macmini', 'mac-studio': 'macstudio' };
  return JSON.stringify({ routeKey, close: compact, back: !compact, filtered: scope.filtered,
    all: scope.selection.environmentIds === null, selectedCount: scope.selected.length, projectKey: scope.selection.projectKey,
    projectLabel: !scope.selection.projectKey ? 'All projects' : scope.projects.find(project => project.key === scope.selection.projectKey)?.title ?? 'Unavailable project',
    environments: scope.available.map(environment => ({ id: environment.environmentId, label: environment.label,
      subtitle: environment.origin, symbol: symbols[environment.machine] || 'desktopcomputer', selected: scope.selected.some(target => target.environmentId === environment.environmentId) })),
    projects: scope.projects.map(project => ({ id: project.key, label: project.title, selected: project.key === scope.selection.projectKey })) });
}
export function decodeSettingsHeaderEvent(text: string): { kind: 'all' | 'environment' | 'project' | 'close' | 'back'; value: string } | null {
  let object: unknown;
  try { object = JSON.parse(text); } catch { return null; }
  if (!object || typeof object !== 'object') return null;
  const event = object as Record<string, unknown>;
  if (typeof event.value !== 'string') return null;
  return event.kind === 'all' || event.kind === 'environment' || event.kind === 'project' || event.kind === 'close' || event.kind === 'back'
    ? { kind: event.kind, value: event.value } : null;
}
