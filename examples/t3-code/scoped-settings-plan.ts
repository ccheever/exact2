// MIT T3 Code 1e2ecbd975 (LICENSE-T3): apps/web/src/components/settings/scopedSettings.ts,
// with the parts of packages/shared/src/projectSettings.ts it calls (resolveProjectSettings,
// clearProjectSettingsOverrides, resolveWorktreeCleanup) and the key lists of
// packages/contracts/src/settings.ts. Changes: settings are plain JSON objects (the clone
// does not decode the Effect schemas), so `Equal.equals` is a canonical, key-order-free
// comparison and a missing `projectSettingsOverrides` reads as `{}`; a model override's
// provider guard applies only when the settings name that provider (the clone's settings
// may come from older servers without `providerInstances`).
import { obj, type Json, type Obj } from './domain';
import type { ResolvedSettingsScope, ScopeMember } from './settings-scope';

export const PROJECT_SCOPED_SERVER_SETTING_KEYS = ['worktreeCleanup', 'defaultModelSelection', 'defaultRuntimeMode', 'defaultThreadEnvMode', 'newWorktreesStartFromOrigin',
  'worktreeSubmodules', 'defaultAutoPull', 'defaultProjectScripts', 'enableAgentBrowserAccess', 'enableAgentDeviceAccess', 'textGenerationModelSelection',
  'sourceControlWriterModelSelection', 'sourceControlWritingStyle', 'branchNamingMode', 'branchNamePrefix', 'branchNameInstructions', 'pullRequestMergeMethod',
  'sidebarAutoSettleOnMerge', 'sidebarAutoSettleAfterDays', 'continueThreadsAfterServerUpdate', 'responseStreamingMode'] as const;
const NULLABLE_PROJECT_SETTINGS_OVERRIDES = new Set(['defaultModelSelection', 'sourceControlWriterModelSelection', 'pullRequestMergeMethod', 'sidebarAutoSettleAfterDays']);
export const PROJECT_FILE_BACKED_SETTINGS: Record<string, { field: string; builtIn: string }> = {
  defaultThreadEnvMode: { field: 'defaultThreadEnvMode', builtIn: 'local' }, worktreeSubmodules: { field: 'worktreeSubmodules', builtIn: 'recursive' } };
const SERVER_KEYS = new Set(['worktreeCleanup', 'storageCleanup', 'responseStreamingMode', 'enableProviderUpdateChecks', 'continueThreadsAfterServerUpdate', 'enableAgentBrowserAccess',
  'projectAgentBrowserAccessOverrides', 'defaultAutoPull', 'defaultProjectScripts', 'projectScriptOverrides', 'projectAutoPullOverrides', 'defaultModelSelection', 'defaultRuntimeMode',
  'projectSettingsOverrides', 'projectSettingsFolded', 'enableAgentDeviceAccess', 'enableDeviceSupport', 'deviceOnboardingCompleted', 'deviceHosts', 'sidebarAutoSettleAfterDays',
  'snoozeLimitedThreads', 'autoResumeLimitedThreads', 'sidebarAutoSettleOnMerge', 'backgroundActivity', 'automaticGitFetchInterval', 'providerHealthRefreshInterval',
  'backgroundActivityProfile', 'defaultTheme', 'defaultThemeSetAt', 'environmentIcon', 'defaultThreadEnvMode', 'newWorktreesStartFromOrigin', 'worktreeSubmodules',
  'addProjectBaseDirectory', 'textGenerationModelSelection', 'branchNamingMode', 'branchNamePrefix', 'branchNameInstructions', 'sourceControlWritingStyle',
  'sourceControlWriterModelSelection', 'pullRequestMergeMethod', 'providers', 'providerInstances', 'observability', 'bitbucket', 'usageLimitSources',
  'cursorKeychainUsageEnabled', 'usagePriceOverrides', 'usageModelAliases']);
const CLIENT_KEYS = new Set(['notificationMode', 'inAppNotificationsEnabled', 'diffColorScheme', 'chatWidth', 'loadBalancingEnabled', 'loadBalancingWeights', 'appearanceContrast',
  'panelAnimationDurationMs', 'confirmQuit', 'confirmThreadArchive', 'confirmThreadDelete', 'confirmThreadUnpin', 'diffFilesCollapsed', 'diffIgnoreWhitespace', 'diffLayout',
  'environmentIdentificationMode', 'glassOpacity', 'fontSizeInterface', 'fontSizePrompt', 'fontSizeCode', 'fontSizeTerminal', 'fontFamilyCode', 'fontFamilyComposer',
  'fontFamilySans', 'fontFamilyTerminal', 'fontSmoothing', 'persistComposerContextStrip', 'planModeEnabled', 'contextWindowMeterEnabled', 'composerCollapseOnScroll',
  'composerRichTextEnabled', 'sendShortcut', 'followUpBehavior', 'proactivePanelsEnabled', 'showSkillsInSlashMenu', 'legacySidebarEnabled', 'sidebarWorkingShelfEnabled',
  'wordWrap', 'theme', 'themeLight', 'themeDark', 'typographyAdvanced', 'sidebarProjectSortOrder']);
const PROJECT_SCOPED_KEYS = new Set<string>(PROJECT_SCOPED_SERVER_SETTING_KEYS);

export const isProjectScopedSettingKey = (key: string) => PROJECT_SCOPED_KEYS.has(key);
export const isNullableProjectSettingsOverride = (key: string) => NULLABLE_PROJECT_SETTINGS_OVERRIDES.has(key);
const isPlainObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const hasOwn = (value: Obj, key: string) => Object.prototype.hasOwnProperty.call(value, key);
// Effect's Equal.equals on decoded settings: structural, independent of key order.
const canonical = (value: unknown): unknown => Array.isArray(value) ? value.map(canonical)
  : isPlainObject(value) ? Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])])) : value ?? null;
export const settingsEqual = (a: unknown, b: unknown) => JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));

// ── projectSettings.ts ────────────────────────────────────────────────────
export type ProjectSettingSource = 'environment' | 'project' | 't3.json';
export type ProjectSettingSources = Readonly<Record<string, ProjectSettingSource>>;
const ENVIRONMENT_SOURCES: ProjectSettingSources = Object.fromEntries(PROJECT_SCOPED_SERVER_SETTING_KEYS.map(key => [key, 'environment']));
const overridesOf = (settings: Obj) => obj(settings.projectSettingsOverrides);

function modelProviderEnabled(settings: Obj, selection: Obj): boolean {
  const instanceId = typeof selection.instanceId === 'string' ? selection.instanceId : '';
  const instance = obj(settings.providerInstances)[instanceId];
  if (instance !== undefined) return obj(instance).enabled !== false;
  const legacy = obj(settings.providers)[instanceId];
  return legacy === undefined ? true : obj(legacy).enabled === true;
}

/** Apply one project's overrides (and, when given, its t3.json) on top of environment settings. */
export function resolveProjectSettings(settings: Obj, projectId: string | null, projectFile?: Obj | null): { settings: Obj; sources: ProjectSettingSources; overrides: Obj } {
  const stored = projectId === null ? undefined : overridesOf(settings)[projectId];
  const overrides = stored === undefined ? {} : obj(stored);
  let resolved: { settings: Obj; sources: ProjectSettingSources; overrides: Obj } = { settings, sources: ENVIRONMENT_SOURCES, overrides };
  if (Object.keys(overrides).length > 0) {
    const sources: Record<string, ProjectSettingSource> = { ...ENVIRONMENT_SOURCES };
    const effective: Obj = { ...settings };
    for (const key of PROJECT_SCOPED_SERVER_SETTING_KEYS) {
      if (!hasOwn(overrides, key)) continue;
      const value = overrides[key];
      if (value === undefined) continue;
      if ((key === 'textGenerationModelSelection' || key === 'defaultModelSelection') && value !== null && !modelProviderEnabled(settings, obj(value))) continue;
      effective[key] = value as Json;
      sources[key] = 'project';
    }
    resolved = { settings: effective, sources, overrides };
  }
  if (projectFile === undefined) return resolved;
  let effective: Obj | null = null, sources: Record<string, ProjectSettingSource> | null = null;
  for (const [key, { field, builtIn }] of Object.entries(PROJECT_FILE_BACKED_SETTINGS)) {
    if (resolved.settings[key] !== null && resolved.settings[key] !== undefined) continue;
    const fromFile = projectFile?.[field];
    effective ??= { ...resolved.settings }; sources ??= { ...resolved.sources };
    effective[key] = (fromFile === undefined ? builtIn : fromFile) as Json;
    sources[key] = fromFile === undefined ? 'environment' : 't3.json';
  }
  return effective === null || sources === null ? resolved : { ...resolved, settings: effective, sources };
}

/** The project's entry with `keys` removed; `null` when that leaves it empty. */
export function clearProjectSettingsOverrides(settings: Obj, projectId: string, keys: readonly string[]): Obj | null {
  const current = overridesOf(settings)[projectId];
  if (current === undefined) return null;
  const next = { ...obj(current) };
  for (const key of keys) delete next[key];
  return Object.keys(next).length === 0 ? null : next;
}

/** Worktree rules are project-scoped; artifact and log retention stays environment-wide. */
export function resolveWorktreeCleanup(settings: Obj, projectId: string | null): Obj {
  const policy = obj(resolveProjectSettings(settings, projectId).settings.worktreeCleanup);
  if (policy.mode === 'custom') return obj(policy.rules);
  if (policy.mode === 'off') return { worktreeAfterDays: null, worktreeOnMerge: false, worktreeOnDelete: false, worktreeUnchanged: false };
  const storage = obj(settings.storageCleanup);
  return { worktreeAfterDays: storage.worktreeAfterDays ?? null, worktreeOnMerge: storage.worktreeOnMerge ?? false, worktreeOnDelete: storage.worktreeOnDelete ?? false,
    worktreeUnchanged: storage.worktreeUnchanged ?? false };
}

// ── scopedSettings.ts ─────────────────────────────────────────────────────
export interface ScopedSettingsEnvironment {
  readonly environmentId: string;
  readonly label: string;
  readonly connection: { readonly phase: string };
  readonly serverConfig: { readonly settings: Obj; readonly environment?: { readonly capabilities: { readonly projectSettingsOverrides?: boolean } } } | null;
}

/** The representative supplies display values, never the set of write targets. */
export function selectScopedSettingsEnvironments<T extends ScopedSettingsEnvironment>(scope: ResolvedSettingsScope, available: readonly T[], primaryEnvironmentId: string | null) {
  const selectedIds = new Set(scope.environmentIds);
  const environments = available.filter(environment => selectedIds.has(environment.environmentId));
  const connectedEnvironments = environments.filter(environment => environment.connection.phase === 'connected' && environment.serverConfig !== null);
  const environment = connectedEnvironments.find(candidate => candidate.environmentId === primaryEnvironmentId) ?? connectedEnvironments[0] ?? null;
  return { environments, connectedEnvironments, environment };
}

/** One (environment, project) pair the scope writes to, with that project's effective settings. */
export interface ScopedSettingsTarget { readonly environmentId: string; readonly label: string; readonly projectId: string | null; readonly settings: Obj; readonly sources: ProjectSettingSources }

/** Effective settings per connected target: members at project scope, environments otherwise. */
export function resolveScopedSettingsTargets(scope: ResolvedSettingsScope, connectedEnvironments: readonly ScopedSettingsEnvironment[],
  projectFiles?: ReadonlyMap<string, Obj | null>): readonly ScopedSettingsTarget[] {
  const byId = new Map(connectedEnvironments.map(environment => [environment.environmentId, environment]));
  if (scope.kind === 'project' || scope.kind === 'checkout') {
    return scope.members.flatMap((member: ScopeMember) => {
      const environment = byId.get(member.environmentId);
      if (!environment?.serverConfig) return [];
      const projectFile = projectFiles?.get(member.physicalProjectKey);
      const resolved = projectFile === undefined ? resolveProjectSettings(environment.serverConfig.settings, member.id)
        : resolveProjectSettings(environment.serverConfig.settings, member.id, projectFile);
      return [{ environmentId: member.environmentId, label: environment.label, projectId: member.id, settings: resolved.settings, sources: resolved.sources }];
    });
  }
  return connectedEnvironments.flatMap(environment => environment.serverConfig ? [{ environmentId: environment.environmentId, label: environment.label, projectId: null,
    settings: environment.serverConfig.settings, sources: resolveProjectSettings(environment.serverConfig.settings, null).sources }] : []);
}

export function scopedSettingsAreMixed(targets: readonly Pick<ScopedSettingsTarget, 'settings'>[], keys: readonly string[]): boolean {
  const first = targets[0];
  return first !== undefined && targets.some(candidate => keys.some(key => !settingsEqual(first.settings[key], candidate.settings[key])));
}

export type ScopedSettingSource = ProjectSettingSource | 'mixed';
/** Whether the keys are overridden on every target, inherited on every target, or split. */
export function scopedSettingsSource(targets: readonly Pick<ScopedSettingsTarget, 'sources'>[], keys: readonly string[]): ScopedSettingSource {
  const scoped = keys.filter(isProjectScopedSettingKey);
  if (scoped.length === 0 || targets.length === 0) return 'environment';
  const sources = new Set(targets.flatMap(target => scoped.map(key => target.sources[key])));
  return sources.size > 1 ? 'mixed' : sources.has('project') ? 'project' : sources.has('t3.json') ? 't3.json' : 'environment';
}

export interface ScopedServerWrite { readonly environmentId: string; readonly label: string; readonly patch: Obj }
export type ScopedSettingsPlan = { clientPatch: Obj; hasClientWrite: boolean; serverWrites: ScopedServerWrite[]; unavailableReason: string | null };

function projectOverrideWrites(scope: Extract<ResolvedSettingsScope, { kind: 'project' | 'checkout' }>, environments: readonly ScopedSettingsEnvironment[],
  update: (current: Obj, settings: Obj, projectId: string) => Obj | null): ScopedServerWrite[] {
  const byId = new Map(environments.map(environment => [environment.environmentId, environment]));
  const writes = new Map<string, ScopedServerWrite>();
  for (const member of scope.members) {
    const environment = byId.get(member.environmentId);
    if (!environment?.serverConfig || environment.connection.phase !== 'connected' || environment.serverConfig.environment?.capabilities.projectSettingsOverrides !== true) continue;
    const settings = environment.serverConfig.settings;
    const entry = update(obj(overridesOf(settings)[member.id]), settings, member.id);
    const existing = writes.get(member.environmentId);
    writes.set(member.environmentId, { environmentId: member.environmentId, label: environment.label,
      patch: { projectSettingsOverrides: { ...obj(existing?.patch.projectSettingsOverrides), [member.id]: entry } } });
  }
  return [...writes.values()];
}

/**
 * Environment scopes write the patch to every connected environment; project and checkout
 * scopes write the scopable keys into each member's override entry on its environment.
 * Client keys always persist locally.
 */
export function planScopedSettingsPatch(scope: ResolvedSettingsScope, environments: readonly ScopedSettingsEnvironment[], patch: Obj): ScopedSettingsPlan {
  const clientPatch = Object.fromEntries(Object.entries(patch).filter(([key]) => CLIENT_KEYS.has(key))) as Obj;
  const serverPatch = Object.fromEntries(Object.entries(patch).filter(([key]) => SERVER_KEYS.has(key))) as Obj;
  const serverKeys = Object.keys(serverPatch);
  const { connectedEnvironments } = selectScopedSettingsEnvironments(scope, environments, null);
  const isProjectScope = scope.kind === 'project' || scope.kind === 'checkout';
  const unscopableKeys = isProjectScope ? serverKeys.filter(key => !isProjectScopedSettingKey(key)) : [];
  const storage = obj(serverPatch.storageCleanup);
  const serverWrites: ScopedServerWrite[] = serverKeys.length === 0 ? []
    : isProjectScope ? (unscopableKeys.length > 0 ? [] : projectOverrideWrites(scope, environments, (current, settings, projectId) => {
      // Object-valued keys arrive as partial patches; an override entry stores the whole value.
      const effective = resolveProjectSettings(settings, projectId).settings;
      const next: Obj = { ...current };
      for (const [key, value] of Object.entries(serverPatch)) {
        const cleanup = obj(serverPatch.worktreeCleanup);
        if (key === 'worktreeCleanup' && cleanup.mode === 'custom') {
          next[key] = { mode: 'custom', rules: { ...resolveWorktreeCleanup(settings, projectId), ...obj(cleanup.rules) } };
          continue;
        }
        // A picker's "Inherit" sends null; for keys whose override cannot store null that means remove the override.
        if (value === null && !isNullableProjectSettingsOverride(key)) { delete next[key]; continue; }
        const base = effective[key];
        next[key] = isPlainObject(value) && isPlainObject(base) ? { ...base, ...value } : value;
      }
      return next;
    }))
    : scope.kind === 'all' || scope.kind === 'environment' ? connectedEnvironments.map(environment => ({
      environmentId: environment.environmentId, label: environment.label,
      patch: environment.serverConfig?.settings.worktreeCleanup != null && serverPatch.storageCleanup && Object.keys(storage).some(key => key.startsWith('worktree'))
        ? { ...serverPatch, worktreeCleanup: { mode: 'custom', rules: { ...resolveWorktreeCleanup(environment.serverConfig.settings, null),
          ...Object.fromEntries(Object.entries(storage).filter(([key]) => key.startsWith('worktree'))) } } }
        : serverPatch }))
    : [];
  const hasClientWrite = Object.keys(clientPatch).length > 0;
  const hasWrite = hasClientWrite || serverWrites.length > 0;
  const unavailableReason = hasWrite || Object.keys(patch).length === 0 ? null
    : scope.kind === 'unavailable' ? scope.message
    : unscopableKeys.length > 0 ? 'This setting is environment-wide and cannot be overridden by a project.'
    : isProjectScope ? 'Connect the selected checkouts, or update their environments, to save a project override.'
    : `Connect ${scope.kind === 'environment' ? scope.label : 'an environment'} to save this setting.`;
  return { clientPatch, hasClientWrite, serverWrites, unavailableReason };
}

/** Remove the keys' project overrides so each member inherits its environment value again. */
export function planScopedSettingsClear(scope: ResolvedSettingsScope, environments: readonly ScopedSettingsEnvironment[], keys: readonly string[]): ScopedSettingsPlan {
  const serverWrites = scope.kind === 'project' || scope.kind === 'checkout'
    ? projectOverrideWrites(scope, environments, (_current, settings, projectId) => clearProjectSettingsOverrides(settings, projectId, keys)) : [];
  return { clientPatch: {}, hasClientWrite: false, serverWrites,
    unavailableReason: serverWrites.length > 0 ? null : 'Connect the selected checkouts, or update their environments, to reset this override.' };
}

export interface ProjectOverrideEntry { readonly environmentId: string; readonly projectId: string }

/** The projects on the selected environments that override `keys`. */
export function listProjectOverrides(environments: readonly ScopedSettingsEnvironment[], keys: readonly string[]): readonly ProjectOverrideEntry[] {
  const scoped = keys.filter(isProjectScopedSettingKey);
  if (scoped.length === 0) return [];
  return environments.flatMap(environment => {
    const overrides = environment.serverConfig?.settings.projectSettingsOverrides;
    if (!isPlainObject(overrides)) return [];
    return Object.entries(overrides).flatMap(([projectId, entry]) => scoped.some(key => hasOwn(obj(entry), key)) ? [{ environmentId: environment.environmentId, projectId }] : []);
  });
}

/** Drop `keys` from the named project entries so they follow the environment again. */
export function planProjectOverridesClear(environments: readonly ScopedSettingsEnvironment[], entries: readonly ProjectOverrideEntry[], keys: readonly string[]): ScopedSettingsPlan {
  const byId = new Map(environments.map(environment => [environment.environmentId, environment]));
  const writes = new Map<string, ScopedServerWrite>();
  for (const { environmentId, projectId } of entries) {
    const environment = byId.get(environmentId);
    if (!environment?.serverConfig || environment.connection.phase !== 'connected') continue;
    const existing = writes.get(environmentId);
    writes.set(environmentId, { environmentId, label: environment.label,
      patch: { projectSettingsOverrides: { ...obj(existing?.patch.projectSettingsOverrides), [projectId]: clearProjectSettingsOverrides(environment.serverConfig.settings, projectId, keys) } } });
  }
  const serverWrites = [...writes.values()];
  return { clientPatch: {}, hasClientWrite: false, serverWrites, unavailableReason: serverWrites.length > 0 ? null : 'Connect the environments to reset these overrides.' };
}

/** Wait for every target so a failed environment does not hide successful or later writes. */
export async function persistScopedSettingsPatch(plan: ScopedSettingsPlan,
  persistServer: (input: { environmentId: string; input: { patch: Obj } }) => Promise<{ readonly _tag: 'Success' | 'Failure' }>, persistClient: (patch: Obj) => void) {
  if (plan.hasClientWrite) persistClient(plan.clientPatch);
  const results = await Promise.allSettled(plan.serverWrites.map(({ environmentId, patch }) => persistServer({ environmentId, input: { patch } })));
  const failedEnvironments = plan.serverWrites.filter((_, index) => { const result = results[index]; return result?.status !== 'fulfilled' || result.value._tag === 'Failure'; });
  return { failedEnvironments, savedEnvironmentCount: plan.serverWrites.length - failedEnvironments.length };
}

/** useRunScopedPlan's toasts: the warning for a refused plan, the error naming environments that did not save. */
export function scopedPlanNotice(plan: Pick<ScopedSettingsPlan, 'unavailableReason'>, result?: { failedEnvironments: readonly { label: string }[]; savedEnvironmentCount: number }):
  { kind: 'warning' | 'error'; title: string; description: string } | null {
  if (plan.unavailableReason) return { kind: 'warning', title: 'Setting not saved', description: plan.unavailableReason };
  if (!result || result.failedEnvironments.length === 0) return null;
  const saved = result.savedEnvironmentCount > 0;
  return { kind: 'error', title: saved ? 'Setting saved on some environments' : 'Setting not saved',
    description: `Could not update ${result.failedEnvironments.map(environment => environment.label).join(', ')}.${saved ? ' The other selected environments saved the change.' : ''}` };
}
