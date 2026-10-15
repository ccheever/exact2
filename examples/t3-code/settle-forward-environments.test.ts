// Parking the open thread moves to the next card in its own environment (Sidebar.tsx planForwardNavigation:
// navigateToThread(scopeThreadRef(nextThread.environmentId, nextThread.id))), and a thread the shell does not hold
// shows the empty state without fetching its detail, which the server would answer with 404.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { T3Client as Client } from './client';
import { Backend, storage } from './client-fixture';
import { initialShell, obj, str, type Obj } from './domain';
import type { Files, Native } from './protocol';
import { sidebarCommand } from './sidebar-commands';
import { setRuntimeClock } from './sidebar-state';
import { fleet, type FleetEntry } from './settings-b-fleet';

beforeEach(() => setRuntimeClock(() => Date.now()));
afterEach(() => { setRuntimeClock(() => Number.NaN); fleet.entries.clear(); });

const NOW = Date.now();
const iso = (offset: number) => new Date(NOW + offset).toISOString();
const CAPS = { threadSettlement: true, threadSnooze: true, threadPinning: true, threadVisitedTracking: true };
const row = (id: string, projectId: string, createdAt: string): Obj => ({ id, projectId, title: `Thread ${id}`, status: 'idle', latestRunId: null,
  createdAt, updatedAt: createdAt, archivedAt: null, settledOverride: null, settledAt: null, modelSelection: { instanceId: 'codex', model: 'm' },
  lineage: { relationshipToParent: null } });

function background(threads: Obj[]): FleetEntry {
  return { key: 'https://second.example\nenv-b', origin: 'https://second.example', environmentId: 'env-b', phase: 'connected', message: '', traceId: '',
    generation: 2, synchronized: 2, lastEvent: 0, subscriptions: {}, config: { environment: { environmentId: 'env-b', label: 'Lane Second', capabilities: CAPS } },
    shell: { ...initialShell(), projects: [{ id: 'pb', title: 'background-only', workspaceRoot: '/b' }], threads }, scopes: [], error: '', requested: true };
}

function focused(threads: Obj[], threadId: string) {
  const opened: string[] = [], drafts: string[] = [], adopted: Obj[] = [], requests: Obj[] = [];
  const client = {
    shell: { projects: [{ id: 'p1', title: 'shared-notes', workspaceRoot: '/a' }], threads, sequence: 1 },
    config: { environment: { capabilities: CAPS }, providers: [], keybindings: [] },
    environmentId: 'env-a', threadId, projectId: 'p1', query: '', connection: 'connected', writable: true, ready: true, presentation: {},
    local: { drafts: {}, selections: {}, snapshotDrafts: {}, snapshotReleases: [], composerControls: { contexts: {} }, deviceSettings: { timestampFormat: '24-hour' },
      clientSettings: { confirmThreadArchive: false, sidebarWorkingShelfEnabled: false }, sidebarWidth: 256 },
    projectGroups() { return [{ key: 'g1', name: 'shared-notes', members: [{ id: 'p1' }] }]; },
    restAccess: () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`), request: async () => ({}), call: async () => ({}) }),
    async openSelected(_native: Native, id: string) { opened.push(id); client.threadId = id; },
    async openDraft(_native: Native, projectId: string) { drafts.push(projectId); client.threadId = ''; },
    adoptStatus(value: Obj, generation: number) { adopted.push({ ...value, generation }); },
  } as unknown as T3Client & { threadId: string };
  const native = { available: true, watch() {}, later: async (request: unknown) => {
    requests.push(obj(request));
    return obj(request).op === 'connect' ? { ok: true, generation: 3, value: { state: 'connected', origin: 'https://second.example', environmentId: 'env-b', message: '' } } : { ok: true, generation: 3, value: {} };
  } } as unknown as Native;
  return { client, native, opened, drafts, adopted, requests };
}

describe('settling the open thread', () => {
  test('a background environment\'s next card opens in that environment, as its row\'s press does', async () => {
    fleet.entries.set('https://second.example\nenv-b', background([row('tb', 'pb', iso(-60_000))]));
    const { client, native, opened, drafts, adopted, requests } = focused([row('a', 'p1', iso(-1_000))], 'a');
    await sidebarCommand(client, native, {} as Files, 'settle', 'a', '');
    expect(opened).toEqual([]);
    expect(drafts).toEqual([]);
    expect(requests.find(request => request.op === 'connect')).toMatchObject({ origin: 'https://second.example' });
    expect(obj(client.local.selections)['env-b']).toEqual({ projectId: 'pb', threadId: 'tb' });
    expect(adopted).toEqual([{ state: 'connected', origin: 'https://second.example', environmentId: 'env-b', message: '', generation: 3 }]);
  });

  test('the focused environment\'s next card still opens here', async () => {
    fleet.entries.set('https://second.example\nenv-b', background([row('tb', 'pb', iso(-60_000))]));
    const { client, native, opened, requests } = focused([row('a', 'p1', iso(-1_000)), row('b', 'p1', iso(-2_000))], 'a');
    await sidebarCommand(client, native, {} as Files, 'settle', 'a', '');
    expect(opened).toEqual(['b']);
    expect(requests.some(request => request.op === 'connect')).toBe(false);
  });
});

describe('a thread the shell does not hold', () => {
  const bounded = (backend: Backend, id: string) => backend.calls.filter(call => str(call.path) === `/api/orchestration/threads/${id}/bounded`).length;

  test('shows the empty state through later refreshes without asking for its detail, and opens once the shell holds it', async () => {
    const client = new Client(), backend = new Backend(), disk = storage();
    await client.refresh(backend, disk.files);
    await client.command('select-thread', 'gone', '', 0, backend, disk.files);
    expect(client.threadId).toBe('gone');
    await client.refresh(backend, disk.files);
    await client.refresh(backend, disk.files);
    expect(bounded(backend, 'gone')).toBe(0);
    expect(client.error).toBe('');
    expect(client.thread).toBeNull();
    // The thread arrives (a reconnect reads the shell again): its detail is read and it opens.
    backend.shell = { ...backend.shell, threads: [...(backend.shell.threads as Obj[]), { ...obj((backend.shell.threads as Obj[])[0]), id: 'gone', title: 'Thread gone' }] };
    backend.details.gone = { ...backend.details.t2, projection: { ...obj(backend.details.t2!.projection), thread: { ...obj(obj(backend.details.t2!.projection).thread), id: 'gone' } } };
    client.shellLoaded = false; backend.generation++;
    await client.refresh(backend, disk.files);
    expect(bounded(backend, 'gone')).toBe(1);
    expect(client.threadId).toBe('gone');
    expect(client.thread).not.toBeNull();
  });
});
