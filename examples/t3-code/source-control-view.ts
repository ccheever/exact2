// Settings → Source control and Integrations view models (reference
// SourceControlSettings.tsx, ProjectDefaultsSettings.tsx, BranchNamingSettings.tsx,
// SourceControlWritingSettings.tsx, IntegrationsSettings.tsx, SettingInheritance.tsx).
// Rows carry their inheritance chain; writes are server.updateSettings patches
// with the reference's project-override replacement semantics.
// 1e2ecbd975 (5318d054a5): Integrations › Devices adds the Simulator support row (device-support.ts).
import { redactedValue } from './redacted-text'; // provider-sign-in-and-install: RedactedAccount
import { arr, obj, str, num, type Json, type Obj } from './domain';
import type { T3Client } from './client';
import { providerAvailable, type Native } from './protocol';
import { deviceTool } from './settings-a-integrations';
import { bitbucketView, type BitbucketView } from './settings-a-bitbucket';
import { deviceHostsView } from './settings-a-hosts';
import { deviceScope } from './settings-integrations-scope';
import { connectedEnvironmentCount, simulatorSupportRows, type SimulatorSupportRow } from './device-support'; // 5318d054a5: Simulator support row
import { letGo } from './let-go';
import { linkTargetPreference } from './browser-links';
import { deviceStateOf, watchDevice } from './r4-surfaces-device'; // useDeviceState

type Choice = { value: string; label: string; selected: boolean };
type Layer = { key: string; label: string; value: string; effective: boolean; set: boolean };
export type ScopedRow = { key: string; kind: string; title: string; description: string; checked: boolean; value: string; valueLabel: string; options: Choice[]; placeholder: string;
  disabled: boolean; first: boolean; summary: string; state: string; layers: Layer[]; reset: string; status: string; child: string;
  /** ScopedSwitch's mixed state (D15): the Integrations device rows compute it across the settings scope's targets (settings-integrations-scope.ts). */
  mixed: boolean };

export const DEFAULTS: Obj = { defaultAutoPull: false, pullRequestMergeMethod: null, branchNamingMode: 'static', branchNamePrefix: 't3code', branchNameInstructions: '',
  sourceControlWritingStyle: { mode: 'repo_conventions', customInstructions: '', followChangeRequestTemplates: true }, sourceControlWriterModelSelection: null,
  enableAgentBrowserAccess: true, enableAgentDeviceAccess: false, enableDeviceSupport: false, defaultModelSelection: null, defaultThreadEnvMode: null, deviceHosts: [] };
const PROJECT_KEYS = ['defaultModelSelection', 'defaultThreadEnvMode', 'defaultAutoPull', 'enableAgentBrowserAccess', 'enableAgentDeviceAccess', 'sourceControlWriterModelSelection', 'sourceControlWritingStyle', 'branchNamingMode', 'branchNamePrefix', 'branchNameInstructions', 'pullRequestMergeMethod'];
const MERGE_LABELS: Record<string, string> = { merge: 'Merge', squash: 'Squash and merge', rebase: 'Rebase and merge' };
const BRANCH_MODES: Record<string, string> = { static: 'Static prefix', semantic: 'Semantic prefix', custom: 'Custom instructions' };
const WRITING_MODES: Record<string, { label: string; description: string }> = {
  repo_conventions: { label: 'Repository conventions', description: 'In each project, matches recent change descriptions and change request titles.' },
  conventional_commits: { label: 'Conventional Commits', description: 'Use Conventional Commit prefixes and keep change request text concise.' },
  custom: { label: 'Custom instructions', description: 'Use your instructions for change descriptions and change requests in every project.' },
};
const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

/** SettingInheritance formatValue. */
export function formatValue(key: string, value: unknown): string {
  if (value === null || value === undefined) return key === 'pullRequestMergeMethod' ? 'Last selected' : key === 'sourceControlWriterModelSelection' ? 'Off' : 'Unset';
  if (typeof value === 'boolean') return value ? 'On' : 'Off';
  if (typeof value === 'number') return String(value);
  if (typeof value === 'string') {
    if (key === 'pullRequestMergeMethod' && MERGE_LABELS[value]) return MERGE_LABELS[value];
    if (key === 'branchNamingMode' && BRANCH_MODES[value]) return BRANCH_MODES[value];
    return value === '' ? 'Empty' : value;
  }
  if (Array.isArray(value)) return `${value.length} ${value.length === 1 ? 'item' : 'items'}`;
  const object = obj(value);
  if (typeof object.model === 'string') return object.model;
  if (typeof object.mode === 'string') return WRITING_MODES[object.mode]?.label ?? object.mode;
  return 'Custom';
}

/** settingInheritanceLayers + the SettingsRow summary for one environment/project target. */
export function inheritance(settings: Obj, projectId: string, key: string, environmentLabel: string) {
  const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
  const scoped = PROJECT_KEYS.includes(key);
  const projectSet = Boolean(projectId) && scoped && Object.prototype.hasOwnProperty.call(override, key);
  const environmentSet = !same(settings[key], DEFAULTS[key]);
  const layers: Layer[] = [];
  if (projectId && scoped) layers.push({ key: 'project', label: 'Project', value: projectSet ? formatValue(key, override[key]) : 'Inherits', effective: projectSet, set: projectSet });
  layers.push({ key: 'environment', label: 'Environment', value: environmentSet ? formatValue(key, settings[key]) : 'Inherits', effective: !projectSet && environmentSet, set: environmentSet });
  layers.push({ key: 'built-in', label: 'Default', value: formatValue(key, DEFAULTS[key]), effective: !projectSet && !environmentSet, set: true });
  const [summary, state] = projectSet ? ['Overridden for this project', 'overridden'] : projectId && scoped ? [`Inherited from ${environmentLabel}`, 'inherited'] : environmentSet ? ['Set on the environment', 'environment'] : ['Built-in default', 'default'];
  return { summary, state, layers };
}

function effective(settings: Obj, projectId: string): Obj {
  const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
  return { ...settings, ...override, sourceControlWritingStyle: { ...obj(DEFAULTS.sourceControlWritingStyle), ...obj(settings.sourceControlWritingStyle), ...obj(override.sourceControlWritingStyle) } };
}

function row(settings: Obj, projectId: string, environmentLabel: string, key: string, partial: Partial<ScopedRow>): ScopedRow {
  const info = inheritance(settings, projectId, key, environmentLabel);
  return { key, kind: 'switch', title: '', description: '', checked: false, value: '', valueLabel: '', options: [], placeholder: '', disabled: false, first: false, status: '', child: '', reset: '', mixed: false, ...info, ...partial };
}

/** Source-control route rows: Repositories and Text generation. */
export function sourceControlRows(settings: Obj, projectId: string, environmentLabel: string, providers: Obj[], writable: boolean) {
  const value = effective(settings, projectId);
  const project = Boolean(projectId);
  const overridden = (key: string) => project && Object.prototype.hasOwnProperty.call(obj(obj(settings.projectSettingsOverrides)[projectId]), key);
  // Environment scope resets to the built-in; a project scope clears its override.
  const resetFor = (key: string, differs: boolean) => project ? (overridden(key) ? `key=${key}&value=__inherit__` : '') : differs ? `key=${key}&value=__default__` : '';
  const style = obj(value.sourceControlWritingStyle);
  const merge = value.pullRequestMergeMethod === null || value.pullRequestMergeMethod === undefined ? 'last' : str(value.pullRequestMergeMethod);
  const repositories = [
    row(settings, projectId, environmentLabel, 'defaultAutoPull', { kind: 'switch', title: 'Automatically pull', first: true, checked: value.defaultAutoPull === true, disabled: !writable,
      description: project ? "Keeps this project's default branch current when the checkout has no local changes or commits." : 'Keeps the default branch current when the checkout has no local changes or commits. Projects can override it.',
      reset: resetFor('defaultAutoPull', value.defaultAutoPull === true) }),
    row(settings, projectId, environmentLabel, 'pullRequestMergeMethod', { kind: 'select', title: 'Default merge method', value: merge, valueLabel: merge === 'last' ? 'Last selected' : MERGE_LABELS[merge] || merge, disabled: !writable,
      description: project ? 'Pull requests in this project start with this method.' : 'Pull requests start with this method. Last selected reuses whatever you chose most recently on this device.',
      options: [['last', 'Last selected'], ['merge', 'Merge'], ['squash', 'Squash and merge'], ['rebase', 'Rebase and merge']].map(([v, l]) => ({ value: v, label: l, selected: v === merge })),
      reset: resetFor('pullRequestMergeMethod', merge !== 'last') }),
  ];
  const mode = str(value.branchNamingMode, 'static'), writing = str(style.mode, 'repo_conventions');
  const writerOn = value.sourceControlWriterModelSelection !== null && value.sourceControlWriterModelSelection !== undefined;
  const writer = obj(value.sourceControlWriterModelSelection);
  const models = providers.filter(provider => providerAvailable(provider)).flatMap(provider => arr(provider.models).filter(model => model.isUnavailable !== true && model.isLegacy !== true).map(model => ({ value: `${provider.instanceId}:${model.slug}`, label: str(model.name, str(model.slug)), selected: provider.instanceId === writer.instanceId && model.slug === writer.model })));
  const text = [
    row(settings, projectId, environmentLabel, 'branchNamingMode', { kind: 'select', title: 'Worktree branch naming', first: true, value: mode, valueLabel: BRANCH_MODES[mode] || mode, disabled: !writable,
      description: 'Choose how new worktree branches are named from your first message.', options: Object.entries(BRANCH_MODES).map(([v, l]) => ({ value: v, label: l, selected: v === mode })), reset: resetFor('branchNamingMode', mode !== 'static') }),
    ...(mode === 'static' ? [row(settings, projectId, environmentLabel, 'branchNamePrefix', { kind: 'text', title: 'Branch prefix', value: str(value.branchNamePrefix), placeholder: 'No prefix', disabled: !writable,
      description: 'For example, t3code or t3code/ produces t3code/add-search. Leave empty for no prefix.', reset: resetFor('branchNamePrefix', str(value.branchNamePrefix) !== 't3code') })] : []),
    ...(mode === 'semantic' ? [row(settings, projectId, environmentLabel, 'branchNamingMode', { kind: 'note', title: '', description: 'The model chooses a prefix that describes the work, such as feat/add-search, fix/login-timeout, or refactor/auth.' })] : []),
    ...(mode === 'custom' ? [row(settings, projectId, environmentLabel, 'branchNameInstructions', { kind: 'textarea', title: 'Branch naming instructions', value: str(value.branchNameInstructions), placeholder: 'Use julius/ followed by the issue ID and a short description.', disabled: !writable,
      description: 'Appended to the naming prompt. The model returns the complete branch name; no prefix or suffix is added.', reset: resetFor('branchNameInstructions', str(value.branchNameInstructions) !== '') })] : []),
    row(settings, projectId, environmentLabel, 'sourceControlWritingStyle', { kind: 'select', title: 'Source control writing style', value: writing, valueLabel: WRITING_MODES[writing]?.label || writing, disabled: !writable,
      description: WRITING_MODES[writing]?.description || '', options: Object.entries(WRITING_MODES).map(([v, l]) => ({ value: v, label: l.label, selected: v === writing })),
      child: writing === 'custom' ? str(style.customInstructions) : '', placeholder: 'Keep titles concise. Use short bullet points in descriptions.',
      reset: project ? (overridden('sourceControlWritingStyle') ? 'key=sourceControlWritingStyle&value=__inherit__' : '') : (writing !== 'repo_conventions' || str(style.customInstructions) !== '' ? 'key=sourceControlWritingStyle&value=__default__' : '') }),
    row(settings, projectId, environmentLabel, 'sourceControlWritingStyle', { key: 'sourceControlWritingStyle.followChangeRequestTemplates', kind: 'switch', title: 'Follow change request templates', checked: style.followChangeRequestTemplates !== false, disabled: !writable,
      description: "Use the repository's template for change request descriptions when available.", reset: project ? '' : style.followChangeRequestTemplates === false ? 'key=sourceControlWritingStyle.followChangeRequestTemplates&value=__default__' : '' }),
    row(settings, projectId, environmentLabel, 'sourceControlWriterModelSelection', { kind: 'model', title: 'Source control writer model', checked: writerOn, disabled: !writable,
      description: "Model for source control text and branch or bookmark names. Off uses the environment's text generation model.",
      value: writerOn ? `${str(writer.instanceId)}:${str(writer.model)}` : '', valueLabel: writerOn ? (models.find(model => model.selected)?.label || str(writer.model)) : '', options: models,
      status: writerOn && !models.length ? 'No text generation providers available.' : '' }),
  ];
  return { repositories, text: text.map((entry, index) => ({ ...entry, first: index === 0 })) };
}

/** Integrations: Agent browser access, and the device hub rows. */
export function integrationRows(settings: Obj, projectId: string, environmentLabel: string, writable: boolean) {
  const value = effective(settings, projectId), project = Boolean(projectId);
  const overridden = (key: string) => project && Object.prototype.hasOwnProperty.call(obj(obj(settings.projectSettingsOverrides)[projectId]), key);
  return {
    browser: [row(settings, projectId, environmentLabel, 'enableAgentBrowserAccess', { kind: 'switch', title: 'Agent browser access', first: true, checked: value.enableAgentBrowserAccess !== false, disabled: !writable,
      description: project ? 'Allow agents in this project to use the shared browser. Applies when the agent session next starts.' : 'Allow agents to use the shared browser. Projects can override it.',
      reset: project ? (overridden('enableAgentBrowserAccess') ? 'key=enableAgentBrowserAccess&value=__inherit__' : '') : value.enableAgentBrowserAccess === false ? 'key=enableAgentBrowserAccess&value=__default__' : '' })],
    deviceHub: row(settings, projectId, environmentLabel, 'enableDeviceSupport', { kind: 'switch', title: 'Device hub', first: true, checked: settings.enableDeviceSupport === true, disabled: project || !writable,
      description: 'Enable this environment to open simulators and emulators, whether they run here or on a remote device host.' }),
    agentDevice: row(settings, projectId, environmentLabel, 'enableAgentDeviceAccess', { kind: 'switch', title: 'Agent device access', checked: value.enableAgentDeviceAccess === true, disabled: !writable || (!project && settings.enableDeviceSupport !== true),
      description: 'Allow new agent sessions in this environment to start and control local and remote devices, with required tools set up automatically.',
      reset: project ? (overridden('enableAgentDeviceAccess') ? 'key=enableAgentDeviceAccess&value=__inherit__' : '') : '' }),
    hostsRow: row(settings, projectId, environmentLabel, 'deviceHosts', { kind: 'hosts', title: 'Device hosts' }),
  };
}

/** planScopedSettingsPatch / planScopedSettingsClear for one key. */
export function scopedPatch(settings: Obj, projectId: string, key: string, raw: string): Obj {
  const [root, field] = key.split('.');
  if (!(root in DEFAULTS)) throw new Error('Unsupported setting.');
  let value: Json | undefined;
  if (raw === '__inherit__') value = undefined;
  else if (raw === '__default__' && root === 'sourceControlWritingStyle' && !field) value = { mode: 'repo_conventions', customInstructions: '' };
  else if (raw === '__default__') value = field ? obj(DEFAULTS[root])[field] as Json : DEFAULTS[root] as Json;
  else if (root === 'pullRequestMergeMethod') { if (!['last', 'merge', 'squash', 'rebase'].includes(raw)) throw new Error('Choose a supported merge method.'); value = raw === 'last' ? null : raw; }
  else if (root === 'branchNamingMode') { if (!BRANCH_MODES[raw]) throw new Error('Choose a supported branch naming mode.'); value = raw; }
  else if (root === 'branchNamePrefix' || root === 'branchNameInstructions') value = raw.trim();
  else if (root === 'sourceControlWritingStyle' && field === 'mode') { if (!WRITING_MODES[raw]) throw new Error('Choose a supported writing style.'); value = raw; }
  else if (root === 'sourceControlWritingStyle' && field === 'customInstructions') value = raw.trim();
  else if (root === 'defaultThreadEnvMode') { if (raw !== 'local' && raw !== 'worktree') throw new Error('Choose a supported workspace.'); value = raw; }
  else if (root === 'defaultModelSelection') {
    // "instance:model" or "instance:model#optionId=value" (TraitsPicker's effort).
    const [model, option] = raw.split('#'), at = model.indexOf(':');
    if (at <= 0) throw new Error('Choose an available model.');
    const [id, choice] = (option || '').split('=');
    value = { instanceId: model.slice(0, at), model: model.slice(at + 1), ...(id ? { options: [{ id, value: choice }] } : {}) };
  }
  else if (root === 'sourceControlWriterModelSelection') {
    if (raw === 'off') value = null;
    else if (raw === 'default') { const text = obj(settings.textGenerationModelSelection); if (!str(text.instanceId) || !str(text.model)) throw new Error('No text generation providers available.'); value = text; }
    else { const at = raw.indexOf(':'); if (at <= 0) throw new Error('Choose an available model.'); value = { instanceId: raw.slice(0, at), model: raw.slice(at + 1) }; }
  } else { if (raw !== 'true' && raw !== 'false') throw new Error('Choose On or Off.'); value = raw === 'true'; }
  if (!projectId) {
    if (value === undefined) throw new Error('Choose a project to clear its override.');
    if (root === 'enableDeviceSupport') throw new Error('Use the device hub switch.');
    return { [root]: field ? { [field]: value } : value };
  }
  if (!PROJECT_KEYS.includes(root)) throw new Error('This setting is environment-wide and cannot be overridden by a project.');
  const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
  if (value === undefined) delete current[root];
  else if (field) current[root] = { ...obj(effective(settings, projectId)[root]), ...obj(current[root]), [field]: value };
  else current[root] = value;
  return { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } };
}

/** Discovery rows (SourceControlSettings DiscoveryItemRow): never carries an account. */
export function discoveryRows(result: Obj) {
  const option = (value: unknown) => { const v = obj(value); return typeof value === 'string' ? value : str(v.value); };
  const vcs = arr(result.versionControlSystems).map(item => {
    const ready = item.implemented === true, available = str(item.status) === 'available';
    return { kind: str(item.kind), label: str(item.label), version: option(item.version), comingSoon: !ready, badge: '', dot: !ready ? 'muted' : available ? 'success' : 'warning',
      summary: !ready ? `Support for ${str(item.label)} is coming soon.` : !available ? `Not available on this server: ${str(item.installHint)}` : 'Available', account: false, enabled: ready && available, details: str(item.kind) === 'git',
      accountValue: '', accountPlaceholder: '' };
  });
  const providers = arr(result.sourceControlProviders).map(item => {
    const auth = obj(item.auth), available = str(item.status) === 'available', status = str(auth.status);
    const summary = !available ? `Not available on this server: ${str(item.installHint)}` : status === 'authenticated' ? 'Authenticated'
      : !item.executable && status === 'unauthenticated' ? `Available. ${str(item.installHint)}`
      : status === 'unauthenticated' ? `${str(item.label)} is not authenticated on this server. Sign in or configure credentials using the ${str(item.executable)} tool on the server host to enable change request features.`
      : `Could not verify ${str(item.label)}. ${option(auth.detail) || str(item.installHint)}`;
    return { kind: str(item.kind), label: str(item.label), version: option(item.version), comingSoon: false, badge: status === 'unauthenticated' && available ? 'Not authenticated' : '',
      dot: !available || status !== 'authenticated' ? 'warning' : 'success', summary, account: available && status === 'authenticated' && option(auth.account) !== '', enabled: available && status === 'authenticated', details: str(item.kind) === 'bitbucket',
      // RedactedAccount (SourceControlSettings.tsx:160-168): each item reveals its own account.
      ...(({ value, placeholder }) => ({ accountValue: value, accountPlaceholder: placeholder }))(redactedValue(available && status === 'authenticated' ? option(auth.account) : '')) };
  });
  return { vcs, providers };
}

export function fetchInterval(settings: Obj) {
  // resolveServerBackgroundActivitySettings: custom overrides, else the profile preset.
  const activity = obj(settings.backgroundActivity), presets: Record<string, number> = { performance: 15, balanced: 30, 'battery-saver': 0 };
  const base = str(activity.profile) === 'custom' ? str(activity.baseProfile, 'balanced') : str(activity.profile, 'balanced');
  const override = obj(activity.overrides).automaticGitFetchInterval;
  const seconds = typeof override === 'number' ? Math.round(override / 1000) : presets[base] ?? 30;
  return { seconds: String(seconds), canReset: seconds !== (presets[base] ?? 30), decrease: String(Math.max(0, seconds - 5)), increase: String(seconds + 5), base };
}
export function fetchIntervalPatch(settings: Obj, raw: string): Obj {
  const activity = obj(settings.backgroundActivity);
  const base = str(activity.profile) === 'custom' ? str(activity.baseProfile, 'balanced') : str(activity.profile, 'balanced');
  const overrides: Obj = { ...obj(activity.overrides) };
  if (raw === '__default__') delete overrides.automaticGitFetchInterval;
  else { const seconds = Number(raw); if (!Number.isFinite(seconds)) throw new Error('Enter a number of seconds.'); overrides.automaticGitFetchInterval = Math.max(0, Math.round(seconds)) * 1000; }
  return { backgroundActivity: { schemaVersion: 1, profile: 'custom', baseProfile: base, overrides } };
}

const discoveries = new WeakMap<T3Client, { key: string; value: Obj; error: string }>();
function scopeError(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string): string {
  if (!native?.available || !client.ready || (environmentId !== "" && environmentId !== client.environmentId)) return 'Connect an environment to inspect its version control tools and hosting integrations.';
  if (projectId && !client.shell.projects.some(project => project.id === projectId)) return 'This scope is unavailable. Choose a connected environment and an existing checkout.';
  return '';
}
function environmentLabel(client: T3Client): string { return str(obj(client.config.environment).label, 'Environment'); }

export async function sourceControlPage(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, active: boolean, rescan: number) {
  const empty = { available: false, error: '', project: projectId !== '', scope: `${environmentId}:${projectId}`, repositories: [] as ScopedRow[], text: [] as ScopedRow[],
    scanned: false, scanError: '', vcs: [] as ReturnType<typeof discoveryRows>['vcs'], providers: [] as ReturnType<typeof discoveryRows>['providers'], fetches: [fetchInterval({})], bitbucket: [] as BitbucketView[] };
  if (!active) return empty;
  const error = scopeError(client, native, environmentId, projectId);
  if (error || !native) return { ...empty, error };
  try {
    const access = client.restAccess(native);
    const config = await access.request('server.getConfig');
    if (projectId && obj(obj(config.environment).capabilities).projectSettingsOverrides !== true) return { ...empty, error: 'Update the selected environment to configure project overrides.' };
    const settings = await access.request('server.getSettings');
    const rows = sourceControlRows(settings, projectId, environmentLabel(client), arr(config.providers), client.writable);
    // Discovery scans one machine's tools (server.discoverSourceControl); a rescan re-runs it.
    const key = `${client.origin}:${client.generation}:${rescan}`;
    let cached = discoveries.get(client);
    if (!cached || cached.key !== key) {
      try { cached = { key, value: await access.request('server.discoverSourceControl', {}), error: '' }; }
      catch (failure) { if (letGo(failure)) throw failure; cached = { key, value: {}, error: failure instanceof Error ? failure.message : 'Could not scan the server environment.' }; }
      discoveries.set(client, cached);
    }
    const found = discoveryRows(cached.value);
    return { ...empty, available: true, ...rows, scanned: true, scanError: cached.error, ...found, fetches: [fetchInterval(settings)], bitbucket: [bitbucketView(settings, client.environmentId, rescan)] };
  } catch (failure) { return { ...empty, error: failure instanceof Error ? failure.message : 'Could not load settings.' }; }
}

export async function integrationsPage(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, active: boolean, machine = '', projectKey = '', checkout = '') {
  const empty = { available: false, error: '', project: projectId !== '', scope: `${environmentId}:${projectId}`, deviceScope: '', browser: [] as ScopedRow[], deviceHub: blankRow(), agentDevice: blankRow(), hubStatus: '', agentStatus: '', hosts: 0,
    hubTool: deviceTool('hub', null), agentTool: deviceTool('agent', null), hostsRow: blankRow(), deviceHosts: deviceHostsView(client, {}, null, projectId !== '', false),
    simulatorSupport: [] as SimulatorSupportRow[], linkTarget: linkTargetPreference(client) }; // browser-surface part 5: "Open links in"
  if (!active) return { ...empty };
  const error = scopeError(client, native, environmentId, projectId);
  if (error || !native) return { ...empty, error };
  try {
    // useScopedSettings and useDeviceState (IntegrationsSettings.tsx): the connection's server config (server.getConfig
    // at connect, then subscribeServerConfig) and its subscribeDeviceState stream, never a read per answer. A read here
    // re-asked this page: device.list's inspection publishes to the device stream, whose event bumps data.revision.
    const config = client.config, settings = obj(config.settings);
    if (projectId && obj(obj(config.environment).capabilities).projectSettingsOverrides !== true) return { ...empty, error: 'Update the selected environment to configure project overrides.' };
    const rows = integrationRows(settings, projectId, environmentLabel(client), client.writable);
    // Device hub and Agent device access follow the settings scope: the representative's value, mixed across
    // the selected targets, and Agent device access open once any connected environment runs the hub.
    // A checkout chosen by this page's own scope (settingsProjectId) is that checkout's group and checkout.
    const legacy = !projectKey && projectId ? client.projectGroups().find(group => group.members.some(member => member.id === projectId)) : undefined;
    const devices = deviceScope(client, machine, legacy ? legacy.key : projectKey, legacy ? projectId : checkout);
    rows.deviceHub = { ...rows.deviceHub, checked: devices.checked.enableDeviceSupport, mixed: devices.mixed.enableDeviceSupport,
      disabled: devices.project || devices.unavailable || devices.connectedCount === 0 || !client.writable };
    rows.agentDevice = { ...rows.agentDevice, checked: devices.checked.enableAgentDeviceAccess, mixed: devices.mixed.enableAgentDeviceAccess,
      disabled: !client.writable || devices.connectedCount === 0 || (!devices.project && !devices.anyHubEnabled) };
    // DeviceToolVersions over the stream's state; "Check versions" (rest:device-tools) is the one inspection.
    await watchDevice(client, native);
    const deviceState = deviceStateOf(client), tools = obj(arr(deviceState?.hosts).find(host => host.kind === 'local')?.tools);
    const hubStatus = deviceState ? toolVersion(tools.hub) : 'Version unknown', agentStatus = deviceState ? toolVersion(tools.agent) : 'Version unknown';
    return { ...empty, available: true, ...rows, deviceScope: devices.scopeKey, hubStatus, agentStatus, hosts: Array.isArray(settings.deviceHosts) ? settings.deviceHosts.length : 0,
      hubTool: deviceTool('hub', deviceState), agentTool: deviceTool('agent', deviceState), deviceHosts: deviceHostsView(client, settings, deviceState, projectId !== '', true),
      simulatorSupport: simulatorSupportRows(client, environmentId, settings.enableDeviceSupport === true, deviceState, connectedEnvironmentCount(client)) };
  } catch (failure) { return { ...empty, error: failure instanceof Error ? failure.message : 'Could not load settings.' }; }
}
export function toolVersion(value: unknown): string {
  if (value === undefined || value === null) return 'Version unknown';
  const tool = obj(value), installed = (Array.isArray(tool.installedVersions) ? tool.installedVersions : []).map(String);
  const version = str(tool.runningVersion) || (installed.includes(str(tool.requiredVersion)) ? str(tool.requiredVersion) : [...installed].sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).pop() || '');
  return version ? `v${version}` : 'Not installed';
}
function blankRow(): ScopedRow {
  return { key: '', kind: 'switch', title: '', description: '', checked: false, value: '', valueLabel: '', options: [], placeholder: '', disabled: true, first: false, summary: '', state: '', layers: [], reset: '', status: '', child: '', mixed: false };
}
