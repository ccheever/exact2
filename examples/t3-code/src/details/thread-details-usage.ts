// The thread details card's Usage item (T3 Code 6d450a4284, MIT, see LICENSE-T3: components/chat/ThreadDetailsUsage.tsx):
// the selected provider's limit windows and the thread's context window. Every row reads as room left, so a longer bar
// always means more headroom; 10% or less is the error tone, 20% or less the warning tone. The limits come from the
// shared usage maths (usage/usage-limits.ts) over the server's provider snapshots and limit sources; the context window
// is the composer meter's snapshot (deriveLatestContextWindowSnapshot). Changes: the reference lines the rows up on one
// CSS subgrid; here the label and value columns get the width their longest text needs, estimated at 12 pt.
import type { T3Client } from '../client/client';
import { arr, str, type Obj } from '../client/domain';
import { collectProviderUsageLimits, formatDuration, limitsNotice, paceOf, remainingPercent, type LimitPace } from '../usage/usage-limits';
import { contextSnapshot, formatTokens } from '../composer/composer-controls-view';
import { formatUpcomingTimestamp } from '../shared/timestamp-format';

/** `left`: the percentage left, -1 when the row has none (its `value` then stands alone). `tone`: '', 'warning' or 'error'. */
export type UsageRow = { id: string; label: string; left: number; value: string; tone: string; muted: boolean; spaced: boolean; aria: string; tip: string; tipLines: string[] };
export type UsageView = { show: boolean; rows: UsageRow[]; labelWidth: number; valueWidth: number };
export const NO_USAGE: UsageView = { show: false, rows: [], labelWidth: 0, valueWidth: 0 };

const PACE_LABEL: Record<LimitPace, string> = { ahead: 'Ahead of pace', on: 'On pace', under: 'Under pace' };
const toneOf = (left: number | null) => left === null ? '' : left <= 10 ? 'error' : left <= 20 ? 'warning' : '';
const row = (id: string, label: string, left: number | null, tip: string, tipLines: string[], extra: { value?: string; muted?: boolean; spaced?: boolean } = {}): UsageRow => {
  const value = extra.value ?? `${left}%`;
  return { id, label, left: left ?? -1, value, tone: toneOf(left), muted: extra.muted === true, spaced: extra.spaced === true, aria: `${label}: ${extra.value ?? `${left}% left`}`, tip, tipLines };
};

/** `instanceId`: the thread's (or the draft's) provider instance; `projection`: the thread shown, null for a draft. */
export function usageView(input: { instanceId: string; providers: readonly Obj[]; sources: readonly Obj[]; projection: Obj | null; now: number; timestampFormat: string }): UsageView {
  const now = Math.floor(input.now / 60_000) * 60_000; // useNowMinute
  const report = input.instanceId ? collectProviderUsageLimits(input.instanceId, input.providers, input.sources, now) : null;
  // Only this instance's data: the report keeps one instance per login, and another entry may be a different login.
  const limits = report?.accounts.find(entry => entry.instanceId === input.instanceId)?.limits
    ?? (input.providers.find(provider => provider.instanceId === input.instanceId)?.usageLimits as Obj | undefined);
  const context = input.projection ? contextSnapshot(input.projection) : null;
  if (!limits && !context) return NO_USAGE;
  const notice = limits ? limitsNotice(limits) : null;
  const rows: UsageRow[] = [];
  if (notice) rows.push(row('limits', 'Limits', null, notice, [], { value: 'Unavailable' }));
  if (limits && !notice) for (const window of arr(limits.windows)) {
    const label = str(window.label), left = remainingPercent(window), pace = paceOf(window, now), resetsAt = Date.parse(str(window.resetsAt));
    const lines = [
      ...(str(window.resetsAt) && resetsAt > now ? [`Resets in ${formatDuration(resetsAt - now)} · ${formatUpcomingTimestamp(str(window.resetsAt), input.timestampFormat, now)}`] : []),
      ...(pace ? [PACE_LABEL[pace]] : []),
    ];
    rows.push(row(`window-${str(window.id, label)}`, label, left, `${label}: ${left}% left`, lines));
  }
  if (context) {
    const max = context.max, usedPercentage = max !== null && max > 0 ? context.used / max * 100 : null;
    const left = max !== null && usedPercentage !== null ? Math.round(100 - Math.max(0, Math.min(100, usedPercentage))) : null;
    const used = formatTokens(context.used), tokens = max !== null ? `${used} of ${formatTokens(max)} tokens used` : `${used} tokens used`;
    rows.push(row('context', 'Context', left, left !== null ? `Context: ${left}% left` : tokens, left !== null ? [tokens] : [],
      { muted: true, spaced: limits !== undefined, ...(left === null ? { value: used } : {}) }));
  }
  // The grid's auto columns: the label column at most 7rem (max-w-28), the value column the widest value.
  const width = (text: string) => text.length * 0.42 + 0.25;
  return { show: true, rows, labelWidth: Math.min(7, Math.max(...rows.map(entry => width(entry.label)))), valueWidth: Math.max(...rows.filter(entry => entry.left >= 0).map(entry => width(entry.value)), 0) };
}

/** The card's readout for the client's thread or draft. */
export function threadUsage(client: T3Client, threadId: string, now: number): UsageView {
  return usageView({ instanceId: client.providerId, providers: arr(client.config.providers), sources: arr(client.config.usageLimitSources),
    projection: threadId && threadId === client.threadId ? client.projection : null, now, timestampFormat: client.local.deviceSettings.timestampFormat });
}
