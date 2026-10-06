// Ported from T3 Code 1e2ecbd975 apps/web/src/components/settings/scheduledTasksSettings.logic.test.ts
// (MIT, see LICENSE-T3), with the original test names. resolveSettingsScope's groups are the
// reduced ScopeGroup records; provider entries are the server's provider snapshots.
import { describe, expect, test } from 'bun:test';
import { automationLine, matchesScheduledTaskScope, relativeLabel, resolveTaskScope, scheduleLabel, scheduledTaskDefaultModel, taskStatus, taskToDraft, validateScheduledTasksSearch, type ScopeGroup, type ScopeMember } from './scheduled-tasks';
import type { Obj } from './domain';

const laptopId = 'laptop', serverId = 'server';
const environments = [{ environmentId: laptopId, label: 'Laptop' }, { environmentId: serverId, label: 'Server' }];
const member = (id: string, environmentId: string): ScopeMember => ({ id, environmentId, physicalProjectKey: `${environmentId}:/repos/${id}` });
const first = member('first', laptopId), second = member('second', laptopId), third = member('third', serverId), other = member('other', serverId);
const group = (projectKey: string, members: ScopeMember[]): ScopeGroup => ({ projectKey, memberProjects: members });
// Project IDs are environment-local. This unrelated server checkout deliberately
// shares an ID with a laptop checkout in the selected group.
const sameIdElsewhere = member('first', serverId);
const groups = [group('t3code', [first, second, third]), group('other', [other, sameIdElsewhere])];
const tasks = [first, second, third, other, sameIdElsewhere].map((project, index) => ({ id: `task-${index}`, environmentId: project.environmentId, projectId: project.id }));

describe('scheduled task settings scope', () => {
  const cases: { search: { machine?: string; project?: string; checkout?: string }; expected: string[] }[] = [
    { search: {}, expected: ['task-0', 'task-1', 'task-2', 'task-3', 'task-4'] },
    { search: { machine: laptopId }, expected: ['task-0', 'task-1'] },
    { search: { project: 't3code' }, expected: ['task-0', 'task-1', 'task-2'] },
    { search: { project: 't3code', machine: serverId }, expected: ['task-2'] },
    { search: { project: 't3code', checkout: second.physicalProjectKey }, expected: ['task-1'] },
    { search: { project: 'missing' }, expected: [] },
    { search: { machine: 'removed' }, expected: [] },
    { search: { project: 't3code', checkout: 'removed' }, expected: [] },
    { search: { project: 'other', machine: laptopId }, expected: [] },
  ];
  test.each(cases)('lists only matching tasks for $search', ({ search, expected }) => {
    const scope = resolveTaskScope(search, groups, environments);
    expect(tasks.filter(task => matchesScheduledTaskScope(scope, task.environmentId, task.projectId)).map(task => task.id)).toEqual(expected);
  });

  test('keeps tasks with removed projects manageable at environment scope', () => {
    expect(matchesScheduledTaskScope(resolveTaskScope({}, groups, environments), laptopId, 'removed')).toBe(true);
    expect(matchesScheduledTaskScope(resolveTaskScope({ project: 't3code' }, groups, environments), laptopId, 'removed')).toBe(false);
  });

  test("does not offer an unrelated environment's same-ID project when creating a task", () => {
    const scope = resolveTaskScope({ project: 't3code' }, groups, environments);
    expect([third, other, sameIdElsewhere].filter(project => matchesScheduledTaskScope(scope, serverId, project.id))).toEqual([third]);
  });
});

const legacyTask: Obj = {
  id: 'legacy-task', title: 'Review issues', prompt: 'Review open issues', enabled: true, schedule: { type: 'interval', everyMs: 60_000 },
  projectId: 'project', threadId: null, workspaceStrategy: { type: 'worktree', baseRef: 'release' }, modelSelection: { instanceId: 'codex', model: 'gpt-5.4' },
  runtimeMode: 'full-access', interactionMode: 'default', createdBy: 'user', creationSource: 'web', createdAt: '2026-09-17T00:00:00.000Z',
  updatedAt: '2026-09-17T00:00:00.000Z', nextRunAt: null, lastRunAt: null, lastRunStatus: 'never', lastRunError: null, runCount: 0,
};

describe('editing scheduled task branch settings', () => {
  test('keeps an omitted origin flag on the local base branch', () => {
    const draft = taskToDraft(legacyTask);
    expect(draft.baseRef).toBe('release');
    expect(draft.startFromOrigin).toBe(false);
  });
  test.each([true, false])('preserves an explicit origin flag of %s', startFromOrigin => {
    const draft = taskToDraft({ ...legacyTask, workspaceStrategy: { type: 'worktree', baseRef: 'release', startFromOrigin } });
    expect(draft.startFromOrigin).toBe(startFromOrigin);
  });
});

describe('scheduled task model defaults', () => {
  const instanceId = 'codex', projectId = 'project';
  const environmentSelection = { instanceId, model: 'environment-model', options: [{ id: 'reasoning', value: 'high' }] };
  const projectSelection = { instanceId, model: 'project-model' };
  const settings: Obj = { defaultModelSelection: environmentSelection };
  const providers: Obj[] = [{ instanceId, driver: 'codex', displayName: 'Codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, models: [
    { slug: 'first-model', name: 'First', isCustom: false, capabilities: null },
    { slug: 'catalog-default', name: 'Default', isDefault: true, isCustom: false, capabilities: null },
    { slug: 'environment-model', name: 'Environment', isCustom: false, capabilities: null },
    { slug: 'project-model', name: 'Project', isCustom: false, capabilities: null },
  ] }];
  test('uses the environment default with its provider options', () => {
    expect(scheduledTaskDefaultModel(settings, { id: projectId }, providers)).toEqual(environmentSelection);
  });
  test("prefers the project's configured model", () => {
    expect(scheduledTaskDefaultModel(settings, { id: projectId, defaultModelSelection: projectSelection }, providers)).toEqual(projectSelection);
    expect(scheduledTaskDefaultModel({ ...settings, projectSettingsOverrides: { [projectId]: { defaultModelSelection: projectSelection } } }, { id: projectId }, providers)).toEqual(projectSelection);
  });
  test('uses the advertised default instead of catalog order when no default is configured', () => {
    expect(scheduledTaskDefaultModel({ ...settings, defaultModelSelection: null }, null, providers)).toEqual({ instanceId, model: 'catalog-default' });
  });
  test('falls back to the environment default when the project provider is unavailable', () => {
    expect(scheduledTaskDefaultModel(settings, { id: projectId, defaultModelSelection: { instanceId: 'unavailable', model: 'missing' } }, providers)).toEqual(environmentSelection);
  });
  test('does not choose an implicit model on a disabled provider', () => {
    expect(scheduledTaskDefaultModel(settings, null, providers.map(provider => ({ ...provider, enabled: false })))).toBeNull();
  });
});

describe('labels (ScheduledTasksSettings.tsx, ThreadAutomationsPanel.tsx)', () => {
  const now = Date.parse('2026-10-06T12:00:00.000Z');
  test('schedule labels and relative run times', () => {
    expect(scheduleLabel({ type: 'interval', everyMs: 300_000 })).toBe('Every 5 min');
    expect(scheduleLabel({ type: 'interval', everyMs: 30_000 })).toBe('Every 30 sec');
    expect(scheduleLabel({ type: 'fixed_time', timeOfDay: '09:00', weekdays: [1, 2, 3, 4, 5] })).toBe('Weekdays at 09:00');
    expect(scheduleLabel({ type: 'fixed_time', timeOfDay: '09:00' })).toBe('Daily at 09:00');
    expect(relativeLabel(null, now)).toBe('Not scheduled');
    expect(relativeLabel('2026-10-06T12:00:30.000Z', now)).toBe('in under a minute');
    expect(relativeLabel('2026-10-06T12:04:30.000Z', now)).toBe('in 5m');
    expect(relativeLabel('2026-10-06T14:00:00.000Z', now)).toBe('in 2h');
    expect(relativeLabel('2026-10-06T11:58:00.000Z', now)).toBe('2m ago');
  });
  test('row status and automation line follow enabled and nextRunAt', () => {
    const task = { schedule: { type: 'interval', everyMs: 60_000 }, enabled: true, nextRunAt: '2026-10-06T12:00:40.000Z' };
    expect(taskStatus(task, now)).toBe('Every 1 min · Next run in under a minute');
    expect(automationLine(task, now)).toBe('Every 1 min · next in under a minute');
    expect(automationLine({ ...task, enabled: false }, now)).toBe('Every 1 min · paused');
    expect(automationLine({ ...task, nextRunAt: null }, now)).toBe('Every 1 min');
    expect(taskStatus({ ...task, enabled: false }, now)).toBe('Every 1 min · Paused');
  });
  test('validateScheduledTasksSearch keeps only non-blank ids', () => {
    expect(validateScheduledTasksSearch({ environmentId: ' ', taskId: 't1', other: 1 })).toEqual({ taskId: 't1' });
  });
});
