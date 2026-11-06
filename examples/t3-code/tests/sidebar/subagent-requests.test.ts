import { describe, expect, test } from 'bun:test';
import { childThreadInputs, childThreadInputsTitle, noteSubagentRequests, pendingSubagentRequestsByThread } from '../../src/sidebar/subagent-requests';
import { canSnooze, raisedHand, sidebarStatus } from '../../src/sidebar/sidebar-model';
import { contextSnapshot, formatTokens } from '../../src/composer/composer-controls-view';
import { endSummary, threadNowRows } from '../../src/composer/thread-end';
import { pendingHandoff, threadFooterRows } from '../../src/timeline/thread-status-line';
import { opened } from '../support/composer-controls-fixture';
import type { Obj } from '../../src/client/domain';

const child = (id: string, parentThreadId: string, kind: string): Obj => ({ id, title: id, status: 'running', activityRunStatus: 'running',
  lineage: { parentThreadId, relationshipToParent: 'subagent' }, pendingRuntimeRequest: { kind } });

test('a hidden child raises its visible ancestors, approval wins, and completed children clear attention', async () => {
  const { client } = await opened();
  const parent = client.shell.threads[0]!, branch = child('branch', 't1', 'user_input'), leaf = child('leaf', 'branch', 'command');
  client.shell.threads = [parent, branch, leaf];
  noteSubagentRequests(client.shell.threads);
  expect([...pendingSubagentRequestsByThread(client.shell.threads)]).toEqual([['branch', 'approval'], ['t1', 'approval']]);
  expect(sidebarStatus(parent)).toBe('approval');
  expect(canSnooze(parent, 0)).toBe(false);
  expect(raisedHand(parent)).toBe(true);
  expect(childThreadInputsTitle(childThreadInputs(client.shell.threads, 't1'))).toBe('2 subagents need you');
  // childInputNowItem: the only line at the end of the timeline that needs you, announced, its action opening the first child.
  const line = threadNowRows(client)[0]!;
  expect(line).toMatchObject({ id: 'child-input:branch', title: '2 subagents need you', icon: 'message-circle', tone: 'info', status: 'alert' });
  expect(line.activities!.filter(entry => entry.status === 'action')).toEqual([expect.objectContaining({ label: 'Open question', detail: 'select-thread', targetId: 'branch' })]);
  expect(endSummary(client)).toBe('Subagent waiting');
  branch.activityRunStatus = null; leaf.activityRunStatus = null;
  client.shell.threads = [...client.shell.threads];
  noteSubagentRequests(client.shell.threads);
  expect(sidebarStatus(parent)).not.toBe('approval');
  expect(threadNowRows(client).some(row => row.id.startsWith('child-input:'))).toBe(false);
});

test('a live subagent question stays inside its parent: no flag up the chain, while an approval still flags it', async () => {
  const { client } = await opened();
  const parent = client.shell.threads[0]!, branch = child('branch', 't1', 'user_input'), leaf = child('leaf', 'branch', 'user_input');
  client.shell.threads = [parent, branch, leaf];
  noteSubagentRequests(client.shell.threads);
  expect(pendingSubagentRequestsByThread(client.shell.threads).size).toBe(0);
  expect(sidebarStatus(parent)).not.toBe('approval');
  expect(raisedHand(parent)).toBe(false);
  expect(childThreadInputs(client.shell.threads, 't1').map(entry => entry.id)).toEqual(['branch', 'leaf']);
  client.shell.threads = [parent, branch, { ...leaf, pendingRuntimeRequest: { kind: 'command' } }];
  noteSubagentRequests(client.shell.threads);
  expect([...pendingSubagentRequestsByThread(client.shell.threads)]).toEqual([['branch', 'approval'], ['t1', 'approval']]);
  expect(sidebarStatus(parent)).toBe('approval');
});

test('a parent request takes precedence and an auth refresh never raises attention', async () => {
  const { client } = await opened();
  client.shell.threads = [...client.shell.threads, child('q', 't1', 'user_input'), child('auth', 't1', 'auth_refresh')];
  expect(childThreadInputs(client.shell.threads, 't1').map(entry => entry.id)).toEqual(['q']);
  client.thread!.projection.runtimeRequests = [{ id: 'a', status: 'pending', kind: 'command', responseCapability: { type: 'live' } }];
  expect(threadNowRows(client).some(row => row.id.startsWith('child-input:'))).toBe(false);
});

/** A thread whose provider session hands context to another provider instead of locking the composer to its own. */
function handsOff(projection: Obj): void {
  Object.assign(projection.thread as Obj, { activeProviderThreadId: 'pt' });
  projection.providerThreads = [{ id: 'pt', appThreadId: 't1', providerSessionId: 'ps' }];
  projection.providerSessions = [{ id: 'ps', capabilities: { sessions: { supportsProviderSwitchingViaHandoff: true } } }];
}

test('a model change previews a required context handoff without modifying history', async () => {
  const { client } = await opened();
  const ready = { enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' } };
  client.config.providers = [{ instanceId: 'codex', driver: 'codex', ...ready, models: [{ slug: 'model-a', name: 'Model A' }], continuation: { groupKey: 'a' } },
    { instanceId: 'other', driver: 'codex', ...ready, models: [{ slug: 'model-b', name: 'Model B' }], continuation: { groupKey: 'b' } }];
  client.thread!.projection.runs = [{ id: 'r', startedAt: '2026-10-03T00:00:00Z', status: 'completed', modelSelection: { instanceId: 'codex', model: 'model-a' } }];
  handsOff(client.thread!.projection);
  const selection = { instanceId: 'other', model: 'model-b' };
  expect(pendingHandoff(client.projection, selection, client.config.providers as Obj[])).toMatchObject({ from: { instanceId: 'codex' }, to: selection });
  client.providerId = 'other'; client.modelId = 'model-b';
  expect(threadFooterRows(client, 0)[0]).toMatchObject({ kind: 'handoff', title: 'Context handoff pending', detail: 'Model A → Model B',
    activities: [{ label: 'Model A', detail: 'codex' }, { label: 'Model B', detail: 'codex' }] });
  expect(client.projection.contextHandoffs).toEqual([]);
  expect(pendingHandoff(client.projection, { instanceId: 'codex', model: 'model-b' }, client.config.providers as Obj[])).toBeNull();
});

test('no pending handoff for a provider the next send would skip (a9af48a3d2); accounts show their badge (27b0dbb30e)', async () => {
  const { client } = await opened();
  const ready = { enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' } };
  client.config.providers = [{ instanceId: 'codex', driver: 'codex', displayName: 'Personal', ...ready, models: [{ slug: 'model-a', name: 'Model A' }], continuation: { groupKey: 'a' } },
    { instanceId: 'codex-work', driver: 'codex', displayName: 'Work', accentColor: '#2563eb', ...ready, models: [{ slug: 'model-b', name: 'Model B' }], continuation: { groupKey: 'b' } }];
  client.thread!.projection.runs = [{ id: 'r', startedAt: '2026-10-03T00:00:00Z', status: 'completed', modelSelection: { instanceId: 'codex', model: 'model-a' } }];
  handsOff(client.thread!.projection);
  client.providerId = 'codex-work'; client.modelId = 'model-b';
  expect(threadFooterRows(client, 0)[0]).toMatchObject({ title: 'Context handoff pending',
    activities: [{ label: 'Model A', badge: 'PE', badgeColor: '' }, { label: 'Model B', badge: 'WO', badgeColor: '#2563eb' }] });
  (client.config.providers as Obj[])[1]!.enabled = false;
  expect(threadFooterRows(client, 0).some(row => row.kind === 'handoff')).toBe(false);
  (client.config.providers as Obj[])[1]!.enabled = true;
  (client.config.providers as Obj[])[1]!.availability = 'unavailable';
  expect(threadFooterRows(client, 0).some(row => row.kind === 'handoff')).toBe(false);
});

test('the context meter takes the newest usage report regardless of provider turn order and rounds across a million', () => {
  expect(contextSnapshot({ providerTurns: [{ tokenUsage: { usedTokens: 120_000, updatedAt: '2026-10-03T12:00:00Z', promptCacheTtlMs: 300_000 } },
    { tokenUsage: { usedTokens: 1, updatedAt: '2026-10-03T11:00:00Z' } }] })).toMatchObject({ used: 120_000, promptCacheTtlMs: 300_000 });
  expect([999_499, 999_500, 1_000_000].map(formatTokens)).toEqual(['999k', '1m', '1m']);
});

// contextWindow.ts at 74541bab17: a run fork keeps its source's usage (#22075); live usage carries the configured threshold (#18332).
describe('context meter usage', () => {
  const sourceUsage = { usedTokens: 513_000, maxTokens: 1_000_000, promptCacheTtlMs: 300_000, updatedAt: '2026-10-19T12:00:00.000Z' };
  const transfer = { id: 'x', type: 'fork', sourceThreadId: 'parent', targetThreadId: 't1', sourcePoint: { threadId: 'parent', runId: 'fork-point' },
    sourceProviderInstanceId: 'claude', sourceTokenUsage: sourceUsage, status: 'pending', resolution: null };
  const fork = (extra: Obj = {}): Obj => ({ thread: { id: 't1', forkedFrom: { type: 'run', threadId: 'parent', runId: 'fork-point' }, modelSelection: { instanceId: 'claude' }, activeProviderThreadId: 'pt' },
    providerTurns: [], providerThreads: [], contextTransfers: [transfer], visibleTurnItems: [], ...extra });
  test('a run fork shows its fork point\'s usage and cache expiry until it reports its own', () => {
    expect(contextSnapshot(fork())).toMatchObject({ used: 513_000, max: 1_000_000, promptCacheTtlMs: 300_000, updatedAt: sourceUsage.updatedAt });
    expect(contextSnapshot(fork({ contextTransfers: [{ ...transfer, status: 'consumed', resolution: { strategy: 'native_fork' } }] }))).toMatchObject({ used: 513_000 });
    expect(contextSnapshot(fork({ providerTurns: [{ tokenUsage: { usedTokens: 42_000, updatedAt: '2026-10-19T12:10:00.000Z' } }] }))).toMatchObject({ used: 42_000 });
    // The target provider thread's own report wins over inherited usage, even at zero.
    expect(contextSnapshot(fork({ providerThreads: [{ id: 'pt', contextUsage: { usedTokens: 0, maxTokens: 200_000 }, updatedAt: '2026-10-19T12:10:00.000Z' }] }))).toMatchObject({ used: 0, max: 200_000 });
  });
  test('a portable handoff, another provider or a different fork point inherits nothing', () => {
    for (const changed of [{ status: 'consumed', resolution: { strategy: 'portable_handoff' } }, { sourceProviderInstanceId: 'codex' }, { sourcePoint: { threadId: 'parent', runId: 'other' } }]) {
      expect(contextSnapshot(fork({ contextTransfers: [{ ...transfer, ...changed }] }))).toBeNull();
    }
  });
  test('live usage carries the configured auto-compact threshold', () => {
    expect(contextSnapshot({ providerTurns: [{ tokenUsage: { usedTokens: 1, updatedAt: 'x', autoCompactThreshold: 150_000 } }] })).toMatchObject({ threshold: 150_000 });
    expect(contextSnapshot({ providerTurns: [{ tokenUsage: { usedTokens: 1, updatedAt: 'x' } }] })).toMatchObject({ threshold: null });
  });
});
