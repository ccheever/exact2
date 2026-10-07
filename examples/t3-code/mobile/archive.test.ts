import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { mobileArchive, mobileArchiveCommand, projectMobileArchive, type ArchiveSnapshot } from './archive';
import { mobileClient } from './client';
import { initialShell, obj, type Obj } from './shared/domain';
import { fleet, type FleetEntry } from './shared/settings-b-fleet';
import type { Native } from './shared/protocol';

const now = Date.parse('2026-10-07T12:00:00Z');
const at = (minutes: number) => new Date(now + minutes * 60_000).toISOString();
const thread = (id: string, patch: Obj = {}): Obj => ({ id, projectId: 'p', title: id, archivedAt: at(-10), updatedAt: at(-1), branch: 'feature', ...patch });
const source = (environmentId: string, threads: Obj[]): ArchiveSnapshot => ({ environmentId, label: `Machine ${environmentId}`, machine: 'laptop', canOperate: true,
  shell: { ...initialShell(), projects: [{ id: 'p', title: 'Project', workspaceRoot: '/workspace/repo' }], threads } });
const threads = (items: ReturnType<typeof projectMobileArchive>) => items.filter(row => row.kind === 'thread');

describe('pinned archive grouping', () => {
  test('environment/project identities remain scoped; newest and oldest sort groups by their first row', () => {
    const input = [source('one', [thread('same', { archivedAt: at(-5) }), thread('early', { archivedAt: at(-20) }), thread('live', { archivedAt: null })]), source('two', [thread('same', { archivedAt: at(-10) })])];
    const before = JSON.stringify(input);
    expect(threads(projectMobileArchive(input, now)).map(row => row.key)).toEqual(['one:same', 'one:early', 'two:same']);
    expect(threads(projectMobileArchive(input, now, '', '', 'oldest')).map(row => row.key)).toEqual(['one:early', 'one:same', 'two:same']);
    expect(JSON.stringify(input)).toBe(before);
    expect(projectMobileArchive(input, now)[0]?.initial).toBe(true);
    expect(projectMobileArchive(input, now).at(-1)?.final).toBe(true);
  });
  test('project/path/environment query keeps whole groups; title and branch query narrows threads', () => {
    const input = [source('one', [thread('Needle', { branch: 'main' }), thread('Other', { branch: 'branch-needle' })]), source('two', [thread('Other')])];
    for (const query of [' PROJECT ', '/WORKSPACE', 'machine one']) expect(threads(projectMobileArchive(input.slice(0, 1), now, query))).toHaveLength(2);
    expect(threads(projectMobileArchive(input, now, 'needle')).map(row => row.title)).toEqual(['Needle', 'Other']);
    expect(threads(projectMobileArchive(input, now, 'main')).map(row => row.title)).toEqual(['Needle']);
    expect(threads(projectMobileArchive(input, now, '', 'two')).map(row => row.environmentId)).toEqual(['two']);
  });
  test('invalid dates sort at zero; equal dates tie on title then id; cards and labels match source', () => {
    const input = source('one', [thread('b', { title: 'Same' }), thread('a', { title: 'Same' }), thread('bad', { archivedAt: 'invalid' })]);
    const rows = threads(projectMobileArchive([input], now));
    expect(rows.map(row => row.threadId)).toEqual(['a', 'b', 'bad']);
    expect(rows.map(row => [row.first, row.last])).toEqual([[true, false], [false, false], [false, true]]);
    expect(rows[0]?.subtitle).toBe('Machine one · feature');
  });
});

let requests: Obj[], saved: Obj[], permission: string[], hook: ((request: Obj) => unknown) | null;
const snapshot = { snapshotSequence: 4, projects: [{ id: 'p', title: 'Project' }], threads: [thread('archived')] };
const native: Native = { available: true, watch() {}, async later(input) {
  const request = obj(input); requests.push(request);
  const generation = request.fleet ? 8 : mobileClient.generation;
  const custom = hook?.(request); if (custom !== undefined) return await custom;
  if (request.op === 'environments') return { ok: true, generation, value: { saved } };
  if (request.op === 'http') return { ok: true, generation, value: { authenticated: true, permissions: permission } };
  if (request.op === 'ids') return { ok: true, generation, value: ['command-from-native'] };
  if (request.method === 'orchestration.getArchivedShellSnapshot') return { ok: true, generation, value: snapshot };
  if (request.method === 'orchestration.dispatchCommand') return { ok: true, generation, value: {} };
  throw new Error(`Unexpected operation ${JSON.stringify(request)}`);
} };
let previousClient: Partial<typeof mobileClient>, previousEntries: typeof fleet.entries, previousSaved: typeof fleet.saved;
beforeEach(async () => {
  previousClient = { ...mobileClient };
  previousEntries = fleet.entries; previousSaved = fleet.saved;
  requests = []; saved = []; hook = null; permission = ['orchestration:operate'];
  fleet.entries = new Map(); fleet.saved = [];
  await mobileArchive(now, '', '', 'newest', native);
  mobileClient.environmentId = 'one'; mobileClient.origin = 'https://one.example'; mobileClient.generation = 3;
  mobileClient.connection = 'connected'; mobileClient.configLive = true; mobileClient.shellLive = true; mobileClient.threadId = '';
  const entry: FleetEntry = { key: 'https://two.example\ntwo', origin: 'https://two.example', environmentId: 'two', phase: 'connected', message: '', traceId: '', generation: 8,
    synchronized: 8, lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(), scopes: [], error: '', requested: true };
  fleet.entries.set(entry.key, entry);
  saved = [{ environmentId: 'one', label: 'Zed', origin: mobileClient.origin, enabled: true }, { environmentId: 'two', label: 'Alpha', origin: entry.origin, enabled: true }];
  fleet.saved = saved; requests = [];
});

afterEach(() => {
  Object.assign(mobileClient, previousClient);
  fleet.entries = previousEntries; fleet.saved = previousSaved;
});

describe('archive transport ownership', () => {
  test('fetches all saved environments even with selector, and uses each transport', async () => {
    const view = await mobileArchive(now, '', 'two', 'newest', native);
    expect(view.error).toBe(''); expect(view.environments.map(row => row.id)).toEqual(['two', 'one']);
    expect(threads(view.items).map(row => row.environmentId)).toEqual(['two']);
    const reads = requests.filter(row => row.method === 'orchestration.getArchivedShellSnapshot');
    expect(reads.map(row => row.fleet ?? '').sort()).toEqual(['', 'https://two.example\ntwo']);
    expect(requests.some(row => ['connect', 'fleetConnect'].includes(String(row.op)))).toBe(false);
    expect(mobileClient.environmentId).toBe('one');
  });
  test('partial failure preserves last successful rows read-only while the other archive refreshes', async () => {
    await mobileArchive(now, '', '', 'newest', native);
    hook = request => request.fleet && request.method === 'orchestration.getArchivedShellSnapshot' ? Promise.reject(new Error('offline')) : undefined;
    const view = await mobileArchive(now, '', '', 'newest', native);
    expect(view.error).toBe('Failed to load archived threads.');
    expect(threads(view.items).map(row => [row.environmentId, row.canOperate]).sort()).toEqual([['one', true], ['two', false]]);
  });
  test('background unarchive dispatches once with native command id and never changes focus', async () => {
    const answer = await mobileArchiveCommand('unarchive', 'two', 'p', 'archived', native);
    expect(answer.message).toBe('');
    const writes = requests.filter(row => row.method === 'orchestration.dispatchCommand');
    expect(writes).toHaveLength(1);
    expect(writes[0]?.fleet).toBe('https://two.example\ntwo'); expect(writes[0]?.generation).toBe(8);
    expect(writes[0]?.payload).toEqual({ type: 'thread.unarchive', commandId: 'command-from-native', threadId: 'archived' });
    expect(mobileClient.environmentId).toBe('one'); expect(mobileClient.origin).toBe('https://one.example');
  });
  test('explicit empty permissions, disabled saved entries and wrong project prevent dispatch', async () => {
    permission = [];
    expect((await mobileArchiveCommand('delete', 'two', 'p', 'archived', native)).message).toContain('permission');
    permission = ['orchestration:operate']; saved[1]!.enabled = false;
    expect((await mobileArchiveCommand('delete', 'two', 'p', 'archived', native)).message).toContain('Connect');
    saved[1]!.enabled = true;
    expect((await mobileArchiveCommand('delete', 'two', 'wrong', 'archived', native)).message).toContain('no longer available');
    expect(requests.some(row => row.method === 'orchestration.dispatchCommand')).toBe(false);
  });
  test('a generation change while authorizing cannot dispatch on the replacement connection', async () => {
    hook = request => {
      if (request.op !== 'http') return undefined;
      fleet.entries.get('https://two.example\ntwo')!.generation = 9;
      return { ok: true, generation: 8, value: { authenticated: true, permissions: permission } };
    };
    expect((await mobileArchiveCommand('delete', 'two', 'p', 'archived', native)).message).toContain('connection changed');
    expect(requests.some(row => row.op === 'ids' || row.method === 'orchestration.dispatchCommand')).toBe(false);
  });
  test('uncertain background write is reported and never replayed', async () => {
    hook = request => request.method === 'orchestration.dispatchCommand' ? Promise.reject(new Error('socket closed')) : undefined;
    const answer = await mobileArchiveCommand('delete', 'two', 'p', 'archived', native);
    expect(answer.message).toContain('may have reached');
    expect(requests.filter(row => row.method === 'orchestration.dispatchCommand')).toHaveLength(1);
  });
  test('concurrent duplicate row actions allocate and dispatch only once', async () => {
    let release!: () => void; const wait = new Promise<void>(resolve => { release = resolve; });
    hook = request => request.op === 'http' ? wait.then(() => ({ ok: true, generation: request.fleet ? 8 : 3, value: { authenticated: true, permissions: permission } })) : undefined;
    const first = mobileArchiveCommand('delete', 'one', 'p', 'archived', native);
    const second = await mobileArchiveCommand('delete', 'one', 'p', 'archived', native);
    expect(second.message).toContain('already running'); release();
    expect((await first).message).toBe('');
    expect(requests.filter(row => row.method === 'orchestration.dispatchCommand')).toHaveLength(1);
  });
});
