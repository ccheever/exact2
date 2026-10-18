// Parking the open thread moves to the next card in its own environment (Sidebar.tsx planForwardNavigation:
// navigateToThread(scopeThreadRef(nextThread.environmentId, nextThread.id))), and a thread the shell does not hold
// shows the empty state without fetching its detail, which the server would answer with 404.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { T3Client as Client } from '../../src/client/client';
import { Backend, storage } from '../support/client-fixture';
import { initialShell, obj, str, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { sidebarCommand } from '../../src/sidebar/sidebar-commands';
import { setRuntimeClock } from '../../src/sidebar/sidebar-state';
import { fleet, type FleetEntry } from '../../src/connections/settings-b-fleet';
import { toasts } from '../../src/shell/toast';

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

// A background environment's row in the flat sidebar (`fleet:<environment>:<id>`) settles on that environment
// (useThreadActions.settleThread over the row's scoped thread ref), and its undo un-settles there too.
describe('settling a background environment\'s row', () => {
  function harness(threads: Obj[], capabilities: Obj = CAPS) {
    const entry = { ...background(threads), config: { environment: { environmentId: 'env-b', label: 'Lane Second', capabilities } } }, focusedRequests: Obj[] = [], sent: Obj[] = [];
    fleet.entries.set(entry.key, entry);
    const client = focused([row('a', 'p1', iso(-1_000))], 'a').client;
    (client as unknown as { restAccess: unknown }).restAccess = () => ({ ids: async (count: number) => Array.from({ length: count }, (_, index) => `c${index}`),
      request: async (method: string, payload: Obj) => { focusedRequests.push({ method, ...payload }); return {}; }, call: async () => ({}) });
    const native = { available: true, watch() {}, later: async (request: unknown) => {
      const value = obj(request);
      if (value.op === 'request' && value.fleet === entry.key) sent.push(obj(value.payload));
      return { ok: true, generation: entry.generation, value: {} };
    } } as unknown as Native;
    return { client, native, sent, focusedRequests };
  }

  test('the hover Settle reaches the background environment with its own thread id, and Undo un-settles it there', async () => {
    const { client, native, sent, focusedRequests } = harness([row('tb', 'pb', iso(-60_000))]);
    await sidebarCommand(client, native, {} as Files, 'settle', 'fleet:env-b:tb', '');
    expect(toasts(client)).toEqual([]);
    expect(sent).toMatchObject([{ type: 'thread.settle', threadId: 'tb' }]);
    expect(focusedRequests).toEqual([]);
    await sidebarCommand(client, native, {} as Files, 'undo', '', '');
    expect(sent.map(payload => [payload.type, payload.threadId, payload.reason])).toEqual([['thread.settle', 'tb', undefined], ['thread.unsettle', 'tb', 'user']]);
    expect(focusedRequests).toEqual([]);
  });

  test('Undo restores a pinned, snoozed background thread\'s pin (by that environment\'s capabilities) and snooze there', async () => {
    const pinned = { ...row('tb', 'pb', iso(-60_000)), pinnedAt: iso(-30_000), pinOrderKey: 'a0', snoozedUntil: iso(3_600_000) };
    const { client, native, sent } = harness([pinned], { ...CAPS, threadPinReorder: true });
    await sidebarCommand(client, native, {} as Files, 'settle', 'fleet:env-b:tb', '');
    await sidebarCommand(client, native, {} as Files, 'undo', '', '');
    expect(sent.map(payload => payload.type)).toEqual(['thread.settle', 'thread.unsettle', 'thread.pin', 'thread.snooze']);
    expect(sent.every(payload => payload.threadId === 'tb')).toBe(true);
    expect(sent[2]).toMatchObject({ orderKey: 'a0' });
  });

  test('Un-settle and Wake on a background row go to its environment', async () => {
    const { client, native, sent } = harness([{ ...row('tb', 'pb', iso(-60_000)), settledOverride: 'settled', settledAt: iso(-1_000) }]);
    await sidebarCommand(client, native, {} as Files, 'unsettle', 'fleet:env-b:tb', '');
    await sidebarCommand(client, native, {} as Files, 'unsnooze', 'fleet:env-b:tb', '');
    expect(sent.map(payload => [payload.type, payload.threadId])).toEqual([['thread.unsettle', 'tb'], ['thread.unsnooze', 'tb']]);
  });

  // The undo's transport is the thread's environment as it stands when ⌘Z is pressed: the focus may have moved since.
  test('Undo after the background environment took the focus un-settles it through the focused transport', async () => {
    const { client, native, sent, focusedRequests } = harness([row('tb', 'pb', iso(-60_000))]);
    await sidebarCommand(client, native, {} as Files, 'settle', 'fleet:env-b:tb', '');
    client.environmentId = 'env-b';
    client.shell = { ...client.shell, threads: [row('tb', 'pb', iso(-60_000))] };
    await sidebarCommand(client, native, {} as Files, 'undo', '', '');
    expect(toasts(client)).toEqual([]);
    expect(sent.map(payload => payload.type)).toEqual(['thread.settle']);
    expect(focusedRequests).toMatchObject([{ method: 'orchestration.dispatchCommand', type: 'thread.unsettle', threadId: 'tb', reason: 'user' }]);
  });

  test('Undo after another environment took the focus un-settles the focused thread on its own environment', async () => {
    const { client, native, sent, focusedRequests } = harness([row('tb', 'pb', iso(-60_000))]);
    await sidebarCommand(client, native, {} as Files, 'settle', 'a', '');
    expect(focusedRequests).toMatchObject([{ type: 'thread.settle', threadId: 'a' }]);
    // env-b is focused now, and env-a is the fleet's background entry.
    const former = { ...background([row('a', 'p1', iso(-1_000))]), key: 'https://first.example\nenv-a', origin: 'https://first.example', environmentId: 'env-a' };
    fleet.entries.set(former.key, former);
    client.environmentId = 'env-b';
    const later = { ...native, later: async (request: unknown) => {
      const value = obj(request);
      if (value.op === 'request' && value.fleet === former.key) sent.push({ environment: 'env-a', ...obj(value.payload) });
      return { ok: true, generation: former.generation, value: {} };
    } } as unknown as Native;
    await sidebarCommand(client, later, {} as Files, 'undo', '', '');
    expect(toasts(client)).toEqual([]);
    expect(focusedRequests).toHaveLength(1);
    expect(sent).toMatchObject([{ environment: 'env-a', type: 'thread.unsettle', threadId: 'a', reason: 'user' }]);
  });

  test('a row no environment holds still reads Thread unavailable', async () => {
    const { client, native, sent } = harness([]);
    await sidebarCommand(client, native, {} as Files, 'settle', 'fleet:env-b:gone', '');
    expect(sent).toEqual([]);
    expect(toasts(client).map(toast => toast.title)).toEqual(['Thread unavailable']);
  });
});
