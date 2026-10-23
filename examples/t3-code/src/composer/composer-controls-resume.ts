// The Compact chip: a large Claude thread whose prompt cache expired compacts
// before its next send, adapted from T3 Code (MIT); see LICENSE-T3. Sources at
// 74541bab17: ChatView.tsx (resumeCompactionTokens, keepFullHistory,
// compactThreadUnavailable, compactBeforeSend), chat/ContextWindowMeter.logic.ts
// (shouldOfferResumeCompaction, hasDismissedResumeCompaction), lib/contextWindow.ts
// (latestProviderTurnTokenUsage) and packages/shared/src/claudeCompaction.ts.
import { feedbackUploading } from './composer-feedback';
import { arr, obj, str, type Obj } from '../client/domain';
import { activeRun } from '../client/protocol';
import { pendingRequests } from './requests';
import type { T3Client } from '../client/client';
import { planFollowUp, resumeState } from './composer-controls';
import { fanoutSelections } from './composer-controls-fanout';

// Used when the provider did not report its cache TTL: Claude's 1h cache plus slack.
const FALLBACK_MS = 70 * 60_000;
// COMPACTION_SUGGESTION_MIN_TOKENS: below it T3 neither suggests nor runs a Claude compaction (Claude Code's idleCompaction default).
const RESUME_TOKENS = 200_000;
export const NEVER_ANSWER = "Don't ask again";
const QUESTION = /^This session is (?:\d+h \d+m|\d+m) old and uses \d{1,3}(?:,\d{3})* tokens\. Compact it before continuing\?$/u;

/**
 * shouldOfferResumeCompaction: a large Claude context whose prompt cache has
 * expired, so the next turn would re-read all of it uncached. `now` counts in
 * whole minutes.
 */
export function shouldOffer(input: { driver: string; usedTokens: number; updatedAt: string; promptCacheTtlMs?: number; now: number }): boolean {
  if (input.driver !== 'claudeAgent' || input.usedTokens < RESUME_TOKENS) return false;
  const updated = Date.parse(input.updatedAt), minute = Math.floor(input.now / 60_000) * 60_000;
  return Number.isFinite(updated) && Number.isFinite(minute) && minute - updated >= (input.promptCacheTtlMs ?? FALLBACK_MS);
}

/** latestProviderTurnTokenUsage: snapshots order turns by provider thread, not time, so the newest report wins (ties go to the later entry). */
export function latestTokenUsage(turns: Obj[]): Obj | null {
  let latest: Obj | null = null;
  for (const turn of turns) {
    if (turn.tokenUsage === undefined || turn.tokenUsage === null) continue;
    const usage = obj(turn.tokenUsage);
    if (latest === null || Date.parse(str(usage.updatedAt)) >= Date.parse(str(latest.updatedAt))) latest = usage;
  }
  return latest;
}

/** hasDismissedResumeCompaction: Claude's own resume question answered "Don't ask again". */
export function nativelyDismissed(projection: Obj): boolean {
  return arr(projection.runtimeRequests).some(request => request.kind === 'user_input' && request.status === 'resolved'
    && Object.entries(obj(request.answers)).some(([question, answer]) => QUESTION.test(question) && answer === NEVER_ANSWER));
}

type Prefs = { resumeCompactionDismissed?: Record<string, boolean> };
const prefs = (client: T3Client): Prefs => client.local.composerControls as Prefs;
const permanentKey = (client: T3Client) => `${client.environmentId}:${client.providerId || 'claudeAgent'}`;

// Threads whose Compact chip is off, for the session. A send that starts its turn clears its thread's entry.
const fullHistory = new WeakMap<object, Set<string>>();
const dismissed = new WeakMap<object, Set<string>>();
export function dismissResumeCompaction(client: T3Client, key: string): void {
  const keys = dismissed.get(client) ?? new Set<string>();
  keys.add(key); dismissed.set(client, keys);
}
const threadKey = (client: T3Client, threadId = client.threadId) => `${client.environmentId}:${threadId}`;
export function keepsFullHistory(client: T3Client, threadId = client.threadId): boolean {
  return fullHistory.get(client)?.has(threadKey(client, threadId)) ?? false;
}
export function setKeepFullHistory(client: T3Client, keep: boolean, threadId = client.threadId): void {
  const keys = fullHistory.get(client) ?? new Set<string>();
  if (keep) keys.add(threadKey(client, threadId)); else keys.delete(threadKey(client, threadId));
  fullHistory.set(client, keys);
}

/** compactThreadUnavailable and its reason (the context meter's Compact while disabled). */
export function compactBlocked(client: T3Client): string {
  const projection = client.projection;
  const conversation = arr(projection.visibleTurnItems).map(row => obj(row.item)).some(item => item.type === 'user_message'
    && (str(item.text).trim().toLowerCase() !== '/compact' || arr(item.attachments).length > 0));
  const provider = arr(client.config.providers).find(entry => entry.instanceId === client.providerId);
  const compactable = arr(provider?.slashCommands).some(command => command.name === 'compact');
  const requests = pendingRequests(projection);
  const sending = client.busy && !!client.pending && str(client.pending.payload.type) === 'message.dispatch';
  const blocked = !client.threadId || !client.thread || !conversation || !client.projectId || !compactable || !!activeRun(projection) || sending
    || !client.ready || client.connection !== 'connected' || requests.approvals.length > 0 || requests.inputs.length > 0 || !!planFollowUp(client)
    || feedbackUploading(client); // ChatView compactThreadUnavailable: a /feedback upload in flight (composer-feedback.ts)
  if (!blocked) return '';
  return !client.projectId ? 'Choose a project before compacting' : !compactable ? 'Compaction is unavailable for this provider' : 'Compacting is unavailable right now';
}

/**
 * resumeCompactionTokens: the tokens a stale Claude session would re-read on
 * its next turn, or null. While set the composer shows the Compact chip and a
 * send compacts first. Held queues and multi-model sends never compact first.
 * Only the live report carries a TTL, and only when it is the window's source.
 */
export function resumeCompactionTokens(client: T3Client, now: number, usage: { used: number; updatedAt: string; promptCacheTtlMs?: number } | null): number | null {
  if (!usage?.promptCacheTtlMs || !client.threadId || !usage.updatedAt) return null;
  const local = prefs(client);
  if (nativelyDismissed(client.projection)) local.resumeCompactionDismissed = { ...local.resumeCompactionDismissed, [permanentKey(client)]: true };
  if (local.resumeCompactionDismissed?.[permanentKey(client)] || compactBlocked(client) || resumeState(client.projection).heldQueue || fanoutSelections(client)) return null;
  const driver = str(arr(client.config.providers).find(entry => entry.instanceId === client.providerId)?.driver);
  return shouldOffer({ driver, usedTokens: usage.used, updatedAt: usage.updatedAt, promptCacheTtlMs: usage.promptCacheTtlMs, now }) ? usage.used : null;
}

/** Older servers have no cache TTL. Keep their opt-in 70-minute notice until they report one. */
export function resumeCompaction(client: T3Client, now: number, usage: { used: number; updatedAt: string; promptCacheTtlMs?: number } | null): { key: string; used: number } | null {
  if (!client.threadId || !usage?.updatedAt || usage.promptCacheTtlMs !== undefined) return null;
  const key = `${client.threadId}:${usage.updatedAt}`, local = prefs(client);
  if (nativelyDismissed(client.projection)) local.resumeCompactionDismissed = { ...local.resumeCompactionDismissed, [permanentKey(client)]: true };
  if (dismissed.get(client)?.has(key) || local.resumeCompactionDismissed?.[permanentKey(client)] || pendingRequests(client.projection).inputs.length || activeRun(client.projection)) return null;
  const driver = str(arr(client.config.providers).find(entry => entry.instanceId === client.providerId)?.driver);
  return shouldOffer({ driver, usedTokens: usage.used, updatedAt: usage.updatedAt, now }) ? { key, used: usage.used } : null;
}
