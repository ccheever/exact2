// The /usage-limits notice and the usage-limit recovery notice,
// adapted from T3 Code (MIT); see LICENSE-T3. Sources:
// chat/ComposerUsageLimits.tsx (accountLabel, AccountSummary, usageLimitsBannerItem,
// UsageLimitsBannerBody), usage/UsageLimits.tsx (WindowBar, LimitWindows, ResetCredits,
// useResetCredit; at 2359643cff), chat/UsageLimitRecoveryBanner.tsx (at 7c3b3b3605ef) and ChatView.tsx (usageLimitsPanel,
// openUsageLimits, limitRecoveryBanner). The report is usage-limits.ts's port of
// collectProviderUsageLimits over the server's provider snapshots and limit sources; nothing
// here asks the agent. Changes: the banner body is data for usage-bars.contract (each window's
// figures, pace and tooltip lines; each account's label, credits and redeem state), the redeem
// state of each opened panel lives here (reset-credits.ts redeemStep), and "Use credit" sends a
// detached request (composer-replies.ts) so the window stays usable while it waits.
import { arr, obj, str, num, type Obj } from '../client/domain';
import { ClientError, type Files, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import type { T3Client } from '../client/client';
import { composerNow } from './composer-controls';
import { formatUpcomingTimestamp } from '../shared/timestamp-format';
import { PACE_LABEL, barColor, collectProviderUsageLimits, formatResetsIn, hasProviderUsageLimits, isUsageLimitsCommand, limitsNotice,
  paceOf, paceRemainingPercent, remainingPercent, type ResetCreditInput, type UsageLimitsAccount, type UsageLimitsReport } from '../usage/usage-limits';
import { REDEEM_IDLE, redeemStep, resetCreditsShown, resetCreditsSummary, type RedeemEvent, type RedeemState } from '../usage/reset-credits';
import { redactedValue } from '../shared/redacted-text';
import { startDetached, type DetachedReply } from './composer-replies';
import { openExternal } from '../client/external-open';
import { draftFallbackOf, draftModelFallback } from './composer-provider-selection';
import { fanoutSelections } from './composer-controls-fanout';

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

/** normalizeProviderAccentColor: a #rrggbb accent, else none. */
const normalizedAccent = (value: string | undefined) => { const accent = (value ?? '').trim(); return /^#[0-9a-fA-F]{6}$/u.test(accent) ? accent : ''; };
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
  const label = str(window.label), remaining = remainingPercent(window), timeLeft = paceRemainingPercent(window, now) ?? -1;
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
  const format = client.local.deviceSettings.timestampFormat, now = panel.now;
  return report.accounts.map((account, index) => {
    const limits = account.limits, notice = limitsNotice(limits), credits = obj(limits.resetCredits), redeem = panel.redeems.get(account.id) ?? REDEEM_IDLE;
    // LimitWindows: the instance's accent colours its bars in both schemes; without one, the driver's series colour.
    const accent = normalizedAccent(account.accentColor), [light, dark]: [string, string] = accent ? [accent, accent] : barColor(account.driver);
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
  action2: string; action2Label: string; dismiss: string; dismissLabel: string; priority: number; lines: string[]; actionReason: string; dismissId: string; segments: Array<{ key: string; label: string; threadId: string; aria: string; last: boolean }>;
  /** The second action shows, disabled. */
  action2Disabled?: boolean;
  /** The action command's id and value (the fallback notice's pick: its model and instance). */
  actionId?: string; actionValue?: string;
  redact?: string; redactMask?: string; redactAfter?: string; usage?: UsageAccountView[] };
/**
 * usageLimitRecoveryBannerItem: a run stopped by its subscription limit. Resume at reset schedules (or cancels) the
 * resume; Snooze until reset snoozes the thread to the reset and then reads "Wake now". The reset ends the snooze (the
 * server leaves snoozedUntil in place after it wakes the thread) and disables Snooze; both say "Saving..." while
 * their change is in flight.
 */
function limitRecovery(client: T3Client, now: number) {
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  if (!shell || shell.status !== 'failed' || shell.lastErrorClass !== 'usage_limit') return null;
  const run = obj(shell.latestRun), runId = str(run.runId, str(shell.latestRunId)), resetAt = str(shell.usageLimitResetAt);
  const stoppedAt = str(run.completedAt, str(shell.updatedAt));
  const recovery = obj(shell.limitRecovery), resetPassed = !!resetAt && Date.parse(resetAt) <= now;
  return { runId, resetAt, resetPassed, canSchedule: !!resetAt && Date.parse(resetAt) > Date.parse(stoppedAt),
    scheduled: recovery.runId === runId && recovery.resetAt === resetAt && recovery.autoResume === true,
    snoozed: recovery.snooze === true && recovery.runId === runId && recovery.resetAt === resetAt && !!resetAt
      && Date.parse(str(shell.snoozedUntil)) === Date.parse(resetAt) && !resetPassed };
}
/** The fallback windows dismissed here, per environment (ChatView's `t3code:fallback-notice-dismissed:<environment>`). */
const dismissedFallback = new WeakMap<object, Map<string, string>>();
export function dismissFallbackNotice(client: T3Client, window: string): void {
  const map = dismissedFallback.get(client) ?? new Map<string, string>();
  map.set(client.environmentId, window);
  dismissedFallback.set(client, map);
}
/**
 * modelFallbackBannerItem: a new thread that started on the fallback model, while it still has it and no other pick,
 * says so until dismissed for that provider's window: "Using <fallback>", "<provider> is out of usage · resets <when>",
 * and "Use <default>" picks the default model.
 */
function fallbackNotice(client: T3Client, now: number): UsageNotice | null {
  const started = draftFallbackOf(client);
  if (!started || client.threadId || fanoutSelections(client)) return null;
  if (client.providerId !== started.selection.instanceId || client.modelId !== started.selection.model) return null;
  // ChatView re-resolves every minute, so the notice goes once the default's window resets.
  const fallback = draftModelFallback(client, started.defaultSelection, now);
  if (!fallback || fallback.selection.instanceId !== started.selection.instanceId || fallback.selection.model !== started.selection.model) return null;
  const window = `${str(fallback.provider.instanceId)}:${fallback.resetsAt ?? ''}`;
  if (dismissedFallback.get(client)?.get(client.environmentId) === window) return null;
  const providers = arr(client.config.providers);
  const name = (instanceId: string, model: string) => str(arr(providers.find(entry => entry.instanceId === instanceId)?.models).find(entry => entry.slug === model)?.name, model);
  const resets = fallback.resetsAt ? ` · resets ${formatUpcomingTimestamp(fallback.resetsAt, client.local.deviceSettings.timestampFormat, now)}` : '';
  return { id: 'fallback-model', variant: 'info', icon: 'gauge', title: `Using ${name(fallback.selection.instanceId, fallback.selection.model)}`, priority: 2,
    description: `${str(fallback.provider.displayName, str(fallback.provider.driver))} is out of usage${resets}`,
    action: 'model', actionLabel: `Use ${name(fallback.defaultSelection.instanceId, fallback.defaultSelection.model)}`, actionId: fallback.defaultSelection.model,
    actionValue: fallback.defaultSelection.instanceId, action2: '', action2Label: '', dismiss: 'cclocal:fallback-dismiss', dismissLabel: 'Dismiss fallback model notice',
    dismissId: window, lines: [], actionReason: '', segments: [] };
}

export function usageNotices(client: T3Client, now: number): UsageNotice[] {
  const notices: UsageNotice[] = [];
  const fallback = fallbackNotice(client, now);
  if (fallback) notices.push(fallback);
  const limit = limitRecovery(client, now);
  if (limit) {
    const saving = client.busy && client.pending?.description === 'Change limit recovery';
    const { runId, resetAt, canSchedule, scheduled, snoozed } = limit;
    notices.push({ id: `usage-limit-recovery:${runId}`, variant: 'warning', icon: 'gauge', title: 'Usage limit reached', priority: 2,
      description: resetAt ? `Resets ${formatUpcomingTimestamp(resetAt, client.local.deviceSettings.timestampFormat, now)}` : 'Reset time unavailable; retry manually',
      action: canSchedule ? 'cc:limit-resume' : '', actionLabel: saving ? 'Saving...' : scheduled ? 'Cancel auto-resume' : 'Resume at reset',
      action2: canSchedule ? 'cc:limit-snooze' : '', action2Label: saving ? 'Saving...' : snoozed ? 'Wake now' : 'Snooze until reset',
      action2Disabled: !snoozed && limit.resetPassed,
      dismiss: '', dismissLabel: '', lines: [], actionReason: '', dismissId: '', segments: [] });
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
    if (url) openExternal(url);
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
    // The AlertDialog hands the focus back to its trigger as it closes.
    const slot = usageReport(client, panel.now)?.accounts.findIndex(entry => entry.id === id) ?? -1;
    return slot >= 0 ? `focus:usage-reset-${slot}` : '';
  }
  throw new ClientError(`Unknown composer action: ${op}`);
}

/** updateThreadMetadata({ limitRecovery }): schedule (or cancel) the resume, or snooze until the reset (or wake). */
export async function changeLimitRecovery(client: T3Client, native: Native, storage: Files, action: 'resume' | 'snooze'): Promise<void> {
  const limit = limitRecovery(client, composerNow(client));
  if (!limit) throw new ClientError('This thread is not waiting on a usage limit.');
  const { runId, resetAt, scheduled, snoozed } = limit;
  if (!resetAt) throw new ClientError('Reset time unavailable; retry manually');
  if (action === 'snooze' && !snoozed && limit.resetPassed) throw new ClientError('The reset time has passed. Retry the thread manually.');
  const access = client.restAccess(native);
  const [commandId] = await access.ids(1);
  await access.dispatch(storage, { type: 'thread.metadata.update', commandId, threadId: client.threadId,
    limitRecovery: { runId, resetAt, ...(action === 'resume' ? { autoResume: !scheduled } : { snooze: !snoozed }) } }, 'Change limit recovery');
}

/**
 * useDeadlinePassed(resetAt): the banner turns at the reset, even one weeks away. The root's minute clock alone would
 * be up to a minute late, so while the reset is under a minute away it ticks each second (`resetSoon`); `resetKey`
 * names the reset it waits for.
 */
export function usageResetClock(client: T3Client, now: number): { resetSoon: boolean; resetKey: string } {
  const limit = client.threadId ? limitRecovery(client, now) : null, at = limit ? Date.parse(limit.resetAt) : NaN;
  const soon = !!limit && limit.canSchedule && Number.isFinite(at) && at > now && at - now <= 60_000;
  return { resetSoon: soon, resetKey: soon ? `${limit!.runId}:${limit!.resetAt}` : '' };
}
