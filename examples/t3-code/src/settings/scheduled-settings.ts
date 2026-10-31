import { obj, str, num, type Obj } from '../client/domain';
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
export function taskFromArguments(args: unknown[]): Obj {
  const [scope, id, title, prompt, enabled, scheduleMode, minutes, time, sunday, monday, tuesday, wednesday, thursday, friday, saturday, projectId, threadId, workspaceMode, baseRef, startFromOrigin, path, instanceId, model, runtimeMode, interactionMode] = args;
  void scope;
  return { ...(id ? { id: String(id), requireExisting: true } : {}), title: String(title), prompt: String(prompt), enabled: enabled === true,
    schedule: scheduleMode === 'interval' ? { type: 'interval', everyMs: Math.round(Number(minutes) * 60000) } : { type: 'fixed_time', timeOfDay: String(time) || '09:00', ...([sunday, monday, tuesday, wednesday, thursday, friday, saturday].filter(Boolean).length % 7 ? { weekdays: [sunday, monday, tuesday, wednesday, thursday, friday, saturday].flatMap((on, day) => on ? [day] : []) } : {}) },
    projectId: String(projectId), threadId: threadId ? String(threadId) : null,
    workspaceStrategy: workspaceMode === 'root' ? { type: 'root' } : workspaceMode === 'existing_worktree' ? { type: 'existing_worktree', worktreePath: String(path).trim() } : { type: 'worktree', baseRef: String(baseRef).trim() || 'main', startFromOrigin: startFromOrigin === true },
    modelSelection: { instanceId: String(instanceId), model: String(model) }, runtimeMode: String(runtimeMode), interactionMode: String(interactionMode) };
}
