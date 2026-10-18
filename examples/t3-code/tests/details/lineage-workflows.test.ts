import { expect, test } from 'bun:test';
import { projectedWorkflowAgents, workflowGroups, workflowPhaseStatus, workflowPhaseTiming, workflowBody, canStopNativeSubagent, canStopWorkflow } from '../../src/details/lineage-workflows';
import type { Obj } from '../../src/client/domain';

const started = '2026-10-18T12:00:00.000Z', ended = '2026-10-18T12:02:00.000Z', at = Date.parse(started);
const coordinator = (overrides: Obj = {}): Obj => ({ id: 'workflow', title: null, prompt: 'Review the change', status: 'running', model: 'opus',
  driver: 'claudeAgent', origin: 'provider_native', providerInstanceId: 'claude', startedAt: started, completedAt: null, updatedAt: started,
  workflow: { name: 'Parallel review', phases: [], agents: [] }, ...overrides });

test('workflow members retain phase, attempts, usage, child links and completion timing', () => {
  const agents = projectedWorkflowAgents([coordinator({ workflow: { name: 'Parallel review', phases: [{ index: 0, title: 'Read' }], runHandles: { scriptPath: '/fixture/review.ts' },
    totalTokens: 21, toolCalls: 2, durationMs: 900,
    agents: [{ index: 2, label: 'Inspect parser', state: 'completed', model: 'sonnet', attempt: 3, phaseIndex: 0, phaseTitle: 'Read',
      childThreadId: 'child', totalTokens: 12, toolCalls: 1, durationMs: 1500, queuedAt: at - 500, startedAt: at, prompt: 'Read the parser', result: 'Looks good' }] } })]);
  expect(agents[0]).toMatchObject({ kind: 'workflow', workflowName: 'Parallel review', usage: { totalTokens: 21, toolUses: 2, durationMs: 900 }, runHandles: { scriptPath: '/fixture/review.ts' } });
  expect(agents[1]).toMatchObject({ id: 'workflow:agent:2', kind: 'workflow_agent', title: 'Inspect parser', model: 'sonnet', activationCount: 3,
    parentAgentId: 'workflow', agentIndex: 2, phaseIndex: 0, phaseTitle: 'Read', attempt: 3, childThreadId: 'child', progress: 'Read the parser', result: 'Looks good', error: null,
    usage: { totalTokens: 12, toolUses: 1, durationMs: 1500 }, firstSeenAt: new Date(at - 500).toISOString(), startedAt: started, completedAt: new Date(at + 1500).toISOString() });
});

test('terminal coordinators settle queued and running members without changing completed members', () => {
  for (const status of ['completed', 'failed', 'cancelled', 'interrupted']) {
    const agents = projectedWorkflowAgents([coordinator({ status, completedAt: ended, workflow: { phases: [], agents: [
      { index: 0, label: 'Queued', state: 'queued', result: 'Stopped before start' },
      { index: 1, label: 'Running', state: 'running', startedAt: at, result: 'Stopped while running' },
      { index: 2, label: 'Done', state: 'completed', startedAt: at, durationMs: 10, result: 'Done' },
    ] } })]);
    expect(agents.slice(1).map(agent => agent.status)).toEqual([status, status, 'completed']);
    expect(agents[1]?.completedAt).toBe(ended);
    expect(agents[2]?.completedAt).toBe(ended);
    expect(agents[3]?.completedAt).toBe(new Date(at + 10).toISOString());
    expect(agents[2]?.error).toBe(status === 'failed' ? 'Stopped while running' : null);
    expect(agents[2]?.result).toBe(status === 'failed' ? null : 'Stopped while running');
  }
  const pending = projectedWorkflowAgents([coordinator({ workflow: { phases: [], agents: [{ index: 0, label: 'Next', state: 'queued', durationMs: 99 }] } })])[1];
  expect(pending).toMatchObject({ status: 'pending', completedAt: null, usage: null, activationCount: 1 });
});

test('declared phases and member phases form a sorted union and declared names win', () => {
  const agents = projectedWorkflowAgents([coordinator({ workflow: { phases: [{ index: 0, title: 'Declared read' }, { index: 3, title: 'Later' }], agents: [
    { index: 4, label: 'Fourth', state: 'completed', phaseIndex: 1, phaseTitle: 'Member title' },
    { index: 2, label: 'Second', state: 'running', phaseIndex: 0, phaseTitle: 'Guessed title' },
    { index: 1, label: 'First', state: 'failed', phaseIndex: 0 },
    { index: 3, label: 'Unphased 3', state: 'queued' }, { index: 0, label: 'Unphased 0', state: 'queued' },
  ] } })]);
  const group = workflowGroups(agents)[0]!;
  expect(group.phases.map(phase => [phase.index, phase.title, phase.state, phase.settledCount, phase.members.map(member => member.agentIndex)]))
    .toEqual([[0, 'Declared read', 'running', 1, [1, 2]], [1, 'Member title', 'done', 1, [4]], [3, 'Later', 'pending', 0, []]]);
  expect(group.unphasedMembers.map(member => member.agentIndex)).toEqual([0, 3]);
});

test('idle members keep phases live and workflows sort by first sight then id', () => {
  const agents = projectedWorkflowAgents([coordinator({ id: 'b' }), coordinator({ id: 'a', workflow: { phases: [], agents: [{ index: 0, label: 'Idle', state: 'running', phaseIndex: 2 }] } }),
    coordinator({ id: 'earlier', startedAt: new Date(at - 1).toISOString() }), { id: 'ordinary', title: null, prompt: 'x'.repeat(81), status: 'completed', startedAt: null, updatedAt: ended }]);
  agents.find(agent => agent.kind === 'workflow_agent')!.status = 'idle';
  const groups = workflowGroups(agents);
  expect(groups.map(group => group.workflow.id)).toEqual(['earlier', 'a', 'b']);
  expect(groups[1]?.phases[0]).toMatchObject({ index: 2, title: 'Phase 3', state: 'running', settledCount: 0 });
  expect(agents.find(agent => agent.id === 'ordinary')).toMatchObject({ kind: 'subagent', title: `${'x'.repeat(77)}...`, firstSeenAt: ended, workflowName: null, usage: null });
});


test('phase timing spans its members and status prioritizes live work, failure, then stopped', () => {
  const members = projectedWorkflowAgents([coordinator({ workflow: { phases: [], agents: [
    { index: 0, label: 'First', state: 'completed', startedAt: at, durationMs: 1000, phaseIndex: 0 },
    { index: 1, label: 'Second', state: 'failed', startedAt: at + 500, durationMs: 2000, phaseIndex: 0 },
  ] } })]);
  const phase = workflowGroups(members)[0]!.phases[0]!;
  expect(workflowPhaseTiming(phase)).toEqual({ status: 'completed', startedAt: started, completedAt: new Date(at + 2500).toISOString() });
  expect(workflowPhaseStatus(phase)).toBe('failed');
  expect(workflowPhaseStatus({ ...phase, state: 'running' })).toBe('running');
  expect(workflowPhaseTiming({ ...phase, state: 'running' })).toEqual({ status: 'running', startedAt: started, completedAt: null });
  phase.members[1]!.status = 'interrupted';
  expect(workflowPhaseStatus(phase)).toBe('stopped');
  phase.members[1]!.status = 'completed';
  expect(workflowPhaseStatus(phase)).toBe('done');
  expect(workflowPhaseStatus({ ...phase, state: 'pending', members: [] })).toBe('not started');
  phase.members.forEach(member => { member.completedAt = null; });
  expect(workflowPhaseTiming(phase)).toEqual({ status: 'completed', startedAt: null, completedAt: null });
});


test('workflow bodies hide empty phases and enable only available child chats', () => {
  const agents = projectedWorkflowAgents([coordinator({ modelSelection: { instanceId: 'claude', model: 'opus', options: [{ id: 'effort', value: 'high' }] },
    workflow: { name: 'Review', phases: [{ index: 0, title: 'Read' }, { index: 1, title: 'Later' }], agents: [
      { index: 0, label: 'Readable', state: 'completed', phaseIndex: 0, startedAt: at, durationMs: 70000, model: 'sonnet', childThreadId: 'available', result: 'All good' },
      { index: 1, label: 'Unavailable', state: 'failed', phaseIndex: 0, startedAt: at, durationMs: 30000, model: 'opus', childThreadId: 'missing', result: 'Could not read' },
      { index: 2, label: 'No child yet', state: 'queued' },
    ] } })]);
  const body = workflowBody(workflowGroups(agents)[0]!, [{ id: 'available' }], [{ instanceId: 'claude', driver: 'claudeAgent', models: [{ slug: 'sonnet', name: 'Sonnet' }, { slug: 'opus', name: 'Opus' }] }], at + 90000);
  expect(body).toMatchObject({ title: 'Review', empty: false });
  expect(body.phases).toHaveLength(1);
  expect(body.phases[0]).toMatchObject({ title: 'Read', status: 'failed', running: false, failed: true, count: '2/2 agents', elapsed: '1m 10s' });
  expect(body.phases[0]!.members.map(member => [member.title, member.disabled, member.elapsed])).toEqual([['Readable', false, '1m 10s'], ['Unavailable', true, '30s']]);
  expect(body.phases[0]!.members[0]!.card).toMatchObject({ modelLine: 'Sonnet', effort: '', speed: '', preview: 'All good' });
  expect(body.phases[0]!.members[1]!.card).toMatchObject({ modelLine: 'Opus', effort: '', preview: 'Could not read' });
  expect(body.unphased[0]).toMatchObject({ title: 'No child yet', disabled: true, elapsed: '' });
  expect(workflowBody(workflowGroups(projectedWorkflowAgents([coordinator()]))[0]!, [], [], at)).toMatchObject({ phases: [], unphased: [], empty: true });
});


test('native Stop requires a live strongly identified Claude task and workflows require operation permission', () => {
  const agent = { origin: 'provider_native', driver: 'claudeAgent', runId: 'owner-run', startedAt: started, status: 'running', nativeTaskRef: { strength: 'strong', nativeId: 'task-17' } };
  expect(canStopNativeSubagent(agent)).toBe(true);
  for (const override of [{ origin: 'app_owned' }, { driver: 'codex' }, { runId: null }, { startedAt: null }, { status: 'completed' },
    { nativeTaskRef: null }, { nativeTaskRef: { strength: 'weak', nativeId: 'task-17' } }, { nativeTaskRef: { strength: 'strong', nativeId: null } }]) {
    expect(canStopNativeSubagent({ ...agent, ...override })).toBe(false);
  }
  expect(canStopNativeSubagent({ ...agent, status: 'waiting' }, { ...agent, status: 'completed' })).toBe(false);
  expect(canStopNativeSubagent({ ...agent, status: 'completed' }, agent)).toBe(false);
  expect(canStopNativeSubagent(undefined)).toBe(false);
  expect(canStopWorkflow(agent, true)).toBe(true);
  expect(canStopWorkflow(agent, false)).toBe(false);
  expect(canStopWorkflow({ ...agent, status: 'waiting' }, true)).toBe(false);
  expect(canStopWorkflow({ ...agent, origin: 'app_owned' }, true)).toBe(false);
  expect(canStopWorkflow({ ...agent, driver: 'codex' }, true)).toBe(false);
});
