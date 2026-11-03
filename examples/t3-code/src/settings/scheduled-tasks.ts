// Scheduled task logic (MIT reference, see LICENSE-T3; T3 Code 9c161462c9, defaultScheduledTaskProjectId at 2359643cff,
// the one-time schedule at 5d3d78ab93: apps/web/src/components/settings/scheduledTasksSettings.logic.ts, and scheduleLabel /
// relativeLabel / statusVariant from ScheduledTasksSettings.tsx). Changes from the
// reference: `now` is an argument (data sources have no clock,
// issues/closed/20261009-typescript-failure-parity.md); the draft's weekdays
// are seven booleans, the editor's Contract fields; provider entries are the server's
// provider snapshots (`deriveProviderInstanceEntries` read inline); a scope is the plain
// record below instead of ResolvedSettingsScope.
import { arr, num, obj, str, type Obj } from '../client/domain';
import { webhookDraft } from './scheduled-webhook';
import { relativeTimeLabel } from './settings-data';
import { RUNTIME_LOCALE } from '../shared/timestamp-format';

/** ResolvedSettingsScope, reduced to what matchesScheduledTaskScope reads. */
export type TaskScope = {
  kind: 'all' | 'environment' | 'project' | 'checkout' | 'unavailable';
  environmentIds: string[];
  members: { environmentId: string; id: string }[];
  message: string;
};

export type ScopeMember = { environmentId: string; id: string; physicalProjectKey: string };
export type ScopeGroup = { projectKey: string; memberProjects: ScopeMember[] };
/** settingsScope.ts resolveSettingsScope (labels left out): a removed target never broadens to All. */
export function resolveTaskScope(search: { machine?: string; project?: string; checkout?: string }, groups: ScopeGroup[], environments: { environmentId: string }[]): TaskScope {
  const unavailable = (message: string): TaskScope => ({ kind: 'unavailable', environmentIds: [], members: [], message });
  if (search.checkout && !search.project) return unavailable('Select a project to choose one of its checkouts.');
  const environment = environments.find(candidate => candidate.environmentId === search.machine);
  if (search.machine && !environment) return unavailable('This environment is no longer available.');
  if (search.project) {
    const group = groups.find(candidate => candidate.projectKey === search.project);
    if (!group) return unavailable('This project is no longer available.');
    const members = group.memberProjects.filter(member => (!search.machine || member.environmentId === search.machine) && (!search.checkout || member.physicalProjectKey === search.checkout));
    if (!members.length) return unavailable(search.checkout ? 'This checkout is no longer available in the selected project and environment.' : 'This project has no checkout on this environment.');
    if (search.checkout) {
      const checkout = members[0]!;
      if (!environments.some(candidate => candidate.environmentId === checkout.environmentId)) return unavailable("This checkout's environment is no longer available.");
      return { kind: 'checkout', environmentIds: [checkout.environmentId], members, message: '' };
    }
    return { kind: 'project', environmentIds: [...new Set(members.map(member => member.environmentId))], members, message: '' };
  }
  if (environment) return { kind: 'environment', environmentIds: [environment.environmentId], members: [], message: '' };
  return { kind: 'all', environmentIds: environments.map(candidate => candidate.environmentId), members: [], message: '' };
}

/**
 * A new task's project: a pinned open project, then any open one. An archived project only when Settings is scoped
 * to it, so that scope can still save.
 */
export function defaultScheduledTaskProjectId(scopedProjects: readonly Obj[], scope: TaskScope): string {
  const open = scopedProjects.filter(project => project.archivedAt == null);
  const scopedToProject = scope.kind === 'project' || scope.kind === 'checkout';
  return str((open.find(project => project.pinnedAt != null) ?? open[0] ?? (scopedToProject ? scopedProjects[0] : undefined))?.id);
}

/** Project IDs belong to an environment, including when a grouped project spans machines. */
export function matchesScheduledTaskScope(scope: TaskScope, environmentId: string, projectId: string): boolean {
  if (scope.kind === 'unavailable' || !scope.environmentIds.includes(environmentId)) return false;
  if (scope.kind === 'project' || scope.kind === 'checkout') return scope.members.some(member => member.environmentId === environmentId && member.id === projectId);
  return true;
}

export function validateScheduledTasksSearch(raw: Record<string, unknown>): { environmentId?: string; taskId?: string } {
  return {
    ...(typeof raw.environmentId === 'string' && raw.environmentId.trim() ? { environmentId: raw.environmentId } : {}),
    ...(typeof raw.taskId === 'string' && raw.taskId.trim() ? { taskId: raw.taskId } : {}),
  };
}

const WEEKDAY_LABELS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const DAY_FIELDS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday'] as const;
export type Weekdays = Record<(typeof DAY_FIELDS)[number], boolean>;
const weekdayFields = (days: number[]): Weekdays => Object.fromEntries(DAY_FIELDS.map((day, index) => [day, days.includes(index)])) as Weekdays;

export type TaskDraft = Weekdays & ReturnType<typeof webhookDraft> & {
  editingId: string; title: string; prompt: string; enabled: boolean; scheduleMode: 'fixed' | 'once' | 'interval' | 'webhook'; intervalMinutes: string; timeOfDay: string;
  /** Local date and time as a `datetime-local` input value, such as 2026-11-02T15:00. */
  runAt: string;
  /** The stored ISO run time, sent back unchanged when the field was not edited. */
  savedRunAt: string;
  projectId: string; threadId: string; workspaceMode: string; baseRef: string; startFromOrigin: boolean; existingWorktreePath: string; modelKey: string;
  runtimeMode: string; interactionMode: string; baseModelSelection: Obj | null;
};

/** EMPTY_DRAFT, with the first project of the scope. */
export function emptyDraft(projectId: string, modelKey = ''): TaskDraft {
  return { ...webhookDraft(), editingId: '', title: '', prompt: '', enabled: true, scheduleMode: 'fixed', intervalMinutes: '15', timeOfDay: '09:00', runAt: '', savedRunAt: '', ...weekdayFields([1, 2, 3, 4, 5]),
    projectId, threadId: '', workspaceMode: 'worktree', baseRef: 'main', startFromOrigin: true, existingWorktreePath: '', modelKey,
    runtimeMode: 'full-access', interactionMode: 'default', baseModelSelection: null };
}

export function taskToDraft(task: Obj): TaskDraft {
  const schedule = obj(task.schedule), workspace = obj(task.workspaceStrategy), selection = obj(task.modelSelection);
  const weekdays = schedule.type === 'fixed_time' && Array.isArray(schedule.weekdays) && schedule.weekdays.length > 0 ? schedule.weekdays.map(Number) : [0, 1, 2, 3, 4, 5, 6];
  return {
    ...webhookDraft(task), editingId: str(task.id), title: str(task.title), prompt: str(task.prompt), enabled: task.enabled === true,
    scheduleMode: schedule.type === 'interval' || schedule.type === 'once' || schedule.type === 'webhook' ? schedule.type : 'fixed',
    intervalMinutes: schedule.type === 'interval' ? String(Math.max(1, num(schedule.everyMs) / 60000)) : '15',
    timeOfDay: schedule.type === 'fixed_time' ? str(schedule.timeOfDay) : '09:00',
    runAt: schedule.type === 'once' ? onceInputValue(str(schedule.runAt)) : '', savedRunAt: schedule.type === 'once' ? str(schedule.runAt) : '', ...weekdayFields(weekdays),
    projectId: str(task.projectId), threadId: str(task.threadId), workspaceMode: str(workspace.type, 'worktree'),
    baseRef: workspace.type === 'worktree' ? str(workspace.baseRef) : 'main',
    startFromOrigin: workspace.type === 'worktree' ? workspace.startFromOrigin === true : true,
    existingWorktreePath: workspace.type === 'existing_worktree' ? str(workspace.worktreePath) : '',
    modelKey: `${str(selection.instanceId)}:${str(selection.model)}`,
    runtimeMode: str(task.runtimeMode), interactionMode: str(task.interactionMode), baseModelSelection: task.modelSelection ? selection : null,
  };
}

/** deriveProviderInstanceEntries' enabled / installed / isAvailable, read from a provider snapshot. */
const entryAvailable = (provider: Obj) => provider.enabled === true && provider.installed === true && provider.availability !== 'unavailable';

/** resolveProjectSettings(...).settings.defaultModelSelection: the override, the project's own field, the environment. */
function configuredSelection(settings: Obj, project: Obj | null): Obj | null {
  const override = project ? obj(obj(settings.projectSettingsOverrides)[str(project.id)]) : {};
  const value = override.defaultModelSelection ?? project?.defaultModelSelection ?? settings.defaultModelSelection;
  return value && typeof value === 'object' ? obj(value) : null;
}

/** Use configured defaults before the catalog's advertised default model. */
export function scheduledTaskDefaultModel(settings: Obj, project: Obj | null, providers: Obj[]): Obj | null {
  const available = providers.filter(provider => entryAvailable(provider) && obj(provider.auth).status !== 'unauthenticated');
  for (const selection of [configuredSelection(settings, project), settings.defaultModelSelection ? obj(settings.defaultModelSelection) : null]) {
    if (selection && available.some(entry => entry.instanceId === selection.instanceId
      && arr(entry.models).find(model => model.slug === selection.model)?.isLegacy !== true)) return selection;
  }
  const models = available.flatMap(entry => arr(entry.models).filter(model => model.isLegacy !== true).map(model => ({ instanceId: str(entry.instanceId), model })));
  const fallback = models.find(entry => entry.model.isDefault === true) ?? models[0];
  return fallback ? { instanceId: fallback.instanceId, model: str(fallback.model.slug) } : null;
}

/** Formats a date as a `datetime-local` input value in the local time zone. */
export function toDateTimeLocalValue(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

const onceInputValue = (runAt: string) => { const date = new Date(runAt); return Number.isNaN(date.getTime()) ? '' : toDateTimeLocalValue(date); };

/** Older servers reject one-time schedules, so Once is offered only where it can save, or for a task that has it. */
export function offersOnceSchedules(config: Obj, task: Obj | undefined): boolean {
  return obj(obj(config.environment).capabilities).oneTimeSchedules === true || obj(task?.schedule).type === 'once';
}

/**
 * Picking Once with no run time suggests the next whole hour, keeping its instant too, so an hour that repeats when
 * clocks go back stays exact. `now` is an epoch, or null when the time is unknown (no suggestion).
 */
export function suggestedRunAt(now: number | null): { runAt: string; savedRunAt: string } {
  if (now === null) return { runAt: '', savedRunAt: '' };
  const suggested = new Date(Math.ceil(now / 3_600_000) * 3_600_000);
  return { runAt: toDateTimeLocalValue(suggested), savedRunAt: suggested.toISOString() };
}

/** scheduleDraftErrors' one-time part. */
export function onceDraftErrors(runAt: string): { title: string; description: string }[] {
  const date = new Date(runAt);
  if (Number.isNaN(date.getTime())) return [{ title: 'Invalid run time', description: 'Pick a date and time.' }];
  // A time skipped by a daylight saving change parses to a different clock time.
  if (toDateTimeLocalValue(date) !== runAt) return [{ title: 'Invalid run time', description: 'That time does not exist in your time zone.' }];
  return [];
}

/** scheduleFromDraft's one-time part, or null when the run time is invalid. */
export function onceSchedule(runAt: string, savedRunAt: string): { type: 'once'; runAt: string } | null {
  if (onceDraftErrors(runAt).length > 0) return null;
  const saved = savedRunAt ? new Date(savedRunAt) : null;
  if (saved !== null && toDateTimeLocalValue(saved) === runAt) return { type: 'once', runAt: savedRunAt };
  // A `datetime-local` value has no offset. In an hour that repeats when clocks go
  // back, keep the saved time's offset; otherwise Date reads it as local time.
  const withSavedOffset = saved === null ? null : new Date(Date.parse(`${runAt}Z`) + saved.getTimezoneOffset() * 60_000);
  return { type: 'once', runAt: withSavedOffset !== null && toDateTimeLocalValue(withSavedOffset) === runAt ? withSavedOffset.toISOString() : new Date(runAt).toISOString() };
}

const ONCE_FORMATTER = new Intl.DateTimeFormat(RUNTIME_LOCALE, { dateStyle: 'medium', timeStyle: 'short' });

export function scheduleLabel(schedule: Obj): string {
  if (schedule.type === 'webhook') return 'On webhook';
  if (schedule.type === 'once') { const date = new Date(str(schedule.runAt)); return Number.isNaN(date.getTime()) ? 'Once' : `Once, ${ONCE_FORMATTER.format(date)}`; }
  if (schedule.type === 'interval') {
    const minutes = num(schedule.everyMs) / 60000;
    return Number.isInteger(minutes) ? `Every ${minutes} min` : `Every ${Math.round(num(schedule.everyMs) / 1000)} sec`;
  }
  const weekdays = Array.isArray(schedule.weekdays) ? schedule.weekdays.map(Number) : [];
  const days = weekdays.length === 0 ? 'Daily' : weekdays.length === 5 && weekdays.every(day => day >= 1 && day <= 5) ? 'Weekdays' : weekdays.map(day => WEEKDAY_LABELS[day]).join(', ');
  return `${days} at ${str(schedule.timeOfDay)}`;
}

/** Human label for a run timestamp: future instants read "in 5m", past ones as formatRelativeTime. */
export function relativeLabel(value: unknown, now: number): string {
  if (!value) return 'Not scheduled';
  const diff = Date.parse(str(value)) - now;
  if (!Number.isFinite(diff)) return 'Not scheduled';
  if (diff <= 0) return relativeTimeLabel(str(value), now) || 'Not scheduled';
  const minutes = Math.ceil(diff / 60000);
  if (minutes < 2) return 'in under a minute';
  if (minutes < 60) return `in ${minutes}m`;
  const hours = Math.round(minutes / 60);
  return hours < 24 ? `in ${hours}h` : `in ${Math.round(hours / 24)}d`;
}

/** The settings row's status line: "Every 5 min · Next run in 4m", "… · Paused", or why a managed policy blocks the task. */
export function taskStatus(task: Obj, now: number): string {
  if (typeof task.blockedByPolicy === 'string') return `${scheduleLabel(obj(task.schedule))} · ${task.blockedByPolicy}`;
  return `${scheduleLabel(obj(task.schedule))} · ${task.enabled === true ? (obj(task.schedule).type === 'webhook' ? 'Listening' : task.nextRunAt ? `Next run ${relativeLabel(task.nextRunAt, now)}` : 'Not scheduled') : 'Paused'}`;
}

/** ThreadAutomationsPanel's line: the schedule, then " · next …" or " · paused". */
export function automationLine(task: Obj, now: number): string {
  const enabled = task.enabled === true;
  return `${scheduleLabel(obj(task.schedule))}${enabled && task.nextRunAt ? ` · next ${relativeLabel(task.nextRunAt, now)}` : enabled ? '' : ' · paused'}`;
}

/** STATUS_DOT_CLASS: never, running (pulses), succeeded, failed. */
export const RUN_STATUSES = ['never', 'running', 'succeeded', 'failed'];
export const runStatus = (task: Obj) => RUN_STATUSES.includes(str(task.lastRunStatus)) ? str(task.lastRunStatus) : 'never';
