// Scheduled task writes (MIT reference, see LICENSE-T3; T3 Code 1e2ecbd975:
// components/settings/ScheduledTasksSettings.tsx ScheduledTaskRow act and
// ScheduledTaskEditorDialog submit, components/chat/ThreadAutomationsPanel.tsx toggleEnabled
// and runNow). Each write is addressed to the task's own environment (the focused transport
// or its fleet transport, live-streams.ts) and checks the task against that environment's
// live list, never a fresh `scheduledTasks.list` once the stream has delivered.
import type { T3Client } from './client';
import { arr, obj, str, type Obj } from './domain';
import { ClientError, providerAvailable, type Native } from './protocol';
import { taskInput } from './scheduled-view';
import { currentTasks, liveEnvironment, type LiveEnvironment } from './live-streams';
import { matchesScheduledTaskScope } from './scheduled-tasks';
import { pushToast } from './toast';

function environmentFor(client: T3Client, native: Native, environmentId: string): LiveEnvironment {
  const environment = liveEnvironment(client, native, environmentId);
  if (!environment) throw new ClientError('That environment is no longer selected.');
  if (!environment.connected) throw new ClientError(`Reconnect ${environment.label} to change its scheduled tasks.`);
  if (environment.focused && !client.writable) throw new ClientError('Wait for synchronization and check your connection permissions.');
  return environment;
}

/** The page's `rest:task` op: save, toggle, run and delete, scoped by the settings selection. */
export async function scheduledTaskCommand(client: T3Client, native: Native, scope: string, input: Record<string, string>): Promise<string> {
  const [, scopeProject = ''] = scope.split(':');
  const environment = environmentFor(client, native, str(input.environmentId));
  const tasks = await currentTasks(environment);
  const task = input.id ? tasks.find(entry => entry.id === input.id) : undefined;
  const scoped = (projectId: string) => !scopeProject || !environment.focused || projectId === scopeProject;
  if (input.id && (!task || !scoped(str(task.projectId)))) throw new ClientError('This scheduled task no longer exists.');
  if (input.action === 'save') {
    let payload: Obj;
    try { payload = taskInput(input, task); } catch (error) { throw new ClientError(error instanceof Error ? error.message : 'Could not save scheduled task.'); }
    if (!environment.shell.projects.some(project => project.id === payload.projectId) || !scoped(str(payload.projectId))) throw new ClientError('Scheduled task is incomplete: Add a title, prompt, project, and model.');
    const selection = obj(payload.modelSelection);
    const provider = arr(environment.config.providers).find(entry => entry.instanceId === selection.instanceId && providerAvailable(entry));
    if (!provider || !arr(provider.models).some(model => model.slug === selection.model && model.isUnavailable !== true)) throw new ClientError('Could not save scheduled task: Choose an available provider and model.');
    await environment.request('scheduledTasks.upsert', payload, true);
    return '';
  }
  if (!task) throw new ClientError('This scheduled task no longer exists.');
  if (input.action === 'toggle') await environment.request('scheduledTasks.setEnabled', { id: str(task.id), enabled: input.enabled === 'true' }, true);
  else if (input.action === 'delete') await environment.request('scheduledTasks.delete', { id: str(task.id) }, true);
  else if (input.action === 'run') await environment.request('scheduledTasks.runNow', { id: str(task.id) }, true);
  else throw new ClientError('Unsupported scheduled task action.');
  return '';
}

/** ThreadAutomationsPanel's busyTaskId: one write at a time across the section. */
const busy = new WeakMap<T3Client, string>();
export const busyAutomation = (client: T3Client) => busy.get(client) ?? '';

/**
 * `shell:automation-run` / `shell:automation-toggle` from the details panel (id: task id,
 * value: `<environmentId>` or `<environmentId>|<enabled>`). Failures are the reference's
 * stacked error toasts, so the command itself succeeds.
 */
export async function automationCommand(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  const [environmentId = '', enabled = ''] = value.split('|');
  if (busy.get(client)) return '';
  const title = op === 'run' ? 'Could not run automation' : 'Could not update automation';
  busy.set(client, id);
  try {
    const environment = environmentFor(client, native, environmentId);
    const task = (await currentTasks(environment)).find(entry => entry.id === id);
    if (!task) throw new ClientError('This automation no longer exists.');
    if (op === 'run') {
      if (str(task.lastRunStatus) === 'running') return '';
      await environment.request('scheduledTasks.runNow', { id }, true);
    } else if (op === 'toggle') {
      // Partial update: only the enabled flag changes, so a toggle never reverts edits made elsewhere.
      await environment.request('scheduledTasks.setEnabled', { id, enabled: enabled === 'true' }, true);
    } else throw new ClientError('Unsupported automation action.');
  } catch (error) {
    pushToast(client, { kind: 'error', title, description: error instanceof Error ? error.message : String(error), stacked: true });
  } finally { busy.delete(client); }
  return '';
}

/** A task's thread binding inside the settings scope: re-exported for the panel's tests. */
export const taskInScope = matchesScheduledTaskScope;
