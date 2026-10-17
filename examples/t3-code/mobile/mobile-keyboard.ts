// @ref llp/1109.011-responsive-workspace.decision.md#navigation-and-data-ownership
// Pinned365aa87982 HardwareKeyboardCommandProvider and threadReference.ts.
// App-only route ownership; clipboard/haptic need no server operation grant.
import type { T3Client } from './shared/client';
import { obj, str } from './shared/domain';
import { bridgeReply, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { resolveThreadReferenceCopyTarget, type ThreadReferenceCopyTarget } from './shared/thread-reference';

export interface MobileKeyboardSnapshot {
  routeKey: string; version: string; configuration: string; enabled: boolean;
  revision: number; feedbackId: number; phase: string; label: string; description: string;
}
interface State {
  routeKey: string; location: string; version: string; target: ThreadReferenceCopyTarget | null;
  revision: number; request: number; feedbackId: number; phase: string; label: string; description: string;
}
const states = new WeakMap<T3Client, State>();
function stateOf(client: T3Client): State {
  let state = states.get(client);
  if (!state) {
    state = { routeKey: '', location: '', version: '', target: null, revision: 0, request: 0,
      feedbackId: 0, phase: '', label: '', description: '' };
    states.set(client, state);
  }
  return state;
}
function clearFeedback(state: State) { state.feedbackId = 0; state.phase = ''; state.label = ''; state.description = ''; }

/** Same encoded physical route parser as mobile; terminal's own shortcuts win.
 * A shell from another selection must never supply its PR. Before adoption,
 * source mobile still permits copying the route's thread ID. */
export function mobileKeyboardTarget(client: T3Client, location: string): ThreadReferenceCopyTarget | null {
  const pathname = location.split(/[?#]/, 1)[0] ?? '';
  const match = /^\/threads\/([^/]+)\/([^/]+)(?:\/|$)/.exec(pathname);
  if (!match || pathname.split('/')[4] === 'terminal') return null;
  let environmentId: string, threadId: string;
  try { environmentId = decodeURIComponent(match[1]!); threadId = decodeURIComponent(match[2]!); } catch { return null; }
  if (!environmentId.trim() || !threadId.trim() || threadId.startsWith('new-task:')) return null;
  const thread = client.environmentId === environmentId && client.threadId === threadId
    ? client.shell.threads.find(row => row.id === threadId) : undefined;
  return resolveThreadReferenceCopyTarget({ threadId, pullRequests: thread?.pullRequests,
    linkedPullRequestUrl: str(obj(thread?.linkedPullRequest ?? thread?.branchPullRequest).url) || null });
}

/** Synchronous admission retires native events and copy feedback on navigation. */
export function mobileKeyboardSnapshot(client: T3Client, routeKey: string, location: string): MobileKeyboardSnapshot {
  const state = stateOf(client), target = routeKey ? mobileKeyboardTarget(client, location) : null;
  const version = JSON.stringify([routeKey, location, target]);
  if (state.routeKey !== routeKey || state.location !== location) { state.request++; clearFeedback(state); }
  if (state.version !== version) state.revision++;
  Object.assign(state, { routeKey, location, version, target });
  return { routeKey, version, enabled: target !== null,
    configuration: JSON.stringify({ routeKey, version, enabled: target !== null }),
    revision: state.revision, feedbackId: state.feedbackId, phase: state.phase, label: state.label, description: state.description };
}

export async function mobileKeyboardCopy(client: T3Client, input: string, native: Native | null | undefined): Promise<{ revision: number }> {
  const state = stateOf(client);
  let event;
  try { event = obj(JSON.parse(input)); } catch { return { revision: state.revision }; }
  if (!state.target || event.kind !== 'copyThreadReference' || event.routeKey !== state.routeKey || event.version !== state.version) return { revision: state.revision };
  // Re-resolve actual adopted data at invocation too: a callback can arrive
  // between a client refresh and the next root resource admission.
  const current = mobileKeyboardTarget(client, state.location);
  if (JSON.stringify(current) !== JSON.stringify(state.target)) return { revision: state.revision };
  const target = state.target, version = state.version, request = ++state.request;
  let copied = false;
  if (native?.available) {
    const results = await Promise.allSettled([
      bridgeReply(native, { op: 'copyText', text: target.value }),
      bridgeReply(native, { op: 'mobileHomeHaptic', kind: 'light' }),
    ]);
    // Exact abandons the whole invocation. A haptic cancellation is the same
    // ownership loss as clipboard cancellation, not an ordinary feedback error.
    for (const result of results) if (result.status === 'rejected' && letGo(result.reason)) throw result.reason;
    const copy = results[0]!;
    copied = copy.status === 'fulfilled' && copy.value.ok && obj(copy.value.value).copied === true;
  }
  if (state.request !== request || state.version !== version || JSON.stringify(mobileKeyboardTarget(client, state.location)) !== JSON.stringify(target)) return { revision: state.revision };
  state.feedbackId = request; state.phase = copied ? 'success' : 'error';
  state.label = copied ? target.successTitle : target.failureTitle;
  state.description = copied ? target.value : 'Try again.';
  state.revision++;
  return { revision: state.revision };
}

/** Root after(3000) and press-to-dismiss both capture the displayed identity. */
export function mobileKeyboardDismiss(client: T3Client, feedbackId: number): { revision: number } {
  const state = stateOf(client);
  if (feedbackId > 0 && state.feedbackId === feedbackId) { clearFeedback(state); state.revision++; }
  return { revision: state.revision };
}
