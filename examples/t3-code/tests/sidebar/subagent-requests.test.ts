import { expect, test } from 'bun:test';
import { childThreadInputs, childThreadInputsTitle, noteSubagentRequests, pendingSubagentRequestsByThread } from '../../src/sidebar/subagent-requests';
import { canSnooze, raisedHand, sidebarStatus } from '../../src/sidebar/sidebar-model';
import { composerNotices, contextSnapshot, formatTokens } from '../../src/composer/composer-controls-view';
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
  expect([...pendingSubagentRequestsByThread(client.shell.threads)]).toEqual([['t1', 'approval'], ['branch', 'approval']]);
  expect(sidebarStatus(parent)).toBe('approval');
  expect(canSnooze(parent, 0)).toBe(false);
  expect(raisedHand(parent)).toBe(true);
  expect(childThreadInputsTitle(childThreadInputs(client.shell.threads, 't1'))).toBe('2 subagents need you');
  expect(composerNotices(client, 0)[0]).toMatchObject({ title: '2 subagents need you', action: 'select-thread', actionId: 'branch' });
  branch.activityRunStatus = null; leaf.activityRunStatus = null;
  client.shell.threads = [...client.shell.threads];
  noteSubagentRequests(client.shell.threads);
  expect(sidebarStatus(parent)).not.toBe('approval');
  expect(composerNotices(client, 0).some(notice => notice.id.startsWith('child-input:'))).toBe(false);
});

test('a parent request takes precedence and an auth refresh never raises attention', async () => {
  const { client } = await opened();
  client.shell.threads = [...client.shell.threads, child('q', 't1', 'user_input'), child('auth', 't1', 'auth_refresh')];
  expect(childThreadInputs(client.shell.threads, 't1').map(entry => entry.id)).toEqual(['q']);
  client.thread!.projection.runtimeRequests = [{ id: 'a', status: 'pending', kind: 'command', responseCapability: { type: 'live' } }];
  expect(composerNotices(client, 0).some(notice => notice.id.startsWith('child-input:'))).toBe(false);
});

test('a model change previews a required context handoff without modifying history', async () => {
  const { client } = await opened();
  client.config.providers = [{ instanceId: 'codex', driver: 'codex', models: [{ slug: 'model-a', name: 'Model A' }], continuation: { groupKey: 'a' } },
    { instanceId: 'other', driver: 'codex', models: [{ slug: 'model-b', name: 'Model B' }], continuation: { groupKey: 'b' } }];
  client.thread!.projection.runs = [{ id: 'r', startedAt: '2026-10-03T00:00:00Z', status: 'completed', modelSelection: { instanceId: 'codex', model: 'model-a' } }];
  const selection = { instanceId: 'other', model: 'model-b' };
  expect(pendingHandoff(client.projection, selection, client.config.providers as Obj[])).toMatchObject({ from: { instanceId: 'codex' }, to: selection });
  client.providerId = 'other'; client.modelId = 'model-b';
  expect(threadFooterRows(client, 0)[0]).toMatchObject({ kind: 'handoff', title: 'Context handoff pending', detail: 'Model A → Model B',
    activities: [{ label: 'Model A', detail: 'codex' }, { label: 'Model B', detail: 'codex' }] });
  expect(client.projection.contextHandoffs).toEqual([]);
  expect(pendingHandoff(client.projection, { instanceId: 'codex', model: 'model-b' }, client.config.providers as Obj[])).toBeNull();
});

test('the context meter takes the newest usage report regardless of provider turn order and rounds across a million', () => {
  expect(contextSnapshot({ providerTurns: [{ tokenUsage: { usedTokens: 120_000, updatedAt: '2026-10-03T12:00:00Z', promptCacheTtlMs: 300_000 } },
    { tokenUsage: { usedTokens: 1, updatedAt: '2026-10-03T11:00:00Z' } }] })).toMatchObject({ used: 120_000, promptCacheTtlMs: 300_000 });
  expect([999_499, 999_500, 1_000_000].map(formatTokens)).toEqual(['999k', '1m', '1m']);
});
