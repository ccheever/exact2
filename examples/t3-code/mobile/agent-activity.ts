// @ref llp/1109.004-home-projection.decision.md#decision
// T3 Code 365aa87982 threadSubagents.ts, ThreadAgentsSheet, SubagentRow and
// subagent-card-presentation.ts. MIT, see ../LICENSE-T3. This is the Agents route,
// not the pinned native Live Activity stub.
import { mobileClient } from './client';
import { mobileProviderIconURL } from './environment-detail';
import { arr, obj, str, type Obj, type Shell } from './shared/domain';
import { activeRun } from './shared/protocol';
import { subagentTitle } from './shared/timeline-events';
import { formatDuration } from './shared/timeline-worklog';
import { reportedModelLabel } from './shared/r3-composer-controls-model';

export interface AgentWorkspace { label: string; value: string; symbol: string }
export interface AgentActivityRow {
  id: string; environmentId: string; childThreadId: string; title: string; detail: string;
  status: string; tone: string; elapsed: string; canOpen: boolean; driver: string; iconUrl: string;
  metadata: string; workspace: AgentWorkspace[];
}
export interface AgentActivityView { rows: AgentActivityRow[]; runId: string; turnActive: boolean; liveCount: number; settledCount: number }
const active = new Set(['pending', 'running', 'waiting']);
const terminal = new Set(['completed', 'failed', 'cancelled', 'interrupted']);
const labels: Record<string, string> = { pending: 'Working', running: 'Working', waiting: 'Waiting', idle: 'Idle', completed: 'Completed', failed: 'Failed', cancelled: 'Cancelled', interrupted: 'Interrupted' };
const brands: Record<string, string> = { codex: 'Codex', claudeAgent: 'Claude', cursor: 'Cursor', grok: 'Grok', acpRegistry: 'ACP Registry', pi: 'Pi', opencode: 'OpenCode', antigravity: 'Antigravity' };
const humanize = (value: string) => value.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/[_-]+/g, ' ').trim().replace(/\b\w/g, char => char.toUpperCase());
/** Pinned providerInstanceDisplay tiering; shared desktop labels do not retain instance slug fallback. */
function providerName(provider: Obj): string {
  const driver = str(provider.driver), name = str(provider.displayName).trim(), brand = brands[driver] ?? humanize(driver);
  if (name && name !== brand) return name;
  if (provider.instanceId !== driver) { const instance = humanize(str(provider.instanceId)); if (instance) return instance; }
  return name || brand;
}
function detail(agent: Obj): string {
  const result = str(agent.result).trim(), progress = str(agent.progress).trim();
  const raw = (terminal.has(str(agent.status)) ? result || progress : progress || result).replace(/\s+/gu, ' ');
  const preview = raw.length > 280 ? `${raw.slice(0, 280).trimEnd()}…` : raw;
  if (/^Child task ended with status\b/i.test(preview)) return '';
  return preview.replace(/\[([^\]]+)\]\([^)]*\)/g, '$1').replace(/`/g, '').replace(/^[ \t]*[-*][ \t]+/gm, '').replace(/\s+/g, ' ').trim();
}
const time = (value: unknown) => Date.parse(str(value));
const basename = (value: string) => value.replace(/\\/g, '/').replace(/\/+$/, '').split('/').pop() ?? '';

/** Pure fold over the shared validated V2 projection and held shell metadata. */
export function projectMobileAgentActivity(environmentId: string, projection: Obj, shell: Shell, config: Obj, now: number): AgentActivityView {
  const all = arr(projection.subagents), run = activeRun(projection);
  const empty: AgentActivityView = { rows: [], runId: '', turnActive: false, liveCount: 0, settledCount: 0 };
  if (!all.length) return empty;
  const latest = all.reduce((previous, agent) => time(agent.updatedAt) > time(previous.updatedAt) ? agent : previous);
  const runId = str(run?.id ?? latest.runId);
  const agents = all.filter(agent => agent.runId === runId).sort((a, b) => time(a.startedAt ?? a.updatedAt) - time(b.startedAt ?? b.updatedAt) || str(a.id).localeCompare(str(b.id)));
  if (!agents.length) return empty;
  const rows = agents.map(agent => {
    const status = str(agent.status), live = active.has(status), prompt = str(agent.prompt).trim(), title = str(agent.title).trim();
    const provider = arr(config.providers).find(candidate => candidate.instanceId === agent.providerInstanceId);
    const parent = shell.threads.find(thread => thread.id === agent.threadId), child = shell.threads.find(thread => thread.id === agent.childThreadId);
    const parentProject = shell.projects.find(project => project.id === parent?.projectId), childProject = shell.projects.find(project => project.id === child?.projectId);
    const parentWorkspace = str(parent?.worktreePath ?? parentProject?.workspaceRoot), childWorkspace = str(child?.worktreePath ?? childProject?.workspaceRoot);
    const workspace: AgentWorkspace[] = [];
    if (parent && childProject && childProject.id !== parent.projectId) workspace.push({ label: 'Project', value: str(childProject.title), symbol: 'folder' });
    if (parentWorkspace && childWorkspace && parentWorkspace !== childWorkspace) {
      const label = child?.branch ? 'Branch' : child?.worktreePath ? 'Worktree' : 'Workspace';
      workspace.push({ label, value: child?.branch != null ? str(child.branch) : basename(childWorkspace), symbol: label === 'Branch' ? 'arrow.triangle.branch' : 'folder' });
    }
    const model = str(agent.model).trim(), driver = str(provider?.driver ?? agent.driver);
    const modelLabel = model ? reportedModelLabel(str(provider?.driver), model, arr(provider?.models)) : 'Not reported';
    const started = time(agent.startedAt), ended = live ? now : time(agent.completedAt);
    const duration = Number.isFinite(started) && Number.isFinite(ended) ? Math.max(0, ended - started) : 0;
    return { id: str(agent.id), environmentId, childThreadId: str(agent.childThreadId),
      title: title ? subagentTitle(title) : prompt.length > 80 ? `${prompt.slice(0, 77)}...` : prompt || 'Subagent',
      detail: detail(agent), status: labels[status] ?? status, tone: live ? 'working' : status === 'completed' ? 'completed' : status === 'failed' ? 'failed' : 'stopped',
      elapsed: duration > 0 ? formatDuration(duration) : '', canOpen: agent.childThreadId != null && !!str(agent.childThreadId), driver,
      iconUrl: mobileProviderIconURL(provider?.iconUrl), metadata: (provider ? `${providerName(provider)} · ` : '') + modelLabel, workspace };
  });
  return { rows, runId, turnActive: !!run, liveCount: agents.filter(agent => active.has(str(agent.status))).length,
    settledCount: agents.filter(agent => terminal.has(str(agent.status))).length };
}

/** The root opens this route only for its selected thread. Never show another thread's retained projection. */
export function mobileAgentActivity(environmentId: string, threadId: string, now: number): AgentActivityView {
  if (environmentId !== mobileClient.environmentId || threadId !== mobileClient.threadId || str(obj(mobileClient.projection.thread).id) !== threadId)
    return { rows: [], runId: '', turnActive: false, liveCount: 0, settledCount: 0 };
  return projectMobileAgentActivity(environmentId, mobileClient.projection, mobileClient.shell, mobileClient.config, now);
}
