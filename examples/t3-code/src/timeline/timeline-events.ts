// Lifecycle rows adapted from T3 Code (MIT, see LICENSE-T3):
// apps/web/src/components/chat/V2LifecycleRow.tsx (interrupts, handoffs,
// forks and subagents; HandoffRow and HandoffEndpoint at 9c161462c9), TimelineV2EventRow.tsx's
// V2SubagentGroup and its .logic.ts subagentGroupTiming at 74541bab17, client-runtime state/subagentDisplay.ts,
// TimelineSystemDivider.tsx, packages/client-runtime/src/handoff.ts
// (resolveHandoffEndpoints) and ProviderInstanceIcon.tsx (providerTextColor).
import { subagentCard } from '../sidebar/subagent-card';
import { arr, obj, str, type Activity, type Message, type Obj } from '../client/domain';
import { triggerModelName } from '../composer/composer-controls-model';

import { resolveProviderInstanceAcpRegistryIconUrl } from '../providers/acp-icons';
import { instanceDisplayName } from '../sidebar/subagent-card';
import { providerBadge, providerInstanceInitials } from '../client/presentation';
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
/** What a lifecycle row reads beside its item: the parent's subagent records, the server's providers and the thread's runs. */
export interface SubagentContext { agents: Obj[]; providers: Obj[]; runs?: Obj[]; threads?: Obj[] }

const NO_CONTEXT: SubagentContext = { agents: [], providers: [] };

interface HandoffEndpoint { instanceId: string; model?: string | undefined }

/** resolveHandoffEndpoints: each side's provider instance and the model it ran, from the item or the thread's runs. */
export function handoffEndpoints(item: Obj, runs: Obj[]): { from: HandoffEndpoint[]; to: HandoffEndpoint } {
  const handoffRun = item.runId == null ? undefined : runs.find(run => run.id === item.runId);
  const toInstance = str(item.toProviderInstanceId);
  const toModel = typeof item.toModel === 'string' ? item.toModel
    : handoffRun !== undefined && handoffRun.providerInstanceId === toInstance ? str(obj(handoffRun.modelSelection).model) || undefined : undefined;
  const selections = arr(item.fromModelSelections);
  const from = selections.length > 0
    ? selections.map(selection => ({ instanceId: str(selection.instanceId), model: typeof selection.model === 'string' ? selection.model : undefined }))
    : (Array.isArray(item.fromProviderInstanceIds) ? item.fromProviderInstanceIds : []).map(value => str(value)).map(instanceId => ({
      instanceId, model: latestRunModelBefore(runs, instanceId, typeof handoffRun?.ordinal === 'number' ? handoffRun.ordinal : undefined) }));
  return { from, to: { instanceId: toInstance, model: toModel } };
}

function latestRunModelBefore(runs: Obj[], instanceId: string, beforeOrdinal: number | undefined): string | undefined {
  let latest: Obj | undefined;
  for (const run of runs) {
    if (run.providerInstanceId !== instanceId) continue;
    const ordinal = Number(run.ordinal);
    if (beforeOrdinal !== undefined && ordinal >= beforeOrdinal) continue;
    if (latest === undefined || ordinal > Number(latest.ordinal)) latest = run;
  }
  const model = obj(latest?.modelSelection).model;
  return typeof model === 'string' ? model : undefined;
}

/** providerTextColor: the driver's icon fill (light, dark), or none where the label keeps the divider's colour. */
const PROVIDER_TEXT_COLOR: Record<string, [string, string]> = {
  codex: ['#000000', '#ffffff'], claudeAgent: ['#d97757', '#d97757'], antigravity: ['#5b87bf', '#5b87bf'],
  acpRegistry: ['#000000', '#FFFFFF'], cursor: ['#26251E', '#EDECEC'], grok: ['#0F0F0F', '#F5F5F5'],
  muse: ['#0081fb', '#0081fb'], opencode: ['#211E1E', '#F1ECEC'], pi: ['#0F0F0F', '#F5F5F5'],
};

/**
 * HandoffEndpoint: the provider's icon (`detail` the driver, `initials`, `driverIcon`) and the model's trigger name, else the
 * reported model, else the instance's name, in the provider's colour (`iconLight`, `iconDark`); `result` is its tooltip.
 * Between two accounts of one provider, the icon carries the instance's badge (`badge`, `badgeColor`; 74541bab17).
 */
export function handoffEndpoint(endpoint: HandoffEndpoint, providers: Obj[], index: number): Activity {
  const entry = providers.find(provider => provider.instanceId === endpoint.instanceId);
  const model = endpoint.model?.trim();
  const providerModel = model ? arr(entry?.models).find(candidate => candidate.slug === model) : undefined;
  const displayName = entry ? instanceDisplayName(entry) : endpoint.instanceId;
  const label = providerModel !== undefined ? triggerModelName(providerModel) : model ? model : displayName;
  const driver = str(entry?.driver) || endpoint.instanceId;
  const [light, dark] = PROVIDER_TEXT_COLOR[driver] ?? ['', ''];
  return { id: `${index}:${endpoint.instanceId}:${endpoint.model ?? ''}`, label, body: '', icon: '', output: '', result: `${displayName} · ${label}`,
    failed: false, timestamp: '', detail: driver, iconLight: light, iconDark: dark, initials: providerInstanceInitials(displayName),
    ...(entry ? { badge: providerBadge(entry, providers).providerBadge, badgeColor: providerBadge(entry, providers).providerBadgeColor } : {}),
    driverIcon: resolveProviderInstanceAcpRegistryIconUrl({ driverKind: driver, iconUrl: str(entry?.iconUrl) || undefined }) ?? '' };
}

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
  const record = agent ? { ...item, ...agent } : item;
  const driver = str(record.driver), provider = context.providers.find(candidate => candidate.instanceId === record.providerInstanceId);
  const card = subagentCard({ title: str(item.title), model: typeof record.model === 'string' ? record.model : null, provider, providers: context.providers,
    providerInstanceId: str(record.providerInstanceId), origin: str(record.origin), modelSelection: record.modelSelection ? obj(record.modelSelection) : undefined,
    childThread: context.threads?.find(thread => thread.id === record.childThreadId), status });
  return { modelLine: card.modelLine, effort: card.effort, speed: card.speed, speedLabel: card.speedLabel, id: str(item.id), label: subagentTitle(str(item.title, 'Subagent')), body: detail, icon: 'bot', output: elapsed === null ? '' : formatElapsedSeconds(elapsed / 1000),
    result: STATUS[status] ?? status, failed: status === 'failed', timestamp: str(item.startedAt ?? item.updatedAt),
    tone: status === 'failed' ? 'error' : status === 'completed' ? 'success' : settled || status === 'idle' ? 'muted' : 'info',
    ok: true, reasoning: false, expandable: false, detail: driver, status, targetId: str(item.childThreadId), answer: '',
    startedMs: Number.isFinite(startedMs) ? startedMs : 0, initials: providerInstanceInitials(str(provider?.displayName, driver)),
    driverIcon: resolveProviderInstanceAcpRegistryIconUrl({ driverKind: driver, iconUrl: str(provider?.iconUrl) || undefined }) ?? '' };
}

const workingStatus = (status: string) => status === 'pending' || status === 'running' || status === 'waiting';
/** subagentGroupSummary (client-runtime state/subagentDisplay.ts): whether any member works, or failed. */
export function subagentGroupSummary(members: ReadonlyArray<{ status: string }>): { active: boolean; failed: boolean } {
  return { active: members.some(member => workingStatus(member.status)), failed: members.some(member => member.status === 'failed') };
}
/** summarizeSubagentStatuses: "2 working · 1 done", in that order of kinds. */
export function summarizeSubagentStatuses(statuses: ReadonlyArray<string>): string {
  const counts = { working: 0, done: 0, failed: 0, stopped: 0, idle: 0 };
  for (const status of statuses) {
    if (workingStatus(status)) counts.working += 1;
    else if (status === 'completed') counts.done += 1;
    else if (status === 'failed') counts.failed += 1;
    else if (status === 'idle') counts.idle += 1;
    else counts.stopped += 1;
  }
  return (Object.keys(counts) as Array<keyof typeof counts>).filter(key => counts[key] > 0).map(key => `${counts[key]} ${key}`).join(' · ');
}
/** subagentGroupTiming (TimelineV2EventRow.logic.ts): the earliest start to the latest end; no end while any works or one has none. */
export function subagentGroupTiming(agents: ReadonlyArray<{ status: string; startedAt: unknown; completedAt: unknown }>): { status: string; startedAt: string | null; completedAt: string | null } {
  let startMs: number | null = null, endMs: number | null = null, endUnknown = false;
  for (const agent of agents) {
    const started = typeof agent.startedAt === 'string' ? Date.parse(agent.startedAt) : NaN;
    if (Number.isFinite(started)) startMs = startMs === null ? started : Math.min(startMs, started);
    const completed = typeof agent.completedAt === 'string' ? Date.parse(agent.completedAt) : NaN;
    if (Number.isFinite(completed)) endMs = endMs === null ? completed : Math.max(endMs, completed);
    else endUnknown = true;
  }
  const live = agents.some(agent => workingStatus(agent.status));
  return { status: live ? 'running' : 'completed', startedAt: startMs === null ? null : new Date(startMs).toISOString(),
    completedAt: live || endUnknown || endMs === null ? null : new Date(endMs).toISOString() };
}

/** A lifecycle item as a timeline row (V2LifecycleRow). */
export function eventRow(item: Obj, group: Obj[], row: Obj, context: SubagentContext = NO_CONTEXT): Partial<Message> & { kind: string } {
  const type = str(item.type);
  switch (type) {
    // A stopped run is no error: a neutral divider with CircleStop.
    case 'run_interrupt_result': return { kind: 'event', title: 'Run interrupted', detail: str(item.message), tone: 'neutral', icon: 'circle-stop', runId: str(item.runId) };
    // HandoffRow: the endpoints (from…, then to) draw beside the label; `detail` is their text for the row's accessible name.
    case 'handoff': {
      const { from, to } = handoffEndpoints(item, context.runs ?? []);
      // `status` places each endpoint ("from" before the arrow, "to" after it); a "from" endpoint before another one trails a comma (`answer`).
      const endpoints = [...from, to].map((endpoint, index) => ({ ...handoffEndpoint(endpoint, context.providers, index),
        status: index < from.length ? 'from' : 'to', answer: index < from.length - 1 ? ',' : '' }));
      return { kind: 'handoff', title: 'Context handoff', tone: item.status === 'failed' ? 'danger' : 'neutral', icon: 'arrow-right-left', activities: endpoints,
        detail: `${endpoints.slice(0, from.length).map(endpoint => endpoint.label).join(', ')}${from.length ? ' → ' : ''}${endpoints[from.length]!.label}`,
        runId: str(item.runId) };
    }
    case 'fork': {
      const source = obj(item.source), run = source.type === 'run';
      return { kind: 'fork', body: run ? 'Forked from conversation' : 'Conversation fork', actionLabel: run ? 'Open source conversation' : 'Open fork',
        targetId: run ? str(source.threadId) : str(item.targetThreadId), sourceThreadId: str(row.sourceThreadId) };
    }
    case 'subagent': {
      const members = (group.length ? group : [item]).map(member => subagent(member, context));
      if (members.length === 1) return { kind: 'subagent', title: members[0]!.label, activities: members, runId: str(item.runId) };
      // V2SubagentGroup: several subagents fold under one row with their avatars, a status summary and the group's time.
      const records = (group.length ? group : [item]).map(member => {
        const agent = member.subagentId === undefined ? undefined : context.agents.find(candidate => candidate.id === member.subagentId);
        return { status: str(agent?.status ?? member.status), startedAt: agent?.startedAt ?? member.startedAt, completedAt: agent?.completedAt ?? member.completedAt };
      });
      const summary = subagentGroupSummary(records), timing = subagentGroupTiming(records);
      const elapsed = timing.status === 'running' ? null : subagentElapsedMs(timing, 0);
      const startedMs = timing.status === 'running' && timing.startedAt ? Date.parse(timing.startedAt) : NaN;
      return { kind: 'subagents', title: `${members.length} subagents`, detail: summarizeSubagentStatuses(records.map(record => record.status)),
        live: summary.active, failed: summary.failed, startedMs: Number.isFinite(startedMs) ? startedMs : 0, elapsed: elapsed === null ? '' : formatElapsedSeconds(elapsed / 1000),
        activities: members, runId: str(item.runId) };
    }
    default: return { kind: 'event', title: str(item.title).trim() || type.replace(/_/g, ' '), detail: '', tone: item.status === 'failed' ? 'danger' : 'neutral', icon: 'wrench' };
  }
}
