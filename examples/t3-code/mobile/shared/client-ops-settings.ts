// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/client-ops-settings.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The settings client.command() ops (client-ops.ts): device settings,
// Settings → General, and the server settings writes (keybindings, scheduled
// tasks, scoped and storage settings, archived threads, model and permission
// defaults, provider instances). Every write re-reads the server's current
// state first and refuses a stale scope.
import { currentTasks, liveEnvironment } from './live-streams';
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { bindingId, shortcutInput, whenExpression, validShortcut, validWhen } from './keybinding-settings';
import { validateTaskInput } from './scheduled-settings';
import { scopedSettingPatch } from './scoped-settings';
import { applyCoreSetting, applyDeviceSetting } from './settings-core';
import { runProviderOp, PROVIDER_OPS } from './providers';
import { effectiveWorktreeRules, obj, str, arr, initialShell, applyShell, type Obj } from './domain';
import { ClientError, providerAvailable, type Native, type Files } from './protocol';

/** Device settings, Settings → General (settings-core.ts) and a diagnostic's trace ID. */
export async function settingsOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'copy-diagnostic') {
      if (!value || value.length > 256) throw new ClientError('That trace ID is unavailable.');
      await this.call(native, { op: 'copyText', text: value });
      resultMessage = 'Copied trace ID';
    } else if (op === 'device-setting') {
      if (!applyDeviceSetting(this.local, id, value)) throw new ClientError('Unsupported device setting.');
    } else if (op === 'settings-core') resultMessage = await applyCoreSetting(this, native, id, value);
    else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
/** Server settings writes: keybindings, scheduled tasks, scoped and storage settings, archived threads, model and permission defaults, providers. */
export async function settingsWrites(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'keybinding-save' || op === 'keybinding-remove') await manageKeybinding.call(this, native, id, value, op);
    else if (['task-save', 'task-toggle', 'task-delete', 'task-run'].includes(op)) await manageScheduledTask.call(this, native, id, value, op);
    else if (op === 'setting-scoped') await setScopedSetting.call(this, native, id, value);
    else if (op === 'setting-storage') await setStorageSettings.call(this, native, id, value);
    else if (op === 'unarchive-thread' || op === 'delete-archived-thread') await this.manageArchivedThread(native, storage, op, id);
    else if (op === 'setting-model') await setModelDefault.call(this, native, id, value);
    else if (op === 'setting-permissions') await setPermissionDefault.call(this, native, id, value);
    else if (PROVIDER_OPS.includes(op)) { resultMessage = await runProviderOp(this, native, op, id, value); if (!this.threadId) this.chooseDefaults(); this.error = ''; }
    else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
async function manageKeybinding(this: T3Client, native: Native, environmentId: string, text: string, op: string): Promise<void> {
  if (!environmentId || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
  const input = obj(JSON.parse(text)), config = await this.request(native, 'server.getConfig', {});
  const previous = str(input.previous) ? arr(config.keybindings).find(binding => bindingId(binding) === input.previous) : undefined;
  if ((op === 'keybinding-remove' || input.previous) && !previous) throw new ClientError('That keybinding is no longer available.');
  const target = previous ? { key: shortcutInput(obj(previous.shortcut)), command: str(previous.command), ...(previous.whenAst ? { when: whenExpression(previous.whenAst) } : {}) } : {};
  let payload: Obj;
  if (op === 'keybinding-remove') payload = target;
  else {
    const key = str(input.key).trim(), command = str(input.command).trim(), when = str(input.when).trim();
    if (!key || key.length > 64 || !command || when.length > 256) throw new ClientError('Enter a command, shortcut (up to 64 characters) and valid condition (up to 256 characters).');
    if (!validShortcut(key)) throw new ClientError('Enter one key with supported shortcut modifiers.');
    if (!validWhen(when)) throw new ClientError('Enter a valid shortcut condition.');
    payload = { key, command, ...(when ? { when } : {}), ...(previous ? { replace: target } : {}) };
  }
  const result = await this.request(native, op === 'keybinding-remove' ? 'server.removeKeybinding' : 'server.upsertKeybinding', payload, this.generation, true);
  this.config = { ...this.config, keybindings: result.keybindings };
}
async function manageScheduledTask(this: T3Client, native: Native, scope: string, text: string, op: string): Promise<void> {
  const [environmentId, projectScope, extra] = scope.split(':');
  if (extra !== undefined || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
  const shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
  if (projectScope && !shell.projects.some(project => project.id === projectScope)) throw new ClientError('That checkout is no longer available.');
  const input = obj(JSON.parse(text)), tasks = await currentTasks(liveEnvironment(this, native, this.environmentId)!); // live-streams.ts: the stream's list
  const task = tasks.find(task => task.id === input.id);
  if ((op !== 'task-save' || input.id) && (!task || (projectScope && task.projectId !== projectScope))) throw new ClientError('This scheduled task no longer exists in the selected scope.');
  let payload: Obj, method: string;
  if (op === 'task-save') {
    payload = validateTaskInput(input); method = 'scheduledTasks.upsert';
    if (input.id) payload.requireExisting = true;
    if (!shell.projects.some(project => project.id === payload.projectId) || (projectScope && payload.projectId !== projectScope)) throw new ClientError('Choose an existing project in this scope.');
    if (payload.threadId && !shell.threads.some(thread => thread.id === payload.threadId && thread.projectId === payload.projectId)) throw new ClientError('Choose a thread belonging to the selected project.');
    const selection = obj(payload.modelSelection), config = await this.request(native, 'server.getConfig', {});
    const provider = arr(config.providers).find(provider => provider.instanceId === selection.instanceId && providerAvailable(provider));
    if (!provider || !arr(provider.models).some(model => model.slug === selection.model && model.isUnavailable !== true)) throw new ClientError('Choose an available provider and model.');
    // Edits retain canonical provider options when the selection is unchanged.
    const original = obj(task?.modelSelection);
    if (original.instanceId === selection.instanceId && original.model === selection.model) payload.modelSelection = original;
  } else {
    method = op === 'task-toggle' ? 'scheduledTasks.setEnabled' : op === 'task-delete' ? 'scheduledTasks.delete' : 'scheduledTasks.runNow';
    payload = { id: str(input.id), ...(op === 'task-toggle' ? { enabled: input.enabled === true } : {}) };
  }
  await this.request(native, method, payload, this.generation, true);
}
async function setScopedSetting(this: T3Client, native: Native, scope: string, text: string): Promise<void> {
  const [environmentId, projectId, extra] = scope.split(':');
  if (extra !== undefined || !environmentId || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
  const shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
  if (projectId && !shell.projects.some(project => project.id === projectId)) throw new ClientError('That checkout is no longer available.');
  const input = obj(JSON.parse(text));
  const config = await this.request(native, 'server.getConfig', {});
  const capabilities = obj(obj(config.environment).capabilities);
  if (projectId && capabilities.projectSettingsOverrides !== true) throw new ClientError('Update the selected environment to configure project overrides.');
  const settings = await this.request(native, 'server.getSettings', {});
  const root = str(input.key).split('.')[0];
  if (!(root in settings)) throw new ClientError('Update the selected environment to configure this setting.');
  if (root === 'continueThreadsAfterServerUpdate' && capabilities.threadRestartContinuation !== true) throw new ClientError('Update the selected environment to configure restart continuation.');
  if (root === 'enableAgentDeviceAccess' && settings.enableDeviceSupport !== true) throw new ClientError('Enable the device hub before changing agent device access.');
  const patch = scopedSettingPatch(settings, projectId, str(input.key), input.value, input.inherit === true);
  const updated = await this.request(native, 'server.updateSettings', { patch }, this.generation, true);
  this.config = { ...this.config, settings: updated };
}
async function setStorageSettings(this: T3Client, native: Native, scope: string, text: string): Promise<void> {
  const [environmentId, projectId, extra] = scope.split(':');
  if (extra !== undefined || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
  const shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
  if (projectId && !shell.projects.some(project => project.id === projectId)) throw new ClientError('That checkout is no longer available.');
  const capabilities = obj(obj((await this.request(native, 'server.getConfig', {})).environment).capabilities);
  if (capabilities.storageCleanup !== true || (projectId && capabilities.projectWorktreeCleanup !== true)) throw new ClientError('Update the selected environment to configure storage cleanup.');
  const input = obj(JSON.parse(text)), key = str(input.key), value = input.value;
  const booleans = ['worktreeOnDelete', 'worktreeOnMerge', 'worktreeUnchanged'];
  const retention = ['worktreeAfterDays', 'browserArtifactsAfterDays', 'logsAfterDays'];
  if (key === 'mode') {
    if (!projectId || !['inherit', 'off', 'custom'].includes(str(value))) throw new ClientError('Unsupported worktree cleanup mode.');
  } else if (booleans.includes(key)) {
    if (typeof value !== 'boolean') throw new ClientError('Choose On or Off.');
  } else if (retention.includes(key)) {
    if (value !== null && (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > 3650)) throw new ClientError('Retention must be between 1 and 3650 days.');
  } else throw new ClientError('Unsupported storage rule.');
  if (projectId && !['mode', ...booleans, 'worktreeAfterDays'].includes(key)) throw new ClientError('Artifact and log retention belongs to the environment.');
  const settings = await this.request(native, 'server.getSettings', {});
  let patch: Obj;
  if (projectId) {
    const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
    const worktree = obj(current.worktreeCleanup);
    if (key === 'mode') {
      if (value === 'inherit') delete current.worktreeCleanup;
      else current.worktreeCleanup = value === 'off' ? { mode: 'off' } : { mode: 'custom', rules: effectiveWorktreeRules(settings, current) };
    } else {
      if (worktree.mode !== 'custom') throw new ClientError('Choose Custom before changing project cleanup rules.');
      current.worktreeCleanup = { mode: 'custom', rules: { ...effectiveWorktreeRules(settings, current), [key]: value } };
    }
    patch = { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } };
  } else patch = { storageCleanup: { ...obj(settings.storageCleanup), [key]: value } };
  const result = await this.request(native, 'server.updateSettings', { patch }, this.generation, true);
  this.config = { ...this.config, settings: result }; this.error = '';
}
async function setModelDefault(this: T3Client, native: Native, scope: string, value: string): Promise<void> {
  const separator = scope.indexOf(':');
  const environmentId = scope.slice(0, separator), projectId = scope.slice(separator + 1);
  if (separator < 0 || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
  if (!['current', 'inherit', 'automatic'].includes(value) || (!projectId && value === 'inherit') || (projectId && value === 'automatic')) throw new ClientError('Unsupported model default action.');
  const shell = applyShell(initialShell(), await this.http(native, '/api/orchestration/shell'));
  if (projectId && !shell.projects.some(project => project.id === projectId)) throw new ClientError('That project is no longer available.');
  // Validate the advertised catalog again; a removed/disabled provider must not
  // become a new default from a stale chat selection.
  const config = await this.request(native, 'server.getConfig', {});
  const provider = arr(config.providers).find(provider => provider.instanceId === this.providerId && providerAvailable(provider));
  if (value === 'current' && (!provider || !arr(provider.models).some(model => model.slug === this.modelId))) throw new ClientError('The selected model is no longer available.');
  const settings = await this.request(native, 'server.getSettings', {});
  const selection = { instanceId: this.providerId, model: this.modelId, options: this.modelOptions };
  let patch: Obj = { defaultModelSelection: value === 'automatic' ? null : selection };
  if (projectId) {
    const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
    if (value === 'inherit') delete current.defaultModelSelection;
    else current.defaultModelSelection = selection;
    patch = { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } };
  }
  const updated = await this.request(native, 'server.updateSettings', { patch }, this.generation, true);
  this.config = { ...this.config, settings: updated };
  if (!this.threadId) this.chooseDefaults();
}
async function setPermissionDefault(this: T3Client, native: Native, scope: string, value: string): Promise<void> {
  const separator = scope.indexOf(':');
  const environmentId = scope.slice(0, separator), projectId = scope.slice(separator + 1);
  if (separator < 0 || environmentId !== this.environmentId) throw new ClientError('That environment is no longer selected.');
  if (!['approval-required', 'full-access', 'inherit'].includes(value)) throw new ClientError('Unsupported permissions default.');
  if (projectId && !this.shell.projects.some(project => project.id === projectId)) throw new ClientError('That project is no longer available.');
  if (!projectId && value === 'inherit') throw new ClientError('Choose an environment default.');
  const settings = obj(this.config.settings);
  let patch: Obj = { defaultRuntimeMode: value };
  if (projectId) {
    const current = { ...obj(obj(settings.projectSettingsOverrides)[projectId]) };
    if (value === 'inherit') delete current.defaultRuntimeMode;
    else current.defaultRuntimeMode = value;
    patch = { projectSettingsOverrides: { [projectId]: Object.keys(current).length ? current : null } };
  }
  const updated = await this.request(native, 'server.updateSettings', { patch }, this.generation, true);
  this.config = { ...this.config, settings: updated };
  if (!this.threadId) this.chooseDefaults();
  this.error = '';
}
