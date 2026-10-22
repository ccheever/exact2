import { expect, test } from 'bun:test';
import { mobileAgentActivity, projectMobileAgentActivity } from './agent-activity';
import { mobileClient } from './client';
import { initialShell, type Obj } from './shared/domain';
const now = Date.parse('2026-10-07T12:00:00Z');
const at = (seconds: number) => new Date(now + seconds * 1000).toISOString();
const agent = (id: string, patch: Obj = {}) => ({ id, runId: 'run', threadId: 'parent', childThreadId: null, title: '', prompt: 'Check code', status: 'running', startedAt: at(-10), updatedAt: at(0), completedAt: null, ...patch });
const view = (subagents: Obj[], runs: Obj[] = []) => projectMobileAgentActivity('env', { subagents, runs }, initialShell(), {}, now);
test('active run roster wins; a new active run with no agents does not leak the previous roster', () => {
  const agents = [agent('old', { runId: 'old', updatedAt: at(20) }), agent('current')];
  expect(view(agents, [{ id: 'run', status: 'running' }]).rows.map(row => row.id)).toEqual(['current']);
  expect(view(agents, [{ id: 'new', status: 'running' }]).rows).toEqual([]);
  expect(view(agents).rows.map(row => row.id)).toEqual(['old']);
});
test('spawn time then id order and live/settled counts preserve idle as neither', () => {
  const result = view([agent('b'), agent('a'), agent('earlier', { startedAt: at(-20), status: 'completed', completedAt: at(-3) }), agent('idle', { status: 'idle' })]);
  expect(result.rows.map(row => row.id)).toEqual(['earlier', 'a', 'b', 'idle']);
  expect(result.liveCount).toBe(2); expect(result.settledCount).toBe(1);
  expect(result.rows[0]?.elapsed).toBe('17s'); expect(result.rows[1]?.elapsed).toBe('10s'); expect(result.rows[3]?.elapsed).toBe('');
});
test('source titles, source status labels, compact details and native read-only semantics', () => {
  const result = view([agent('a', { title: 'Subagent: /root/test_code', progress: '[checking](https://example.test) `code`', result: 'old result' }),
    agent('b', { status: 'failed', result: 'Child task ended with status failed', childThreadId: 'child' }),
    agent('c', { status: 'cancelled', prompt: 'x'.repeat(90), result: 'complete' })]);
  expect(result.rows[0]).toMatchObject({ title: 'Test Code', detail: 'checking code', status: 'Working', tone: 'working', canOpen: false });
  expect(result.rows[1]).toMatchObject({ detail: '', status: 'Failed', tone: 'failed', canOpen: true, childThreadId: 'child' });
  expect(result.rows[2]).toMatchObject({ title: 'x'.repeat(77) + '...', status: 'Cancelled', tone: 'stopped' });
  expect(view([agent('a', { progress: 'a'.repeat(300) })]).rows[0]?.detail).toBe('a'.repeat(280) + '…');
});
test('provider alias resolves shared model helper and workspace metadata uses only held shell', () => {
  const shell = { ...initialShell(), projects: [{ id: 'p1', title: 'Parent', workspaceRoot: '/parent' }, { id: 'p2', title: 'Other', workspaceRoot: '/other' }],
    threads: [{ id: 'parent', projectId: 'p1' }, { id: 'child', projectId: 'p2', worktreePath: '/worktree', branch: 'fix/agents' }] };
  const config = { providers: [{ instanceId: 'codex_personal', driver: 'codex', displayName: 'Codex', models: [{ slug: 'model', aliases: ['ALIAS'], name: 'Brand Long', shortName: 'Brand: Short', subProvider: 'Brand' }] }] };
  const result = projectMobileAgentActivity('env', { runs: [], subagents: [agent('a', { providerInstanceId: 'codex_personal', model: 'alias', childThreadId: 'child' })] }, shell, config, now);
  expect(result.rows[0]?.metadata).toBe('Codex Personal · Short');
  expect(result.rows[0]?.workspace).toEqual([{ label: 'Project', value: 'Other', symbol: 'folder' }, { label: 'Branch', value: 'fix/agents', symbol: 'arrow.triangle.branch' }]);
  expect(view([agent('native', { model: null })]).rows[0]?.metadata).toBe('Not reported');
});
test('zero/invalid/missing elapsed timestamps never manufacture a duration', () => {
  for (const patch of [{ startedAt: null }, { startedAt: 'bad' }, { startedAt: at(0) }, { status: 'completed', completedAt: null }]) expect(view([agent('a', patch)]).rows[0]?.elapsed).toBe('');
});
test('selected route cannot display a retained projection for another thread or environment', () => {
  const before = { environmentId: mobileClient.environmentId, threadId: mobileClient.threadId, thread: mobileClient.thread };
  try {
    mobileClient.environmentId = 'actual'; mobileClient.threadId = 'thread';
    expect(mobileAgentActivity('other', 'thread', now).rows).toEqual([]);
    expect(mobileAgentActivity('actual', 'other-thread', now).rows).toEqual([]);
  } finally { Object.assign(mobileClient, before); }
});
