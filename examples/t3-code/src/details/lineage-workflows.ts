// T3 Code 9c161462c9 (MIT, see LICENSE-T3): state/subagentRuntime.ts,
// projectedSubagentsToRuntime and deriveWorkflowGroups. A workflow's declared phases can lag its members.
import { arr, obj, str, type Obj } from '../client/domain';
import { agentElapsed, statusDot, statusLabel } from './shell-lineage';
import { subagentCard } from '../sidebar/subagent-card';

export type WorkflowAgent = Obj & {
  id: string; kind: 'subagent' | 'workflow' | 'workflow_agent'; title: string; status: string;
  model: string | null; activationCount: number; usage: Obj | null; progress: string | null;
  result: string | null; error: string | null; parentAgentId: string | null;
  agentIndex: number | null; phaseIndex: number | null; phaseTitle: string | null; attempt: number | null;
  workflowName: string | null; phases: { index: number; title: string }[]; runHandles: Obj | null;
  childThreadId: string | null; firstSeenAt: string; startedAt: string | null; completedAt: string | null; updatedAt: string;
};
export type WorkflowPhase = {
  index: number; title: string; members: WorkflowAgent[]; state: 'pending' | 'running' | 'done'; settledCount: number;
};
export type WorkflowGroup = { workflow: WorkflowAgent; phases: WorkflowPhase[]; unphasedMembers: WorkflowAgent[] };

const TERMINAL = new Set(['completed', 'failed', 'cancelled', 'interrupted']);
const ACTIVE = new Set(['pending', 'running', 'waiting']);
export const terminalWorkflowAgent = (status: string) => TERMINAL.has(status);
const epoch = (value: unknown): string | null => typeof value === 'number' ? new Date(value).toISOString() : null;
const optionalNumber = (value: unknown): number | null => typeof value === 'number' ? value : null;
const optionalText = (value: unknown): string | null => typeof value === 'string' ? value : null;
function usage(source: Obj): Obj | null {
  if (source.totalTokens === undefined) return null;
  return { totalTokens: source.totalTokens, ...(source.toolCalls === undefined ? {} : { toolUses: source.toolCalls }),
    ...(source.durationMs === undefined ? {} : { durationMs: source.durationMs }) };
}

export function projectedWorkflowAgents(subagents: Obj[]): WorkflowAgent[] {
  return subagents.flatMap(subagent => {
    const workflow = subagent.workflow == null ? null : obj(subagent.workflow);
    const startedAt = optionalText(subagent.startedAt), updatedAt = str(subagent.updatedAt), firstSeenAt = startedAt ?? updatedAt;
    const prompt = str(subagent.prompt), status = str(subagent.status);
    const coordinator: WorkflowAgent = {
      ...subagent, id: str(subagent.id), kind: workflow ? 'workflow' : 'subagent',
      title: optionalText(subagent.title) ?? (prompt.length > 80 ? `${prompt.slice(0, 77)}...` : prompt), model: optionalText(subagent.model), status,
      activationCount: 1, usage: workflow ? usage(workflow) : null, progress: optionalText(subagent.progress), result: optionalText(subagent.result),
      error: status === 'failed' ? optionalText(subagent.result) : null, parentAgentId: null, agentIndex: null, phaseIndex: null, phaseTitle: null, attempt: null,
      workflowName: optionalText(workflow?.name), phases: arr(workflow?.phases).map(phase => ({ index: Number(phase.index), title: str(phase.title) })),
      runHandles: workflow?.runHandles == null ? null : obj(workflow.runHandles), childThreadId: optionalText(subagent.childThreadId),
      firstSeenAt, startedAt, completedAt: optionalText(subagent.completedAt), updatedAt,
    };
    if (!workflow) return [coordinator];
    const members: WorkflowAgent[] = arr(workflow.agents).map(agent => {
      const status = (agent.state === 'queued' || agent.state === 'running') && TERMINAL.has(coordinator.status)
        ? coordinator.status : agent.state === 'queued' ? 'pending' : str(agent.state);
      const failed = status === 'failed', startedAt = epoch(agent.startedAt);
      const completedAt = TERMINAL.has(status)
        ? epoch(agent.completedAt) ?? (typeof agent.startedAt === 'number' && typeof agent.durationMs === 'number' ? epoch(agent.startedAt + agent.durationMs) : coordinator.completedAt)
        : null;
      return {
        ...coordinator, id: `${coordinator.id}:agent:${agent.index}`, kind: 'workflow_agent', title: str(agent.label), model: optionalText(agent.model), status,
        activationCount: typeof agent.attempt === 'number' ? agent.attempt : 1, usage: usage(agent), progress: optionalText(agent.prompt),
        result: failed ? null : optionalText(agent.result), error: failed ? optionalText(agent.result) : null, parentAgentId: coordinator.id,
        agentIndex: optionalNumber(agent.index), phaseIndex: optionalNumber(agent.phaseIndex), phaseTitle: optionalText(agent.phaseTitle), attempt: optionalNumber(agent.attempt),
        phases: [], childThreadId: optionalText(agent.childThreadId), firstSeenAt: epoch(agent.queuedAt) ?? startedAt ?? coordinator.firstSeenAt,
        startedAt, completedAt, updatedAt: completedAt ?? startedAt ?? coordinator.firstSeenAt,
      };
    });
    return [coordinator, ...members];
  });
}

export function workflowGroups(agents: WorkflowAgent[]): WorkflowGroup[] {
  const workflows = agents.filter(agent => agent.kind === 'workflow').sort((a, b) => a.firstSeenAt.localeCompare(b.firstSeenAt) || a.id.localeCompare(b.id));
  return workflows.map(workflow => {
    const members = agents.filter(agent => agent.kind !== 'workflow' && agent.parentAgentId === workflow.id);
    const phaseTitles = new Map<number, string>();
    for (const member of members) if (member.phaseIndex !== null) phaseTitles.set(member.phaseIndex, member.phaseTitle ?? phaseTitles.get(member.phaseIndex) ?? `Phase ${member.phaseIndex + 1}`);
    for (const phase of workflow.phases) phaseTitles.set(phase.index, phase.title);
    const byAgentIndex = (a: WorkflowAgent, b: WorkflowAgent) => (a.agentIndex ?? 0) - (b.agentIndex ?? 0);
    const phases = [...phaseTitles].sort(([a], [b]) => a - b).map(([index, title]): WorkflowPhase => {
      const phaseMembers = members.filter(member => member.phaseIndex === index).sort(byAgentIndex);
      const active = phaseMembers.some(member => ACTIVE.has(member.status) || member.status === 'idle');
      return { index, title, members: phaseMembers, state: phaseMembers.length === 0 ? 'pending' : active ? 'running' : 'done',
        settledCount: phaseMembers.filter(member => TERMINAL.has(member.status)).length };
    });
    return { workflow, phases, unphasedMembers: members.filter(member => member.phaseIndex === null).sort(byAgentIndex) };
  });
}

/** ThreadLineageWorkflowRow's phase timer covers its earliest start through its latest completion. */
export function workflowPhaseTiming(phase: WorkflowPhase) {
  const instants = (key: 'startedAt' | 'completedAt') => phase.members.map(member => member[key]).filter((value): value is string => value !== null).sort();
  const running = phase.state === 'running', startedAt = instants('startedAt')[0] ?? null;
  const completedAt = running ? null : instants('completedAt').at(-1) ?? null;
  return { status: running ? 'running' : 'completed', startedAt: running || completedAt !== null ? startedAt : null, completedAt };
}
export function workflowPhaseStatus(phase: WorkflowPhase): string {
  if (phase.state === 'running') return 'running';
  if (phase.members.some(member => member.status === 'failed')) return 'failed';
  if (phase.members.some(member => member.status === 'cancelled' || member.status === 'interrupted')) return 'stopped';
  return phase.state === 'done' ? 'done' : 'not started';
}


/** Workflow body rows use each member's reported model and provider, without borrowing the coordinator's traits. */
export function workflowBody(group: WorkflowGroup, threads: Obj[], providers: Obj[], now: number) {
  const providerInstanceId = str(group.workflow.providerInstanceId), provider = providers.find(entry => entry.instanceId === providerInstanceId);
  const driver = str(provider?.driver) || str(group.workflow.driver);
  const member = (agent: WorkflowAgent) => {
    const elapsed = agentElapsed(agent, now), childThreadId = agent.childThreadId ?? '';
    return { id: agent.id, title: agent.title, childThreadId, disabled: !childThreadId || !threads.some(thread => thread.id === childThreadId),
      driver, dot: statusDot(agent.status), status: statusLabel(agent.status), elapsed,
      card: subagentCard({ title: agent.title, model: agent.model, providerInstanceId, origin: 'provider_native', provider, providers, driver,
        elapsed, status: agent.status, result: agent.result ?? agent.error, progress: agent.progress }) };
  };
  const phases = group.phases.filter(phase => phase.members.length > 0).map(phase => ({
    id: `${group.workflow.id}:phase:${phase.index}`, index: phase.index, title: phase.title, status: workflowPhaseStatus(phase),
    running: phase.state === 'running', failed: workflowPhaseStatus(phase) === 'failed',
    count: `${phase.settledCount}/${phase.members.length} ${phase.members.length === 1 ? 'agent' : 'agents'}`,
    elapsed: agentElapsed(workflowPhaseTiming(phase), now), members: phase.members.map(member),
  }));
  const unphased = group.unphasedMembers.map(member);
  return { id: group.workflow.id, title: group.workflow.workflowName ?? group.workflow.title, phases, unphased, empty: phases.length === 0 && unphased.length === 0 };
}

/** Provider-native Stop targets a strongly identified live Claude task, never its owner turn. */
export function canStopNativeSubagent(agent: Obj | undefined, nativeAgent: Obj | undefined = agent): boolean {
  const ref = obj(nativeAgent?.nativeTaskRef);
  return !!agent && !!nativeAgent && nativeAgent.origin === 'provider_native' && nativeAgent.driver === 'claudeAgent'
    && !!str(nativeAgent.runId) && ref.strength === 'strong' && !!str(ref.nativeId)
    && ACTIVE.has(str(nativeAgent.status)) && !!agent.startedAt && ACTIVE.has(str(agent.status));
}
export function canStopWorkflow(agent: Obj, canOperate: boolean, driver = str(agent.driver)): boolean {
  return canOperate && agent.status === 'running' && agent.origin === 'provider_native' && driver === 'claudeAgent';
}
