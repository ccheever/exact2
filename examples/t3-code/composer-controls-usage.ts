// The /usage-limits notice and the usage-limit recovery notice (lane
// composer-controls), adapted from T3 Code (MIT); see LICENSE-T3. Sources:
// chat/ComposerUsageLimits.tsx (accountLabel, AccountSummary, usageLimitsBannerItem,
// UsageLimitsBannerBody), usage/UsageLimits.tsx (WindowBar, LimitWindows, ResetCredits,
// useResetCredit), chat/UsageLimitRecoveryBanner.tsx and ChatView.tsx (usageLimitsPanel,
// openUsageLimits, limitRecoveryBanner). The report is usage-limits.ts's port of
// collectProviderUsageLimits over the server's provider snapshots and limit sources; nothing
// here asks the agent. Changes: the banner body is data for usage-bars.contract (each window's
// figures, pace and tooltip lines; each account's label, credits and redeem state), the redeem
// state of each opened panel lives here (reset-credits.ts redeemStep), and "Use credit" sends a
// detached request (composer-replies.ts) so the window stays usable while it waits.
import { arr, obj, str, num, type Obj } from './domain';
import { ClientError, bridgeReply, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { composerNow } from './composer-controls';
import { RUNTIME_LOCALE, formatUpcomingTimestamp } from './timestamp-format';
import { PACE_LABEL, barColor, collectProviderUsageLimits, elapsedShare, formatResetsIn, hasProviderUsageLimits, isUsageLimitsCommand, limitsNotice,
  paceOf, remainingPercent, type ResetCreditInput, type UsageLimitsAccount, type UsageLimitsReport } from './usage-limits';
import { REDEEM_IDLE, redeemStep, resetCreditsShown, resetCreditsSummary, type RedeemEvent, type RedeemState } from './reset-credits';
import { redactedValue } from './redacted-text';
import { settleStaleReplies, startDetached, type DetachedReply } from './composer-replies';
import { letGo } from './let-go';

export { isUsageLimitsCommand };
const DRIVER_LABELS: Record<string, string> = { codex: 'Codex', claudeAgent: 'Claude', cursor: 'Cursor', grok: 'Grok', opencode: 'OpenCode', antigravity: 'Antigravity', pi: 'Pi', acpRegistry: 'ACP Registry' };

/** hasProviderUsageLimits for the composer's instance, over the snapshots and the server's limit sources. */
export function usageLimitsOffered(client: T3Client): boolean {
  const providers = arr(client.config.providers), provider = providers.find(entry => entry.instanceId === client.providerId);
  return !!provider && hasProviderUsageLimits(provider.driver, providers, arr(client.config.usageLimitSources));
}
/** collectProviderUsageLimits for the composer's instance at `now`. */
export function usageReport(client: T3Client, now: number): UsageLimitsReport | null {
  if (!client.providerId) return null;
  return collectProviderUsageLimits(client.providerId, arr(client.config.providers), arr(client.config.usageLimitSources), now);
}

/** accountLabel: the driver, then the instance when there could be more than one of that driver. */
function accountLabel(account: UsageLimitsAccount): string {
  if (!account.instanceId) return account.label;
  const driver = DRIVER_LABELS[account.driver] ?? account.driver;
  const instance = account.displayName?.trim() || (account.instanceId !== account.driver ? account.instanceId : '');
  // The default instance is often named after its driver; saying it twice adds nothing.
  return instance && instance.toLowerCase() !== driver.toLowerCase() ? `${driver} · ${instance}` : driver;
}
/** AccountSummary: a label with an address is RedactedSensitiveText until clicked, then " · plan". */
function accountSummary(account: UsageLimitsAccount) {
  const label = accountLabel(account), hidden = label.includes('@') ? redactedValue(label) : { value: '', placeholder: '' };
  return { label: hidden.value ? '' : label, redact: hidden.value, redactMask: hidden.placeholder, plan: account.plan ? ` · ${account.plan}` : '' };
}
/** WindowBar and its LimitWindows row: what remains, the even-spending mark, pace and countdown. */
function windowView(window: Obj, now: number, format: string) {
  const label = str(window.label), remaining = remainingPercent(window), elapsed = elapsedShare(window, now);
  // The fill is quota left, so the even-spending mark is the time left.
  const timeLeft = elapsed === null ? -1 : Math.round((1 - elapsed) * 100);
  const resetsIn = formatResetsIn(window, now) ?? '', pace = paceOf(window, now);
  const resetsAt = str(window.resetsAt) ? formatUpcomingTimestamp(str(window.resetsAt), format, now) : '';
  return { id: str(window.id, label), label, remaining, timeLeft, pace: pace ?? '', paceLabel: pace ? PACE_LABEL[pace] : '', resetsIn,
    summary: `${label}: ${remaining}% left${timeLeft < 0 ? '' : `, ${timeLeft}% of the window left`}${resetsIn ? `, ${resetsIn}` : ''}`,
    tipFigures: `${remaining}% left${timeLeft < 0 ? '' : ` · ${timeLeft}% of the window left`}`,
    tipResets: resetsAt ? `Resets ${resetsAt}${resetsIn ? ` · ${resetsIn}` : ''}` : '' };
}
/** The redemption target: the report's, else the account's own instance. */
const creditInput = (account: UsageLimitsAccount): ResetCreditInput | undefined => account.resetCreditInput ?? (account.instanceId ? { instanceId: account.instanceId } : undefined);
/** UsageLimitsBannerBody, one entry per account. */
function accountViews(client: T3Client, report: UsageLimitsReport, panel: Panel) {
  settleStaleReplies(client);
  const format = client.local.deviceSettings.timestampFormat, now = panel.now;
  return report.accounts.map((account, index) => {
    const limits = account.limits, notice = limitsNotice(limits), credits = obj(limits.resetCredits), redeem = panel.redeems.get(account.id) ?? REDEEM_IDLE;
    const [light, dark] = barColor(account.driver);
    const showCredits = !!creditInput(account) && !!limits.resetCredits && resetCreditsShown(credits, redeem);
    return { key: account.id, slot: String(index), showLabel: report.accounts.length > 1, ...accountSummary(account), notice: notice ?? '',
      windows: notice ? [] : arr(limits.windows).map(window => windowView(window, now, format)), light, dark,
      manage: str(obj(limits.externalUsage).url) !== '', credits: showCredits ? resetCreditsSummary(credits, now) : '', showCredits,
      canRedeem: num(credits.availableCount) > 0, busy: redeem.busy, status: redeem.status ?? '' };
  });
}

// The open panel: thread, provider, latest run and pending request. Anything that spends quota closes it.
// Each opening has its own redeem states and confirm (ResetCredits remounts under a fresh banner id).
type Panel = { key: string; now: number; redeems: Map<string, RedeemState>; confirm: string };
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
  panels.set(client, { key: panelKey(client), now, redeems: new Map(), confirm: '' });
  return true;
}
export function closeUsageLimits(client: T3Client): void { panels.delete(client); }

export type UsageAccountView = ReturnType<typeof accountViews>[number];
export type UsageNotice = { id: string; variant: string; icon: string; title: string; description: string; action: string; actionLabel: string;
  action2: string; action2Label: string; dismiss: string; dismissLabel: string; priority: number; lines: string[]; actionReason: string; dismissId: string; segments: Array<{ key: string; label: string; threadId: string; last: boolean }>;
  redact?: string; redactMask?: string; redactAfter?: string; usage?: UsageAccountView[] };
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
    if (report) {
      // usageLimitsBannerItem: one account names itself (AccountSummary), several are counted.
      const single = report.accounts.length === 1 ? accountSummary(report.accounts[0]!) : null;
      notices.push({ id: `usage-limits:${panel.key}:${panel.now}`, variant: 'info', icon: 'gauge', title: 'Usage limits', priority: 2,
        description: single ? (single.redact ? '' : `${single.label}${single.plan}`) : `${report.accounts.length} accounts`,
        redact: single?.redact ?? '', redactMask: single?.redactMask ?? '', redactAfter: single?.redact ? single.plan : '',
        action: '', actionLabel: '', action2: '', action2Label: '', dismiss: 'cclocal:usage-limits-dismiss', dismissLabel: 'Dismiss usage limits',
        lines: report.notices, actionReason: '', dismissId: '', segments: [], usage: accountViews(client, report, panel) });
    }
  } else if (panel) panels.delete(client);
  return notices;
}

/** The account whose "Use reset" confirm is open in this thread's banner ('' when none). */
export function usageConfirm(client: T3Client): string {
  const panel = panels.get(client);
  return panel && panel.key === panelKey(client) ? panel.confirm : '';
}

/** consumeResetCredit's reply as the redeem's next step: a typed server error says its message. */
function redeemEvent(reply: DetachedReply): RedeemEvent {
  if (reply.ok) return { type: 'done', outcome: str(reply.value.outcome), ...(str(reply.value.warning) ? { warning: str(reply.value.warning) } : {}) };
  const typed = !reply.interrupted && ['ProviderSetupError', 'UsageLimitSourceError', 'EnvironmentAuthorizationError'].includes(reply.error.kind);
  return { type: 'failed', message: typed ? reply.error.message : null };
}
/**
 * `cclocal:usage-*`: "Manage usage" (shell.openExternal; a failure is ignored, as `void` there),
 * "Use reset" (opens the confirm), Cancel or Escape (closes it and returns the focus to "Use
 * reset"), and "Use credit" (closes it and redeems once, as a composer job).
 */
export async function usageLocal(client: T3Client, native: Native, op: string, id: string): Promise<string> {
  const panel = panels.get(client);
  if (!panel || panel.key !== panelKey(client)) return '';
  const account = usageReport(client, panel.now)?.accounts.find(entry => entry.id === id);
  const current = panel.redeems.get(id) ?? REDEEM_IDLE;
  if (op === 'usage-manage') {
    const url = str(obj(account?.limits.externalUsage).url);
    if (url) await bridgeReply(native, { op: 'remoteEditorsOpen', url }).catch(error => { if (letGo(error)) throw error; });
    return '';
  }
  if (op === 'usage-reset-ask') {
    if (!account || current.busy) return '';
    panel.redeems.set(id, redeemStep(current, { type: 'ask' })); panel.confirm = id;
    return 'focus:reset-credit-cancel'; // the confirm takes the focus (AlertDialog's first tabbable)
  }
  if (op === 'usage-reset-cancel') {
    panel.redeems.set(id, redeemStep(current, { type: 'cancel' })); panel.confirm = '';
    const slot = usageReport(client, panel.now)?.accounts.findIndex(entry => entry.id === id) ?? -1;
    return slot >= 0 ? `focus:usage-reset-${slot}` : '';
  }
  if (op === 'usage-reset-confirm') {
    panel.confirm = '';
    const input = account ? creditInput(account) : undefined;
    if (!input || current.busy) { panel.redeems.set(id, redeemStep(current, { type: 'cancel' })); return ''; }
    panel.redeems.set(id, redeemStep(current, { type: 'start' }));
    // Detached (composer-replies.ts): "Using…" shows now and the window stays usable until the reply.
    const { reply } = await startDetached(client, native, 'provider.consumeResetCredit', input);
    void reply.then(result => { panel.redeems.set(id, redeemStep(panel.redeems.get(id) ?? REDEEM_IDLE, redeemEvent(result))); });
    return '';
  }
  throw new ClientError(`Unknown composer action: ${op}`);
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
