// @ref llp/1107.005-composer-and-transcript.decision.md#transcript-ownership
// Pinned365aa87982 FloatingWorkingControl, floating-working-status and threadSubagents.
import { mobileClient } from './client';
import type { T3Client } from './shared/client';
import { arr, obj, str, visibleTurnItems, type Obj } from './shared/domain';
import { queueState } from './shared/composer-controls-queue';
import { presentBackgroundWork } from './shared/composer-controls-view';
import { subagentStatus } from './shared/composer-controls-subagent';
import { requestPresentation } from './shared/requests';
import { formatDuration } from './shared/timeline-worklog';
import { projectMobileAgentActivity, type AgentActivityView } from './agent-activity';
import { mobileStreamOwner } from './browser-mobile-owner';
import { mobileBrowserSnapshot } from './browser-mobile-data';
import { mobileDevicesSnapshot } from './devices-mobile-data';

interface Status { kind: string; label: string; accessibility: string; symbol: string; spinning: boolean; reconnect: boolean }
const emptyStatus = (): Status => ({ kind: '', label: '', accessibility: '', symbol: '', spinning: false, reconnect: false });
const status = (kind: string, label: string, symbol = '', accessibility = label): Status => ({ kind, label, accessibility, symbol, spinning: kind === 'syncing', reconnect: kind === 'connection' });
const active = new Set(['preparing', 'starting', 'running', 'waiting']);
const goalLabels: Record<string, string> = { active: 'Goal set', paused: 'Goal paused', blocked: 'Goal blocked', usage_limited: 'Goal hit a usage limit', budget_limited: 'Goal reached its token budget', complete: 'Goal complete' };
export function workingControlDuration(startedAt: string, now: number) {
  const start = Date.parse(startedAt), seconds = Number.isFinite(start) && now > start ? Math.floor((now - start) / 1000) : 0;
  return seconds < 60 ? `${seconds}s` : seconds < 3600 ? `${Math.floor(seconds / 60)}m ${String(seconds % 60).padStart(2, '0')}s` : formatDuration(seconds * 1000);
}
export function workingAgentsSegment(agents: AgentActivityView) {
  const total = agents.rows.length, visible = total > 0 && (agents.turnActive || agents.liveCount > 0);
  return { label: !visible ? '' : agents.liveCount > 0 ? `${agents.liveCount}/${total}` : `${total} done`,
    accessibility: !visible ? '' : agents.liveCount > 0 ? `${agents.liveCount} of ${total} agents working` : `${total} ${total === 1 ? 'agent' : 'agents'} done` };
}
interface Sync { owner: string; latest: string; shown: string; showAt: number; holdUntil: number }
const syncing = new WeakMap<T3Client, Sync>();
// Explicit root clock replaces source timers; no native handle or autonomous timer is retained.
function syncLabel(client: T3Client, owner: string, value: string, now: number) {
  let state = syncing.get(client);
  if (!state || state.owner !== owner) { state = { owner, latest: '', shown: '', showAt: 0, holdUntil: 0 }; syncing.set(client, state); }
  if (state.showAt && now >= state.showAt) { if (state.latest) { state.shown = state.latest; state.holdUntil = now + 400; } state.showAt = 0; }
  if (state.holdUntil && now >= state.holdUntil) { state.holdUntil = 0; if (!state.latest) state.shown = ''; }
  state.latest = value;
  if (!state.shown) { if (!value) state.showAt = 0; else if (!state.showAt) state.showAt = now + 400; }
  else if (value && value !== state.shown) { state.shown = value; state.holdUntil = now + 400; }
  else if (!value && !state.holdUntil) state.shown = '';
  return { label: state.shown, next: state.showAt || state.holdUntil };
}
function connection(client: T3Client): Status | null {
  const name = str(obj(client.config.environment).label) || 'Environment';
  const error = client.statusMessage;
  switch (client.connection) {
    case 'connected': return null;
    case 'connecting': case 'reconnecting': return { ...status('connection', error ? `Failed to connect. Retrying ${name}...` : `Reconnecting to ${name}...`), spinning: true };
    case 'offline': return status('connection', 'You are offline');
    case 'unsupported': return status('connection', 'Client not supported');
    case 'error': return status('connection', error ? `Failed to connect to ${name}: ${error}` : `Failed to connect to ${name}`);
    default: return status('connection', `${name} is not connected`);
  }
}
function workStatus(client: T3Client, now: number, sync: string): Status {
  const disconnected = connection(client); if (disconnected) return disconnected;
  const requests = requestPresentation(client);
  if (requests.approvals.length || requests.questions.length) return emptyStatus();
  if (sync) return status('syncing', sync);
  if (!client.thread || str(obj(client.projection.thread).id) !== client.threadId) return emptyStatus();
  const projection = client.projection, runs = arr(projection.runs).filter(run => active.has(str(run.status))).sort((a, b) => Number(b.ordinal) - Number(a.ordinal)), run = runs[0];
  const items = visibleTurnItems(client.thread).map(row => obj(row.item)), compact = run && items.some(item => item.runId === run.id && item.type === 'user_message' && str(item.text).trim().toLowerCase() === '/compact' && arr(item.attachments).length === 0)
    && !items.some(item => item.runId === run.id && item.type === 'compaction' && ['completed', 'failed'].includes(str(item.status)));
  if (compact) return status('compacting', 'Compacting…', 'arrow.down.right.and.arrow.up.left');
  const runless = subagentStatus(projection), startedAt = str(run?.workStartedAt ?? run?.startedAt ?? run?.requestedAt) || (runless && ['pending', 'running', 'waiting'].includes(runless.status) ? runless.startedAt : '');
  if (startedAt && Number.isFinite(Date.parse(startedAt))) return status('working', `Working ${workingControlDuration(startedAt, now)}`);
  const shell = client.shell.threads.find(thread => thread.id === client.threadId) ?? {}, background = presentBackgroundWork(arr(shell.pendingBackgroundTasks));
  if (background) return status('background', background.title, background.waiting ? 'bolt' : 'terminal', background.description ? `${background.title}: ${background.description}` : background.title);
  const goal = obj(shell.goal), goalLabel = goalLabels[str(goal.status)];
  return goalLabel ? status('goal', goalLabel, 'target', `${goalLabel}: ${str(goal.objective)}`) : emptyStatus();
}
export function workingControlSegments(state: Status, agents: ReturnType<typeof workingAgentsSegment>, browserCount: number, deviceCount: number, queueCount: number) {
  const compact = !!state.kind || !!agents.label || queueCount > 0 || browserCount > 0 && deviceCount > 0;
  return { visible: !!state.kind || !!agents.label || browserCount > 0 || deviceCount > 0 || queueCount > 0,
    statusKind: state.kind, statusLabel: state.label, statusAccessibility: state.accessibility, statusSymbol: state.symbol, statusSpinning: state.spinning, reconnect: state.reconnect,
    agentsLabel: agents.label, agentsAccessibility: agents.accessibility ? `Open agents, ${agents.accessibility}` : '',
    browserCount, deviceCount, queueCount, compact,
    browserLabel: browserCount === 1 ? 'One tab open' : `${browserCount} tabs open`, browserAccessibility: browserCount === 1 ? 'View browser' : `View ${browserCount} browser tabs`,
    deviceLabel: deviceCount === 1 ? 'One device open' : `${deviceCount} devices open`, deviceAccessibility: deviceCount === 1 ? 'View device' : `View ${deviceCount} devices` };
}
export function mobileWorkingControl(environmentId: string, threadId: string, now: number, client: T3Client = mobileClient) {
  const valid = !!threadId && environmentId === client.environmentId && threadId === client.threadId;
  const owner = valid ? JSON.stringify([client.origin, environmentId, threadId, client.generation]) : '';
  if (!valid) { syncing.delete(client); return { ...workingControlSegments(emptyStatus(), { label: '', accessibility: '' }, 0, 0, 0), owner: '', nextRefreshAt: 0 }; }
  const projection: Obj = client.projection, detailMatches = str(obj(projection.thread).id) === threadId;
  const sync = syncLabel(client, owner, !client.threadLive ? client.thread ? 'Syncing messages...' : 'Loading messages...' : '', now);
  const agents = detailMatches ? projectMobileAgentActivity(environmentId, projection, client.shell, client.config, now) : { rows: [], runId: '', turnActive: false, liveCount: 0, settledCount: 0 };
  const streamOwner = mobileStreamOwner(client), browserCount = streamOwner ? mobileBrowserSnapshot(streamOwner, client).count : 0, deviceCount = streamOwner ? mobileDevicesSnapshot(streamOwner, client).count : 0;
  return { ...workingControlSegments(workStatus(client, now, sync.label), workingAgentsSegment(agents), browserCount, deviceCount, detailMatches ? queueState(projection).queued.length : 0), owner, nextRefreshAt: sync.next };
}
