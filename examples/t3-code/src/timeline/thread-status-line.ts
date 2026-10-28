// The rows after a thread's last message: a pending context handoff and the
// thread's settled, snoozed or woke line, adapted from T3 Code (MIT); see
// LICENSE-T3. Sources at 9c161462c9: ChatView.tsx (threadStatusLine, the
// timeline footer, pendingHandoff and its usable selection at 74541bab17), chat/ThreadStatusLine.tsx,
// chat/V2LifecycleRow.tsx (HandoffRow, HandoffEndpoint),
// packages/client-runtime/src/handoff.ts (resolvePendingHandoff) and
// timestampFormat.ts (formatRelativeTimeLabel). The snoozed label reads the sidebar's snoozeWakeLabel on
// the minute (useThreadLifecycle.tsx since c3888da8b4), so both say the same hour.
// Sending a message clears each state, so the line's action is only the explicit way out.
import { arr, obj, str, type Message, type Obj } from '../client/domain';
import type { T3Client } from '../client/client';
import { wokeAt, wokeWatermark } from '../composer/composer-controls-commands';
import { composerSelection, instanceUsable } from '../composer/composer-provider-selection';
import { fanoutSelections } from '../composer/composer-controls-fanout';
import { handoffEndpoint } from './timeline-events';
import { formatRelativeTimeLabel } from '../browser/browser-history';
import { snoozeWakeLabel } from '../sidebar/sidebar-model';
import { threadEndRows } from '../composer/thread-end';

/**
 * threadStatusLine: one quiet line for thread state that does not change what
 * Enter does. The row's `detail` is the op its action sends and `targetId` the thread.
 */
export function threadStatusLine(client: T3Client, now: number): Message | null {
  if (!client.threadId) return null;
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  if (!shell) return null;
  const capabilities = obj(obj(client.config.environment).capabilities);
  // Snooze is derived: a passed wake time stops classifying as snoozed and feeds the woke line.
  const snoozeSupported = capabilities.threadSnooze !== false;
  const woke = snoozeSupported ? wokeAt(shell, now) : '';
  const until = Date.parse(str(shell.snoozedUntil));
  const snoozed = snoozeSupported && Number.isFinite(until) && until > now && !woke;
  const settled = capabilities.threadSettlement !== false && shell.settledOverride === 'settled';
  const pendingType = client.pending ? str(client.pending.payload.type) : '';
  const line = (icon: string, title: string, op: string, actionLabel: string): Message =>
    ({ id: `thread-status:${client.threadId}`, kind: 'status', title, body: '', icon, detail: op, targetId: client.threadId, actionLabel });
  if (snoozed) {
    const wake = snoozeWakeLabel(shell.snoozedUntil, Math.floor(now / 60_000) * 60_000);
    return line('alarm-clock', wake !== 'now' ? `Snoozed, ${wake} left` : 'Snoozed', 'cc:unsnooze', pendingType === 'thread.unsnooze' ? 'Waking...' : 'Wake now');
  }
  if (settled) {
    const at = Date.parse(str(shell.settledAt)), ago = Number.isFinite(at) ? formatRelativeTimeLabel(at, now) : '';
    return line('circle-check', ago ? `Settled ${ago}` : 'Settled', 'unsettle', pendingType === 'thread.unsettle' ? 'Un-settling...' : 'Un-settle');
  }
  if (!woke) return null;
  // isThreadWokeUnseen: the sidebar's watermark (server-owned when tracked), floored at the latest run's completion.
  const seen = Date.parse(wokeWatermark(client, shell)), completed = Date.parse(str(obj(shell.latestRun).completedAt));
  const visited = Math.max(Number.isFinite(seen) ? seen : -Infinity, Number.isFinite(completed) ? completed : -Infinity);
  return visited < Date.parse(woke) ? line('alarm-clock', 'Woke from snooze', 'cclocal:dismiss-woke', 'Dismiss') : null;
}

type Endpoint = { instanceId: string; model: string };
/**
 * resolvePendingHandoff: the next send's handoff, previewed without adding
 * anything to durable history. Accounts sharing a continuation group resume
 * the same native thread.
 */
export function pendingHandoff(projection: Obj, selection: Endpoint | null, providers: Obj[]): { from: Endpoint; to: Endpoint } | null {
  // The handoff row is committed before the provider starts: a starting handoff run is the source, so the
  // footer does not repeat the committed handoff.
  const run = [...arr(projection.runs)].reverse().find(entry => entry.status !== 'rolled_back'
    && (entry.startedAt != null || entry.status === 'starting' && entry.contextHandoffId != null));
  const from = run ? obj(run.modelSelection) : null;
  if (!from || !str(from.instanceId) || !selection?.instanceId) return null;
  const source = providers.find(provider => provider.instanceId === from.instanceId);
  const target = providers.find(provider => provider.instanceId === selection.instanceId);
  const sourceGroup = obj(source?.continuation).groupKey;
  const needsHandoff = from.instanceId === selection.instanceId
    ? str(from.model) !== selection.model && target?.requiresNewThreadForModelChange === true
    : sourceGroup === undefined || source?.driver !== target?.driver || sourceGroup !== obj(target?.continuation).groupKey;
  return needsHandoff ? { from: { instanceId: str(from.instanceId), model: str(from.model) }, to: selection } : null;
}

/** The timeline's footer rows: the pending handoff ("Context handoff pending"), then threadEndRows (what the thread is doing
 *  now, the status line and the queued turns; ChatView timelineFooter). */
export function threadFooterRows(client: T3Client, now: number): Message[] {
  if (!client.threadId) return [];
  // Before the thread's detail arrives: ThreadNow's sync line ("Loading messages...") and the status line, which reads
  // the shell.
  if (!client.thread) return threadEndRows(client, [], threadStatusLine(client, now));
  const providers = arr(client.config.providers), chosen = composerSelection(client, !!fanoutSelections(client));
  const instanceId = chosen.instanceId || client.providerId, model = chosen.entry ? chosen.model : client.modelId;
  // A provider the next send would skip (disabled or unavailable) previews no handoff.
  const handoff = pendingHandoff(client.projection, instanceId && instanceUsable(client, instanceId) ? { instanceId, model } : null, providers);
  const endpoints = handoff ? [handoffEndpoint(handoff.from, providers, 0), handoffEndpoint(handoff.to, providers, 1)] : [];
  const rows: Message[] = handoff ? [{ id: `pending-handoff:${client.threadId}`, kind: 'handoff', title: 'Context handoff pending', body: '', tone: 'neutral',
    icon: 'arrow-right-left', detail: endpoints.map(endpoint => endpoint.label).join(' → '), activities: endpoints }] : [];
  return threadEndRows(client, rows, threadStatusLine(client, now));
}
