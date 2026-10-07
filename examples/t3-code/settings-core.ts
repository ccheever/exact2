// Settings shell + General route data (lane settings-core).
// Reference: apps/web/src/components/settings/{SettingsPanels,ProjectDefaultsSettings,
// settingsLayout,SettingsScopeSentence,settingsScope,scopedSettings}.tsx and
// packages/shared/src/projectSettings.ts. Server rows read and write the connected
// environment's ServerSettings (server.updateSettings); device rows are the
// reference ClientSettings, saved in this app's own preferences file.
import { arr, num, obj, str, type Json, type Obj } from './domain';
import { ClientError, providerAvailable, type Native } from './protocol';
import { projectIdentity, providerBadge } from './presentation';
import type { T3Client } from './client';
import { duplicateTheme, parseThemeFile, type CustomTheme } from './settings-themes';
import { themeRoles } from './settings-appearance';
import { advancedBackgroundValue, baseProfile, profileOption } from './settings-a-background';
import { seedDiffState } from './settings-appearance-look';
import { themeEditorCommand } from './settings-appearance-editor';
import { themeImportCommand } from './settings-appearance-import';
import { aboutRows, updateCommand } from './settings-a-about';
import { removeThemes, useCollectionDefaults } from './settings-a-collections';
import type { DiffState } from './diff';
import { notificationPermission, notificationPermissionMessage } from './shell-notify';
import { MOBILE_BETA_ROW, mobileBetaCommand, type QrMark } from './settings-mobile-beta';
import { CLIENT_VERSION } from './connections';
import { ALL_ENVIRONMENTS_VALUE, environmentAxisValue, resolveSettingsScope, scopeSearch, settingsScopeEnvironmentLabel, type ResolvedSettingsScope } from './settings-scope';
import { persistScopedSettingsPatch, planScopedSettingsClear, planScopedSettingsPatch, resolveScopedSettingsTargets, scopedPlanNotice, scopedSettingsSource,
  selectScopedSettingsEnvironments, type ScopedSettingsEnvironment, type ScopedSettingsPlan, type ScopedSettingsTarget } from './scoped-settings-plan';
import { postScopedNotice, scopeEnvironments, scopeMemberFiles, scopeProjectGroups, scopedWriter, type ScopeEnvironment } from './settings-scope-sources';
import { fleet, type EnvironmentFleet } from './settings-b-fleet';

export type CoreOption = { id: string; value: string; label: string; detail: string; icon: string; selected: boolean; disabled: boolean };
export type CoreRow = {
  id: string; title: string; description: string; status: string; kind: string; checked: boolean; value: string; label: string;
  icon: string; width: number; options: CoreOption[]; value2: string; label2: string; options2: CoreOption[]; driver: string; badge: string; badgeColor: string;
  inheritance: string; inheritanceSummary: string; resettable: boolean; resetLabel: string; disabled: boolean; inert: boolean;
  /** ScopedSwitch: the selected targets disagree (D15); the switch draws its thumb centred and a press turns it on everywhere. */
  mixed: boolean;
  note: string; min: number; max: number; step: number; target: string; placeholder: string; suffix: string; info: string; divider: boolean;
  layers: CoreOption[]; layerTitle: string; amount: number; menuWidth: number;
  /** About → Mobile app's QR codes (settings-mobile-beta.ts); empty on every other row. */
  qr: QrMark[];
};
export type CoreSection = { id: string; title: string; collapsible: boolean; rows: CoreRow[]; toggle: boolean };
export type ScopeChoice = { id: string; label: string; mark: string; ink: string; surface: string; member: string; selected: boolean; offline: boolean };

const option = (value: string, label: string, selected: boolean, extra: Partial<CoreOption> = {}): CoreOption =>
  ({ id: value || 'none', value, label, detail: '', icon: '', selected, disabled: false, ...extra });
export function coreRow(id: string, title: string, description: string, kind: string, extra: Partial<CoreRow> = {}): CoreRow {
  return { id, title, description, status: '', kind, checked: false, value: '', label: '', icon: '', width: 160, options: [], value2: '', label2: '',
    options2: [], driver: '', badge: '', badgeColor: '', inheritance: '', inheritanceSummary: '', resettable: false, resetLabel: title.toLowerCase(), disabled: false, inert: false, mixed: false,
    note: '', min: 0, max: 0, step: 1, target: '', placeholder: '', suffix: '', info: '', divider: false, layers: [], layerTitle: '', amount: 0, menuWidth: 0, qr: [], ...extra };
}

// ── Device (client) settings ───────────────────────────────────────────────
// Reference ClientSettingsSchema defaults for the rows this lane renders. The
// five legacy device keys stay in deviceSettings, which other code reads.
export const CLIENT_DEFAULTS = {
  notificationMode: 'off', inAppNotificationsEnabled: false, diffColorScheme: 'red-green', chatWidth: 'comfortable',
  appearanceContrast: 100, panelAnimationDurationMs: 0, confirmThreadArchive: false, confirmThreadDelete: true, confirmThreadUnpin: false,
  diffFilesCollapsed: true, diffIgnoreWhitespace: true, diffLayout: 'stacked', environmentIdentificationMode: 'artwork', glassOpacity: 80,
  fontSizeInterface: 16, fontSizePrompt: 14, fontSizeCode: 13, fontSizeTerminal: 12, fontFamilyCode: '', fontFamilyComposer: '', fontFamilySans: '',
  fontFamilyTerminal: '', fontSmoothing: true, persistComposerContextStrip: false, contextWindowMeterEnabled: false, composerRichTextEnabled: true,
  followUpBehavior: 'queue', proactivePanelsEnabled: false, showSkillsInSlashMenu: true, legacySidebarEnabled: false, sidebarWorkingShelfEnabled: false,
  wordWrap: true, browserLinkTarget: 'system', theme: 't3-code', themeLight: 't3-code', themeDark: 't3-code', typographyAdvanced: false, confirmQuit: 'hold',
  sidebarProjectSortOrder: 'updated_at',
  // legacy-sidebar: the legacy sidebar's Sidebar options (contracts settings.ts: sort orders, preview count 1-15, default 6).
  sidebarThreadSortOrder: 'updated_at', sidebarThreadPreviewCount: 6,
} as const;
export type ClientPrefs = { -readonly [K in keyof typeof CLIENT_DEFAULTS]: (typeof CLIENT_DEFAULTS)[K] extends number ? number : (typeof CLIENT_DEFAULTS)[K] extends boolean ? boolean : string };
const CHOICES: Record<string, readonly string[]> = {
  browserLinkTarget: ['system', 'app'],
  notificationMode: ['off', 'notifications', 'sound', 'notifications-and-sound'], diffColorScheme: ['red-green', 'blue-orange'],
  chatWidth: ['comfortable', 'wide', 'full'], diffLayout: ['stacked', 'split'], environmentIdentificationMode: ['artwork', 'pill', 'none'],
  followUpBehavior: ['queue', 'steer'], confirmQuit: ['direct', 'hold', 'double-click'], sidebarProjectSortOrder: ['updated_at', 'created_at', 'manual'],
  sidebarThreadSortOrder: ['updated_at', 'created_at'],
};
const BOUNDS: Record<string, [number, number, number]> = {
  appearanceContrast: [50, 200, 5], glassOpacity: [40, 100, 5], panelAnimationDurationMs: [0, 400, 25],
  fontSizeInterface: [12, 20, 1], fontSizePrompt: [12, 20, 1], fontSizeCode: [10, 18, 1], fontSizeTerminal: [8, 20, 1],
  sidebarThreadPreviewCount: [1, 15, 1],
};
const FONT_FAMILY = /^[^"\\;{}<>]{0,120}$/;
const FONT_SIZE_KEYS: Record<string, string> = { fontFamilySans: 'fontSizeInterface', fontFamilyComposer: 'fontSizePrompt', fontFamilyCode: 'fontSizeCode', fontFamilyTerminal: 'fontSizeTerminal' };
/** One decoded value, or undefined when it is not a valid value for the key. */
export function clientValue(key: string, raw: unknown): string | number | boolean | undefined {
  if (!(key in CLIENT_DEFAULTS)) return undefined;
  const fallback = CLIENT_DEFAULTS[key as keyof typeof CLIENT_DEFAULTS];
  if (typeof fallback === 'boolean') return typeof raw === 'boolean' ? raw : raw === 'true' ? true : raw === 'false' ? false : undefined;
  if (typeof fallback === 'number') {
    const value = typeof raw === 'number' ? raw : typeof raw === 'string' && raw.trim() !== '' ? Number(raw) : NaN;
    const [min, max, step] = BOUNDS[key]!;
    return Number.isInteger(value) && value >= min && value <= max && (value - min) % step === 0 ? value : undefined;
  }
  if (typeof raw !== 'string') return undefined;
  if (key.startsWith('fontFamily')) return FONT_FAMILY.test(raw) ? raw.trim() : undefined;
  // Built-in or saved custom theme ids; a removed custom id falls back to T3 Code when painted.
  if (key === 'theme' || key === 'themeLight' || key === 'themeDark') return /^[a-z0-9](?:[a-z0-9-]{0,46}[a-z0-9])?$/.test(raw) ? raw : undefined;
  return CHOICES[key]?.includes(raw) ? raw : undefined;
}
export function decodeClientPrefs(saved: unknown): ClientPrefs {
  const source = obj(saved), next = { ...CLIENT_DEFAULTS } as ClientPrefs;
  for (const key of Object.keys(CLIENT_DEFAULTS)) {
    const value = clientValue(key, source[key]);
    if (value !== undefined) (next as Record<string, unknown>)[key] = value;
  }
  return next;
}

type Device = { composerCollapseOnScroll: boolean; planModeEnabled: boolean; timestampFormat: string; appearanceMode: string; sendShortcut: string };
export const DEVICE_DEFAULTS: Device = { composerCollapseOnScroll: true, planModeEnabled: false, timestampFormat: 'locale', appearanceMode: 'system', sendShortcut: 'enter' };
const DEVICE_CHOICES: Record<string, readonly string[]> = { timestampFormat: ['locale', '12-hour', '24-hour'], appearanceMode: ['system', 'light', 'dark'], sendShortcut: ['enter', 'mod-enter-multiline', 'mod-enter'] };
type LocalPrefs = { deviceSettings: Device; clientSettings?: ClientPrefs; groupingMode: string; lastGroupingMode?: string; customThemes?: CustomTheme[] };
/** Apply one device-local change in place. Returns false when the key is not a device setting. */
export function applyDeviceSetting(local: LocalPrefs, key: string, value: string): boolean {
  const device = local.deviceSettings as Record<string, unknown>;
  if (key in DEVICE_CHOICES || key === 'composerCollapseOnScroll' || key === 'planModeEnabled') {
    if (key in DEVICE_CHOICES ? !DEVICE_CHOICES[key]!.includes(value) : !['true', 'false'].includes(value)) throw new ClientError('Unsupported device setting.');
    device[key] = key in DEVICE_CHOICES ? value : value === 'true';
    return true;
  }
  if (key === 'projectGrouping') {
    if (!['true', 'false'].includes(value)) throw new ClientError('Unsupported device setting.');
    if (value === 'false' && local.groupingMode !== 'separate') local.lastGroupingMode = local.groupingMode;
    local.groupingMode = value === 'true' ? (local.lastGroupingMode && local.lastGroupingMode !== 'separate' ? local.lastGroupingMode : 'repository') : 'separate';
    return true;
  }
  if (key in CLIENT_DEFAULTS) {
    const decoded = clientValue(key, value);
    if (decoded === undefined) throw new ClientError('Unsupported device setting.');
    local.clientSettings = { ...(local.clientSettings || decodeClientPrefs({})), [key]: decoded };
    if (key === 'theme') local.clientSettings = { ...local.clientSettings, themeLight: decoded as string, themeDark: decoded as string };
    return true;
  }
  return false;
}
/** Reference "Restore device defaults": every device-local General/Appearance value. */
export function changedDeviceLabels(local: LocalPrefs): string[] {
  const prefs = local.clientSettings || decodeClientPrefs({});
  const labels: string[] = [];
  for (const [key, value] of Object.entries(DEVICE_DEFAULTS)) if ((local.deviceSettings as Record<string, unknown>)[key] !== value) labels.push(key);
  for (const [key, value] of Object.entries(CLIENT_DEFAULTS)) if ((prefs as Record<string, unknown>)[key] !== value) labels.push(key);
  if (local.groupingMode !== 'repository') labels.push('projectGrouping');
  return labels;
}
export function restoreDeviceDefaults(local: LocalPrefs): void {
  Object.assign(local.deviceSettings, DEVICE_DEFAULTS);
  local.clientSettings = decodeClientPrefs({});
  local.groupingMode = 'repository';
}

// ── Scope ─────────────────────────────────────────────────────────────────
// settings-scope.ts resolves the selection across every environment the app knows
// (settings-scope-sources.ts: the focused one and each switched-on background one).
export type CoreScope = {
  kind: string; message: string; environmentLabel: string; projectLabel: string; projectMark: string; projectInk: string; projectSurface: string;
  connective: string; members: Obj[]; connected: boolean; environmentChoices: ScopeChoice[]; projectChoices: ScopeChoice[];
  /** The ported resolution, every known environment, the selected ones and the axis's machine icon. */
  resolved: ResolvedSettingsScope; environments: ScopeEnvironment[]; selected: ScopeEnvironment[]; environmentIcon: string;
};
/** resolveSettingsScope over the app's environments and project groups. A removed target never broadens to All. */
export function resolveScope(client: T3Client, machine: string, projectKey: string, checkout: string, source?: EnvironmentFleet): CoreScope {
  const environments = scopeEnvironments(client, source);
  const groups = scopeProjectGroups(client, source);
  const search = scopeSearch(machine, projectKey, checkout);
  const resolved = resolveSettingsScope(search, groups, environments);
  const axis = environmentAxisValue(search, resolved.kind === 'checkout' ? resolved.environmentId : null);
  const current = environments.find(environment => environment.environmentId === axis);
  const group = groups.find(candidate => candidate.projectKey === projectKey);
  const { environments: selected, connectedEnvironments } = selectScopedSettingsEnvironments(resolved, environments, client.environmentId || null);
  const environmentChoices: ScopeChoice[] = [{ id: '', label: 'All environments', mark: '', ink: '', surface: '', member: '', selected: axis === ALL_ENVIRONMENTS_VALUE, offline: false },
    ...environments.map(environment => ({ id: environment.environmentId, label: settingsScopeEnvironmentLabel(environment, environments), mark: environment.kind, ink: '', surface: '', member: '',
      selected: environment.environmentId === axis, offline: environment.connection.phase !== 'connected' }))];
  const projectChoices: ScopeChoice[] = [{ id: '', label: 'All projects', mark: '', ink: '', surface: '', member: '', selected: projectKey === '', offline: false },
    ...groups.map(candidate => ({ id: candidate.projectKey, label: candidate.displayName, ...markOf(candidate.displayName), member: str(candidate.memberProjects[0]?.id), selected: candidate.projectKey === projectKey, offline: false }))];
  const marks = group ? markOf(group.displayName) : { mark: '', ink: '', surface: '' };
  return { kind: resolved.kind, message: resolved.kind === 'unavailable' ? resolved.message : '', members: resolved.members.map(member => member as unknown as Obj),
    environmentLabel: current ? settingsScopeEnvironmentLabel(current, environments) : axis !== ALL_ENVIRONMENTS_VALUE ? 'Unavailable environment' : 'All environments',
    projectLabel: projectKey ? (group ? group.displayName : 'Unavailable project') : 'All projects', projectMark: marks.mark, projectInk: marks.ink, projectSurface: marks.surface,
    connective: machine || resolved.kind === 'checkout' ? 'on' : 'across', connected: connectedEnvironments.length > 0, environmentChoices, projectChoices,
    resolved, environments, selected, environmentIcon: current?.kind ?? '' };
}
function markOf(name: string) { const identity = projectIdentity(name); return { mark: identity.projectMark, ink: identity.projectInk, surface: identity.projectSurface }; }

// ── Server rows ───────────────────────────────────────────────────────────
export const PROJECT_SCOPED = new Set(['worktreeCleanup', 'defaultModelSelection', 'defaultRuntimeMode', 'defaultThreadEnvMode', 'newWorktreesStartFromOrigin', 'worktreeSubmodules',
  'defaultAutoPull', 'defaultProjectScripts', 'enableAgentBrowserAccess', 'enableAgentDeviceAccess', 'textGenerationModelSelection', 'sourceControlWriterModelSelection',
  'sourceControlWritingStyle', 'branchNamingMode', 'branchNamePrefix', 'branchNameInstructions', 'pullRequestMergeMethod', 'sidebarAutoSettleOnMerge',
  'sidebarAutoSettleAfterDays', 'continueThreadsAfterServerUpdate', 'responseStreamingMode']);
export const SERVER_DEFAULTS: Record<string, Json> = {
  defaultModelSelection: null, defaultRuntimeMode: 'full-access', defaultThreadEnvMode: null, worktreeSubmodules: null, autoResumeLimitedThreads: false,
  snoozeLimitedThreads: false, sidebarAutoSettleOnMerge: true, sidebarAutoSettleAfterDays: 3, responseStreamingMode: 'paragraph', enableProviderUpdateChecks: true,
  continueThreadsAfterServerUpdate: false, backgroundActivity: { schemaVersion: 1, profile: 'balanced', overrides: {} }, newWorktreesStartFromOrigin: true,
  addProjectBaseDirectory: '', textGenerationModelSelection: { instanceId: 'codex', model: 'gpt-6-luna', options: [{ id: 'reasoningEffort', value: 'low' }] },
};
const FILE_BACKED: Record<string, [string, string]> = { defaultThreadEnvMode: ['defaultThreadEnvMode', 'local'], worktreeSubmodules: ['worktreeSubmodules', 'recursive'] };
// Effect's Equal.equals on decoded settings: structural, independent of key order.
const canonical = (value: unknown): unknown => Array.isArray(value) ? value.map(canonical)
  : value && typeof value === 'object' ? Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical((value as Record<string, unknown>)[key])])) : value ?? null;
const same = (a: unknown, b: unknown) => JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
const overridesOf = (settings: Obj, projectId: string) => obj(obj(settings.projectSettingsOverrides)[projectId]);
const has = (value: Obj, key: string) => Object.prototype.hasOwnProperty.call(value, key) && value[key] !== undefined;

/** The checkout's decoded t3.json, or null for a missing, oversized or invalid file (projectSettings.ts applyProjectFile). */
export function parseProjectFile(contents: string): Obj | null {
  try { const value = JSON.parse(contents); return value && typeof value === 'object' && !Array.isArray(value) ? value : null; } catch { return null; }
}
/** resolveProjectSettings for one member: project override, environment value, t3.json (file-backed keys), built-in. */
export function effectiveSetting(settings: Obj, projectId: string, key: string, file: Obj | null | undefined): { value: Json; source: string } {
  const overrides = overridesOf(settings, projectId);
  if (projectId && has(overrides, key) && PROJECT_SCOPED.has(key)) {
    const value = overrides[key] as Json;
    if (!FILE_BACKED[key] || value !== null) return { value, source: 'project' };
  }
  const environmentValue = (key in settings ? settings[key] : SERVER_DEFAULTS[key]) as Json;
  const fileBacked = FILE_BACKED[key];
  if (fileBacked && (environmentValue === null || environmentValue === undefined)) {
    const fromFile = file?.[fileBacked[0]];
    if (projectId && typeof fromFile === 'string') return { value: fromFile, source: 't3.json' };
    return { value: fileBacked[1], source: 'environment' };
  }
  return { value: environmentValue ?? null, source: 'environment' };
}

/** One control's context: the representative target's settings (display) and every connected target (mixed, writes). */
type ServerContext = { settings: Obj; scope: CoreScope; files: Map<string, Obj | null>; capabilities: Obj; providers: Obj[]; environmentLabel: string;
  targets: readonly ScopedSettingsTarget[]; restartEverywhere: boolean };
/** A target's value for a key: its effective value, a file-backed key's built-in when nothing set it (effectiveSetting). */
function targetValue(target: ScopedSettingsTarget, key: string): Json {
  const value = (key in target.settings ? target.settings[key] : SERVER_DEFAULTS[key]) as Json;
  return (value === null || value === undefined) && FILE_BACKED[key] ? FILE_BACKED[key]![1] : value ?? null;
}
function serverState(context: ServerContext, key: string) {
  const { settings, scope, targets } = context;
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  const values = targets.map(target => targetValue(target, key));
  const first = values.length ? values[0]! : effectiveSetting(settings, '', key, null).value;
  // scopedSettingsAreMixed over the effective values (a file-backed key's unset value reads as its built-in).
  const mixed = values.some(value => !same(value, first));
  const scoped = PROJECT_SCOPED.has(key);
  const environmentWide = project && !scoped;
  const source = project ? scopedSettingsSource(targets, [key]) : 'environment';
  const customized = !same(settings[key], SERVER_DEFAULTS[key]) && key in SERVER_DEFAULTS;
  const inheritance = mixed ? ['mixed', 'Mixed across selected targets'] : project && source === 'project' ? ['overridden', 'Overridden for this project']
    : project && source === 't3.json' ? ['inherited', "Inherited from the repository's t3.json"]
    : project && scoped ? ['inherited', `Inherited from ${context.environmentLabel}`]
    : customized ? ['environment', 'Set on the environment'] : ['default', 'Built-in default'];
  const note = !scope.connected ? 'Reconnect the selected environment to change this setting.' : environmentWide ? 'Environment-wide setting. Select an environment to change it.' : '';
  return { value: first, mixed, source, inheritance, note, inert: note !== '', resettableProject: project && scoped && (source === 'project' || source === 'mixed') };
}
/** SettingInheritance formatValue: human labels for the chain's values. */
export function formatSettingValue(key: string, value: unknown): string {
  if (value === null || value === undefined) return key === 'pullRequestMergeMethod' ? 'Last selected' : key === 'sidebarAutoSettleAfterDays' ? 'Never'
    : key === 'defaultModelSelection' ? 'Automatic' : key === 'defaultThreadEnvMode' || key === 'worktreeSubmodules' ? 'Inherit' : 'Not set';
  if (typeof value === 'boolean') return value ? 'On' : 'Off';
  if (typeof value === 'number') return key === 'sidebarAutoSettleAfterDays' ? `${value} ${value === 1 ? 'day' : 'days'}` : String(value);
  if (typeof value === 'string') {
    if (key === 'defaultThreadEnvMode' && (value === 'local' || value === 'worktree')) return value === 'worktree' ? 'New worktree' : 'Current checkout';
    if (key === 'worktreeSubmodules') return { recursive: 'Recursive', 'top-level': 'Top level only', none: 'Skip' }[value] ?? value;
    return value === '' ? 'Empty' : value;
  }
  if (Array.isArray(value)) return `${value.length} ${value.length === 1 ? 'item' : 'items'}`;
  const entry = obj(value);
  if (typeof entry.model === 'string') return entry.model;
  if (typeof entry.profile === 'string') return { balanced: 'Balanced', performance: 'Performance', 'battery-saver': 'Battery saver', custom: 'Advanced' }[entry.profile] ?? entry.profile;
  return 'Custom';
}
/** settingInheritanceLayers for the representative target: project, environment, t3.json, built-in. */
function inheritanceLayers(context: ServerContext, key: string): CoreOption[] {
  const { settings, scope } = context;
  const member = scope.kind === 'project' || scope.kind === 'checkout' ? str(scope.members[0]?.id) : '';
  const resolved = effectiveSetting(settings, member, key, member ? context.files.get(member) : null);
  const environmentValue = key in settings ? settings[key] : SERVER_DEFAULTS[key];
  const environmentSet = !same(environmentValue, SERVER_DEFAULTS[key]);
  const layers: CoreOption[] = [];
  if (member && PROJECT_SCOPED.has(key)) layers.push(option('project', 'Project', resolved.source === 'project', { detail: resolved.source === 'project' ? formatSettingValue(key, resolved.value) : 'Inherits' }));
  layers.push(option('environment', 'Environment', resolved.source === 'environment' && environmentSet, { detail: environmentSet ? formatSettingValue(key, environmentValue) : 'Inherits' }));
  if (member && FILE_BACKED[key]) layers.push(option('t3.json', 't3.json', resolved.source === 't3.json', { detail: resolved.source === 't3.json' ? formatSettingValue(key, resolved.value) : 'Inherits' }));
  const builtIn = FILE_BACKED[key] ? FILE_BACKED[key]![1] : SERVER_DEFAULTS[key];
  layers.push(option('default', 'Default', !layers.some(layer => layer.selected), { detail: formatSettingValue(key, builtIn) }));
  return layers;
}
function serverRow(context: ServerContext, key: string, id: string, title: string, description: string, kind: string, extra: Partial<CoreRow> = {}, resetDefault?: boolean): CoreRow {
  const state = serverState(context, key);
  const project = context.scope.kind === 'project' || context.scope.kind === 'checkout';
  const resettable = !state.inert && (project ? state.resettableProject : resetDefault ?? !same(context.settings[key], SERVER_DEFAULTS[key]));
  return coreRow(id, title, description, kind, { inheritance: state.inheritance[0]!, inheritanceSummary: state.inheritance[1]!, inert: state.inert, note: state.note,
    layers: inheritanceLayers(context, key), layerTitle: context.environmentLabel,
    resettable, resetLabel: extra.resetLabel ?? title.toLowerCase(), ...(kind === 'switch' ? { checked: state.value === true, mixed: state.mixed } : {}), ...extra });
}

const RUNTIME = [['approval-required', 'Supervised', 'Ask before commands and file changes.', 'lock'], ['auto-accept-edits', 'Auto-accept edits', 'Auto-approve edits, ask before other actions.', 'pen-line'],
  ['auto', 'Auto', 'Supported providers approve routine actions; others still ask.', 'sparkles'], ['full-access', 'Full access', 'Allow commands and edits without prompts.', 'lock-open']] as const;
const BACKGROUND = { balanced: ['Balanced', 'Pauses probes for idle clients, locked hosts, or low power mode.'], performance: ['Performance', 'Allows scoped background probes while any subscribed client remains connected.'],
  'battery-saver': ['Battery saver', 'Also pauses background probes when the host or client is on battery.'] } as Record<string, [string, string]>;

/** ProviderModelPicker + TraitsPicker rows: an advertised model and its reasoning effort. */
function modelControl(context: ServerContext, selection: Obj | null, textGeneration: boolean): Partial<CoreRow> {
  const providers = context.providers.filter(provider => providerAvailable(provider) && (!textGeneration || provider.supportsTextGeneration !== false));
  const chosen = selection && providers.find(provider => provider.instanceId === selection.instanceId && arr(provider.models).some(model => model.slug === selection.model));
  const provider = chosen || providers[0];
  if (!provider) return { kind: 'text-only', label: textGeneration ? 'No text generation providers available.' : 'No providers available' };
  const models = arr(provider.models);
  const model = chosen ? models.find(entry => entry.slug === selection!.model)! : models.find(entry => entry.isDefault === true) || models[0];
  // TraitsPicker: every select trait in its own section (Reasoning, Service Tier), radio rows with a Default badge.
  const descriptors = arr(obj(model?.capabilities).optionDescriptors).filter(descriptor => descriptor.type === 'select' && arr(descriptor.options).length > 0);
  const selections = arr(chosen ? selection!.options : []);
  const current = (descriptor: Obj) => str(selections.find(entry => entry.id === descriptor.id)?.value,
    str(arr(descriptor.options).find(entry => entry.isDefault === true)?.id, str(arr(descriptor.options)[0]?.id)));
  const effort = descriptors.find(descriptor => ['reasoningEffort', 'effort'].includes(str(descriptor.id)));
  const efforts = arr(effort?.options);
  const effortValue = effort ? current(effort) : '';
  // buildTraitsTriggerDisplay: a Fast or Ultrafast service tier draws a bolt beside the effort.
  const tier = descriptors.find(descriptor => descriptor.id === 'serviceTier');
  const tierLabel = tier ? str(arr(tier.options).find(entry => entry.id === current(tier))?.label) : '';
  // Codex only, and only beside another label: a tier alone reads as its own label ("Fast"), with no bolt.
  const speed = str(provider.driver) !== 'codex' || !effort ? '' : tierLabel === 'Ultrafast' ? 'ultrafast' : tierLabel === 'Fast' ? 'fast' : '';
  const traits = descriptors.flatMap((descriptor, index) => [option(`section:${str(descriptor.id)}`, str(descriptor.label, str(descriptor.id)), false, { icon: index ? 'section-rule' : 'section', disabled: true }),
    ...arr(descriptor.options).map(entry => option(`${str(descriptor.id)}=${str(entry.id)}`, str(entry.label, str(entry.id)), current(descriptor) === entry.id,
      { detail: str(entry.description), icon: entry.isDefault === true ? 'default' : '' }))]);
  return {
    value: `${str(provider.instanceId)}|${str(model?.slug)}`, label: str(model?.name, str(model?.slug)), driver: str(provider.driver),
    badge: providerBadge(provider, context.providers).providerBadge, badgeColor: providerBadge(provider, context.providers).providerBadgeColor,
    options: providers.flatMap(entry => arr(entry.models).map(candidate => option(`${str(entry.instanceId)}|${str(candidate.slug)}`, str(candidate.name, str(candidate.slug)),
      entry.instanceId === provider.instanceId && candidate.slug === model?.slug, { detail: str(entry.displayName, str(entry.driver)), icon: str(entry.driver) }))),
    value2: effortValue, label2: str(efforts.find(entry => entry.id === effortValue)?.label, effortValue) || tierLabel, icon: speed,
    options2: traits,
  };
}

/** Every General row of the served reference, in order, with its live value. */
export function generalSections(client: T3Client, context: ServerContext): CoreSection[] {
  const { scope, settings, capabilities } = context;
  const local = client.local as unknown as LocalPrefs;
  const prefs = local.clientSettings || decodeClientPrefs({});
  const device = local.deviceSettings;
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  const deviceRow = (key: string, id: string, title: string, description: string, kind: string, extra: Partial<CoreRow> = {}): CoreRow => {
    const value = key in DEVICE_DEFAULTS ? (device as Record<string, unknown>)[key] : (prefs as Record<string, unknown>)[key];
    const fallback = key in DEVICE_DEFAULTS ? (DEVICE_DEFAULTS as Record<string, unknown>)[key] : (CLIENT_DEFAULTS as Record<string, unknown>)[key];
    return coreRow(id, title, description, kind, { checked: value === true, value: String(value), resettable: value !== fallback, target: key, ...extra });
  };
  const select = (current: string, choices: readonly (readonly [string, string])[]) => ({ value: current, label: choices.find(([value]) => value === current)?.[1] ?? current,
    options: choices.map(([value, label]) => option(value, label, value === current)) });
  const value = (key: string) => serverState(context, key);
  const modelState = value('defaultModelSelection');
  const runtime = value('defaultRuntimeMode'), workspace = value('defaultThreadEnvMode'), submodules = value('worktreeSubmodules');
  const runtimeEntry = RUNTIME.find(entry => entry[0] === runtime.value) ?? RUNTIME[3];
  const autoSettle = capabilities.threadAutoSettlement === true && scope.connected;
  const days = value('sidebarAutoSettleAfterDays');
  const streaming = value('responseStreamingMode');
  const background = obj(value('backgroundActivity').value);
  // The select shows the normalized value; Advanced needs exactly one environment (isEnvironmentScope).
  const profile = profileOption({ ...settings, backgroundActivity: background });
  // isEnvironmentScope: Advanced needs exactly one selected environment.
  const envScope = scope.kind !== 'unavailable' && scope.selected.length === 1;
  const restart = context.restartEverywhere;
  const mod = '⌘';
  const textSelection = obj(value('textGenerationModelSelection').value);
  const newThreads: CoreRow[] = [
    serverRow(context, 'defaultModelSelection', 'default-model', 'Model', project ? 'Model for new threads in this project.' : 'Default model for new threads. Projects can override it.', 'model',
      { status: !scope.connected || modelState.mixed || modelState.source === 'project' ? '' : modelState.value === null ? 'Automatic' : '', resetLabel: 'default model',
        ...modelControl(context, modelState.value === null ? null : obj(modelState.value), false), ...(modelState.mixed ? { label: 'Mixed' } : {}) }, modelState.value !== null),
    serverRow(context, 'defaultRuntimeMode', 'default-permissions', 'Permissions', project ? 'Permissions for new threads in this project.' : 'Default permissions for new threads. Projects can override them.', 'select',
      { value: str(runtime.value), label: runtime.mixed ? 'Mixed' : runtimeEntry[1], icon: runtime.mixed ? '' : runtimeEntry[3], resetLabel: 'default permissions', menuWidth: 372,
        options: RUNTIME.map(([id, label, detail, icon]) => option(id, label, id === runtime.value, { detail, icon })) }),
    serverRow(context, 'defaultThreadEnvMode', 'new-threads', 'Workspace', project ? 'Where new threads in this project start.' : 'Where new threads start. Projects and their t3.json can override it.', 'select',
      { ...select(str(workspace.value), [['local', 'Current checkout'], ['worktree', 'New worktree']]), ...(workspace.mixed ? { label: scope.connected ? 'Mixed' : 'Unavailable' } : {}), resetLabel: 'default workspace' },
      !project && settings.defaultThreadEnvMode !== null && settings.defaultThreadEnvMode !== undefined),
    serverRow(context, 'worktreeSubmodules', 'worktree-submodules', 'Submodules', project ? 'How new worktrees in this project populate git submodules.' : 'How new worktrees populate git submodules. Projects and their t3.json can override it.', 'select',
      { ...select(str(submodules.value), [['recursive', 'Recursive'], ['top-level', 'Top level only'], ['none', 'Skip']]), ...(submodules.mixed ? { label: 'Mixed' } : {}), resetLabel: 'worktree submodules' },
      !project && settings.worktreeSubmodules !== null && settings.worktreeSubmodules !== undefined),
  ];
  const organization: CoreRow[] = [
    coreRow('project-grouping', 'Project grouping', 'Combine matching repositories across environments.', 'switch', { checked: local.groupingMode !== 'separate', resettable: local.groupingMode !== 'repository', target: 'projectGrouping' }),
    // c5a929e1: Project order (client setting sidebarProjectSortOrder), w-full sm:w-44.
    deviceRow('sidebarProjectSortOrder', 'project-order', 'Project order', 'Order of projects in the sidebar project picker and command palette.', 'select',
      { ...select(prefs.sidebarProjectSortOrder, [['updated_at', 'Last user message'], ['created_at', 'Created at'], ['manual', 'Manual']]), width: 176, resetLabel: 'project order' }),
    serverRow(context, 'autoResumeLimitedThreads', 'auto-resume-limited-threads', 'Auto-resume limited threads', 'Resume usage-limit stops at the reported reset time. Each thread can cancel its scheduled continuation.', 'switch', {}, false),
    serverRow(context, 'snoozeLimitedThreads', 'snooze-limited-threads', 'Snooze limited threads', 'Snooze usage-limit stops until the reported reset time. Combine with auto-resume to continue when they wake.', 'switch', {}, false),
    deviceRow('sidebarWorkingShelfEnabled', 'working-shelf', 'Working section (beta)', 'Fold working and monitoring threads into a Working section. They return to the top of the inbox when they need you.', 'switch'),
    ...(autoSettle ? [
      serverRow(context, 'sidebarAutoSettleOnMerge', 'auto-settle-merged-threads', 'Auto-settle merged threads', 'Settle a thread when its pull request merges. Closed pull requests still settle automatically.', 'switch', { resetLabel: 'auto-settle on merge' }),
      serverRow(context, 'sidebarAutoSettleAfterDays', 'auto-settle-inactive-threads', 'Auto-settle inactive threads', 'Sidebar threads with no activity for this long settle automatically.', 'switch', { checked: days.value !== null, resetLabel: 'auto-settle' }),
      ...(days.value !== null ? [serverRow(context, 'sidebarAutoSettleAfterDays', 'days-before-auto-settle', 'Days of inactivity before auto-settle', 'Any new activity un-settles a thread automatically.', 'number',
        { value: String(num(days.value, 3)), min: 1, max: 90, width: 96, target: 'days' }, false)] : []),
    ] : []),
  ];
  const behavior: CoreRow[] = [
    deviceRow('notificationMode', 'thread-notifications', 'Thread notifications', notificationPermissionMessage(client) || 'System alerts when a thread finishes, fails, or needs input or approval. Applies to this device while T3 Code is open.', 'select',
      { ...select(prefs.notificationMode, [['off', 'Off'], ['notifications', 'Notifications only'], ['sound', 'Sound only'], ['notifications-and-sound', 'Notifications with sound']]), width: 224, resettable: false }),
    deviceRow('inAppNotificationsEnabled', 'in-app-notifications', 'In-app notifications', 'Show a toast when another thread finishes, fails, or needs input or approval while this app has focus.', 'switch', { resettable: false }),
    deviceRow('timestampFormat', 'time-format', 'Time format', 'System default follows your browser or OS clock preference.', 'select', { ...select(device.timestampFormat, [['locale', 'System default'], ['12-hour', '12-hour'], ['24-hour', '24-hour']]) }),
    serverRow(context, 'responseStreamingMode', 'response-streaming', 'Response streaming', streaming.mixed ? 'The selected targets use different streaming modes.' : streaming.value === 'turn' ? 'Text appears once the agent finishes its turn.' : 'Each paragraph or code block appears as soon as it is complete.', 'select',
      { ...select(str(streaming.value), [['turn', 'Wait for the full response'], ['paragraph', 'Show finished paragraphs']]), ...(streaming.mixed ? { label: 'Mixed' } : {}), width: 224 }),
    deviceRow('diffIgnoreWhitespace', 'hide-whitespace-changes', 'Hide whitespace changes', 'Set whether the diff panel ignores whitespace-only edits by default.', 'switch', { resetLabel: 'diff whitespace changes' }),
    deviceRow('diffFilesCollapsed', 'default-diff-file-state', 'Default diff file state', "Start with files expanded or collapsed when opening diffs or a pull request's Code tab.", 'select',
      { value: prefs.diffFilesCollapsed ? 'collapsed' : 'expanded', label: prefs.diffFilesCollapsed ? 'Collapsed' : 'Expanded', options: [option('expanded', 'Expanded', !prefs.diffFilesCollapsed), option('collapsed', 'Collapsed', prefs.diffFilesCollapsed)] }),
    deviceRow('diffLayout', 'diff-layout', 'Diff layout', 'Show diffs stacked or side by side. The toggle in the diff toolbar changes this too.', 'select', { ...select(prefs.diffLayout, [['stacked', 'Stacked'], ['split', 'Split']]) }),
    deviceRow('proactivePanelsEnabled', 'proactive-panels', 'Proactive panels', 'Open linked pull requests first. Otherwise, open Changes for edits to at least 3 files or 50 lines.', 'switch'),
    deviceRow('showSkillsInSlashMenu', 'skills-in-slash-menu', 'Show skills in slash menu', 'Also include skills in the / command menu. Skills always appear when you type $.', 'switch'),
    deviceRow('composerRichTextEnabled', 'composer-rich-text', 'Rich text composer', 'Show formatted Markdown as you type.', 'switch'),
    deviceRow('composerCollapseOnScroll', 'composer-collapse', 'Collapse composer on scroll', 'Rest the composer of an existing thread into a single line when you scroll the conversation. Focus the composer or start typing to expand it again.', 'switch'),
    deviceRow('sendShortcut', 'send-shortcut', 'Send shortcut', 'Choose when Enter sends a prompt or inserts a new line', 'select',
      { ...select(device.sendShortcut, [['enter', 'Enter'], ['mod-enter-multiline', `${mod} + Enter for multiline prompts`], ['mod-enter', `${mod} + Enter always`]]), width: 0, icon: 'checks' }),
    deviceRow('followUpBehavior', 'follow-up-behavior', 'Follow-up behavior', 'Queue follow-ups while the agent runs or steer the current run. ' + (device.sendShortcut === 'mod-enter-multiline'
      ? `Press ${mod} + Enter for single-line prompts or ${mod} + Shift + Enter for multiline prompts to do the opposite for one message.`
      : `Press ${mod}${device.sendShortcut === 'mod-enter' ? ' + Shift' : ''} + Enter to do the opposite for one message.`), 'select', { ...select(prefs.followUpBehavior, [['queue', 'Queue'], ['steer', 'Steer']]), width: 0 }),
    serverRow(context, 'enableProviderUpdateChecks', 'provider-update-checks', 'Provider update checks', 'Check installed provider CLIs for newer available versions.', 'switch'),
    serverRow(context, 'continueThreadsAfterServerUpdate', 'continue-threads-after-server-update', 'Continue threads after restarts', 'Automatically resume interrupted threads after an update, crash, or machine restart on the selected environments.', 'switch',
      { status: restart || !scope.connected ? '' : 'All selected connected environments must support restart continuation.', disabled: !restart, resetLabel: 'continue threads after restarts' }, restart && settings.continueThreadsAfterServerUpdate === true),
    serverRow(context, 'backgroundActivity', 'background-activity', 'Background activity', profile === 'advanced' ? `Uses custom intervals. Shared policy: ${(BACKGROUND[baseProfile(background)] ?? BACKGROUND.balanced!)[0]}.` : (BACKGROUND[profile] ?? BACKGROUND.balanced!)[1], 'select',
      { value: profile, label: profile === 'advanced' ? 'Advanced' : (BACKGROUND[profile] ?? BACKGROUND.balanced!)[0], info: 'This shared policy gates background work such as Git refreshes and provider health probes after their individual intervals elapse.',
        width: 160, value2: profile === 'advanced' && envScope ? 'configure' : '',
        options: [...Object.entries(BACKGROUND).map(([id, [label]]) => option(id, label, id === profile)), option('advanced', envScope ? 'Advanced' : 'Advanced (one environment)', profile === 'advanced', { disabled: !envScope })] }),
  ];
  const projects: CoreRow[] = [
    serverRow(context, 'newWorktreesStartFromOrigin', 'start-from-origin', 'Start from origin', 'Creates the worktree from the latest matching branch on origin instead of your local branch.', 'switch', { resetLabel: 'new worktrees start from origin' }),
    serverRow(context, 'addProjectBaseDirectory', 'add-project-starts-in', 'Add project starts in', 'Leave empty to use "~/" when the Add Project browser opens.', 'text',
      { value: str(value('addProjectBaseDirectory').value), placeholder: '~/', width: 288, resetLabel: 'add project base directory' }),
  ];
  const confirmations: CoreRow[] = [
    deviceRow('confirmThreadUnpin', 'unpin-confirmation', 'Unpin confirmation', 'Ask before unpinning a thread from the pinned section.', 'switch'),
    deviceRow('confirmThreadArchive', 'archive-confirmation', 'Archive confirmation', 'Require a second click on the inline archive action before a thread is archived.', 'switch'),
    deviceRow('confirmThreadDelete', 'delete-confirmation', 'Delete confirmation', 'Ask before deleting a thread and its chat history.', 'switch'),
    // Desktop-only (SettingsPanels.tsx isElectron): the ⌘Q behaviour (T3Menus.swift).
    deviceRow('confirmQuit', 'quit-confirmation', 'Quit shortcut', 'Hold mode also quits on two quick presses.', 'select', { ...select(prefs.confirmQuit, [['direct', 'Direct'], ['hold', 'Hold'], ['double-click', 'Double press']]), resetLabel: 'quit shortcut behavior' }),
  ];
  const textGeneration: CoreRow[] = [serverRow(context, 'textGenerationModelSelection', 'text-generation-model', 'Text generation model',
    'Used for thread titles and other generated text on connected devices with this provider. Source control can override it.', 'model',
    scope.connected ? modelControl(context, textSelection, true) : { kind: 'text-only', label: 'Connect an environment to choose its text generation model.' })];
  // AboutVersionTitle: this app's own release (APP_VERSION), not the connected server's.
  const about = aboutRows(client, CLIENT_VERSION); // settings-a-about.ts: the update button and Update track
  const diagnostics = [
    coreRow('diagnostics', 'Diagnostics', envScope ? 'Inspect processes, resource use, and logs on this environment.' : 'Inspect processes, resource use, and logs on one environment at a time.', 'link', { label: 'View diagnostics', target: 'diagnostics', width: 0 }),
    coreRow('open-source-licenses', 'Open source licenses', 'Notices for dependencies, assets, and optional tools used by T3 Code.', 'link', { label: 'View licenses', target: 'open-source-licenses', width: 0 }),
  ];
  const legacy = [
    deviceRow('planModeEnabled', 'legacy-plan-mode', 'Plan mode (legacy)', 'Restore Build/Plan, /plan, /default, and Shift+Tab. Off uses build mode.', 'switch', { resettable: false }),
    deviceRow('contextWindowMeterEnabled', 'legacy-context-window-indicator', 'Context window indicator (legacy)', 'Shows context window usage as a circular indicator in the composer.', 'switch', { resettable: false }),
    deviceRow('legacySidebarEnabled', 'legacy-sidebar', 'Sidebar (legacy)', 'Restore per-project thread trees instead of the default flat sidebar.', 'switch', { resettable: false }),
  ];
  const section = (id: string, title: string, rows: CoreRow[], collapsible = false): CoreSection => ({ id, title, collapsible, rows, toggle: false });
  return [section('project-defaults', 'New threads', newThreads), section('organization', 'Organization', organization), section('behavior', 'Behavior', behavior),
    section('projects-and-threads', 'Projects & threads', projects), section('confirmations', 'Confirmations', confirmations), section('text-generation', 'Text generation', textGeneration),
    section('about', 'About', about), section('diagnostics-section', 'Diagnostics', diagnostics), section('legacy-features', 'Legacy features', legacy, true)];
}

// ── Reading and writing ───────────────────────────────────────────────────
type Bridge = { request(native: Native, method: string, payload: Obj, write?: boolean): Promise<Obj> };
const bridge = (client: T3Client) => client as unknown as Bridge & Pick<T3Client, 'config'> & { settingsCoreRequest(native: Native, method: string, payload: Obj, write?: boolean): Promise<Obj> };
/** t3.json per member, read through its environment's projects.readFile; absent or unreadable files resolve to null. */
export async function memberFiles(client: T3Client, native: Native | null | undefined, members: Obj[], scope?: CoreScope): Promise<Map<string, Obj | null>> {
  const environments = scope?.environments ?? [{ environmentId: client.environmentId, label: '', displayUrl: null, kind: '', fleetKey: '', connection: { phase: 'connected' }, serverConfig: null }];
  const scoped = members.map(member => ({ ...member, id: str(member.id), environmentId: str(member.environmentId, client.environmentId), workspaceRoot: str(member.workspaceRoot), physicalProjectKey: str(member.physicalProjectKey, str(member.id)) }));
  return scopeMemberFiles(bridge(client), native, scoped, environments, parseProjectFile);
}
/** The scope's connected environments with every value a control reads filled (the reference decodes ServerSettings defaults). */
function connectedOf(client: T3Client, scope: CoreScope, settings?: Obj): ScopedSettingsEnvironment[] {
  return selectScopedSettingsEnvironments(scope.resolved, scope.environments, client.environmentId || null).connectedEnvironments.map(environment => ({
    ...environment, serverConfig: { ...environment.serverConfig!, settings: { ...SERVER_DEFAULTS, projectSettingsOverrides: {},
      ...(settings && environment.environmentId === client.environmentId && !(environment as ScopeEnvironment).fleetKey ? settings : environment.serverConfig!.settings) } } }));
}
export function serverContext(client: T3Client, scope: CoreScope, files: Map<string, Obj | null>, settings?: Obj): ServerContext {
  const { environment: representative } = selectScopedSettingsEnvironments(scope.resolved, scope.environments, client.environmentId || null);
  const focused = !representative || (representative.environmentId === client.environmentId && !representative.fleetKey);
  const config = focused ? client.config : obj(fleetConfig(representative!.fleetKey));
  const environment = obj(config.environment);
  const raw = focused ? (settings ?? obj(client.config.settings)) : obj(representative!.serverConfig?.settings);
  const connected = scope.environments.filter(entry => scope.selected.includes(entry) && entry.connection.phase === 'connected');
  const restartEverywhere = connected.length > 0 && connected.every(entry => obj(obj((entry.fleetKey ? obj(fleetConfig(entry.fleetKey)) : client.config).environment).capabilities).threadRestartContinuation === true);
  return { settings: raw, scope, files, capabilities: obj(environment.capabilities), providers: arr(config.providers), environmentLabel: representative?.label || str(environment.label, 'environment'),
    targets: resolveScopedSettingsTargets(scope.resolved, connectedOf(client, scope, settings), files), restartEverywhere };
}
const fleetConfig = (key: string) => fleet.entries.get(key)?.config ?? {};

/** The plan one control writes (planScopedSettingsPatch / planScopedSettingsClear): one patch per connected target environment. */
export function settingPlan(client: T3Client, scope: CoreScope, key: string, value: Json | undefined, clear: boolean, settings?: Obj): ScopedSettingsPlan {
  const environments = connectedOf(client, scope, settings);
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  if (clear && project) return planScopedSettingsClear(scope.resolved, environments, [key]);
  return planScopedSettingsPatch(scope.resolved, environments, { [key]: clear ? SERVER_DEFAULTS[key] ?? null : value as Json });
}
const SERVER_ROW_KEYS: Record<string, string> = {
  'default-model': 'defaultModelSelection', 'default-permissions': 'defaultRuntimeMode', 'new-threads': 'defaultThreadEnvMode', 'worktree-submodules': 'worktreeSubmodules',
  'auto-resume-limited-threads': 'autoResumeLimitedThreads', 'snooze-limited-threads': 'snoozeLimitedThreads', 'auto-settle-merged-threads': 'sidebarAutoSettleOnMerge',
  'auto-settle-inactive-threads': 'sidebarAutoSettleAfterDays', 'days-before-auto-settle': 'sidebarAutoSettleAfterDays', 'response-streaming': 'responseStreamingMode',
  'provider-update-checks': 'enableProviderUpdateChecks', 'continue-threads-after-server-update': 'continueThreadsAfterServerUpdate', 'background-activity': 'backgroundActivity',
  'background-advanced': 'backgroundActivity', 'start-from-origin': 'newWorktreesStartFromOrigin', 'add-project-starts-in': 'addProjectBaseDirectory', 'text-generation-model': 'textGenerationModelSelection',
};
/** Decode one control's string payload into the schema value, refusing anything the reference would not write. */
export function serverValue(key: string, raw: string, context: ServerContext, part = ''): Json {
  const bool = () => { if (raw !== 'true' && raw !== 'false') throw new ClientError('Choose On or Off.'); return raw === 'true'; };
  switch (key) {
    case 'defaultRuntimeMode': if (!RUNTIME.some(([id]) => id === raw)) throw new ClientError('Choose a supported permission mode.'); return raw;
    case 'defaultThreadEnvMode': if (!['local', 'worktree'].includes(raw)) throw new ClientError('Choose a supported workspace.'); return raw;
    case 'worktreeSubmodules': if (!['recursive', 'top-level', 'none'].includes(raw)) throw new ClientError('Choose a supported submodule mode.'); return raw;
    case 'responseStreamingMode': if (!['turn', 'paragraph'].includes(raw)) throw new ClientError('Choose a supported streaming mode.'); return raw;
    case 'sidebarAutoSettleAfterDays': {
      if (part !== 'days') return bool() ? 3 : null;
      const days = Number(raw);
      if (!Number.isInteger(days) || days < 1 || days > 90) throw new ClientError('Enter a whole number of days from 1 to 90.');
      return days;
    }
    case 'backgroundActivity':
      if (!(raw in BACKGROUND)) throw new ClientError('Advanced background activity needs one environment.');
      return { schemaVersion: 1, profile: raw, overrides: {} };
    case 'addProjectBaseDirectory': if (raw.length > 4096) throw new ClientError('Use a shorter directory.'); return raw.trim();
    case 'defaultModelSelection': case 'textGenerationModelSelection': return modelValue(raw, part, key, context);
    default: return bool();
  }
}
function modelValue(raw: string, part: string, key: string, context: ServerContext): Json {
  const current = serverState(context, key).value;
  const providers = context.providers.filter(provider => providerAvailable(provider) && (key !== 'textGenerationModelSelection' || provider.supportsTextGeneration !== false));
  if (part === 'effort') {
    const control = modelControl(context, current === null ? null : obj(current), key === 'textGenerationModelSelection');
    const [instanceId, model] = str(control.value).split('|');
    // A bare value is a reasoning effort (older ids); "<descriptor>=<option>" picks one trait and keeps the others.
    const pick = raw.includes('=') ? raw : str(control.options2?.find(entry => /^(reasoningEffort|effort)=/.test(entry.value) && entry.value.endsWith(`=${raw}`))?.value);
    if (!control.options2?.some(entry => entry.value === pick && !entry.icon.startsWith('section'))) throw new ClientError('Choose a supported reasoning effort.');
    const [id, value] = [pick.slice(0, pick.indexOf('=')), pick.slice(pick.indexOf('=') + 1)];
    const previous = current !== null && obj(current).instanceId === instanceId && obj(current).model === model ? arr(obj(current).options) : [];
    return { instanceId: instanceId!, model: model!, options: [...previous.filter(entry => entry.id !== id).map(entry => ({ id: str(entry.id), value: entry.value as Json })), { id, value }] };
  }
  const separator = raw.indexOf('|');
  const instanceId = raw.slice(0, separator), model = raw.slice(separator + 1);
  const provider = providers.find(entry => entry.instanceId === instanceId);
  if (separator < 0 || !provider || !arr(provider.models).some(entry => entry.slug === model)) throw new ClientError(`This model is unavailable on ${context.scope.environmentLabel === 'All environments' ? 'a selected environment' : context.scope.environmentLabel}.`);
  return { instanceId, model };
}

/** settings-core command id: `<rowId>:<part>|<machine>|<projectKey>|<checkout>`; the value is the control's raw value.
 * part is '' for a server row, the device key for a device row, 'reset', 'effort' or 'days'. */
export function parseCoreTarget(id: string) {
  const first = id.indexOf('|'), last = id.lastIndexOf('|');
  const rest = id.slice(first + 1, last), second = rest.indexOf('|');
  if (first < 0 || last <= first || second < 0) throw new ClientError('That setting is no longer available.');
  const head = id.slice(0, first), colon = head.indexOf(':');
  return { row: colon < 0 ? head : head.slice(0, colon), part: colon < 0 ? '' : head.slice(colon + 1), machine: rest.slice(0, second), projectKey: rest.slice(second + 1), checkout: id.slice(last + 1) };
}
const SERVER_LABELS: Record<string, string> = {
  autoResumeLimitedThreads: 'Auto-resume limited threads', snoozeLimitedThreads: 'Snooze limited threads', sidebarAutoSettleOnMerge: 'Auto-settle merged threads',
  sidebarAutoSettleAfterDays: 'Auto-settle inactive threads', responseStreamingMode: 'Response streaming', enableProviderUpdateChecks: 'Provider update checks',
  continueThreadsAfterServerUpdate: 'Continue threads after restarts', backgroundActivity: 'Background activity', defaultThreadEnvMode: 'New thread mode',
  newWorktreesStartFromOrigin: 'New worktrees start from origin', addProjectBaseDirectory: 'Add project base directory', textGenerationModelSelection: 'Text generation model',
};
/** useSettingsRestore: the labels it lists in its confirmation, device values first, then the environment's. */
export function restoreLabels(local: LocalPrefs, settings: Obj, connected: boolean): string[] {
  const names: Record<string, string> = { appearanceMode: 'Follow system', theme: 'Theme', appearanceContrast: 'Contrast', glassOpacity: 'Glass opacity', diffColorScheme: 'Diff colors',
    chatWidth: 'Chat width', panelAnimationDurationMs: 'Panel animations', environmentIdentificationMode: 'Environment identification', timestampFormat: 'Time format',
    notificationMode: 'Thread notifications', inAppNotificationsEnabled: 'In-app notifications', projectGrouping: 'Project Grouping', sidebarProjectSortOrder: 'Project order', sidebarWorkingShelfEnabled: 'Working section',
    wordWrap: 'Word wrap', persistComposerContextStrip: 'Composer context', fontFamilySans: 'Interface font', fontSizeInterface: 'Interface font size', fontFamilyComposer: 'Prompt font',
    fontSizePrompt: 'Prompt font size', fontFamilyCode: 'Monospace font', fontSizeCode: 'Code font size', fontFamilyTerminal: 'Terminal font', fontSizeTerminal: 'Terminal font size',
    fontSmoothing: 'Font smoothing', diffFilesCollapsed: 'Default diff file state', diffIgnoreWhitespace: 'Diff whitespace changes', diffLayout: 'Diff layout',
    proactivePanelsEnabled: 'Proactive panels', showSkillsInSlashMenu: 'Show skills in slash menu', composerCollapseOnScroll: 'Collapse composer on scroll',
    composerRichTextEnabled: 'Rich text composer', sendShortcut: 'Send shortcut', followUpBehavior: 'Follow-up behavior', contextWindowMeterEnabled: 'Context window indicator',
    confirmThreadUnpin: 'Unpin confirmation', confirmThreadArchive: 'Archive confirmation', confirmThreadDelete: 'Delete confirmation', confirmQuit: 'Quit shortcut' };
  const device = changedDeviceLabels(local).filter(key => key in names && key !== 'themeLight' && key !== 'themeDark').map(key => names[key]!);
  const server = connected ? Object.keys(SERVER_LABELS).filter(key => key in settings && !same(settings[key], SERVER_DEFAULTS[key])).map(key => SERVER_LABELS[key]!) : [];
  return [...new Set([...device, ...server])];
}

export async function applyCoreSetting(client: T3Client, native: Native, id: string, value: string): Promise<string> {
  const target = parseCoreTarget(id);
  const local = client.local as unknown as LocalPrefs;
  const key = SERVER_ROW_KEYS[target.row];
  if (target.row === 'theme-import' || target.row === 'theme-save' || target.row === 'theme-remove') return applyThemeCommand(local, target.row, value);
  if (target.row === 'theme-editor' || target.row === 'theme-editor-save' || target.row === 'theme-export') return themeEditorCommand(client, native, target.row, target.part, value);
  if (target.row === 'theme-add') return themeImportCommand(client, native, target.part, value);
  if (target.row === 'theme-collection' || target.row === 'theme-remove-many') { // settings-a-collections.ts
    local.clientSettings = local.clientSettings || decodeClientPrefs({});
    if (target.row === 'theme-collection') useCollectionDefaults(local, value); else removeThemes(local, value.split(',').filter(Boolean));
    return '';
  }
  if (target.row === 'version' || target.row === 'update') return updateCommand(client, target.row, target.part);
  if (target.row === MOBILE_BETA_ROW) return mobileBetaCommand(client, native, target.part, value);
  if (target.row !== 'restore-device-defaults' && !key) {
    // Device rows: saved in this app's preferences, never sent to a server.
    if (target.part === 'reset') {
      if (value === 'projectGrouping') { local.groupingMode = 'repository'; return ''; }
      const fallback = value in DEVICE_DEFAULTS ? (DEVICE_DEFAULTS as Record<string, unknown>)[value] : (CLIENT_DEFAULTS as Record<string, unknown>)[value];
      if (fallback === undefined) throw new ClientError('That setting has no default.');
      applyDeviceSetting(local, value, String(fallback));
      const size = FONT_SIZE_KEYS[value];
      if (size) applyDeviceSetting(local, size, String(CLIENT_DEFAULTS[size as keyof typeof CLIENT_DEFAULTS]));
    } else if (target.part === 'notificationMode' && !(await notificationPermission(client, native, value))) { // shell-notify.ts permission gate
      return '';
    } else if (!applyDeviceSetting(local, target.part, value)) throw new ClientError('Unsupported setting.');
    seedDiffState(client, (client as unknown as { diffState: DiffState }).diffState);
    return '';
  }
  // Server rows re-resolve the selection against the live shell, the fleet and each
  // environment's config: a stale or removed target is refused, never widened (planScopedSettingsPatch).
  const scope = resolveScope(client, target.machine, target.projectKey, target.checkout);
  const project = scope.kind === 'project' || scope.kind === 'checkout';
  const focusedSelected = client.ready && scope.selected.some(environment => environment.environmentId === client.environmentId && !environment.fleetKey);
  const settings = focusedSelected ? await bridge(client).settingsCoreRequest(native, 'server.getSettings', {}) : undefined;
  const restore = target.row === 'restore-device-defaults';
  let plan: ScopedSettingsPlan;
  if (restore) {
    restoreDeviceDefaults(local);
    const targets = serverContext(client, scope, new Map(), settings).targets;
    const keys = Object.keys(SERVER_LABELS).filter(name => (!project || PROJECT_SCOPED.has(name))
      && targets.some(entry => name in entry.settings && !same(entry.settings[name], SERVER_DEFAULTS[name])));
    if (keys.length === 0 || scope.kind === 'unavailable' || !scope.connected) return 'Device settings restored';
    plan = project ? planScopedSettingsClear(scope.resolved, connectedOf(client, scope, settings), keys)
      : planScopedSettingsPatch(scope.resolved, connectedOf(client, scope, settings), Object.fromEntries(keys.map(name => [name, SERVER_DEFAULTS[name] ?? null])));
  } else if (scope.kind === 'unavailable') {
    plan = { clientPatch: {}, hasClientWrite: false, serverWrites: [], unavailableReason: scope.message };
  } else {
    const context = serverContext(client, scope, await memberFiles(client, native, scope.members, scope), settings);
    if (key === 'continueThreadsAfterServerUpdate' && !context.restartEverywhere && scope.connected) throw new ClientError('All selected connected environments must support restart continuation.');
    const clear = target.part === 'reset';
    plan = target.row === 'background-advanced' ? settingPlan(client, scope, key!, advancedBackgroundValue(context.settings, target.part, value), false, settings)
      : settingPlan(client, scope, key!, clear ? undefined : serverValue(key!, value, context, target.part), clear, settings);
  }
  // useRunScopedPlan: a refused plan warns; every write is awaited and the failures are named.
  const refused = scopedPlanNotice(plan);
  if (refused) { postScopedNotice(client, refused); return restore ? 'Device settings restored' : ''; }
  const result = await persistScopedSettingsPatch(plan, scopedWriter(bridge(client), native, scope.environments), () => {});
  postScopedNotice(client, scopedPlanNotice(plan, result));
  return restore ? 'Device settings restored' : '';
}

/** Theme library writes: import a theme file, save a duplicate, or remove a saved theme (device-local). */
function applyThemeCommand(local: LocalPrefs, row: string, value: string): string {
  const themes = local.customThemes || [];
  const prefs = local.clientSettings || decodeClientPrefs({});
  const taken = ['t3-code', 't3-chat', 'grove', 'ocean', 'ember', 'iris', ...themes.map(theme => theme.id)];
  if (row === 'theme-remove') {
    if (!themes.some(theme => theme.id === value)) throw new ClientError('That theme is no longer installed.');
    local.customThemes = themes.filter(theme => theme.id !== value);
    const fallback = (id: string) => id === value ? 't3-code' : id;
    local.clientSettings = { ...prefs, theme: fallback(prefs.theme), themeLight: fallback(prefs.themeLight), themeDark: fallback(prefs.themeDark) };
    return '';
  }
  let theme: CustomTheme;
  if (row === 'theme-import') theme = parseThemeFile(value, taken);
  else {
    // The editor sends source|name|light canvas|light accent|dark canvas|dark accent, each URI-encoded.
    const [source = 't3-code', name = '', lc = '', la = '', dc = '', da = ''] = value.split('|').map(part => { try { return decodeURIComponent(part); } catch { return ''; } });
    const input: Obj = { name, light: { canvas: lc, accent: la }, dark: { canvas: dc, accent: da } };
    theme = duplicateTheme(input, mode => themeRoles(source, mode, themes) as unknown as Record<string, string>, taken);
  }
  if (themes.length >= 100) throw new ClientError('Remove a theme before adding another.');
  local.customThemes = [...themes, theme];
  const both = theme.light && theme.dark;
  local.clientSettings = { ...prefs, ...(both || theme.appearance === 'light' ? { themeLight: theme.id } : {}), ...(both || theme.appearance === 'dark' ? { themeDark: theme.id } : {}), ...(both ? { theme: theme.id } : {}) };
  return '';
}
