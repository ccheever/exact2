// Scheduled tasks against T3 Code 1e2ecbd975 (MIT, see
// LICENSE-T3): ScheduledTaskEditorDialog's "Runs on" switches a new task's environment (the scope's connected
// environments; project, model, base branch, Start from remote and checkout path start over, the rest is kept);
// ProjectCloneToastCoordinator's "Open project" on every environment's success toast (handleNewThread with that
// environment's project ref); ThreadAutomationsPanel's three TooltipPopups.
import { afterEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { OpOut } from '../../src/client/client-ops';
import type { Files, Native } from '../../src/client/protocol';
import { initialShell, obj, type Obj } from '../../src/client/domain';
import { liveEvent, liveFleetEvent, liveFleetPass, watchLive, PROJECT_CLONES_KEY, SCHEDULED_TASKS_KEY } from '../../src/client/live-streams';
import { editTarget, scheduledPage } from '../../src/settings/scheduled-view';
import { cloneToasts } from '../../src/shell/project-clones-live';
import { cloneToast } from '../../src/shell/project-clones';
import { toasts } from '../../src/shell/toast';
import { fleet, focusFleetProject, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { threadOps } from '../../src/client/client-ops-threads';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';

const NOW = Date.parse('2026-10-11T12:00:00.000Z');

type Call = { method: string; payload: Obj; fleet: string };
const provider = (instanceId: string, slug: string) => ({ instanceId, driver: instanceId, enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, models: [{ slug, name: slug.toUpperCase(), isDefault: true }] });
let serial = 0;
/** The focused environment "Laptop" (env, project p1 at /repo) and the native bridge every environment answers through. */
function harness() {
  const calls: Call[] = [];
  const subscriptions: Record<string, string> = {};
  const client = {
    environmentId: 'env', threadId: '', projectId: 'p1', ready: true, writable: true, revision: 0, generation: 3, busy: false,
    config: { environment: { label: 'Laptop', capabilities: { projectCloneTracking: true } }, providers: [provider('codex', 'gpt')], settings: {} },
    local: { drafts: {} as Record<string, string>, selections: {} as Record<string, { projectId: string; threadId: string }> },
    shell: { ...initialShell(), projects: [{ id: 'p1', title: 'fixture', workspaceRoot: '/repo' }], threads: [] },
    projectGroups() { return [{ key: 'g1', name: 'fixture', members: this.shell.projects }]; },
    restAccess: () => ({
      request: async (method: string, payload: Obj) => { calls.push({ method, payload, fleet: '' }); return method === 'vcs.listRefs' ? { refs: [{ name: 'main', isRemote: false }] } : {}; },
      read: async (method: string, payload: Obj) => { calls.push({ method, payload, fleet: '' }); return method === 'vcs.listRefs' ? { refs: [{ name: 'main', isRemote: false }] } : {}; },
      call: async (request: Obj) => { const id = `3-${++serial}`; subscriptions[String(request.method)] = id; return { id }; },
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `id-${index}`),
    }),
  };
  const native = {
    available: true, watch: () => {},
    later: async (input: unknown) => {
      const request = obj(input), key = String(request.fleet ?? '');
      calls.push({ method: String(request.method ?? request.op), payload: request, fleet: key });
      if (request.op === 'request' && request.method === 'vcs.listRefs') return { ok: true, generation: 5, value: { refs: [{ name: 'develop', isRemote: false }, { name: 'origin/develop', isRemote: true }] } };
      if (request.op === 'connect') return { ok: true, generation: 4, value: { state: 'connecting', origin: request.origin, environmentId: 'env2', message: '' } };
      return { ok: true, generation: 5, value: {} };
    },
  } as unknown as Native;
  return { client: client as unknown as T3Client & typeof client, native, calls, subscriptions };
}

/** A synchronized background environment "Studio" (env2, project q1 at /studio) in the window's fleet. */
async function studio(): Promise<FleetEntry> {
  const entry: FleetEntry = {
    key: 'http://127.0.0.1:16423\nenv2', origin: 'http://127.0.0.1:16423', environmentId: 'env2', phase: 'connected', message: '', traceId: '',
    generation: 5, synchronized: 5, lastEvent: 0, subscriptions: {}, scopes: [], error: '', requested: false,
    config: { environment: { label: 'Studio', capabilities: { projectCloneTracking: true } }, providers: [provider('claudeAgent', 'sonnet')], settings: {} },
    shell: { ...initialShell(), projects: [{ id: 'q1', title: 'studio-app', workspaceRoot: '/studio' }] },
  };
  fleet.entries.set(entry.key, entry);
  let id = 0;
  await liveFleetPass(async () => ({ id: `5-${++id}` }), entry);
  liveFleetEvent(entry, { key: SCHEDULED_TASKS_KEY, generation: 5, subscriptionId: '5-1', value: { tasks: [] } });
  return entry;
}
afterEach(() => { for (const key of [...fleet.entries.keys()]) if (key.endsWith('\nenv2')) fleet.entries.delete(key); });

describe('Runs on switches a new task to another environment', () => {
  test('`new|<environment>` is a new task on that environment', () => {
    expect(editTarget('new|env2', 'env')).toEqual({ environmentId: 'env2', taskId: '', link: false });
    expect(editTarget('new|', 'env')).toEqual({ environmentId: 'env', taskId: '', link: false });
    expect(editTarget('env2|t9', 'env')).toEqual({ environmentId: 'env2', taskId: 't9', link: false });
  });
  test("the editor reads the chosen environment's projects, model and refs, keeps its key and is not a missing task", async () => {
    const { client, native, calls, subscriptions } = harness();
    await studio();
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionsOf(subscriptions), value: { tasks: [] } });
    const here = await scheduledPage(client, native, 'env', '', 'task', '', true, NOW);
    expect(here.editors.map(editor => [editor.key, editor.environmentId, editor.projectId, editor.modelKey, editor.missing])).toEqual([['new', 'env', 'p1', 'codex:gpt', false]]);
    expect(here.environments.map(({ value, label, selected }) => ({ value, label, selected }))).toEqual([{ value: 'env', label: 'Laptop', selected: true }, { value: 'env2', label: 'Studio', selected: false }]);
    const there = await scheduledPage(client, native, 'env', '', 'task', 'new|env2', true, NOW);
    expect([there.environment, there.available]).toEqual(['Studio', true]);
    expect(there.editors.map(editor => [editor.key, editor.environmentId, editor.projectId, editor.modelKey, editor.baseRef, editor.startFromOrigin, editor.path, editor.missing]))
      .toEqual([['new', 'env2', 'q1', 'claudeAgent:sonnet', 'main', true, '', false]]);
    expect(there.projects).toEqual([{ value: 'q1', label: 'studio-app', selected: false, secondary: '', kind: '' }]);
    expect(there.environments.map(option => [option.value, option.selected])).toEqual([['env', false], ['env2', true]]);
    expect(there.marks.map(mark => mark.value)).toContain('claudeAgent:sonnet');
    // The base branch picker lists Studio's refs, read over Studio's own transport, and looks the reset base "main" up by
    // name there (selectedRefQuery), since Studio's first page does not list it.
    expect(there.branches.map(group => [group.projectId, group.refs.map(ref => [ref.value, ref.badge])])).toEqual([['q1', [['develop', ''], ['origin/develop', 'remote']]]]);
    const reads = calls.filter(call => call.method === 'vcs.listRefs').map(call => [call.fleet, call.fleet ? obj(call.payload.payload) : call.payload]);
    expect(reads).toEqual([['', { cwd: '/repo', limit: 100 }], ['http://127.0.0.1:16423\nenv2', { cwd: '/studio', limit: 100 }], ['http://127.0.0.1:16423\nenv2', { cwd: '/studio', query: 'main', limit: 10 }]]);
  });
  // ScheduledTaskEditorForm at 2359643cff: pinned projects first, archived ones left out, the default a pinned open one.
  test("the editor's projects: pinned first, archived left out, a new task on the pinned project", async () => {
    const { client, native, subscriptions } = harness();
    const at = '2026-10-31T00:00:00.000Z';
    client.shell.projects = [{ id: 'p1', title: 'fixture', workspaceRoot: '/repo' }, { id: 'p2', title: 'old', workspaceRoot: '/old', archivedAt: at } as Obj as typeof client.shell.projects[number],
      { id: 'p3', title: 'favorite', workspaceRoot: '/favorite', pinnedAt: at } as Obj as typeof client.shell.projects[number]];
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionsOf(subscriptions), value: { tasks: [] } });
    const page = await scheduledPage(client, native, 'env', '', 'task', '', true, NOW);
    expect(page.projects.map(project => project.value)).toEqual(['p3', 'p1']);
    expect(page.editors.map(editor => editor.projectId)).toEqual(['p3']);
  });
  test('a new task starts on the primary environment, listed first, when the window is focused elsewhere', async () => {
    const { client, native, subscriptions } = harness();
    await studio();
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionsOf(subscriptions), value: { tasks: [] } });
    primaryAt('http://127.0.0.1:16423', 'env2', 'Studio');
    try {
      const page = await scheduledPage(client, native, 'env', '', 'task', '', true, NOW);
      expect([page.environment, page.editors.map(editor => editor.environmentId)]).toEqual(['Studio', ['env2']]);
      expect(page.environments.map(option => [option.value, option.selected])).toEqual([['env2', true], ['env', false]]);
    } finally { resetPrimary(); }
  });
  test("Runs on lists the settings scope's connected environments only", async () => {
    const { client, native } = harness();
    await studio();
    const page = await scheduledPage(client, native, 'env', '', 'task', '', true, NOW, 'env');
    expect(page.environments.map(option => option.value)).toEqual(['env']);
  });
  test('machine headers use the primary or connection route; empty summaries qualify duplicate names', async () => {
    const { client, native, subscriptions } = harness();
    Object.assign(client, { origin: 'http://127.0.0.1:16422' });
    client.config.environment.label = 'Studio';
    const remote = await studio();
    remote.config.environment = { ...obj(remote.config.environment), platform: { machine: 'laptop' } };
    liveFleetEvent(remote, { key: SCHEDULED_TASKS_KEY, generation: 5, subscriptionId: '5-1', value: { tasks: [
      { id: 'paused', projectId: 'q1', title: 'Paused task', prompt: '', enabled: false, schedule: { type: 'interval', everyMs: 3600000 } },
    ] } });
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionsOf(subscriptions), value: { tasks: [] } });
    primaryAt('http://127.0.0.1:16422', 'env', 'Studio');
    const saved = fleet.saved;
    try {
      const page = await scheduledPage(client, native, 'env', '', '', '', true, NOW);
      expect(page.sections.map(section => [section.id, section.label, section.secondary, section.transport])).toEqual([
        ['env', 'Studio', '127.0.0.1:16422', 'This machine'],
        ['env2', 'Studio', '127.0.0.1:16423', 'http://127.0.0.1:16423/'],
      ]);
      expect(page.summaries.map(summary => summary.text)).toEqual(['No tasks on Studio · 127.0.0.1:16422']);
      const ssh = { id: 'ssh', origin: remote.origin, kind: 'ssh', ssh: { alias: 'build', hostname: 'build', username: 'dev', port: 2222 } };
      fleet.saved = [{ origin: remote.origin, environmentId: remote.environmentId, routes: [ssh] }];
      expect((await scheduledPage(client, native, 'env', '', '', '', true, NOW)).sections.find(section => section.id === 'env2')?.transport).toBe('SSH dev@build:2222');
      fleet.saved = [{ origin: remote.origin, environmentId: remote.environmentId, routes: [{ id: 'lan', origin: 'http://192.168.1.20:16424' }, ssh] }];
      expect((await scheduledPage(client, native, 'env', '', '', '', true, NOW)).sections.find(section => section.id === 'env2')?.transport).toBe('LAN');
    } finally { fleet.saved = saved; resetPrimary(); }
  });
});

describe('New task, the route in use and the destination (ScheduledTasksSettings at 2359643cff)', () => {
  test('New task opens on the representative when it permits tasks, else on the first connected one that does, else is disabled', async () => {
    const { client, native, subscriptions } = harness();
    const remote = await studio();
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionsOf(subscriptions), value: { tasks: [] } });
    const opened = async () => { const page = await scheduledPage(client, native, 'env', '', 'task', '', true, NOW); return [page.creatable, page.editors.map(editor => editor.environmentId)]; };
    // Neither may create tasks (no orchestration:operate): disabled.
    expect(await opened()).toEqual([false, ['env']]);
    // Only Studio may: New task opens there.
    remote.scopes = ['orchestration:read', 'orchestration:operate'];
    expect(await opened()).toEqual([true, ['env2']]);
    // The representative may too: it wins.
    Object.assign(client, { scopes: ['orchestration:read', 'orchestration:operate'] });
    expect(await opened()).toEqual([true, ['env']]);
  });
  test('a connected environment with several routes names the one in use ("via <route>")', async () => {
    const { client, native, subscriptions } = harness();
    const remote = await studio();
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionsOf(subscriptions), value: { tasks: [] } });
    const saved = fleet.saved;
    try {
      fleet.saved = [{ origin: remote.origin, environmentId: remote.environmentId, routes: [{ id: 'lan', origin: 'http://192.168.1.20:16424' }, { id: 'tail', origin: 'http://studio.tail1234.ts.net:16424' }] }];
      const transport = async () => (await scheduledPage(client, native, 'env', '', '', '', true, NOW)).sections.find(section => section.id === 'env2')?.transport;
      expect(await transport()).toBe('LAN');
      remote.activeRouteId = 'tail';
      expect(await transport()).toMatch(/^via /);
      expect(await transport()).not.toBe('via LAN');
      // Not connected: the preferred route, whatever was in use.
      remote.phase = 'reconnecting';
      expect(await transport()).toBe('LAN');
    } finally { fleet.saved = saved; }
  });
  test('Destination names the thread a run continues, or says each run starts a new thread', async () => {
    const { client, native, subscriptions } = harness();
    client.shell.threads = [{ id: 't1', projectId: 'p1', title: '  Nightly triage ' }];
    await watchLive(client, native);
    liveEvent(client, { key: SCHEDULED_TASKS_KEY, subscriptionId: subscriptionsOf(subscriptions), value: { tasks: [
      { id: 'a', projectId: 'p1', title: 'Continues', prompt: 'go', enabled: true, threadId: 't1', schedule: { type: 'interval', everyMs: 3600000 } },
      { id: 'b', projectId: 'p1', title: 'Gone thread', prompt: 'go', enabled: true, threadId: 't9', schedule: { type: 'interval', everyMs: 3600000 } },
      { id: 'c', projectId: 'p1', title: 'Fresh', prompt: 'go', enabled: true, schedule: { type: 'interval', everyMs: 3600000 } },
    ] } });
    const destination = async (id: string) => (await scheduledPage(client, native, 'env', '', 'task', `env|${id}`, true, NOW)).editors.map(editor => [editor.threadId, editor.destination]);
    expect(await destination('a')).toEqual([['t1', 'Nightly triage']]);
    expect(await destination('b')).toEqual([['t9', 'Thread t9']]);
    expect(await destination('c')).toEqual([['', '']]);
    const source = await Bun.file(new URL('../../src/settings/settings-scheduled.contract', import.meta.url)).text();
    expect(source).toContain('"Each run starts a new thread with the workspace and model below."');
    expect(source).toContain('"Each run continues this conversation, in its workspace and with its current model."');
  });
});

describe("Open project on another environment's clone toast", () => {
  const done = { projectId: 'q7', destinationPath: '/work/widgets', repository: null, phase: 'done', stage: 'checkout', percent: 100, detail: null, error: null };
  test('every success toast offers Open project, naming its environment', async () => {
    expect(cloneToast(done, 'env2').action).toEqual({ label: 'Open project', op: 'new-thread', id: 'fleet:env2:q7', value: '' });
    const { client, native } = harness();
    const entry = await studio();
    liveFleetEvent(entry, { key: PROJECT_CLONES_KEY, generation: 5, subscriptionId: '5-2', value: [done] });
    await cloneToasts(client, native);
    expect(toasts(client).find(toast => toast.title === 'Cloned widgets')).toMatchObject({ kind: 'success', timeoutMs: 8000, action: { label: 'Open project', op: 'new-thread', id: 'fleet:env2:q7' } });
  });
  test("Open project on a background environment moves the window there, on that project's draft", async () => {
    const { client, native, calls } = harness();
    const entry = await studio();
    const adopted: Obj[] = [];
    const self = Object.assign(client, { adoptStatus: (value: Obj, generation: number) => { adopted.push({ ...value, generation }); } });
    const out = {} as OpOut;
    expect(await threadOps.call(self, 'new-thread', 'fleet:env2:q7', '', 0, native, {} as Files, out)).toBe(true);
    expect(client.local.selections.env2).toEqual({ projectId: 'q7', threadId: '' });
    expect(calls.filter(call => call.payload.op === 'fleetStop' || call.payload.op === 'connect').map(call => [call.payload.op, call.fleet || String(call.payload.origin)]))
      .toEqual([['fleetStop', entry.key], ['connect', 'http://127.0.0.1:16423']]);
    expect(adopted).toEqual([{ state: 'connecting', origin: 'http://127.0.0.1:16423', environmentId: 'env2', message: '', generation: 4 }]);
    expect(fleet.entries.has(entry.key)).toBe(false);
    await expect(focusFleetProject(client, native, 'gone', 'q7')).rejects.toThrow('no longer connected');
  });
  test("Open project on the focused environment opens that project's draft here", async () => {
    const { client, native, calls } = harness();
    client.shell.projects.push({ id: 'p2', title: 'widgets', workspaceRoot: '/work/widgets' });
    client.threadId = 't1';
    const self = Object.assign(client, { connection: 'disconnected', chooseDefaults: () => {}, subscriptions: {} as Record<string, string> });
    expect(await threadOps.call(self, 'new-thread', 'fleet:env:p2', '', 0, native, {} as Files, {} as OpOut)).toBe(true);
    expect([client.projectId, client.threadId]).toEqual(['p2', '']);
    expect(calls.some(call => call.payload.op === 'connect')).toBe(false);
  });
});

function subscriptionsOf(subscriptions: Record<string, string>): string { return subscriptions['scheduledTasks.subscribe']!; }
