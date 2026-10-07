// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r3-composer-controls-resume.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// "Resume with less context" (lane composer-controls, round 3), adapted from
// T3 Code (MIT); see LICENSE-T3. Sources: ChatView.tsx (resumeCompactionBannerItem,
// compactThreadUnavailable, compactBeforeSend — a976f8c74c),
// chat/ContextWindowMeter.logic.ts (shouldOfferResumeCompaction,
// hasDismissedResumeCompaction) and packages/shared/src/claudeCompaction.ts.
import { arr, obj, str, type Obj } from './domain';
import { activeRun } from './protocol';
import { pendingRequests } from './requests';
import type { T3Client } from './client';
import { planFollowUp } from './composer-controls';

const RESUME_MINUTES = 70, RESUME_TOKENS = 100_000;
export const NEVER_ANSWER = "Don't ask again";
const QUESTION = /^This session is (?:\d+h \d+m|\d+m) old and uses \d{1,3}(?:,\d{3})* tokens\. Compact it before continuing\?$/u;

/** shouldOfferResumeCompaction: a Claude session idle 70 minutes with 100k tokens of context. `now` counts in whole minutes. */
export function shouldOffer(input: { driver: string; usedTokens: number; updatedAt: string; now: number }): boolean {
  if (input.driver !== 'claudeAgent' || input.usedTokens < RESUME_TOKENS) return false;
  const updated = Date.parse(input.updatedAt), minute = Math.floor(input.now / 60_000) * 60_000;
  return Number.isFinite(updated) && Number.isFinite(minute) && minute - updated >= RESUME_MINUTES * 60_000;
}

/** hasDismissedResumeCompaction: Claude's own resume question answered "Don't ask again". */
export function nativelyDismissed(projection: Obj): boolean {
  return arr(projection.runtimeRequests).some(request => request.kind === 'user_input' && request.status === 'resolved'
    && Object.entries(obj(request.answers)).some(([question, answer]) => QUESTION.test(question) && answer === NEVER_ANSWER));
}

// Session-scoped dismissals, one key per (thread, context snapshot).
const dismissed = new WeakMap<object, Set<string>>();
type Prefs = { resumeCompactionDismissed?: Record<string, boolean> };
const prefs = (client: T3Client): Prefs => client.local.composerControls as Prefs;
const permanentKey = (client: T3Client) => `${client.environmentId}:${client.providerId || 'claudeAgent'}`;

export function dismissResumeCompaction(client: T3Client, key: string): void {
  const keys = dismissed.get(client) ?? new Set<string>();
  keys.add(key); dismissed.set(client, keys);
}

/** compactThreadUnavailable and its reason (the Compact action's tooltip while disabled). */
export function compactBlocked(client: T3Client): string {
  const projection = client.projection;
  const conversation = arr(projection.visibleTurnItems).map(row => obj(row.item)).some(item => item.type === 'user_message'
    && (str(item.text).trim().toLowerCase() !== '/compact' || arr(item.attachments).length > 0));
  const provider = arr(client.config.providers).find(entry => entry.instanceId === client.providerId);
  const compactable = arr(provider?.slashCommands).some(command => command.name === 'compact');
  const requests = pendingRequests(projection);
  const sending = client.busy && !!client.pending && str(client.pending.payload.type) === 'message.dispatch';
  const blocked = !client.threadId || !client.thread || !conversation || !client.projectId || !compactable || !!activeRun(projection) || sending
    || !client.ready || client.connection !== 'connected' || requests.approvals.length > 0 || requests.inputs.length > 0 || !!planFollowUp(client);
  if (!blocked) return '';
  return !client.projectId ? 'Choose a project before compacting' : !compactable ? 'Compaction is unavailable for this provider' : 'Compacting is unavailable right now';
}

/** resumeCompactionBannerItem's conditions: the key and token count to show, or null. */
export function resumeCompaction(client: T3Client, now: number, usage: { used: number; updatedAt: string } | null): { key: string; used: number } | null {
  if (!client.threadId || !usage || !usage.updatedAt) return null;
  const key = `${client.threadId}:${usage.updatedAt}`;
  const local = prefs(client);
  if (nativelyDismissed(client.projection)) local.resumeCompactionDismissed = { ...local.resumeCompactionDismissed, [permanentKey(client)]: true };
  if (dismissed.get(client)?.has(key) || local.resumeCompactionDismissed?.[permanentKey(client)]) return null;
  if (pendingRequests(client.projection).inputs.length > 0 || activeRun(client.projection)) return null;
  const driver = str(arr(client.config.providers).find(entry => entry.instanceId === client.providerId)?.driver);
  return shouldOffer({ driver, usedTokens: usage.used, updatedAt: usage.updatedAt, now }) ? { key, used: usage.used } : null;
}
