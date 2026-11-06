// The thread details card's Usage limits and Context window items (T3 Code 2359643cff, MIT, see LICENSE-T3:
// components/chat/ThreadDetailsUsage.tsx). Usage limits: the selected provider's limit windows, each row reading as room
// left (10% or less the error tone, 20% or less the warning tone), from the shared usage maths (usage/usage-limits.ts)
// over the server's provider snapshots and limit sources. Context window: the thread's context window, whose bar fills
// as context is used (80% the warning tone, 90% the error tone) with a tick where it compacts automatically, from the
// composer meter's snapshot (deriveLatestContextWindowSnapshot). Changes: the reference lines the rows up on one CSS
// subgrid; here the label and value columns get the width their longest text needs, estimated at 12 pt.
import type { T3Client } from '../client/client';
import { arr, str, type Obj } from '../client/domain';
import { collectProviderUsageLimits, formatDuration, limitsNotice, paceOf, remainingPercent, type LimitPace } from '../usage/usage-limits';
import { contextSnapshot, formatTokens } from '../composer/composer-controls-view';
import { formatUpcomingTimestamp } from '../shared/timestamp-format';

/** `fill`: the bar's fill from 0 to 100, -1 when the row has none (its `value` then stands alone). `tick`: a marker on the
 *  bar from 0 to 100, -1 for none. `tone`: '', 'warning' or 'error'. */
export type UsageRow = { id: string; label: string; fill: number; tick: number; value: string; tone: string; muted: boolean; aria: string; tip: string; tipLines: string[] };
export type UsageView = { show: boolean; rows: UsageRow[]; labelWidth: number; valueWidth: number };
export const NO_USAGE: UsageView = { show: false, rows: [], labelWidth: 0, valueWidth: 0 };

const PACE_LABEL: Record<LimitPace, string> = { ahead: 'Ahead of pace', on: 'On pace', under: 'Under pace' };
const toneOf = (left: number | null) => left === null ? '' : left <= 10 ? 'error' : left <= 20 ? 'warning' : '';
const row = (id: string, label: string, left: number | null, tip: string, tipLines: string[], extra: { value?: string } = {}): UsageRow => {
  const value = extra.value ?? `${left}%`;
  return { id, label, fill: left ?? -1, tick: -1, value, tone: toneOf(left), muted: false, aria: `${label}: ${extra.value ?? `${left}% left`}`, tip, tipLines };
};
// The grid's auto columns: the label column at most 7rem (max-w-28), the value column the widest value.
const width = (text: string) => text.length * 0.42 + 0.25;
const sized = (rows: UsageRow[]): UsageView => ({ show: true, rows, labelWidth: Math.min(7, Math.max(...rows.map(entry => width(entry.label)))), valueWidth: Math.max(...rows.filter(entry => entry.fill >= 0).map(entry => width(entry.value)), 0) });

/** ThreadDetailsUsage. `instanceId`: the thread's (or the draft's) provider instance. */
export function usageView(input: { instanceId: string; providers: readonly Obj[]; sources: readonly Obj[]; now: number; timestampFormat: string }): UsageView {
  const now = Math.floor(input.now / 60_000) * 60_000; // useNowMinute
  const report = input.instanceId ? collectProviderUsageLimits(input.instanceId, input.providers, input.sources, now) : null;
  // Only this instance's data: the report keeps one instance per login, and another entry may be a different login.
  const limits = report?.accounts.find(entry => entry.instanceId === input.instanceId)?.limits
    ?? (input.providers.find(provider => provider.instanceId === input.instanceId)?.usageLimits as Obj | undefined);
  if (!limits) return NO_USAGE;
  const notice = limitsNotice(limits);
  if (notice) return sized([row('limits', 'Limits', null, notice, [], { value: 'Unavailable' })]);
  const rows = arr(limits.windows).map(window => {
    const label = str(window.label), left = remainingPercent(window), pace = paceOf(window, now), resetsAt = Date.parse(str(window.resetsAt));
    const lines = [
      ...(str(window.resetsAt) && resetsAt > now ? [`Resets in ${formatDuration(resetsAt - now)} · ${formatUpcomingTimestamp(str(window.resetsAt), input.timestampFormat, now)}`] : []),
      ...(pace ? [PACE_LABEL[pace]] : []),
    ];
    return row(`window-${str(window.id, label)}`, label, left, `${label}: ${left}% left`, lines);
  });
  return rows.length ? sized(rows) : NO_USAGE;
}

/** ThreadDetailsContext. `projection`: the thread shown, null for a draft. */
export function contextView(projection: Obj | null): UsageView {
  const usage = projection ? contextSnapshot(projection) : null;
  if (!usage) return NO_USAGE;
  const max = usage.max !== null && usage.max > 0 ? usage.max : null;
  const used = max !== null ? Math.round(Math.max(0, Math.min(100, usage.used / max * 100))) : null;
  const threshold = usage.threshold;
  const tick = max !== null && threshold !== null && threshold > 0 && threshold < max ? threshold / max * 100 : -1;
  const usedTokens = formatTokens(usage.used);
  const tipLines = [
    ...(max !== null ? [`${usage.used.toLocaleString('en-US')} of ${max.toLocaleString('en-US')} tokens`] : []),
    // formatContextWindowCompactionMessage(null, threshold)
    ...(threshold !== null && threshold > 0 ? [`Compacts automatically at about ${threshold.toLocaleString('en-US')} tokens.`] : []),
  ];
  const value = max !== null ? `${usedTokens} / ${formatTokens(max)}` : usedTokens;
  return sized([{ id: 'context', label: 'Context', fill: used ?? -1, tick, value, tone: used === null ? '' : used >= 90 ? 'error' : used >= 80 ? 'warning' : '', muted: true,
    aria: `Context: ${used !== null ? `${used}% used` : `${usedTokens} tokens used`}`, tip: used !== null ? `Context: ${used}% used` : `Context: ${usedTokens} tokens used`, tipLines }]);
}

/** The card's Usage limits for the client's thread or draft. */
export function threadUsage(client: T3Client, now: number): UsageView {
  return usageView({ instanceId: client.providerId, providers: arr(client.config.providers), sources: arr(client.config.usageLimitSources), now, timestampFormat: client.local.deviceSettings.timestampFormat });
}
/** The card's Context window for the client's thread (a draft has none). */
export const threadContext = (client: T3Client, threadId: string): UsageView => contextView(threadId && threadId === client.threadId ? client.projection : null);
