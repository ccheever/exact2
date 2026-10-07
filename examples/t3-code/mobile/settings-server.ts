// @ref llp/1106.009-mobile-settings.decision.md#root-and-native-lifetime
// @ref llp/1106.003-pairing-and-transport.decision.md#decision
// T3 Code 365aa87982 SettingsServerControlsRouteScreen/settings-scoped-server.
import { mobileClient } from './client';
import { arr, obj, str, type Obj, type Json } from './shared/domain';
import { letGo } from './shared/let-go';
import { ClientError, type Native } from './shared/protocol';
import { clearProjectSettingsOverrides, isNullableProjectSettingsOverride, resolveProjectSettings } from './shared/scoped-settings-plan';
import { settingsAdoptConfig, settingsCall, settingsEndpoint, settingsGrants, settingsNative, settingsSources,
  type MobileSettingsSource } from './settings-server-source';

export type MobileServerPage = 'new-threads' | 'source-control' | 'agent-behavior' | 'maintenance';
export interface MobileServerScope { environmentIds: string[]; members: { environmentId: string; id: string }[] | null; projectLabel: string }
export const MOBILE_SERVER_ROUTES: Record<string, MobileServerPage> = {
  SettingsEnvironmentNewThreads: 'new-threads', SettingsEnvironmentSourceControl: 'source-control',
  SettingsEnvironmentAgentBehavior: 'agent-behavior', SettingsEnvironmentMaintenance: 'maintenance',
};
export const MOBILE_SERVER_PAGE_KEYS: Record<MobileServerPage, readonly string[]> = {
  'new-threads': ['defaultThreadEnvMode', 'worktreeSubmodules', 'defaultRuntimeMode'],
  'source-control': ['defaultAutoPull', 'removeAgentCreditsOnMerge', 'newWorktreesStartFromOrigin', 'branchNamingMode', 'branchNamePrefix', 'branchNameInstructions'],
  'agent-behavior': ['responseStreamingMode', 'enableAgentBrowserAccess'], maintenance: ['continueThreadsAfterServerUpdate'],
};
const titles = { 'new-threads': 'New threads', 'source-control': 'Source control', 'agent-behavior': 'Agent behavior', maintenance: 'Maintenance' };
export function decodeMobileServerScope(raw: string): MobileServerScope {
  const value = obj(JSON.parse(raw));
  if (!Array.isArray(value.environmentIds) || value.environmentIds.some(id => typeof id !== 'string' || !id)
    || !(value.members === null || Array.isArray(value.members) && value.members.every(member => typeof obj(member).environmentId === 'string' && !!obj(member).environmentId && typeof obj(member).id === 'string' && !!obj(member).id)))
    throw new ClientError('The settings scope is invalid.');
  return { environmentIds: [...new Set(value.environmentIds as string[])], members: value.members === null ? null
    : arr(value.members).map(member => ({ environmentId: str(member.environmentId), id: str(member.id) })), projectLabel: str(value.projectLabel, 'Unavailable project') };
}
export function mobileServerTargets(scope: MobileServerScope, sources: MobileSettingsSource[]) {
  const selected = sources.filter(source => scope.environmentIds.includes(source.environmentId) && source.enabled && source.phase === 'connected' && Object.keys(obj(source.config.settings)).length > 0);
  return selected.flatMap(source => (scope.members === null ? [null] : scope.members.filter(member => member.environmentId === source.environmentId).map(member => member.id))
    .map(projectId => {
      const stored = obj(source.config.settings), resolved = resolveProjectSettings(stored, projectId);
      // Mobile pin adds this project key after the adopted shared client's revision.
      // Keep the shared body intact; apply only the pinned mobile addition at this boundary.
      const credits = projectId === null ? undefined : obj(obj(stored.projectSettingsOverrides)[projectId]).removeAgentCreditsOnMerge;
      return { source, projectId, ...resolved, ...(typeof credits === 'boolean' ? {
        settings: { ...resolved.settings, removeAgentCreditsOnMerge: credits },
        sources: { ...resolved.sources, removeAgentCreditsOnMerge: 'project' as const },
      } : {}) };
    }));
}
type Target = ReturnType<typeof mobileServerTargets>[number];
export function mobileServerPlan(targets: Target[], patch: Obj, clearKeys?: readonly string[]): { environmentId: string; patch: Obj }[] {
  const writes = new Map<string, Obj>();
  for (const target of targets) {
    if (target.projectId === null) { if (!clearKeys) writes.set(target.source.environmentId, patch); continue; }
    if (obj(obj(target.source.config.environment).capabilities).projectSettingsOverrides !== true) throw new ClientError('Update the selected environments to edit project overrides.');
    let next: Obj | null;
    if (clearKeys) next = clearProjectSettingsOverrides(obj(target.source.config.settings), target.projectId, clearKeys);
    else {
      next = { ...obj(obj(obj(target.source.config.settings).projectSettingsOverrides)[target.projectId]) };
      for (const [key, value] of Object.entries(patch)) {
        if (value === null && !isNullableProjectSettingsOverride(key)) delete next[key];
        else next[key] = value;
      }
    }
    const write = writes.get(target.source.environmentId) ?? { projectSettingsOverrides: {} };
    writes.set(target.source.environmentId, { projectSettingsOverrides: { ...obj(write.projectSettingsOverrides), [target.projectId]: next } });
  }
  return [...writes].map(([environmentId, patch]) => ({ environmentId, patch }));
}
const choices = {
  defaultThreadEnvMode: [['null', 'Inherit', "Use the repository's t3.json, or the current checkout."], ['local', 'Current checkout', 'Start new threads in the existing workspace.'], ['worktree', 'New worktree', 'Give each new thread a separate checkout.']],
  worktreeSubmodules: [['null', 'Inherit', "Use the repository's t3.json, or initialize recursively."], ['recursive', 'Recursive', 'Initialize nested submodules too.'], ['top-level', 'Top level only', 'Skip submodules declared inside other submodules.'], ['none', 'Skip', 'Leave submodules empty for a setup script.']],
  defaultRuntimeMode: [['approval-required', 'Supervised', 'Ask before commands and file changes.'], ['auto-accept-edits', 'Auto-accept edits', 'Auto-approve edits, ask before other actions.'], ['auto', 'Auto', 'Supported providers approve routine actions; others still ask.'], ['full-access', 'Full access', 'Allow commands and edits without prompts.']],
  branchNamingMode: [['static', 'Static prefix', 'Add your prefix to the generated branch name.'], ['semantic', 'Semantic prefix', 'Let the model choose feat/, fix/, refactor/, or another prefix.'], ['custom', 'Custom instructions', 'Generate the complete name with no added prefix or suffix.']],
  responseStreamingMode: [['turn', 'After the turn', 'Show the answer when the agent finishes.'], ['paragraph', 'Finished paragraphs', 'Show each paragraph or code block as it completes.']],
} as const;
export function mobileServerValue(page: MobileServerPage, key: string, raw: string, project: boolean): Json {
  if (!(MOBILE_SERVER_PAGE_KEYS[page].includes(key) || page === 'maintenance' && key === 'enableProviderUpdateChecks' && !project)) throw new ClientError('This setting does not belong to the selected page or scope.');
  if (key in choices) {
    if (!(choices[key as keyof typeof choices] as readonly (readonly string[])[]).some(choice => choice[0] === raw) || project && raw === 'null') throw new ClientError('Choose a supported setting value.');
    return raw === 'null' ? null : raw;
  }
  if (key === 'branchNamePrefix' || key === 'branchNameInstructions') return raw.trim();
  if (raw !== 'true' && raw !== 'false') throw new ClientError('Choose On or Off.');
  return raw === 'true';
}
export interface ServerSettingsRow { key: string; kind: string; label: string; description: string; icon: string; value: string; selected: boolean; mixed: boolean; disabled: boolean; separated: boolean; placeholder: string }
export interface ServerSettingsSection { title: string; mixed: boolean; rows: ServerSettingsRow[] }
const inFlight = new Set<string>();
const pageErrors = new Map<string, string>();
export function mobileServerProjection(page: MobileServerPage, scope: MobileServerScope, sources: MobileSettingsSource[], writable: Set<string>, pending = false, error = '') {
  const targets = mobileServerTargets(scope, sources), project = scope.members !== null;
  const supportsOverrides = targets.every(target => obj(obj(target.source.config.environment).capabilities).projectSettingsOverrides === true);
  const disabled = pending || targets.length === 0 || targets.some(target => !writable.has(target.source.environmentId)) || project && !supportsOverrides;
  const mixed = (key: string) => targets.length === 0 || targets.some(target => target.settings[key] !== targets[0]!.settings[key]);
  const uniform = (key: string) => mixed(key) ? null : targets[0]!.settings[key];
  const sections: ServerSettingsSection[] = [];
  const base = (key: string, kind: string, label: string): ServerSettingsRow => ({ key, kind, label, description: '', icon: '', value: '', selected: false, mixed: false, disabled, separated: false, placeholder: '' });
  const choiceSection = (key: keyof typeof choices, title: string) => {
    const rows: ServerSettingsRow[] = choices[key].filter(choice => !project || choice[0] !== 'null').map((choice, index) => ({ ...base(key, 'choice', choice[1]), description: choice[2], value: choice[0], selected: !mixed(key) && uniform(key) === (choice[0] === 'null' ? null : choice[0]), separated: index > 0 }));
    const section: ServerSettingsSection = { title, mixed: !pending && mixed(key), rows }; sections.push(section); return section;
  };
  const toggle = (section: ServerSettingsSection, key: string, label: string, description: string, icon: string, additionalDisabled = false) => section.rows.push({ ...base(key, 'switch', label), description, icon, selected: uniform(key) === true, mixed: mixed(key), value: uniform(key) === true ? 'false' : 'true', disabled: disabled || additionalDisabled, separated: section.rows.length > 0 });
  const section = (title: string) => { const item = { title, mixed: false, rows: [] as ServerSettingsRow[] }; sections.push(item); return item; };
  if (targets.length) {
    if (page === 'new-threads') {
      choiceSection('defaultThreadEnvMode', 'Default workspace'); choiceSection('worktreeSubmodules', 'Worktree submodules'); choiceSection('defaultRuntimeMode', 'Default permissions');
    } else if (page === 'source-control') {
      const naming = choiceSection('branchNamingMode', 'Worktree branch naming'), mode = uniform('branchNamingMode');
      if (mode === 'static' || mode === 'custom') {
        const key = mode === 'static' ? 'branchNamePrefix' : 'branchNameInstructions';
        naming.rows.push({ ...base(key, mode === 'static' ? 'input' : 'textarea', mode === 'static' ? 'Branch prefix' : 'Branch naming instructions'), value: str(uniform(key)),
          description: mode === 'static' ? 'Use t3 or t3/ for t3/add-search. Leave empty for no prefix.' : 'Append instructions to the naming prompt.',
          placeholder: mixed(key) ? mode === 'static' ? 'Mixed' : 'Mixed. Enter instructions for all selected targets.' : mode === 'static' ? 'No prefix' : 'Use julius/ followed by the issue ID and a short description.' });
      }
      toggle(section('Pull requests'), 'removeAgentCreditsOnMerge', 'Remove agent credits when merging', 'Remove recognized agent credits from GitHub merge and squash messages, keeping human co-authors. Includes auto-merge. Excludes merge queues, stack merges, and existing commits.', 'arrow.triangle.merge');
      toggle(section('Default branch'), 'defaultAutoPull', 'Automatically pull', 'Keep the default branch current when there are no local changes.', 'arrow.down.circle');
      toggle(section('Worktrees'), 'newWorktreesStartFromOrigin', 'Start from origin', 'Base new worktrees on the remote branch.', 'arrow.triangle.branch');
    } else if (page === 'agent-behavior') {
      choiceSection('responseStreamingMode', 'Response streaming');
      toggle(section('Preview browser'), 'enableAgentBrowserAccess', 'Agent browser access', 'Allow agents to use the in-app preview browser.', 'globe');
    } else {
      if (!project) section('Manage environments').rows = [...new Map(targets.map(target => [target.source.environmentId, target.source])).values()].map(source => ({ ...base('environment', 'navigate', source.label), icon: 'server.rack', description: 'Server and provider updates', value: source.key, disabled: false }));
      const updates = section('Updates'), continuation = targets.every(target => obj(obj(target.source.config.environment).capabilities).threadRestartContinuation === true);
      toggle(updates, 'enableProviderUpdateChecks', 'Check provider updates', project ? 'Environment-wide setting. Select All projects to change it.' : 'Check installed provider CLIs for newer versions.', 'arrow.clockwise', project);
      toggle(updates, 'continueThreadsAfterServerUpdate', 'Continue after restart', continuation ? 'Resume interrupted threads after an update or restart.' : 'Update older servers to control restart continuation.', 'arrow.uturn.forward', !continuation);
    }
  }
  return { ready: true, page, title: titles[page], error, pending, emptyMessage: targets.length ? '' : project ? 'Select a project with a checkout on a connected environment.' : 'Use the filter above to select a connected environment.',
    project, projectLabel: scope.projectLabel, supportsOverrides, hasOverrides: targets.some(target => MOBILE_SERVER_PAGE_KEYS[page].some(key => target.sources[key] === 'project')),
    clearDisabled: disabled, sections };
}
async function load(scope: MobileServerScope, native: Native, freshConfig: boolean) {
  const sources = (await settingsSources(native)).filter(source => scope.environmentIds.includes(source.environmentId) && source.enabled && source.phase === 'connected');
  const endpoints = sources.map(source => settingsEndpoint(source, native)), writable = new Set<string>();
  await Promise.all(endpoints.map(async endpoint => {
    const session = await settingsCall(endpoint, { op: 'http', path: '/api/auth/session' });
    if (settingsGrants(session, 'settings:write')) writable.add(endpoint.source.environmentId);
    if (freshConfig) settingsAdoptConfig(endpoint, await settingsCall(endpoint, { op: 'request', method: 'server.getConfig', payload: {} }));
  }));
  return { sources, endpoints, writable };
}
export async function mobileServerSettings(page: MobileServerPage, scopeJSON: string, nativeInput: Native | null | undefined) {
  const scope = decodeMobileServerScope(scopeJSON), key = `${page}:${scopeJSON}`;
  if (!nativeInput?.available) return mobileServerProjection(page, scope, [], new Set(), false, 'Open T3 Code on your iPhone or iPad to manage server settings.');
  try {
    const { sources, writable } = await load(scope, settingsNative(nativeInput), false);
    return mobileServerProjection(page, scope, sources, writable, inFlight.size > 0, pageErrors.get(key) ?? '');
  } catch (error) { if (letGo(error)) throw error; return mobileServerProjection(page, scope, [], new Set(), false, error instanceof Error ? error.message : 'Could not load server settings.'); }
}
export async function mobileServerSettingsCommand(page: MobileServerPage, scopeJSON: string, key: string, raw: string, nativeInput: Native | null | undefined) {
  const identity = `${page}:${scopeJSON}`;
  if (inFlight.size > 0) return { revision: mobileClient.revision, message: 'A settings update is already in progress.' };
  inFlight.add(identity); pageErrors.delete(identity);
  try {
    if (!nativeInput?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to manage server settings.');
    const scope = decodeMobileServerScope(scopeJSON), { sources, writable, endpoints } = await load(scope, settingsNative(nativeInput), true);
    const targets = mobileServerTargets(scope, sources);
    if (!targets.length || targets.some(target => !writable.has(target.source.environmentId))) throw new ClientError('The selected environments do not allow settings changes.');
    // A scope captured before disconnection must never silently write a smaller target set.
    const expected = scope.members === null ? scope.environmentIds : [...new Set(scope.members.map(member => member.environmentId))];
    if (expected.some(id => !targets.some(target => target.source.environmentId === id))) throw new ClientError('The selected settings scope changed. Reopen this page.');
    if (key === 'continueThreadsAfterServerUpdate' && targets.some(target => obj(obj(target.source.config.environment).capabilities).threadRestartContinuation !== true)) throw new ClientError('Update older servers to control restart continuation.');
    const patch = key === 'clear' ? {} : { [key]: mobileServerValue(page, key, raw, scope.members !== null) };
    const writes = mobileServerPlan(targets, patch, key === 'clear' ? MOBILE_SERVER_PAGE_KEYS[page] : undefined);
    if (!writes.length) throw new ClientError('There are no settings changes to save.');
    const results = await Promise.allSettled(writes.map(async write => {
      const endpoint = endpoints.find(entry => entry.source.environmentId === write.environmentId)!;
      const updated = await settingsCall(endpoint, { op: 'request', method: 'server.updateSettings', payload: { patch: write.patch } });
      settingsAdoptConfig(endpoint, { ...endpoint.source.config, settings: updated });
    }));
    const abandoned = results.find(result => result.status === 'rejected' && letGo(result.reason));
    if (abandoned?.status === 'rejected') throw abandoned.reason;
    const failures = results.flatMap((result, index) => result.status === 'rejected' ? [`${sources.find(source => source.environmentId === writes[index]!.environmentId)?.label ?? writes[index]!.environmentId}: ${result.reason instanceof Error ? result.reason.message : 'Setting not saved.'}`] : []);
    if (failures.length) throw new ClientError(`${failures.length < writes.length ? 'Setting saved on some environments. ' : ''}${failures.join('\n')}`);
    return { revision: ++mobileClient.revision, message: '' };
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'Could not save server settings.';
    pageErrors.set(identity, message); return { revision: ++mobileClient.revision, message };
  } finally { inFlight.delete(identity); }
}
