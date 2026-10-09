// Mutations for the settings routes the settings-rest lane owns: Projects,
// Integrations, Keybindings, Source control, Storage, Archive, Scheduled
// tasks, Diagnostics and licenses. Every write re-reads the server's current
// state first and refuses a stale scope rather than broadening it.
import { toggleRemovalPick } from './settings-a-collections';
import type { T3Client } from './client';
import { ClientError, type Files, type Native } from './protocol';
import { applyShell, arr, initialShell, obj, str, type Json, type Obj } from './domain';
import { shortcutInput, whenExpression, validShortcut, validWhen } from './keybinding-settings';
import { rowId } from './keybinding-view';
import { scheduledTaskCommand } from './scheduled-tasks-commands';
import { scopedPatch, fetchIntervalPatch } from './source-control-view';
import { archiveCommand } from './settings-a-archive';
import { deviceToolsCommand } from './settings-a-integrations';
import { devicePlatformsCommand } from './device-support';
import { telemetryCommand, telemetryLocal } from './settings-a-telemetry';
import { openLogsFolder } from './diagnostics-view';
import { bitbucketCommand } from './settings-a-bitbucket';
import { deviceHostsCommand } from './settings-a-hosts';
import { deviceScopedCommand } from './settings-integrations-scope';
import { clientValue, decodeClientPrefs, type ClientPrefs } from './settings-core'; // browser-surface part 3: the Browser defaults
import { copyThreadReference } from './thread-reference'; // thread-commands-and-keys: ⇧⌘C copies the PR link or the thread ID

/** Contract sends `a=encodeURIComponent(x)&b=…`; Hermes has no URLSearchParams. */
export function params(value: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const part of value.split('&')) {
    if (!part) continue;
    const at = part.indexOf('=');
    const key = decodeURIComponent(at < 0 ? part : part.slice(0, at));
    out[key] = at < 0 ? '' : decodeURIComponent(part.slice(at + 1));
  }
  return out;
}

export function splitScope(scope: string): { environmentId: string; projectId: string } {
  const parts = scope.split(':');
  if (parts.length !== 2) throw new ClientError('That environment or project is no longer selected.');
  return { environmentId: parts[0], projectId: parts[1] };
}

type Access = ReturnType<T3Client['restAccess']>;

async function currentScope(client: T3Client, access: Access, scope: string) {
  const { environmentId, projectId } = splitScope(scope);
  if (environmentId && environmentId !== client.environmentId) throw new ClientError('That environment is no longer selected.');
  const shell = applyShell(initialShell(), await access.http('/api/orchestration/shell'));
  if (projectId && !shell.projects.some(project => project.id === projectId)) throw new ClientError('That project is no longer available.');
  return { environmentId, projectId, shell };
}

const worktreeKeys = ['worktreeOnDelete', 'worktreeAfterDays', 'worktreeOnMerge', 'worktreeUnchanged'];
const retentionKeys = ['worktreeAfterDays', 'browserArtifactsAfterDays', 'logsAfterDays'];

/** resolveWorktreeCleanup from packages/shared/projectSettings.ts. */
export function resolveWorktreeCleanup(settings: Obj, projectId: string): Obj {
  const override = projectId ? obj(obj(settings.projectSettingsOverrides)[projectId]) : {};
  const policy = obj(override.worktreeCleanup ?? settings.worktreeCleanup);
  if (policy.mode === 'custom') return { ...obj(policy.rules) };
  if (policy.mode === 'off') return { worktreeAfterDays: null, worktreeOnMerge: false, worktreeOnDelete: false, worktreeUnchanged: false };
  const cleanup = obj(settings.storageCleanup);
  return { worktreeAfterDays: cleanup.worktreeAfterDays ?? null, worktreeOnMerge: cleanup.worktreeOnMerge === true, worktreeOnDelete: cleanup.worktreeOnDelete === true, worktreeUnchanged: cleanup.worktreeUnchanged === true };
}

/** StorageSettings.tsx writes, through planScopedSettingsPatch / planScopedSettingsClear. */
export function storagePatch(settings: Obj, projectId: string, key: string, raw: string): Obj {
  let value: Json;
  if (key === 'mode') {
    if (!projectId) throw new ClientError('Choose a project to change its worktree cleanup.');
    if (!['inherit', 'off', 'custom'].includes(raw)) throw new ClientError('Unsupported worktree cleanup mode.');
    const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
    if (raw === 'inherit') delete current.worktreeCleanup;
    else current.worktreeCleanup = raw === 'off' ? { mode: 'off' } : { mode: 'custom', rules: resolveWorktreeCleanup(settings, projectId) };
    return { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } };
  }
  if (retentionKeys.includes(key)) {
    if (raw === 'null') value = null;
    else {
      const days = Number(raw);
      if (!/^\s*\d+(\.\d+)?\s*$/.test(raw) || !Number.isFinite(days)) throw new ClientError('Enter a number of days.');
      value = Math.min(3650, Math.max(1, Math.round(days)));
    }
  } else if (['worktreeOnDelete', 'worktreeOnMerge', 'worktreeUnchanged'].includes(key)) {
    if (raw !== 'true' && raw !== 'false') throw new ClientError('Choose On or Off.');
    value = raw === 'true';
  } else throw new ClientError('Unsupported storage rule.');
  if (projectId) {
    if (!worktreeKeys.includes(key)) throw new ClientError('Artifact and log retention belongs to the environment.');
    const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
    if (obj(current.worktreeCleanup).mode !== 'custom') throw new ClientError('Choose Custom before changing project cleanup rules.');
    current.worktreeCleanup = { mode: 'custom', rules: { ...resolveWorktreeCleanup(settings, projectId), [key]: value } };
    return { projectSettingsOverrides: { [projectId]: current } };
  }
  const patch: Obj = { storageCleanup: { [key]: value } };
  // An environment that already stores a worktreeCleanup policy keeps it in
  // step with the worktree rules (planScopedSettingsPatch, environment scope).
  if (settings.worktreeCleanup != null && worktreeKeys.includes(key)) patch.worktreeCleanup = { mode: 'custom', rules: { ...resolveWorktreeCleanup(settings, ''), [key]: value } };
  return patch;
}

export async function restCommand(client: T3Client, native: Native, storage: Files, op: string, scope: string, value: string): Promise<string> {
  if (op.startsWith('archive-')) return archiveCommand(client, native, storage, op, scope, value);
  if (op === 'device-tools') return deviceToolsCommand(client, native, params(value));
  if (op === 'device-platforms') return devicePlatformsCommand(client, native); // 5318d054a5: Simulator support Refresh
  if (op === 'diag-open-logs') return openLogsFolder(client, native);
  if (op === 'bitbucket') return bitbucketCommand(client, native, str(params(value).environment), params(value), () => { viewState(client).rescan++; });
  if (op.startsWith('diag-')) return telemetryCommand(client, native, op, params(value)); // settings-a-telemetry.ts
  if (op.startsWith('hosts-')) return deviceHostsCommand(client, native, op, params(value), splitScope(scope).projectId !== ''); // settings-a-hosts.ts
  if (op === 'browser-default') return browserDefault(client, storage, params(value)); // browser-surface part 3
  const access = client.restAccess(native);
  const input = params(value);
  if (op === 'storage') {
    const { projectId } = await currentScope(client, access, scope);
    const config = await access.request('server.getConfig');
    const capabilities = obj(obj(config.environment).capabilities);
    if (capabilities.storageCleanup !== true || (projectId && capabilities.projectWorktreeCleanup !== true)) throw new ClientError('Update the selected environment to configure storage cleanup.');
    const settings = await access.request('server.getSettings');
    const updated = await access.request('server.updateSettings', { patch: storagePatch(settings, projectId, str(input.key), str(input.value)) }, true);
    client.config = { ...client.config, settings: updated };
    return '';
  }
  if (op === 'keybinding') {
    const { environmentId } = splitScope(scope);
    if (environmentId && environmentId !== client.environmentId) throw new ClientError('That environment is no longer selected.');
    const config = await access.request('server.getConfig');
    const result = await access.request(input.action === 'remove' ? 'server.removeKeybinding' : 'server.upsertKeybinding', keybindingPayload(arr(config.keybindings), input), true);
    client.config = { ...client.config, keybindings: result.keybindings ?? client.config.keybindings };
    return '';
  }
  if (op === 'task') return scheduledTaskCommand(client, native, scope, input); // scheduled-tasks-commands.ts: the live list, any environment
  // Integrations device switches across the settings scope (settings-integrations-scope.ts).
  if (op === 'device' && input.scope !== undefined) return deviceScopedCommand(client, native, input.scope, str(input.key), str(input.value));
  if (op === 'scoped' || op === 'fetch-interval' || op === 'device') {
    // ProjectDefaultsSettings / SourceControlWritingSettings / BranchNamingSettings /
    // GitFetchIntervalSettings / DeviceIntegrationControls writes.
    const { projectId } = await currentScope(client, access, scope);
    const config = await access.request('server.getConfig');
    if (projectId && obj(obj(config.environment).capabilities).projectSettingsOverrides !== true) throw new ClientError('Connect the selected checkouts, or update their environments, to save a project override.');
    const settings = await access.request('server.getSettings');
    if (op === 'device') {
      if (projectId && input.key === 'enableAgentDeviceAccess') {
        const updated = await access.request('server.updateSettings', { patch: scopedPatch(settings, projectId, 'enableAgentDeviceAccess', input.value) }, true);
        client.config = { ...client.config, settings: updated }; return '';
      }
      if (projectId) throw new ClientError('This setting is environment-wide and cannot be overridden by a project.');
      const enabled = input.value === 'true';
      const payload = input.key === 'enableDeviceSupport' ? { enabled, ...(enabled ? { onboardingCompleted: true } : { agentAccessEnabled: false }) } : { agentAccessEnabled: enabled };
      if (input.key === 'enableAgentDeviceAccess' && settings.enableDeviceSupport !== true) throw new ClientError('Enable the device hub before changing agent device access.');
      await access.request('device.configure', payload, true);
      client.config = { ...client.config, settings: await access.request('server.getSettings') };
      return '';
    }
    let patch: Obj;
    try { patch = op === 'fetch-interval' ? fetchIntervalPatch(settings, str(input.value)) : scopedPatch(settings, projectId, str(input.key), str(input.value)); }
    catch (error) { throw new ClientError(error instanceof Error ? error.message : 'Could not save this setting.'); }
    if (op === 'fetch-interval' && projectId) throw new ClientError('This setting is environment-wide and cannot be overridden by a project.');
    const updated = await access.request('server.updateSettings', { patch }, true);
    client.config = { ...client.config, settings: updated };
    return '';
  }
  if (op === 'project-icon') {
    // ProjectSettingsPanel setProjectIcon({ faviconPath: null, projectIcon: null }), fanned out to the group.
    const { projectId, shell } = await currentScope(client, access, scope);
    const group = client.projectGroups().find(candidate => candidate.members.some(member => member.id === projectId));
    if (!projectId || !group) throw new ClientError('Choose a project to change its icon.');
    for (const member of group.members) {
      if (!shell.projects.some(project => project.id === member.id)) throw new ClientError('Project group membership changed. Reopen its settings.');
      const [commandId] = await access.ids(1);
      await access.write(storage, { method: 'projects.mutate', description: 'Reset project icon', threadId: '', text: '', uncertain: false,
        payload: { type: 'project.update', commandId, projectId: str(member.id), faviconPath: null, projectIcon: null } });
    }
    client.shell = applyShell(initialShell(), await access.http('/api/orchestration/shell'));
    return '';
  }
  if (op === 'keybinding-open') {
    // useOpenInPreferredEditor: the server opens its own keybindings.json in the
    // first available editor of contracts' EDITORS order.
    const { environmentId } = splitScope(scope);
    if (environmentId && environmentId !== client.environmentId) throw new ClientError('That environment is no longer selected.');
    const config = await access.request('server.getConfig');
    const path = str(config.keybindingsConfigPath);
    if (!path) throw new ClientError('The keybindings file was not opened.');
    const available = (Array.isArray(config.availableEditors) ? config.availableEditors : []).filter((editor): editor is string => typeof editor === 'string');
    const editor = editorOrder.find(id => available.includes(id));
    if (!editor) throw new ClientError(`No available editor can open ${path} in environment ${environmentId}.`);
    await access.request('shell.openInEditor', { cwd: path, editor }, true);
    return '';
  }
  throw new ClientError(`Unknown settings action: ${op}`);
}

const editorOrder = ['cursor', 'trae', 'kiro', 'vscode', 'vscode-insiders', 'vscodium', 'zed', 'antigravity', 'idea', 'aqua', 'clion', 'datagrip', 'dataspell', 'goland', 'phpstorm', 'pycharm', 'rider', 'rubymine', 'rustrover', 'webstorm', 'file-manager'];

/** KeybindingsSettingsPanel save/remove/reset: one binding, replaced by its exact target. */
export function keybindingPayload(bindings: Obj[], input: Record<string, string>): Obj {
  const target = (binding: Obj) => ({ command: str(binding.command), key: shortcutInput(obj(binding.shortcut)), when: whenExpression(binding.whenAst) });
  const previous = input.previous ? bindings.map(target).find(row => rowId(row.command, row.key, row.when) === input.previous) : undefined;
  if ((input.action === 'remove' || input.previous) && !previous) throw new ClientError('That keybinding is no longer available.');
  const replace = previous ? { command: previous.command, key: previous.key, ...(previous.when ? { when: previous.when } : {}) } : undefined;
  if (input.action === 'remove') return replace!;
  const key = str(input.key).trim(), command = str(input.command).trim(), when = str(input.when).trim();
  if (!command) throw new ClientError('Choose a command.');
  if (!key || key.length > 64 || when.length > 256) throw new ClientError('Enter a shortcut (up to 64 characters) and a condition (up to 256 characters).');
  if (!validShortcut(key)) throw new ClientError('Enter one key with supported shortcut modifiers.');
  if (!validWhen(when)) throw new ClientError('Use variables with !, &&, ||, and parentheses.');
  return { command, key, ...(when ? { when } : {}), ...(replace ? { replace } : {}) };
}

/** Per-client view toggles that are not settings: discovery rescans (an account's reveal is RedactedText's own state). */
export const restView = new WeakMap<T3Client, { rescan: number }>();
export function viewState(client: T3Client) {
  let state = restView.get(client);
  if (!state) { state = { rescan: 0 }; restView.set(client, state); }
  return state;
}

export async function restLocal(client: T3Client, native: Native, storage: Files, op: string, scope: string, value: string): Promise<string> {
  void native; void storage; void scope; void value;
  if (op === 'rescan') { viewState(client).rescan++; return ''; }
  if (op.startsWith('diag-')) return telemetryLocal(client, op, value); // settings-a-telemetry.ts
  // The collection removal dialog's checkboxes: device-local, never gated on a pending write.
  if (op === 'theme-pick') { const bar = value.lastIndexOf('|'); toggleRemovalPick(client, value.slice(0, bar), value.slice(bar + 1)); return ''; }
  // thread.copyReference: the PR link, else the thread ID, with the reference's toasts (thread-reference.ts).
  if (op === 'copy-thread') return copyThreadReference(client, native);
  throw new ClientError(`Unknown settings action: ${op}`);
}

/** browser-surface part 3: the Browser section's client defaults (IntegrationsSettings BrowserRecordingFrameRateSetting,
 *  BrowserRecordingInputSettings, BrowserAutoShowFloatingPreviewSetting): `key=<setting>&value=<value>`, kept on this device. */
const BROWSER_DEFAULT_KEYS = ['browserRecordingFrameRate', 'browserRecordingShowKeyPresses', 'browserRecordingShowMousePresses', 'browserAutoShowFloatingPreview'];
async function browserDefault(client: T3Client, storage: Files, input: Record<string, string>): Promise<string> {
  const key = str(input.key), parsed = BROWSER_DEFAULT_KEYS.includes(key) ? clientValue(key, str(input.value)) : undefined;
  if (parsed === undefined) throw new ClientError('Unsupported browser setting.');
  const local = client.local as unknown as { clientSettings?: ClientPrefs };
  local.clientSettings = { ...(local.clientSettings ?? decodeClientPrefs({})), [key]: parsed };
  await client.savePreferences(storage);
  return '';
}
