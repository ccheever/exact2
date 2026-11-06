// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/timeline-events.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lifecycle rows adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/components/chat/V2LifecycleRow.tsx (interrupts, handoffs,
// forks and subagents) and TimelineSystemDivider.tsx.
import { arr, obj, str, type Activity, type Message, type Obj } from './domain';
import { formatDuration } from './timeline-worklog';

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
/** One subagent: title, status (Queued … Stopped), its last progress or result, and its elapsed time. */
export function subagent(item: Obj): Activity {
  const status = str(item.status), settled = SETTLED.has(status);
  const result = str(item.result).trim(), progress = str(item.progress).trim();
  const raw = settled ? result || progress : progress || result;
  const detail = raw && !/^Child task ended with status\b/i.test(raw) ? plainDetail(raw) : '';
  const started = Date.parse(str(item.startedAt)), ended = Date.parse(str(item.completedAt));
  const elapsed = Number.isFinite(started) && settled && Number.isFinite(ended) ? formatDuration(ended - started) : '';
  return { id: str(item.id), label: subagentTitle(str(item.title, 'Subagent')), body: detail, icon: 'bot', output: elapsed,
    result: STATUS[status] ?? status, failed: status === 'failed', timestamp: str(item.startedAt ?? item.updatedAt),
    tone: status === 'failed' ? 'error' : status === 'completed' ? 'success' : settled || status === 'idle' ? 'muted' : 'info',
    ok: true, reasoning: false, expandable: false, detail: str(item.driver), status, targetId: str(item.childThreadId), answer: '' };
}

/** A lifecycle item as a timeline row (V2LifecycleRow). */
export function eventRow(item: Obj, group: Obj[], row: Obj): Partial<Message> & { kind: string } {
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
      const members = (group.length ? group : [item]).map(subagent);
      return { kind: members.length > 1 ? 'subagents' : 'subagent', title: members.length > 1 ? `${members.length} subagents` : members[0]!.label,
        activities: members, runId: str(item.runId) };
    }
    default: return { kind: 'event', title: str(item.title).trim() || type.replace(/_/g, ' '), detail: '', tone: item.status === 'failed' ? 'danger' : 'neutral', icon: 'wrench' };
  }
}
export const subagentRows = (items: Obj[]) => arr(items).map(subagent);
