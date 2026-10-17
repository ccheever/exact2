import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { T3Client } from './shared/client';
import { arr, type Obj } from './shared/domain';
import { mobileWorkingControl, workingControlDuration, workingControlSegments, workingAgentsSegment } from './working-control';
const at = '2026-10-07T12:00:00Z', now = Date.parse(at);
function fixture(projection: Obj = {}) {
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env', threadId: 'thread', origin: 'https://example.test', generation: 1, connection: 'connected', threadLive: true,
    configLive: true, shellLive: true, config: { environment: { label: 'Server' } } });
  client.thread = { projection: { thread: { id: 'thread' }, runs: [], turnItems: [], ...projection }, sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  client.thread.projection.visibleTurnItems = arr(projection.turnItems).map(item => ({ visibility: 'local', sourceThreadId: 'thread', sourceItemId: item.id, item }));
  client.shell.threads = [{ id: 'thread' }];
  return client;
}
const run = { id: 'running', status: 'running', ordinal: 1, startedAt: at, requestedAt: at };
const view = (client: T3Client, time = now) => mobileWorkingControl('env', 'thread', time, client);
test('timer uses work start and does not let a newer queued run own status', () => {
  const client = fixture({ runs: [{ ...run, workStartedAt: new Date(now - 61_000).toISOString() }, { id: 'next', status: 'queued', ordinal: 2 }] });
  expect(view(client)).toMatchObject({ statusKind: 'working', statusLabel: 'Working 1m 01s', queueCount: 1 });
  expect(workingControlDuration('invalid', now)).toBe('0s');
  expect(workingControlDuration(at, now + 3_661_000)).toBe('1h 1m 1s');
});
test('compaction clears when the actual compact item settles', () => {
  const client = fixture({ runs: [run], turnItems: [{ id: 'message', runId: run.id, type: 'user_message', text: '/compact', attachments: [] }] });
  expect(view(client).statusKind).toBe('compacting');
  client.thread!.projection.visibleTurnItems = [{ visibility: 'local', item: { runId: run.id, type: 'compaction', status: 'completed' } }];
  expect(view(client).statusKind).toBe('working');
});
test('agent segment shows live ratio, done during a live turn, and hides settled rosters', () => {
  const client = fixture({ runs: [run], subagents: [{ id: 'agent', runId: run.id, status: 'running', startedAt: at, updatedAt: at }] });
  expect(view(client)).toMatchObject({ agentsLabel: '1/1', agentsAccessibility: 'Open agents, 1 of 1 agents working' });
  client.thread!.projection.subagents = [{ id: 'agent', runId: run.id, status: 'completed', updatedAt: at }];
  expect(view(client).agentsLabel).toBe('1 done');
  client.thread!.projection.runs = [{ ...run, status: 'completed' }];
  expect(view(client).agentsLabel).toBe('');
});
test('background commands and idle provider goals come from the actual shell', () => {
  const client = fixture(); client.shell.threads = [{ id: 'thread', pendingBackgroundTasks: [{ kind: 'command', description: 'dev server' }] }];
  expect(view(client)).toMatchObject({ statusKind: 'background', statusSymbol: 'terminal', statusLabel: 'Running: dev server' });
  client.shell.threads = [{ id: 'thread', goal: { status: 'active', objective: 'Build the feature' } }];
  expect(view(client)).toMatchObject({ statusKind: 'goal', statusLabel: 'Goal set', statusAccessibility: 'Goal set: Build the feature' });
});
test('sync has a400ms delay and minimum display time and clears on owner change', () => {
  const client = fixture({ runs: [run] }); client.threadLive = false;
  expect(view(client).statusKind).toBe('working'); expect(view(client).nextRefreshAt).toBe(now + 400);
  expect(view(client, now + 400).statusLabel).toBe('Syncing messages...');
  client.threadLive = true; expect(view(client, now + 450).statusKind).toBe('syncing');
  expect(view(client, now + 800).statusKind).toBe('working');
  client.threadLive = false; view(client, now + 1000); client.generation++;
  expect(view(client, now + 1400).statusKind).toBe('working');
});
test('connection wins and unrelated route identities never expose counts or status', () => {
  const client = fixture({ runs: [run] }); client.connection = 'offline';
  expect(view(client)).toMatchObject({ statusLabel: 'You are offline', reconnect: true });
  expect(mobileWorkingControl('other', 'thread', now, client)).toMatchObject({ visible: false, owner: '', queueCount: 0 });
  client.connection = 'connected'; client.thread!.projection.thread = { id: 'old' };
  expect(view(client).statusKind).toBe('');
});
test('preview labels and badge compactness follow actual counts and other capsule segments', () => {
  const blank = { kind: '', label: '', accessibility: '', symbol: '', spinning: false, reconnect: false }, agents = workingAgentsSegment({ rows: [], runId: '', turnActive: false, liveCount: 0, settledCount: 0 });
  expect(workingControlSegments(blank, agents, 1, 0, 0)).toMatchObject({ compact: false, browserLabel: 'One tab open', browserAccessibility: 'View browser' });
  expect(workingControlSegments(blank, agents, 2, 1, 0)).toMatchObject({ compact: true, browserAccessibility: 'View 2 browser tabs', deviceAccessibility: 'View device' });
  expect(workingControlSegments(blank, agents, 0, 0, 0).visible).toBe(false);
});
test('empty and populated snapshots match the exact declared producer shape', () => {
  const contract = readFileSync(new URL('./working-control.contract', import.meta.url), 'utf8');
  const fields = [...contract.split('shape WorkingControlData\n')[1]!.split('component MobileWorkingControl')[0]!.matchAll(/^  (\w+):/gm)].map(match => match[1]).sort();
  expect(fields.length).toBe(19);
  for (const data of [view(fixture()), view(fixture({ runs: [run] })), mobileWorkingControl('', '', now, fixture())]) expect(Object.keys(data).sort()).toEqual(fields);
});

test('connecting and reconnecting preserve the pinned connection-error retry copy', () => {
  const client = fixture();
  for (const phase of ['connecting', 'reconnecting']) {
    client.connection = phase; client.statusMessage = '';
    expect(view(client)).toMatchObject({ statusLabel: 'Reconnecting to Server...', statusSpinning: true, reconnect: true });
    client.statusMessage = 'Connection refused';
    expect(view(client)).toMatchObject({ statusLabel: 'Failed to connect. Retrying Server...', statusSpinning: true, reconnect: true });
  }
});
