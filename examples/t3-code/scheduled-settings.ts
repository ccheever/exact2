import { obj, arr, str, num, type Obj } from './domain';
import type { T3Client } from './client';
import { providerAvailable, type Native } from './protocol';
export const taskDraft = (task: Obj = {}, client?: T3Client) => {
  const schedule = obj(task.schedule), workspace = obj(task.workspaceStrategy), model = obj(task.modelSelection);
  const weekdays = Array.isArray(schedule.weekdays) && schedule.weekdays.length ? schedule.weekdays : task.id ? [0, 1, 2, 3, 4, 5, 6] : [1, 2, 3, 4, 5];
  return { id: str(task.id), title: str(task.title), prompt: str(task.prompt), enabled: task.enabled !== false,
    scheduleMode: schedule.type === 'interval' ? 'interval' : 'fixed_time', minutes: String(Math.max(1, num(schedule.everyMs, 900000) / 60000)), time: str(schedule.timeOfDay, '09:00'),
    sunday: weekdays.includes(0), monday: weekdays.includes(1), tuesday: weekdays.includes(2), wednesday: weekdays.includes(3), thursday: weekdays.includes(4), friday: weekdays.includes(5), saturday: weekdays.includes(6),
    projectId: str(task.projectId, client?.projectId || ''), threadId: str(task.threadId), workspaceMode: str(workspace.type, 'worktree'), baseRef: str(workspace.baseRef, 'main'), startFromOrigin: task.id && workspace.type === 'worktree' ? workspace.startFromOrigin === true : true, path: str(workspace.worktreePath),
    instanceId: str(model.instanceId, client?.providerId || ''), model: str(model.model, client?.modelId || ''), runtimeMode: str(task.runtimeMode, 'full-access'), interactionMode: str(task.interactionMode, 'default') };
};
export function validateTaskInput(input: Obj): Obj {
  if (!str(input.title).trim() || !str(input.prompt).trim()) throw new Error('Enter a task name and prompt.');
  const schedule = obj(input.schedule), workspace = obj(input.workspaceStrategy);
  if (schedule.type === 'interval') {
    if (!Number.isInteger(schedule.everyMs) || num(schedule.everyMs) < 60000) throw new Error('Use an interval of at least one minute.');
  } else if (schedule.type === 'fixed_time') {
    if (!/^([01]?\d|2[0-3]):[0-5]\d$/.test(str(schedule.timeOfDay)) || (schedule.weekdays !== undefined && (!Array.isArray(schedule.weekdays) || schedule.weekdays.some(day => !Number.isInteger(day) || Number(day) < 0 || Number(day) > 6)))) throw new Error('Choose a valid time and at least one weekday.');
  } else throw new Error('Choose a supported schedule.');
  if (!['root', 'worktree', 'existing_worktree'].includes(str(workspace.type))) throw new Error('Choose a supported workspace.');
  if (workspace.type === 'worktree' && !str(workspace.baseRef).trim()) throw new Error('Enter the worktree base branch.');
  if (workspace.type === 'existing_worktree' && !str(workspace.worktreePath).trim()) throw new Error('Enter an existing checkout path.');
  if (!['approval-required', 'full-access'].includes(str(input.runtimeMode)) || !['default', 'plan'].includes(str(input.interactionMode)) || typeof input.enabled !== 'boolean') throw new Error('Choose supported permissions and mode.');
  return { ...input, title: str(input.title).trim(), prompt: str(input.prompt).trim(), creationSource: 'web' };
}
export async function scheduledSettings(client: T3Client, native: Native | null | undefined, environmentId: string, projectId: string, editingId: string, active: boolean) {
  const empty = { available: false, writable: false, error: '', scope: `${environmentId}:${projectId}`, draft: taskDraft(), tasks: [] as { id: string; title: string; project: string; summary: string; enabled: boolean; status: string; error: string }[], projects: [] as { id: string; title: string }[], models: [] as { id: string; instanceId: string; model: string; label: string }[] };
  if (!active) return empty;
  try {
    if (!native?.available || !client.ready || environmentId !== client.environmentId || (projectId && !client.shell.projects.some(project => project.id === projectId))) throw new Error('This scope is unavailable. Choose a connected environment and an existing checkout.');
    const value = await client.readSettings(native, 'scheduledTasks.list');
    const all = arr(value.tasks), task = editingId ? all.find(task => task.id === editingId) : undefined;
    if (editingId && (!task || (projectId && task.projectId !== projectId))) throw new Error('This scheduled task no longer exists in the selected scope.');
    const settings = obj(client.config.settings), override = obj(obj(settings.projectSettingsOverrides)[projectId || client.projectId]);
    const providers = arr(client.config.providers).filter(providerAvailable);
    const choices = providers.flatMap(provider => arr(provider.models).filter(model => model.isUnavailable !== true && model.isLegacy !== true).map(model => ({ provider, model })));
    const configured = [override.defaultModelSelection, settings.defaultModelSelection].map(obj).find(selection => choices.some(choice => choice.provider.instanceId === selection.instanceId && choice.model.slug === selection.model));
    const fallback = choices.find(choice => choice.model.isDefault === true) || choices[0];
    const initial = task || { projectId: projectId || client.projectId, modelSelection: configured || (fallback ? { instanceId: str(fallback.provider.instanceId), model: str(fallback.model.slug) } : {}), runtimeMode: 'full-access' };
    return { ...empty, available: true, writable: client.writable, draft: taskDraft(initial, client), projects: client.shell.projects.filter(project => !projectId || project.id === projectId).map(project => ({ id: str(project.id), title: str(project.title) })),
      models: arr(client.config.providers).filter(provider => providerAvailable(provider) && obj(provider.auth).status !== 'unauthenticated').flatMap(provider => arr(provider.models).filter(model => model.isUnavailable !== true && model.isLegacy !== true).map(model => ({ id: `${provider.instanceId}:${model.slug}`, instanceId: str(provider.instanceId), model: str(model.slug), label: `${provider.displayName || provider.instanceId} / ${model.name || model.slug}` }))),
      tasks: all.filter(task => !projectId || task.projectId === projectId).map(task => { const schedule = obj(task.schedule); return { id: str(task.id), title: str(task.title), project: str(client.shell.projects.find(project => project.id === task.projectId)?.title, 'Unavailable project'), summary: schedule.type === 'interval' ? `Every ${num(schedule.everyMs) / 60000} minutes` : `${schedule.timeOfDay} · ${Array.isArray(schedule.weekdays) ? schedule.weekdays.join(', ') : 'Every day'}`, enabled: task.enabled === true, status: `${task.lastRunStatus} · ${num(task.runCount)} runs`, error: str(task.lastRunError) }; }) };
  } catch (error) { return { ...empty, error: error instanceof Error ? error.message : 'Could not load scheduled tasks.' }; }
}
export function taskFromArguments(args: unknown[]): Obj {
  const [scope, id, title, prompt, enabled, scheduleMode, minutes, time, sunday, monday, tuesday, wednesday, thursday, friday, saturday, projectId, threadId, workspaceMode, baseRef, startFromOrigin, path, instanceId, model, runtimeMode, interactionMode] = args;
  void scope;
  return { ...(id ? { id: String(id), requireExisting: true } : {}), title: String(title), prompt: String(prompt), enabled: enabled === true,
    schedule: scheduleMode === 'interval' ? { type: 'interval', everyMs: Math.round(Number(minutes) * 60000) } : { type: 'fixed_time', timeOfDay: String(time) || '09:00', ...([sunday, monday, tuesday, wednesday, thursday, friday, saturday].filter(Boolean).length % 7 ? { weekdays: [sunday, monday, tuesday, wednesday, thursday, friday, saturday].flatMap((on, day) => on ? [day] : []) } : {}) },
    projectId: String(projectId), threadId: threadId ? String(threadId) : null,
    workspaceStrategy: workspaceMode === 'root' ? { type: 'root' } : workspaceMode === 'existing_worktree' ? { type: 'existing_worktree', worktreePath: String(path).trim() } : { type: 'worktree', baseRef: String(baseRef).trim() || 'main', startFromOrigin: startFromOrigin === true },
    modelSelection: { instanceId: String(instanceId), model: String(model) }, runtimeMode: String(runtimeMode), interactionMode: String(interactionMode) };
}
