// The live scheduled-task and clone streams through the Settings page, the details panel's
// Automations section and the writes, against a recording fake of the native bridge
// (reference: ScheduledTasksSettings.tsx, ThreadAutomationsPanel.tsx, ProjectCloneToastCoordinator.tsx).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { LIVE_KEYS, liveEvent, liveFleetEvent, liveFleetPass, watchLive, PROJECT_CLONES_KEY, SCHEDULED_TASKS_KEY } from './live-streams';
import { editTarget, scheduledPage, taskSection } from './scheduled-view';
import { automationsView, threadAutomations } from './thread-automations';
import { automationCommand, scheduledTaskCommand } from './scheduled-tasks-commands';
import { activeProjectClone, cloneCommand, cloneToasts, projectCloneBlock, projectCloneNotice, startTrackedClone } from './project-clones-live';
import { toasts } from './toast';
import { EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { initialShell } from './domain';

type Call = { method: string; payload: Record<string, unknown>; write: boolean };
const NOW = Date.parse('2026-10-06T12:00:00.000Z');
const task = (over: Record<string, unknown> = {}) => ({ id: 'task-1', title: 'Nightly triage', prompt: 'Triage issues', enabled: true, schedule: { type: 'interval', everyMs: 60_000 },
  projectId: 'p1', threadId: 't1', workspaceStrategy: { type: 'root' }, modelSelection: { instanceId: 'codex', model: 'gpt' }, runtimeMode: 'full-access', interactionMode: 'default',
  nextRunAt: '2026-10-06T12:00:40.000Z', lastRunAt: null, lastRunStatus: 'never', lastRunError: null, runCount: 0, ...over });

const subs: Record<string, string> = {};
let serial = 0;
function fakeClient(tracking = true) {
  const calls: Call[] = [];
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, writable: true, revision: 0, generation: 3, busy: false,
    config: { environment: { label: 'Laptop', capabilities: { projectCloneTracking: tracking } }, providers: [{ instanceId: 'codex', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, models: [{ slug: 'gpt', name: 'GPT' }] }], settings: {} },
    local: { drafts: {} as Record<string, string> },
    shell: { ...initialShell(), projects: [{ id: 'p1', title: 'fixture', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1' }] },
    projectGroups() { return [{ key: 'g1', name: 'fixture', members: this.shell.projects }]; },
    async openProjectDraft(_native: Native, projectId: string) { calls.push({ method: 'openProjectDraft', payload: { projectId }, write: false }); this.projectId = projectId; this.threadId = ''; },
    restAccess: () => ({
      request: async (method: string, payload: Record<string, unknown>, write = false) => { calls.push({ method, payload, write }); return method === 'vcs.listRefs' ? { refs: [] } : {}; },
      call: async (request: Record<string, unknown>) => { calls.push({ method: String(request.op), payload: request, write: false }); const id = `3-${++serial}`; subs[String(request.method)] = id; return { id }; },
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `id-${index}`),
      http: async () => { calls.push({ method: 'http', payload: {}, write: false }); return { snapshotSequence: 1, projects: [], threads: [] }; },
      write: async (_storage: unknown, pending: { method: string; payload: Record<string, unknown> }) => { calls.push({ method: pending.method, payload: pending.payload, write: true }); return {}; },
    }),
  };
  return { client: client as unknown as T3Client & typeof client, calls };
}
const native = { available: true, watch: () => {}, later: async () => ({ ok: true, value: {} }) } as unknown as Native;
const subscriptionOf = (_calls: Call[], method: string) => subs[method]!;

describe('live streams', () => {
  test('one subscribe per stream per connection; the newest list replaces the last; a failure keeps it with its error', async () => {
    const { client, calls } = fakeClient();
    await watchLive(client, native); await watchLive(client, native);
    expect(calls.filter(call => call.method === 'subscribe').map(call => call.payload.method)).toEqual(['scheduledTasks.subscribe', 'subscribeProjectClones']);
    const id = subscriptionOf(calls, 'scheduledTasks.subscribe');
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: id, value: { tasks: [task()] } });
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: id, value: { tasks: [task({ lastRunStatus: 'running' })] } });
    expect((await threadAutomations(client, native, 't1', NOW)).rows[0]).toMatchObject({ status: 'running', runDisabled: true });
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: id, value: { _transportError: { message: 'socket closed' } } });
    const view = await threadAutomations(client, native, 't1', NOW);
    expect(view.error).toBe('Could not load automations: socket closed');
    expect(view.rows.length).toBe(1);
    // A retry is due: the next pass subscribes again.
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: id, value: { _retryDue: true } });
    await watchLive(client, native);
    expect(calls.filter(call => call.method === 'subscribe').length).toBe(3);
    expect(calls.some(call => call.method === 'scheduledTasks.list')).toBe(false);
  });
  test('a server without clone tracking gets no clone stream', async () => {
    const { client, calls } = fakeClient(false);
    await watchLive(client, native);
    expect(calls.filter(call => call.method === 'subscribe').map(call => call.payload.method)).toEqual(['scheduledTasks.subscribe']);
  });
  test('a background environment streams over its fleet transport', async () => {
    const entry = { key: 'o\nenv2', environmentId: 'env2', generation: 4, config: { environment: { capabilities: { projectCloneTracking: true } } } } as unknown as FleetEntry;
    const requests: Record<string, unknown>[] = [];
    await liveFleetPass(async request => { requests.push(request); return { id: `4-${requests.length}` }; }, entry);
    expect(requests.map(request => request.method)).toEqual(['scheduledTasks.subscribe', 'subscribeProjectClones']);
    expect(liveFleetEvent(entry, { key: PROJECT_CLONES_KEY, generation: 4, subscriptionId: '4-2', value: [{ projectId: 'q' }] })).toBe(true);
    expect(liveFleetEvent(entry, { key: 'shell', generation: 4, subscriptionId: '4-9', value: {} })).toBe(false);
    expect(LIVE_KEYS).toEqual(['scheduled-tasks', 'project-clones']);
    expect(EnvironmentFleet.native(native, entry.key).available).toBe(true);
  });
});

describe('Settings › Scheduled tasks', () => {
  test('loading, then rows with status and run state; a deep link to a missing task shows Task unavailable and no editor', async () => {
    const { client, calls } = fakeClient();
    let page = await scheduledPage(client, native, 'env', '', '', '', true, NOW);
    expect(page.sections.map(section => [section.state, section.title, section.heading])).toEqual([['loading', 'Loading scheduled tasks…', false]]);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionOf(calls, 'scheduledTasks.subscribe'), value: { tasks: [task({ lastRunStatus: 'failed', lastRunError: 'Provider exited' })] } });
    page = await scheduledPage(client, native, 'env', '', 'task', 'link|env|gone', true, NOW);
    expect(page.sections[0]).toMatchObject({ state: 'ready', linkMissing: true });
    expect(page.sections[0]!.tasks[0]).toMatchObject({ status: 'Every 1 min · Next run in under a minute', runStatus: 'failed', runError: 'Provider exited', environmentId: 'env' });
    expect(page.editors).toEqual([]);
    page = await scheduledPage(client, native, 'env', '', 'task', 'link|env|task-1', true, NOW);
    expect(page.editors.map(editor => [editor.id, editor.environmentId, editor.missing])).toEqual([['task-1', 'env', false]]);
    expect(calls.some(call => call.method === 'scheduledTasks.list')).toBe(false);
  });
  test('scope: an unavailable machine is an error; the project filter applies', async () => {
    const { client, calls } = fakeClient();
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionOf(calls, 'scheduledTasks.subscribe'), value: { tasks: [task(), task({ id: 'task-2', projectId: 'elsewhere' })] } });
    expect((await scheduledPage(client, native, 'env', '', '', '', true, NOW, 'removed')).error).toBe('This environment is no longer available.');
    expect((await scheduledPage(client, native, 'env', 'p1', '', '', true, NOW, '', 'g1')).sections[0]!.tasks.map(row => row.id)).toEqual(['task-1']);
    expect((await scheduledPage(client, native, 'env', '', '', '', true, NOW)).sections[0]!.tasks.map(row => row.id)).toEqual(['task-1', 'task-2']);
  });
  test('a disconnected environment and a stream error are their own states', () => {
    const base = { environmentId: 'env2', label: 'Server', focused: false, connected: false, config: {}, shell: initialShell(), key: 'k', clones: { value: null, error: '', subscribes: 0 }, request: async () => ({}), ids: async () => [] };
    const scope = { kind: 'all' as const, environmentIds: ['env2'], members: [], message: '' };
    expect(taskSection({ ...base, tasks: { value: null, error: '', subscribes: 0 } }, scope, true, '', NOW)).toMatchObject({ state: 'disconnected', title: 'Environment disconnected', description: 'Reconnect Server to view its scheduled tasks.', heading: true });
    expect(taskSection({ ...base, connected: true, tasks: { value: [], error: 'boom', subscribes: 1 } }, scope, false, '', NOW)).toMatchObject({ state: 'error', title: 'Could not load scheduled tasks', description: 'boom' });
    expect(editTarget('env2|t9', 'env')).toEqual({ environmentId: 'env2', taskId: 't9', link: false });
  });
  test('row writes check the live list and go to the task environment', async () => {
    const { client, calls } = fakeClient();
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionOf(calls, 'scheduledTasks.subscribe'), value: { tasks: [task()] } });
    await scheduledTaskCommand(client, native, 'env:', { action: 'toggle', id: 'task-1', environmentId: 'env', enabled: 'false' });
    await scheduledTaskCommand(client, native, 'env:', { action: 'run', id: 'task-1', environmentId: 'env' });
    await expect(scheduledTaskCommand(client, native, 'env:', { action: 'delete', id: 'gone', environmentId: 'env' })).rejects.toThrow('no longer exists');
    expect(calls.filter(call => call.write).map(call => [call.method, call.payload])).toEqual([['scheduledTasks.setEnabled', { id: 'task-1', enabled: false }], ['scheduledTasks.runNow', { id: 'task-1' }]]);
  });
});

describe('Thread details › Automations', () => {
  test('only for a thread with bound tasks or a load error; rows carry their accessible names', () => {
    expect(automationsView([task({ threadId: 'other' })], '', 't1', 'env', '', NOW).show).toBe(false);
    expect(automationsView(null, '', '', 'env', '', NOW).show).toBe(false);
    expect(automationsView(null, 'denied', 't1', 'env', '', NOW)).toMatchObject({ show: true, error: 'Could not load automations: denied', rows: [] });
    const view = automationsView([task(), task({ id: 'task-2', title: 'Weekly', enabled: false })], '', 't1', 'env', '', NOW);
    expect(view.rows.map(row => [row.line, row.editLabel, row.runLabel, row.switchLabel])).toEqual([
      ['Every 1 min · next in under a minute', 'Edit Nightly triage', 'Run Nightly triage now', 'Pause Nightly triage'],
      ['Every 1 min · paused', 'Edit Weekly', 'Run Weekly now', 'Resume Weekly']]);
    expect(automationsView([task()], '', 't1', 'env', 'task-1', NOW).rows[0]).toMatchObject({ runDisabled: true, switchDisabled: true });
  });
  test('Run now and the switch send runNow and setEnabled; a failure is a toast', async () => {
    const { client, calls } = fakeClient();
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionOf(calls, 'scheduledTasks.subscribe'), value: { tasks: [task()] } });
    await automationCommand(client, native, 'run', 'task-1', 'env');
    await automationCommand(client, native, 'toggle', 'task-1', 'env|false');
    await automationCommand(client, native, 'run', 'missing', 'env');
    expect(calls.filter(call => call.write).map(call => call.method)).toEqual(['scheduledTasks.runNow', 'scheduledTasks.setEnabled']);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not run automation', description: 'This automation no longer exists.' });
  });
});

describe('tracked clones on the client', () => {
  test('the viewed draft shows the banner and cannot send; other drafts get the toast', async () => {
    const { client, calls } = fakeClient();
    client.threadId = '';
    await watchLive(client, native);
    liveEvent(client, { key: PROJECT_CLONES_KEY, subscriptionId: subscriptionOf(calls, 'subscribeProjectClones'), value: [{ projectId: 'p1', destinationPath: '/work/pr-demo', repository: null, phase: 'running', stage: 'receiving', percent: 10, detail: null, error: null }] });
    expect(activeProjectClone(client)?.projectId).toBe('p1');
    expect(projectCloneBlock(client)).toBe('Cloning repository');
    expect(projectCloneNotice(client)?.title).toBe('Cloning pr-demo');
    await cloneToasts(client, native);
    expect(toasts(client).some(toast => toast.title === 'Cloning pr-demo')).toBe(false);
    client.projectId = 'p2';
    await cloneToasts(client, native);
    expect(toasts(client).find(toast => toast.title === 'Cloning pr-demo')).toMatchObject({ kind: 'loading', timeoutMs: 0, action: { label: 'Cancel', op: 'shell:clone-cancel', id: 'p1', value: 'env' } });
  });
  test('Cancel and Retry reach projectClone.*; Remove project deletes without force and leaves the draft', async () => {
    const { client, calls } = fakeClient();
    client.threadId = '';
    await cloneCommand(client, native, {} as never, 'clone-cancel', '', '');
    await cloneCommand(client, native, {} as never, 'clone-retry', 'p1', 'env');
    client.local.drafts['env:new:p1'] = 'hello';
    await cloneCommand(client, native, {} as never, 'clone-remove', 'p1', 'env');
    expect(calls.filter(call => call.write).map(call => [call.method, call.payload])).toEqual([
      ['projectClone.cancel', { projectId: 'p1' }], ['projectClone.retry', { projectId: 'p1' }],
      ['projects.mutate', { type: 'project.delete', commandId: 'id-0', projectId: 'p1' }]]);
    expect(client.local.drafts['env:new:p1']).toBeUndefined();
    expect(calls.some(call => call.method === 'openProjectDraft')).toBe(true);
  });
  test('the palette starts a tracked clone and opens its project', async () => {
    const { client, calls } = fakeClient();
    const started = await startTrackedClone(client, native, 'file:///repos/pr-demo-origin.git', '/work/pr-demo', 'pr-demo');
    const start = calls.find(call => call.method === 'projectClone.start')!;
    expect(start.payload).toMatchObject({ projectId: 'id-0', title: 'pr-demo', remoteUrl: 'file:///repos/pr-demo-origin.git', destinationPath: '/work/pr-demo' });
    expect(start.write).toBe(true);
    expect(started).toEqual({ ok: true, projectId: '', message: '' }); // the fake shell never gains the project
    expect(calls.filter(call => call.method === 'http').length).toBe(3);
  });
});
