// The /usage-limits notice and the usage-limit recovery notice (lane
// composer-controls), adapted from T3 Code (MIT); see LICENSE-T3. Sources:
// packages/shared/src/usageLimits.ts (collectProviderUsageLimits, limitsNotice,
// isUsageLimitsCommand), chat/ComposerUsageLimits.tsx, chat/UsageLimitRecoveryBanner.tsx
// and ChatView.tsx (openUsageLimits, limitRecoveryBanner). Limits come from the
// server's provider snapshots; nothing here asks the agent.
import { arr, obj, str, num, type Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { composerNow } from './composer-controls';
import { RUNTIME_LOCALE } from './timestamp-format';

const DRIVER_LABELS: Record<string, string> = { codex: 'Codex', claudeAgent: 'Claude', cursor: 'Cursor', grok: 'Grok', opencode: 'OpenCode', antigravity: 'Antigravity', pi: 'Pi', acpRegistry: 'ACP Registry' };

export function isUsageLimitsCommand(prompt: string): boolean { return prompt.trim().toLowerCase() === '/usage-limits'; }

/** limitsNotice: the one line shown when there are no bars to draw. */
export function limitsNotice(limits: Obj): string {
  const unavailable = obj(limits.unavailable);
  if (unavailable.reason === 'unsupported') return str(unavailable.message, 'This account has no subscription limits.');
  if (unavailable.reason === 'probeFailed') return str(unavailable.message, 'Could not read limits.');
  return arr(limits.windows).length === 0 ? 'No limits reported.' : '';
}
function duration(ms: number): string {
  const days = Math.floor(ms / 86_400_000), hours = Math.floor(ms % 86_400_000 / 3_600_000), minutes = Math.floor(ms % 3_600_000 / 60_000);
  return days > 0 ? `${days}d ${hours}h` : hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}
/** A compact limit window: what remains, and when it resets. */
export function windowLine(window: Obj, now: number): string {
  const remaining = Math.round(100 - Math.max(0, Math.min(100, num(window.usedPercent))));
  const resetsAt = Date.parse(str(window.resetsAt));
  const reset = Number.isFinite(resetsAt) ? (resetsAt <= now ? 'resets now' : `resets in ${duration(resetsAt - now)}`) : '';
  return [`${str(window.label, str(window.kind, 'Limit'))} · ${remaining}% left`, reset].filter(Boolean).join(' · ');
}
function accountSummary(provider: Obj): string {
  const driver = DRIVER_LABELS[str(provider.driver)] ?? str(provider.driver);
  const instance = str(provider.displayName).trim() || (str(provider.instanceId) !== str(provider.driver) ? str(provider.instanceId) : '');
  const label = instance && instance.toLowerCase() !== driver.toLowerCase() ? `${driver} · ${instance}` : driver;
  const plan = str(obj(provider.auth).label);
  return plan ? `${label} · ${plan}` : label;
}
const withLimits = (providers: Obj[]) => providers.filter(provider => provider.usageLimits && typeof provider.usageLimits === 'object');
/** hasProviderUsageLimits over the provider snapshots and the server's limit sources. */
export function usageLimitsOffered(client: T3Client): boolean {
  const providers = arr(client.config.providers), provider = providers.find(entry => entry.instanceId === client.providerId);
  if (!provider) return false;
  const sources = arr(client.config.usageLimitSources);
  return withLimits(providers).some(entry => entry.driver === provider.driver)
    || sources.some(source => arr(source.accounts).some(account => account.driver === provider.driver) || (source.error !== undefined && !arr(source.accounts).length));
}
/** collectProviderUsageLimits, as the notice's description and body lines. */
export function usageReport(client: T3Client, now: number): { summary: string; lines: string[] } | null {
  if (!usageLimitsOffered(client)) return null;
  const providers = arr(client.config.providers), selected = providers.find(entry => entry.instanceId === client.providerId)!;
  const native = withLimits(providers).filter(provider => provider.driver === selected.driver);
  const sourceAccounts = arr(client.config.usageLimitSources).flatMap(source => arr(source.accounts)).filter(account => account.driver === selected.driver);
  const accounts = [...native.map(provider => ({ summary: accountSummary(provider), limits: obj(provider.usageLimits) })),
    ...sourceAccounts.map(account => ({ summary: str(account.label, str(account.email, 'Account')), limits: obj(account.usageLimits) }))];
  const lines: string[] = [];
  for (const account of accounts) {
    if (accounts.length > 1) lines.push(account.summary);
    const notice = limitsNotice(account.limits);
    if (notice) lines.push(notice);
    else lines.push(...arr(account.limits.windows).map(window => windowLine(window, now)));
  }
  const errors = arr(client.config.usageLimitSources).filter(source => source.error !== undefined && !arr(source.accounts).length).map(source => str(obj(source.error).message, 'Could not read limits.'));
  return { summary: accounts.length === 1 ? accounts[0]!.summary : `${accounts.length} accounts`, lines: [...lines, ...errors] };
}

// The open panel: thread, provider, latest run and pending request. Anything that spends quota closes it.
type Panel = { key: string; now: number };
const panels = new WeakMap<T3Client, Panel>();
function panelKey(client: T3Client): string {
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  const request = arr(client.projection.runtimeRequests).find(entry => entry.status === 'pending');
  return [client.environmentId, client.threadId || `new:${client.projectId}`, client.providerId, str(obj(shell?.latestRun).runId, str(shell?.latestRunId)), str(request?.id)].join(':');
}
export function openUsageLimits(client: T3Client, now: number): boolean {
  if (!usageReport(client, now)) {
    panels.delete(client);
    pushToast(client, { kind: 'info', title: 'Usage limits are unavailable for this provider' });
    return false;
  }
  panels.set(client, { key: panelKey(client), now });
  return true;
}
export function closeUsageLimits(client: T3Client): void { panels.delete(client); }

export type UsageNotice = { id: string; variant: string; icon: string; title: string; description: string; action: string; actionLabel: string;
  action2: string; action2Label: string; dismiss: string; dismissLabel: string; priority: number; lines: string[]; actionReason: string; dismissId: string; segments: Array<{ key: string; label: string; threadId: string; last: boolean }> };
export function usageNotices(client: T3Client, now: number): UsageNotice[] {
  const notices: UsageNotice[] = [];
  // usageLimitRecoveryBannerItem: a run stopped by its subscription limit.
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  if (shell && shell.status === 'failed' && shell.lastErrorClass === 'usage_limit') {
    const run = obj(shell.latestRun), runId = str(run.runId, str(shell.latestRunId));
    const resetAt = str(shell.usageLimitResetAt);
    const stoppedAt = str(run.completedAt, str(shell.updatedAt));
    const canSchedule = !!resetAt && Date.parse(resetAt) > Date.parse(stoppedAt);
    const recovery = obj(shell.limitRecovery);
    const scheduled = recovery.runId === runId && recovery.resetAt === resetAt && recovery.autoResume === true;
    const snoozed = recovery.snooze === true && recovery.runId === runId && recovery.resetAt === resetAt && !!resetAt
      && Date.parse(str(shell.snoozedUntil)) === Date.parse(resetAt);
    notices.push({ id: `usage-limit-recovery:${runId}`, variant: 'warning', icon: 'gauge', title: 'Usage limit reached', priority: 1,
      description: resetAt ? `Resets ${new Date(resetAt).toLocaleString(RUNTIME_LOCALE)}` : 'Reset time unavailable; retry manually',
      action: canSchedule ? 'cc:limit-resume' : '', actionLabel: scheduled ? 'Cancel auto-resume' : 'Resume at reset',
      action2: canSchedule && !snoozed && Date.parse(resetAt) > now ? 'cc:limit-snooze' : '', action2Label: 'Snooze until reset', dismiss: '', dismissLabel: '', lines: [], actionReason: '', dismissId: '', segments: [] });
  }
  const panel = panels.get(client);
  if (panel && panel.key === panelKey(client)) {
    const report = usageReport(client, panel.now);
    if (report) notices.push({ id: `usage-limits:${panel.key}:${panel.now}`, variant: 'info', icon: 'gauge', title: 'Usage limits', priority: 2,
      description: report.summary, action: '', actionLabel: '', action2: '', action2Label: '', dismiss: 'cclocal:usage-limits-dismiss', dismissLabel: 'Dismiss usage limits', lines: report.lines, actionReason: '', dismissId: '', segments: [] });
  } else if (panel) panels.delete(client);
  return notices;
}

/** updateThreadMetadata({ limitRecovery }): schedule (or cancel) the resume, or snooze until the reset. */
export async function changeLimitRecovery(client: T3Client, native: Native, storage: Files, action: 'resume' | 'snooze'): Promise<void> {
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  if (!shell || shell.lastErrorClass !== 'usage_limit') throw new ClientError('This thread is not waiting on a usage limit.');
  const runId = str(obj(shell.latestRun).runId, str(shell.latestRunId)), resetAt = str(shell.usageLimitResetAt);
  if (!resetAt) throw new ClientError('Reset time unavailable; retry manually');
  if (action === 'snooze' && Date.parse(resetAt) <= composerNow(client)) throw new ClientError('The reset time has passed. Retry the thread manually.');
  const recovery = obj(shell.limitRecovery);
  const scheduled = recovery.runId === runId && recovery.resetAt === resetAt && recovery.autoResume === true;
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  await access.dispatch(storage, { type: 'thread.metadata.update', commandId, threadId: client.threadId,
    limitRecovery: { runId, resetAt, ...(action === 'resume' ? { autoResume: !scheduled } : { snooze: true }) } }, 'Change limit recovery');
}
