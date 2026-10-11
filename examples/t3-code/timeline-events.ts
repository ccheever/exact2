// Lifecycle rows adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/components/chat/V2LifecycleRow.tsx (interrupts, handoffs,
// forks and subagents) and TimelineSystemDivider.tsx.
import { obj, str, type Activity, type Message, type Obj } from './domain';
import { resolveProviderInstanceAcpRegistryIconUrl } from './acp-icons';
import { instanceInitials } from './usage-pooled-view';
import { formatElapsedSeconds, subagentElapsedMs, workActive } from './timeline-work-rows';

const STATUS: Record<string, string> = {
  pending: 'Queued', running: 'Running', waiting: 'Waiting', idle: 'Idle · resumable', completed: 'Completed',
  failed: 'Failed', cancelled: 'Stopped', interrupted: 'Stopped',
};
const SETTLED = new Set(['completed', 'failed', 'cancelled', 'interrupted']);

export function subagentTitle(title: string): string {
  const display = title.replace(/^Subagent:\s*/i, '');
  const path = /^\/root\/(?:[^/]+\/)*([^/]+)\/?$/u.exec(display);
  if (!path) return display;
  return path[1]!.replace(/[_\s]+/gu, ' ').trim().replace(/(^|\s)\S/gu, letter => letter.toUpperCase()) || display;
}
function plainDetail(text: string): string {
  return text.replace(/\[([^\]]+)\]\([^)]*\)/g, '$1').replace(/`/g, '').replace(/^[ \t]*[-*][ \t]+/gm, '').replace(/\s+/g, ' ').trim();
}
/** What a subagent card reads beside its item: the parent's subagent records and the server's providers. */
export interface SubagentContext { agents: Obj[]; providers: Obj[] }
const NO_CONTEXT: SubagentContext = { agents: [], providers: [] };

/**
 * One subagent (SubagentTimelineLink): title, status (Queued … Stopped), its last progress or result
 * and its elapsed time, read from the parent's record of it where there is one. SubagentAvatar draws
 * the driver's ProviderInstanceIcon (`detail`, `initials`, `driverIcon`). A live agent's elapsed time
 * ticks on the timeline clock from `startedMs` (AgentElapsed); a settled one is fixed in `output`.
 */
export function subagent(item: Obj, context: SubagentContext = NO_CONTEXT): Activity & { startedMs: number; initials: string; driverIcon: string } {
  const agent = item.subagentId === undefined ? undefined : context.agents.find(candidate => candidate.id === item.subagentId);
  const status = str(agent?.status ?? item.status), settled = SETTLED.has(status);
  const result = str(agent?.result ?? item.result).trim(), progress = str(agent?.progress ?? item.progress).trim();
  const raw = settled ? result || progress : progress || result;
  const detail = raw && !/^Child task ended with status\b/i.test(raw) ? plainDetail(raw) : '';
  const timing = { status, startedAt: agent?.startedAt ?? item.startedAt, completedAt: agent?.completedAt ?? item.completedAt };
  const startedMs = workActive(status) && typeof timing.startedAt === 'string' ? Date.parse(timing.startedAt) : NaN;
  const elapsed = Number.isFinite(startedMs) ? null : subagentElapsedMs(timing, 0);
  const driver = str(item.driver), provider = context.providers.find(candidate => candidate.instanceId === item.providerInstanceId);
  return { id: str(item.id), label: subagentTitle(str(item.title, 'Subagent')), body: detail, icon: 'bot', output: elapsed === null ? '' : formatElapsedSeconds(elapsed / 1000),
    result: STATUS[status] ?? status, failed: status === 'failed', timestamp: str(item.startedAt ?? item.updatedAt),
    tone: status === 'failed' ? 'error' : status === 'completed' ? 'success' : settled || status === 'idle' ? 'muted' : 'info',
    ok: true, reasoning: false, expandable: false, detail: driver, status, targetId: str(item.childThreadId), answer: '',
    startedMs: Number.isFinite(startedMs) ? startedMs : 0, initials: instanceInitials(str(provider?.displayName, driver)),
    driverIcon: resolveProviderInstanceAcpRegistryIconUrl({ driverKind: driver, iconUrl: str(provider?.iconUrl) || undefined }) ?? '' };
}

/** A lifecycle item as a timeline row (V2LifecycleRow). */
export function eventRow(item: Obj, group: Obj[], row: Obj, context: SubagentContext = NO_CONTEXT): Partial<Message> & { kind: string } {
  const type = str(item.type);
  switch (type) {
    case 'run_interrupt_result': return { kind: 'event', title: 'Run interrupted', detail: str(item.message), tone: 'danger', icon: 'x', runId: str(item.runId) };
    case 'handoff': return { kind: 'event', title: 'Context handoff', tone: item.status === 'failed' ? 'danger' : 'neutral', icon: 'arrow-right-left',
      detail: `${(Array.isArray(item.fromProviderInstanceIds) ? item.fromProviderInstanceIds : []).map(value => str(value)).join(', ')} → ${str(item.toProviderInstanceId)}`,
      runId: str(item.runId) };
    case 'fork': {
      const source = obj(item.source), run = source.type === 'run';
      return { kind: 'fork', body: run ? 'Forked from conversation' : 'Conversation fork', actionLabel: run ? 'Open source conversation' : 'Open fork',
        targetId: run ? str(source.threadId) : str(item.targetThreadId), sourceThreadId: str(row.sourceThreadId) };
    }
    case 'subagent': {
      const members = (group.length ? group : [item]).map(member => subagent(member, context));
      return { kind: members.length > 1 ? 'subagents' : 'subagent', title: members.length > 1 ? `${members.length} subagents` : members[0]!.label,
        activities: members, runId: str(item.runId) };
    }
    default: return { kind: 'event', title: str(item.title).trim() || type.replace(/_/g, ' '), detail: '', tone: item.status === 'failed' ? 'danger' : 'neutral', icon: 'wrench' };
  }
}
