// Adapted from T3 Code 9c161462c9 (MIT); see LICENSE-T3.
import { hostCredentials, type HostCredentials } from './source-control-credentials';
import { can } from '../connections/command-permissions';
// Settings → Source control and Integrations view models (reference
// SourceControlSettings.tsx, ProjectDefaultsSettings.tsx, BranchNamingSettings.tsx,
// SourceControlWritingSettings.tsx, IntegrationsSettings.tsx, SettingInheritance.tsx).
// Rows carry their inheritance chain; writes are server.updateSettings patches
// with the reference's project-override replacement semantics.
// 1e2ecbd975 (5318d054a5): Integrations › Devices adds the Simulator support row (device-support.ts).
import { redactedValue } from '../shared/redacted-text'; // RedactedAccount
import { arr, obj, str, num, type Json, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { providerAvailable, type Native } from '../client/protocol';
import { deviceTool } from '../device/settings-a-integrations';
import { deviceHostsView, scopedDeviceHostsView } from '../device/settings-a-hosts';
import { deviceScope } from '../device/settings-integrations-scope';
import { connectedEnvironmentCount, simulatorSupportRows, type SimulatorSupportRow } from '../device/device-support'; // 5318d054a5: Simulator support row
import { letGo } from '../client/let-go';
import { linkTargetPreference } from '../browser/browser-links';
import { resolveScope, memberFiles, serverContext, inheritanceGroups, type InheritanceGroup, formatSettingValue as formatValue, traitsControl, type CoreOption } from './settings-core';
import { resolveTextGenerationSelection } from './text-generation-model';
import { providerBadge } from '../client/presentation';
import { triggerModelName } from '../composer/composer-controls-model';
import { watchDeviceHostStates } from '../client/live-streams';
import { deviceStateOf, watchDevice } from '../device/device-surface'; // useDeviceState
import { environmentClient, groupOfMember } from './projects-fleet';
import { scopeRepresentative } from './settings-core-view';

type Choice = { value: string; label: string; selected: boolean };
type Layer = { key: string; label: string; value: string; effective: boolean; set: boolean };
export type ScopedRow = { key: string; kind: string; title: string; description: string; checked: boolean; value: string; valueLabel: string; options: Choice[]; placeholder: string;
  disabled: boolean; first: boolean; summary: string; state: string; layers: Layer[]; layerGroups: InheritanceGroup[]; reset: string; status: string; child: string;
  /** ScopedSwitch's mixed state: the Integrations device rows compute it across the settings scope's targets (settings-integrations-scope.ts). */
  mixed: boolean;
  /** The control's accessible name and SettingResetButton's `label` ("Reset <label> to default"), as the reference names them; '' uses the title. */
  control: string; resetLabel: string;
  /** A model row's picker trigger mark: the instance's driver and ProviderInstanceIcon badge. */
  driver: string; badge: string; accent: string;
  /** The writer model's TraitsPicker: its trigger label ('' draws none) and menu rows. */
  traitsLabel: string; traits: CoreOption[] };

export const DEFAULTS: Obj = { defaultAutoPull: false, pullRequestMergeMethod: null, branchNamingMode: 'static', branchNamePrefix: 't3', branchNameInstructions: '',
  sourceControlWritingStyle: { mode: 'repo_conventions', customInstructions: '', followChangeRequestTemplates: true }, sourceControlWriterModelSelection: null, reviewGuideModelSelection: null, reviewGuidesEnabled: false, removeAgentCreditsOnMerge: false,
  enableAgentBrowserAccess: true, enableAgentDeviceAccess: false, enableDeviceSupport: false, defaultModelSelection: null, defaultThreadEnvMode: null, deviceHosts: [] };
const PROJECT_KEYS = ['defaultModelSelection', 'defaultThreadEnvMode', 'defaultAutoPull', 'enableAgentBrowserAccess', 'enableAgentDeviceAccess', 'sourceControlWriterModelSelection', 'reviewGuideModelSelection', 'sourceControlWritingStyle', 'removeAgentCreditsOnMerge', 'branchNamingMode', 'branchNamePrefix', 'branchNameInstructions', 'pullRequestMergeMethod'];
const MERGE_LABELS: Record<string, string> = { merge: 'Merge', squash: 'Squash and merge', rebase: 'Rebase and merge' };
const BRANCH_MODES: Record<string, string> = { static: 'Static prefix', semantic: 'Semantic prefix', custom: 'Custom instructions' };
const WRITING_MODES: Record<string, { label: string; description: string }> = {
  repo_conventions: { label: 'Repository conventions', description: 'In each project, matches recent change descriptions and change request titles.' },
  conventional_commits: { label: 'Conventional Commits', description: 'Use Conventional Commit prefixes and keep change request text concise.' },
  custom: { label: 'Custom instructions', description: 'Use your instructions for change descriptions and change requests in every project.' },
};
const same = (a: unknown, b: unknown) => JSON.stringify(a ?? null) === JSON.stringify(b ?? null);

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
  return { summary, state, layers, layerGroups: [] as InheritanceGroup[] };
}

function effective(settings: Obj, projectId: string): Obj {
  const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
  return { ...settings, ...override, sourceControlWritingStyle: { ...obj(DEFAULTS.sourceControlWritingStyle), ...obj(settings.sourceControlWritingStyle), ...obj(override.sourceControlWritingStyle) } };
}

function row(settings: Obj, projectId: string, environmentLabel: string, key: string, partial: Partial<ScopedRow>): ScopedRow {
  const info = inheritance(settings, projectId, key, environmentLabel);
  return { key, kind: 'switch', title: '', description: '', checked: false, value: '', valueLabel: '', options: [], placeholder: '', disabled: false, first: false, status: '', child: '', reset: '', mixed: false, control: '', resetLabel: '', driver: '', badge: '', accent: '',
    traitsLabel: '', traits: [], ...info, ...partial };
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
    row(settings, projectId, environmentLabel, 'defaultAutoPull', { kind: 'switch', title: 'Automatically pull', control: 'Default automatic pull', resetLabel: 'default automatic pull', first: true, checked: value.defaultAutoPull === true, disabled: !writable,
      description: project ? "Keeps this project's default branch current when the checkout has no local changes or commits." : 'Keeps the default branch current when the checkout has no local changes or commits. Projects can override it.',
      reset: resetFor('defaultAutoPull', value.defaultAutoPull === true) }),
    row(settings, projectId, environmentLabel, 'pullRequestMergeMethod', { kind: 'select', title: 'Default merge method', control: 'Default pull request merge method', resetLabel: 'default merge method', value: merge, valueLabel: merge === 'last' ? 'Last selected' : MERGE_LABELS[merge] || merge, disabled: !writable,
      description: project ? 'Pull requests in this project start with this method.' : 'Pull requests start with this method. Last selected reuses whatever you chose most recently on this device.',
      options: [['last', 'Last selected'], ['merge', 'Merge'], ['squash', 'Squash and merge'], ['rebase', 'Rebase and merge']].map(([v, l]) => ({ value: v, label: l, selected: v === merge })),
      reset: resetFor('pullRequestMergeMethod', merge !== 'last') }),
  ];
  const mode = str(value.branchNamingMode, 'static'), writing = str(style.mode, 'repo_conventions');
  const textProviders = providers.filter(provider => provider.supportsTextGeneration !== false && providerAvailable(provider));
  const available = textProviders.some(provider => arr(provider.models).some(model => model.isUnavailable !== true));
  const modelRow = (key: string, title: string, description: string) => {
    const enabled = value[key] != null;
    const resolved = resolveTextGenerationSelection(obj(value[key] ?? value.sourceControlWriterModelSelection ?? value.textGenerationModelSelection), textProviders);
    const badge = providerBadge(resolved?.provider, providers);
    const traits = resolved ? traitsControl({ providers }, resolved.provider, resolved.model, resolved.options) : null;
    return row(settings, projectId, environmentLabel, key, { kind: 'model', title, checked: enabled, disabled: !writable,
      description, value: resolved ? `${str(resolved.provider.instanceId)}:${str(resolved.model?.slug)}` : '',
      valueLabel: resolved?.model ? triggerModelName(resolved.model) : 'Choose model', driver: str(resolved?.provider.driver),
      badge: badge.providerBadge, accent: badge.providerBadgeColor, traitsLabel: str(traits?.label2), traits: traits?.options2 ?? [],
      status: !available ? 'No text generation providers available.' : '' });
  };
  const text = [
    row(settings, projectId, environmentLabel, 'branchNamingMode', { kind: 'select', title: 'Worktree branch naming', resetLabel: 'branch naming', first: true, value: mode, valueLabel: BRANCH_MODES[mode] || mode, disabled: !writable,
      description: 'Choose how new worktree branches are named from your first message.', options: Object.entries(BRANCH_MODES).map(([v, l]) => ({ value: v, label: l, selected: v === mode })), reset: resetFor('branchNamingMode', mode !== 'static'),
      status: mode === 'semantic' ? 'The model chooses a prefix that describes the work, such as feat/add-search, fix/login-timeout, or refactor/auth.' : '' }),
    ...(mode === 'static' ? [row(settings, projectId, environmentLabel, 'branchNamePrefix', { kind: 'text', title: 'Branch prefix', resetLabel: 'branch prefix', value: str(value.branchNamePrefix), placeholder: 'No prefix', disabled: !writable,
      description: 'For example, t3 or t3/ produces t3/add-search. Leave empty for no prefix.', reset: resetFor('branchNamePrefix', str(value.branchNamePrefix) !== 't3') })] : []),
    ...(mode === 'custom' ? [row(settings, projectId, environmentLabel, 'branchNameInstructions', { kind: 'textarea', title: 'Branch naming instructions', resetLabel: 'branch naming instructions', value: str(value.branchNameInstructions), placeholder: 'Use julius/ followed by the issue ID and a short description.', disabled: !writable,
      description: 'Appended to the naming prompt. The model returns the complete branch name; no prefix or suffix is added.', reset: resetFor('branchNameInstructions', str(value.branchNameInstructions) !== '') })] : []),
    row(settings, projectId, environmentLabel, 'sourceControlWritingStyle', { kind: 'select', title: 'Source control writing style', resetLabel: 'source control writing style', value: writing, valueLabel: WRITING_MODES[writing]?.label || writing, disabled: !writable,
      description: WRITING_MODES[writing]?.description || '', options: Object.entries(WRITING_MODES).map(([v, l]) => ({ value: v, label: l.label, selected: v === writing })),
      child: writing === 'custom' ? str(style.customInstructions) : '', placeholder: 'Keep titles concise. Use short bullet points in descriptions.',
      reset: project ? (overridden('sourceControlWritingStyle') ? 'key=sourceControlWritingStyle&value=__inherit__' : '') : (writing !== 'repo_conventions' || str(style.customInstructions) !== '' ? 'key=sourceControlWritingStyle&value=__default__' : '') }),
    row(settings, projectId, environmentLabel, 'sourceControlWritingStyle', { key: 'sourceControlWritingStyle.followChangeRequestTemplates', kind: 'switch', title: 'Follow change request templates', resetLabel: 'change request templates', checked: style.followChangeRequestTemplates !== false, disabled: !writable,
      description: "Use the repository's template for change request descriptions when available.", reset: project ? '' : style.followChangeRequestTemplates === false ? 'key=sourceControlWritingStyle.followChangeRequestTemplates&value=__default__' : '' }),
    row(settings, projectId, environmentLabel, 'removeAgentCreditsOnMerge', { title: 'Remove agent credits on merge', checked: value.removeAgentCreditsOnMerge === true, disabled: !writable,
      description: 'Omit agent credits from generated merge messages.', reset: resetFor('removeAgentCreditsOnMerge', value.removeAgentCreditsOnMerge === true) }),
    modelRow('sourceControlWriterModelSelection', 'Source control writer model', "Model for source control text and branch or bookmark names. Off uses the environment's text generation model."),
    row(settings, projectId, environmentLabel, 'reviewGuidesEnabled', { title: 'Review guides', checked: settings.reviewGuidesEnabled === true, disabled: project || !writable,
      description: 'Adds a Guide tab to pull requests: an agent writes a tour of the change, stop by stop, with the calls a maintainer should make.' }),
    ...(settings.reviewGuidesEnabled === true ? [modelRow('reviewGuideModelSelection', 'Review guide model',
      'Model that reads a pull request to write its review guide. It reads the change itself over many steps, so a stronger model helps. Off uses the source control writer model.')] : []),
  ];
  return { repositories, text: text.map((entry, index) => ({ ...entry, first: index === 0 })) };
}

/** Integrations: Agent browser access, and the device hub rows. */
export function integrationRows(settings: Obj, projectId: string, environmentLabel: string, writable: boolean) {
  const value = effective(settings, projectId), project = Boolean(projectId);
  const overridden = (key: string) => project && Object.prototype.hasOwnProperty.call(obj(obj(settings.projectSettingsOverrides)[projectId]), key);
  return {
    browser: [row(settings, projectId, environmentLabel, 'enableAgentBrowserAccess', { kind: 'switch', title: 'Agent browser access', resetLabel: 'default browser access', first: true, checked: value.enableAgentBrowserAccess !== false, disabled: !writable,
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
export function scopedPatch(settings: Obj, projectId: string, key: string, raw: string, providers: Obj[] = []): Obj {
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
    value = { instanceId: model.slice(0, at), model: model.slice(at + 1), ...(id ? { options: [{ id, value: choice === 'true' ? true : choice === 'false' ? false : choice }] } : {}) };
  }
  else if ((root === 'sourceControlWriterModelSelection' || root === 'reviewGuideModelSelection')) {
    if (raw === 'off') value = null;
    else if (raw === 'default') {
      const values = effective(settings, projectId), fallback = obj(root === 'reviewGuideModelSelection' ? values.sourceControlWriterModelSelection ?? values.textGenerationModelSelection : values.textGenerationModelSelection);
      const resolved = resolveTextGenerationSelection(fallback, providers.filter(provider => providerAvailable(provider) && provider.supportsTextGeneration !== false));
      if (resolved?.model) value = { instanceId: str(resolved.provider.instanceId), model: str(resolved.model.slug), options: resolved.options };
      else if (str(fallback.instanceId) && str(fallback.model)) value = fallback;
      else throw new Error('No text generation providers available.');
    }
    else {
      // "instance:model", or "instance:model#optionId=value" (TraitsPicker): one trait changes, the others stay.
      const [model, option] = raw.split('#'), at = model!.indexOf(':');
      if (at <= 0) throw new Error('Choose an available model.');
      const instanceId = model!.slice(0, at), slug = model!.slice(at + 1);
      if (option === undefined) value = { instanceId, model: slug };
      else {
        const split = option.indexOf('='), id = option.slice(0, split), choice = option.slice(split + 1);
        if (split <= 0 || !choice) throw new Error('Choose a supported option.');
        const current = obj(effective(settings, projectId)[root]);
        const kept = current.instanceId === instanceId && current.model === slug ? arr(current.options).filter(entry => entry.id !== id).map(entry => ({ id: str(entry.id), value: entry.value as Json })) : [];
        value = { instanceId, model: slug, options: [...kept, { id, value: choice === 'true' ? true : choice === 'false' ? false : choice }] };
      }
    }
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
      dot: !available || status !== 'authenticated' ? 'warning' : 'success', summary, account: available && status === 'authenticated' && option(auth.account) !== '', enabled: available && status === 'authenticated', details: ['github', 'gitcafe', 'bitbucket'].includes(str(item.kind)),
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

const discoveries = new WeakMap<T3Client, Map<string, { key: string; value: Obj; error: string }>>();
function scopeError(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string): string {
  if (!native?.available || !client.ready || (environmentId !== "" && environmentId !== client.environmentId)) return 'Connect an environment to inspect its version control tools and hosting integrations.';
  if (projectId && !client.shell.projects.some(project => project.id === projectId)) return 'This scope is unavailable. Choose a connected environment and an existing checkout.';
  return '';
}
function environmentLabel(client: T3Client): string { return str(obj(client.config.environment).label, 'Environment'); }

export async function sourceControlPage(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, active: boolean, rescan: number, machine = '', projectKey = '', checkout = '') {
  const empty = { available: false, error: '', project: projectId !== '', scope: `${environmentId}:${projectId}`, repositories: [] as ScopedRow[], text: [] as ScopedRow[],
    scanned: false, scanError: '', vcs: [] as ReturnType<typeof discoveryRows>['vcs'], providers: [] as ReturnType<typeof discoveryRows>['providers'], fetches: [fetchInterval({})], hostCredentials: [] as HostCredentials[] };
  if (!active) return empty;
  if (!native?.available) return { ...empty, error: 'Connect an environment to inspect its version control tools and hosting integrations.' };
  try {
    const legacy = !projectKey && projectId ? groupOfMember(client, projectId) : undefined;
    if (projectId && !projectKey && !legacy) return { ...empty, error: 'This scope is unavailable. Choose a connected environment and an existing checkout.' };
    const scope = resolveScope(client, machine, legacy?.key ?? projectKey, checkout);
    const context = serverContext(client, scope, await memberFiles(client, native, scope.members, scope));
    const representative = scopeRepresentative(scope);
    if (!representative) return { ...empty, error: scope.message || 'Connect an environment to inspect its version control tools and hosting integrations.' };
    const owner = environmentClient(client, representative.environmentId, native), access = owner.restAccess(native);
    const selectedProjectId = str(scope.members.find(member => member.environmentId === owner.environmentId)?.id);
    // The selected connection's subscribed config supplies both tools and ordinary preference rows.
    const config = owner.config, settings = obj(config.settings);
    if (selectedProjectId && obj(obj(config.environment).capabilities).projectSettingsOverrides !== true) return { ...empty, error: 'Update the selected environment to configure project overrides.' };
    const rows = sourceControlRows(settings, selectedProjectId, environmentLabel(owner), arr(config.providers), can(owner, 'settings:write'));
    for (const row of [...rows.repositories, ...rows.text]) row.layerGroups = inheritanceGroups(context, row.key, DEFAULTS);
    // Discovery scans one machine's tools (server.discoverSourceControl); a rescan re-runs it.
    const key = `${owner.origin}:${owner.generation}:${rescan}`;
    let owned = discoveries.get(client);
    if (!owned) { owned = new Map(); discoveries.set(client, owned); }
    let cached = owned.get(owner.environmentId);
    if (!cached || cached.key !== key) {
      try { cached = { key, value: await access.request('server.discoverSourceControl', {}), error: '' }; }
      catch (failure) { if (letGo(failure)) throw failure; cached = { key, value: {}, error: failure instanceof Error ? failure.message : 'Could not scan the server environment.' }; }
      owned.set(owner.environmentId, cached);
    }
    const found = discoveryRows(cached.value);
    return { ...empty, project: Boolean(selectedProjectId), available: true, ...rows, scanned: true, scanError: cached.error, ...found, fetches: [fetchInterval(settings)],
      hostCredentials: hostCredentials(obj(owner.config.settings), cached.value, owner.environmentId, rescan, can(owner, 'settings:write') && scope.kind !== 'project' && scope.kind !== 'checkout' && !projectId) };
  } catch (failure) { return { ...empty, error: failure instanceof Error ? failure.message : 'Could not load settings.' }; }
}

export async function integrationsPage(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, active: boolean, machine = '', projectKey = '', checkout = '') {
  const empty = { available: false, error: '', project: projectId !== '', scope: `${environmentId}:${projectId}`, deviceScope: '', browser: [] as ScopedRow[], deviceHub: blankRow(), agentDevice: blankRow(), hubStatus: '', agentStatus: '', hosts: 0,
    hubTool: deviceTool('hub', null), agentTool: deviceTool('agent', null), hostsRow: blankRow(), deviceHosts: deviceHostsView(client, {}, null, projectId !== '', false),
    simulatorSupport: [] as SimulatorSupportRow[], linkTarget: linkTargetPreference(client) }; // "Open links in"
  if (!active) return { ...empty };
  const error = scopeError(client, native, environmentId, projectId);
  if (error || !native) return { ...empty, error };
  try {
    // useScopedSettings and useDeviceState (IntegrationsSettings.tsx): the connection's server config (server.getConfig
    // at connect, then subscribeServerConfig) and its subscribeDeviceState stream, never a read per answer. A read here
    // re-asked this page: device.list's inspection publishes to the device stream, whose event bumps data.revision.
    const config = client.config, settings = obj(config.settings);
    if (projectId && obj(obj(config.environment).capabilities).projectSettingsOverrides !== true) return { ...empty, error: 'Update the selected environment to configure project overrides.' };
    const rows = integrationRows(settings, projectId, environmentLabel(client), can(client, 'settings:write'));
    // Device hub and Agent device access follow the settings scope: the representative's value, mixed across
    // the selected targets, and Agent device access open once any connected environment runs the hub.
    // A bare project id (settingsProjectId with no project key: Project settings' target) is its project on every environment (settingsScopeOf).
    const legacy = !projectKey && projectId ? groupOfMember(client, projectId) : undefined;
    const devices = deviceScope(client, machine, legacy ? legacy.key : projectKey, legacy ? '' : checkout);
    const scope = resolveScope(client, machine, legacy?.key ?? projectKey, checkout);
    const context = serverContext(client, scope, await memberFiles(client, native, scope.members, scope));
    for (const row of [...rows.browser, rows.deviceHub, rows.agentDevice, rows.hostsRow]) row.layerGroups = inheritanceGroups(context, row.key, DEFAULTS);
    rows.deviceHub = { ...rows.deviceHub, checked: devices.checked.enableDeviceSupport, mixed: devices.mixed.enableDeviceSupport,
      disabled: devices.project || devices.unavailable || devices.connectedCount === 0 || !can(client, 'settings:write') };
    rows.agentDevice = { ...rows.agentDevice, checked: devices.checked.enableAgentDeviceAccess, mixed: devices.mixed.enableAgentDeviceAccess,
      disabled: !can(client, 'settings:write') || devices.connectedCount === 0 || (!devices.project && !devices.anyHubEnabled) };
    // DeviceToolVersions over the stream's state; "Check versions" (rest:device-tools) is the one inspection.
    await watchDevice(client, native);
    await watchDeviceHostStates(client, native, scope.selected.map(environment => environment.environmentId));
    const deviceState = deviceStateOf(client), tools = obj(arr(deviceState?.hosts).find(host => host.kind === 'local')?.tools);
    const hubStatus = deviceState ? toolVersion(tools.hub) : 'Version unknown', agentStatus = deviceState ? toolVersion(tools.agent) : 'Version unknown';
    return { ...empty, available: true, ...rows, deviceScope: devices.scopeKey, hubStatus, agentStatus, hosts: Array.isArray(settings.deviceHosts) ? settings.deviceHosts.length : 0,
      hubTool: deviceTool('hub', deviceState), agentTool: deviceTool('agent', deviceState), deviceHosts: scopedDeviceHostsView(client, native, resolveScope(client, machine, legacy?.key ?? projectKey, checkout).selected, resolveScope(client, machine, legacy?.key ?? projectKey, checkout).environments, deviceState, projectId !== ''),
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
  return { key: '', kind: 'switch', title: '', description: '', checked: false, value: '', valueLabel: '', options: [], placeholder: '', disabled: true, first: false, summary: '', state: '', layers: [], layerGroups: [], reset: '', status: '', child: '', mixed: false, control: '', resetLabel: '', driver: '', badge: '', accent: '', traitsLabel: '', traits: [] };
}
