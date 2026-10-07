// @ref llp/1107.009-mobile-settings.decision.md#root-and-native-lifetime
// @ref llp/1107.003-pairing-and-transport.decision.md#decision
// Mobile365aa87982 scheduledTaskDraft/scheduledTaskWebhook; shared drafts predate webhooks.
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError } from './shared/protocol';
export interface MobileSchedule { mode: string; timeOfDay: string; weekdays: number[]; intervalMinutes: string; signature: Obj | null; maxDeliveryAgeMinutes: string }
export interface MobileTaskDraft { task: Obj | null; title: string; prompt: string; projectId: string; modelSelection: Obj | null; modelSelectionIsExplicit: boolean;
  schedule: MobileSchedule; workspace: string; baseRef: string; checkoutPath: string; enabled: boolean; startFromOrigin: boolean; runtimeMode: string }
export const WEBHOOK_PROMPT = 'Handle this webhook:\n{{body}}';
export function mobileSchedule(task: Obj | null = null): MobileSchedule {
  const schedule = obj(task?.schedule), days = Array.isArray(schedule.weekdays) ? schedule.weekdays.map(Number) : [];
  return { mode: str(schedule.type, 'fixed_time'), timeOfDay: str(schedule.timeOfDay, '09:00'),
    weekdays: task && schedule.type === 'fixed_time' ? days.length ? [...new Set(days)].sort((a,b) => a-b) : [0,1,2,3,4,5,6] : [1,2,3,4,5],
    intervalMinutes: schedule.type === 'interval' ? String(Math.max(1, Number(schedule.everyMs) / 60000)) : '15',
    signature: schedule.type === 'webhook' && schedule.signature ? obj(schedule.signature) : null,
    maxDeliveryAgeMinutes: schedule.maxDeliveryAgeMinutes == null ? '' : String(schedule.maxDeliveryAgeMinutes) };
}
export function mobileTaskDraft(projectId: string, modelSelection: Obj | null, task: Obj | null = null): MobileTaskDraft {
  const workspace = obj(task?.workspaceStrategy);
  return { task, title: str(task?.title), prompt: str(task?.prompt), projectId: task ? str(task.projectId) : projectId,
    modelSelection: task ? obj(task.modelSelection) : modelSelection, modelSelectionIsExplicit: task !== null,
    schedule: mobileSchedule(task), workspace: str(workspace.type, 'worktree'), baseRef: str(workspace.baseRef, 'main'), checkoutPath: str(workspace.worktreePath),
    enabled: task ? task.enabled === true : true, startFromOrigin: task && workspace.type === 'worktree' ? workspace.startFromOrigin === true : true,
    runtimeMode: str(task?.runtimeMode, 'full-access') };
}
export function mobileScheduleInput(draft: MobileSchedule): Obj {
  if (draft.mode === 'webhook') {
    const text = draft.maxDeliveryAgeMinutes.trim(), minutes = Number(text);
    if (text && (!Number.isInteger(minutes) || minutes < 1 || minutes > 1440)) throw new ClientError('Enter whole minutes from 1 to 1440, or leave it blank.');
    const signature = draft.signature;
    return { type: 'webhook', signature: signature === null ? null : { header: signature.header, encoding: signature.encoding, prefix: signature.prefix }, maxDeliveryAgeMinutes: text ? minutes : null };
  }
  if (draft.mode === 'interval') {
    const minutes = Number(draft.intervalMinutes), everyMs = Math.round(minutes * 60000);
    if (!(minutes >= 1) || !Number.isSafeInteger(everyMs)) throw new ClientError('Intervals must be at least 1 minute. Update this interval before saving.');
    return { type: 'interval', everyMs };
  }
  const weekdays = [...new Set(draft.weekdays)].sort((a,b) => a-b);
  if (draft.mode !== 'fixed_time' || !/^([01]?\d|2[0-3]):[0-5]\d$/.test(draft.timeOfDay) || !weekdays.length || weekdays.some(day => !Number.isInteger(day) || day < 0 || day > 6)) throw new ClientError('Choose a valid time and at least one weekday.');
  return { type: 'fixed_time', timeOfDay: draft.timeOfDay, ...(weekdays.length === 7 ? {} : { weekdays }) };
}
export function mobileTaskInput(draft: MobileTaskDraft, liveTask: Obj | null): Obj {
  if (draft.task && !liveTask) throw new ClientError('This task no longer exists.');
  const liveSchedule = obj(liveTask?.schedule);
  const schedule = mobileScheduleInput(draft.schedule.mode === 'webhook' && liveSchedule.type === 'webhook' ? { ...draft.schedule, signature: liveSchedule.signature ? obj(liveSchedule.signature) : null } : draft.schedule);
  if (!draft.title.trim() || !draft.prompt.trim() || !draft.projectId || !draft.modelSelection || !str(draft.modelSelection.instanceId) || !str(draft.modelSelection.model)
    || draft.workspace === 'existing_worktree' && !draft.checkoutPath.trim()) throw new ClientError('Add a name, prompt, project, model, valid schedule, and checkout path if needed.');
  if (!['root','worktree','existing_worktree'].includes(draft.workspace) || !['approval-required','auto-accept-edits','auto','full-access'].includes(draft.runtimeMode)) throw new ClientError('Choose a supported workspace and permissions.');
  return { ...(draft.task ? { id: str(draft.task.id), requireExisting: true } : {}), title: draft.title.trim(), prompt: draft.prompt.trim(), projectId: draft.projectId,
    modelSelection: draft.modelSelection, schedule, enabled: draft.enabled, threadId: draft.task?.threadId ?? null,
    workspaceStrategy: draft.workspace === 'root' ? { type: 'root' } : draft.workspace === 'existing_worktree' ? { type: 'existing_worktree', worktreePath: draft.checkoutPath.trim() }
      : { type: 'worktree', baseRef: draft.baseRef.trim() || 'main', startFromOrigin: draft.startFromOrigin },
    runtimeMode: draft.runtimeMode, interactionMode: draft.task?.interactionMode ?? 'default', creationSource: draft.task?.creationSource ?? 'mobile' };
}
export function mobileTaskSignature(draft: MobileTaskDraft): string {
  return JSON.stringify([draft.title, draft.prompt, draft.projectId, draft.modelSelection?.instanceId, draft.modelSelection?.model,
    arr(draft.modelSelection?.options).sort((a,b) => str(a.id).localeCompare(str(b.id))).map(option => [option.id,option.value]), draft.schedule.mode, draft.schedule.timeOfDay,
    [...draft.schedule.weekdays].sort((a,b) => a-b), draft.schedule.intervalMinutes, draft.schedule.maxDeliveryAgeMinutes, draft.workspace, draft.baseRef, draft.checkoutPath, draft.enabled, draft.startFromOrigin, draft.runtimeMode]);
}
export function mobileWebhookAddress(endpoint: Obj, origin: string | null) {
  if (typeof endpoint.url === 'string' && endpoint.url) return { address: endpoint.url, copyable: true, note: '' };
  if (!origin) return { address: str(endpoint.path), copyable: false, note: 'Link this environment to T3 Connect for a public URL.' };
  const url = new URL(str(endpoint.path), origin), loopback = /^(localhost|127(?:\.\d{1,3}){3}|\[?::1\]?)$/i.test(url.hostname);
  return { address: url.href, copyable: true, note: loopback ? 'Only this computer can call this address. Link T3 Connect for a public URL.'
    : "Works wherever this environment's address is reachable, for example over Tailscale or your own proxy. Link T3 Connect for a public URL." };
}
