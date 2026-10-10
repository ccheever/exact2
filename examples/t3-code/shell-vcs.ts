// The workspace card's version-control status (MIT reference, see LICENSE-T3:
// packages/client-runtime/src/state/vcs.ts `status`, packages/shared/src/git.ts
// applyGitStatusStreamEvent / mergeGitStatusParts): one `subscribeVcsStatus`
// stream for the card's workspace while the card is shown, folded snapshot →
// localUpdated → remoteUpdated into one status. The card is open by default
// beside chat, so it asks `vcs.refreshStatus` (which can wait on a remote fetch)
// only when the reference's GitActionsControl does: when the window regains the
// focus or becomes visible (`refreshVcsOnFocus`, exactPage(): exact2 #219) and
// when a git menu opens. The client's event drain hands each entry here (client.ts).
//
// The stream's first events can be drained before the subscribe reply reaches
// the answer that asked (or that reply can be dropped with an abandoned
// answer), so the current stream is the newest subscription id seen after the
// latest subscribe: T3Transport numbers requests `<generation>-<serial>` with
// a serial that only grows, and subscribing again under one key replaces the
// stream.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { letGo } from './let-go';

export const VCS_STATUS_KEY = 'shell-vcs-status';
type VcsState = { cwd: string; id: string; status: Obj | null; error: string; floor: number; maxSeen: number; recent: Map<string, Obj>;
  tried: boolean; attemptAt: number; attemptRevision: number; generation: number };
const states = new WeakMap<T3Client, VcsState>();
const vcsState = (client: T3Client): VcsState => {
  let state = states.get(client);
  if (!state) {
    state = { cwd: '', id: '', status: null, error: '', floor: 0, maxSeen: 0, recent: new Map(), tried: false, attemptAt: 0, attemptRevision: -1, generation: -1 };
    states.set(client, state);
  }
  return state;
};
// Without a stream yet, subscribe again after this long (or on the next revision while the clock stands still).
const RETRY_MS = 3000;

/** Proven local data for one live subscription, distinct from the card's warm
 * display cache. No remote-only event can supply a checkout branch. */
export type CurrentVcsStatus = { origin: string; environmentId: string; generation: number; cwd: string;
  subscriptionId: string; status: Obj };
type CurrentEvidence = Omit<CurrentVcsStatus, 'status' | 'subscriptionId'> & {
  subscriptionId: string; eventId: string; local: Obj | null; floor: number;
};
const currentEvidence = new WeakMap<T3Client, CurrentEvidence>();
const sameCurrentOwner = (client: T3Client, evidence: CurrentEvidence, cwd: string) =>
  client.connection === 'connected' && !!client.environmentId && !!client.origin && evidence.cwd === cwd &&
  evidence.origin === client.origin && evidence.environmentId === client.environmentId && evidence.generation === client.generation;
const currentId = (evidence: CurrentEvidence, id: string) =>
  id.startsWith(`${evidence.generation}-`) && /^\d+$/.test(id.slice(String(evidence.generation).length + 1)) &&
  subscriptionSerial(id) > evidence.floor;
const cloneLocal = (value: unknown): Obj | null => {
  if (!value || typeof value !== 'object' || Array.isArray(value) || typeof obj(value).isRepo !== 'boolean') return null;
  try { return JSON.parse(JSON.stringify(value)) as Obj; } catch { return null; }
};
function observeCurrentEvent(client: T3Client, cwd: string, entry: Obj): void {
  const evidence = currentEvidence.get(client);
  if (!evidence) return;
  if (!sameCurrentOwner(client, evidence, cwd)) { currentEvidence.delete(client); return; }
  const id = str(entry.subscriptionId);
  if (entry.generation !== undefined && entry.generation !== evidence.generation || !currentId(evidence, id) ||
    subscriptionSerial(id) < subscriptionSerial(evidence.eventId)) return;
  // The display may learn a replacement stream before its subscribe reply.
  // Keep this event provisional until that exact request ID is acknowledged.
  if (id !== evidence.eventId) { evidence.eventId = id; evidence.local = null; }
  const item = obj(entry.value);
  if (item._retryDue || item._streamEnded || item._transportError) { currentEvidence.delete(client); return; }
  if (item._tag === 'snapshot' || item._tag === 'localUpdated') evidence.local = cloneLocal(item.local);
}
/** Current endpoint/cwd local evidence only. A dropped subscribe reply keeps
 * this null even when the display can show its provisional stream data. */
export function peekCurrentVcsStatus(client: T3Client, cwd: string): CurrentVcsStatus | null {
  const evidence = currentEvidence.get(client);
  if (!evidence || !cwd) return null;
  // An observed endpoint/disconnection boundary cannot regain old evidence
  // merely by switching back. A query for another cwd does not end this stream.
  if (!sameCurrentOwner(client, evidence, evidence.cwd)) { currentEvidence.delete(client); return null; }
  if (evidence.cwd !== cwd || !evidence.subscriptionId || evidence.subscriptionId !== evidence.eventId || !evidence.local) return null;
  return { origin: evidence.origin, environmentId: evidence.environmentId, generation: evidence.generation,
    cwd, subscriptionId: evidence.subscriptionId, status: cloneLocal(evidence.local)! };
}

/** The serial of a transport request id (`<generation>-<serial>`), or 0. */
export const subscriptionSerial = (id: string): number => { const serial = Number(id.split('-').pop()); return Number.isFinite(serial) ? serial : 0; };

const EMPTY_REMOTE = { hasUpstream: false, aheadCount: 0, behindCount: 0, pr: null };
const LOCAL_KEYS = ['isRepo', 'sourceControlProvider', 'hasPrimaryRemote', 'isDefaultRef', 'refName', 'hasWorkingTreeChanges', 'workingTree', 'branchChanges'];
const REMOTE_KEYS = ['hasUpstream', 'aheadCount', 'behindCount', 'aheadOfDefaultCount', 'pr'];
const pick = (value: Obj, keys: string[]) => Object.fromEntries(keys.filter(key => value[key] !== undefined).map(key => [key, value[key]]));

/** applyGitStatusStreamEvent. */
export function applyStatusEvent(current: Obj | null, event: Obj): Obj | null {
  if (event._tag === 'snapshot') return { ...obj(event.local), ...(event.remote ? obj(event.remote) : EMPTY_REMOTE) };
  if (event._tag === 'localUpdated') return { ...obj(event.local), ...(current ? pick(current, REMOTE_KEYS) : EMPTY_REMOTE) };
  if (event._tag === 'remoteUpdated') {
    const local = current ? pick(current, LOCAL_KEYS) : { isRepo: true, hasPrimaryRemote: false, isDefaultRef: false, refName: null, hasWorkingTreeChanges: false, workingTree: { files: [], insertions: 0, deletions: 0 } };
    return { ...local, ...(event.remote ? obj(event.remote) : EMPTY_REMOTE) };
  }
  return current;
}

const RECENT = 8;
function remember(state: VcsState, cwd: string, status: Obj): void {
  state.recent.delete(cwd); state.recent.set(cwd, status);
  while (state.recent.size > RECENT) state.recent.delete(state.recent.keys().next().value!);
}

/** One `shell-vcs-status` inbox entry: the newest stream since the latest subscribe is the card's. */
export function vcsStatusEvent(client: T3Client, entry: Obj): void {
  const state = states.get(client);
  if (!state || !state.cwd) return;
  observeCurrentEvent(client, state.cwd, entry);
  const id = str(entry.subscriptionId), serial = subscriptionSerial(id);
  state.maxSeen = Math.max(state.maxSeen, serial);
  if (serial <= state.floor || (state.id && serial < subscriptionSerial(state.id))) return;
  if (id !== state.id) state.id = id;
  const item = obj(entry.value);
  if (item._retryDue || item._streamEnded) return;
  if (item._transportError) { state.error = str(obj(item._transportError).message, 'Git status is unavailable.'); return; }
  state.status = applyStatusEvent(state.status, item);
  state.error = '';
  if (state.status) remember(state, state.cwd, state.status);
}

/**
 * Keeps the stream on `cwd` ('' ends it) and returns the latest status. The
 * first answer after a change of workspace has no status yet; the stream's
 * snapshot revises the client, which asks the card again.
 */
export async function watchVcsStatus(client: T3Client, native: Native, cwd: string, now: number): Promise<{ status: Obj | null; error: string }> {
  const state = vcsState(client);
  const previousEvidence = currentEvidence.get(client);
  if (previousEvidence && !sameCurrentOwner(client, previousEvidence, cwd)) currentEvidence.delete(client);
  // A new connection carries no streams: subscribe again.
  if (state.generation !== client.generation) { state.generation = client.generation; state.id = ''; state.status = null; state.error = ''; state.tried = false; }
  if (state.cwd !== cwd) {
    const previous = state.id;
    // vcs.ts keeps a workspace's status for an idle moment: a card that returns to it shows the last one until the stream answers.
    state.cwd = cwd; state.id = ''; state.status = state.recent.get(cwd) ?? null; state.error = ''; state.tried = false;
    if (previous && !cwd) await client.restAccess(native).call({ op: 'unsubscribe', key: VCS_STATUS_KEY }).catch(() => undefined);
  }
  const due = !state.tried || now - state.attemptAt >= RETRY_MS || (now === state.attemptAt && client.revision !== state.attemptRevision);
  if (cwd && !state.id && due) {
    state.tried = true; state.attemptAt = now; state.attemptRevision = client.revision;
    // Every stream seen so far is older than the one this asks for.
    state.floor = state.maxSeen;
    const evidence: CurrentEvidence = { origin: client.origin, environmentId: client.environmentId, generation: client.generation,
      cwd, subscriptionId: '', eventId: '', local: null, floor: state.maxSeen };
    currentEvidence.set(client, evidence);
    try {
      const reply = await client.restAccess(native).call({ op: 'subscribe', key: VCS_STATUS_KEY, method: 'subscribeVcsStatus', payload: { cwd } });
      const id = str(reply.id), serial = subscriptionSerial(id);
      if (currentEvidence.get(client) === evidence && sameCurrentOwner(client, evidence, cwd) && currentId(evidence, id)) {
        evidence.subscriptionId = id;
        if (serial > subscriptionSerial(evidence.eventId)) { evidence.eventId = id; evidence.local = null; }
      }
      state.maxSeen = Math.max(state.maxSeen, serial);
      if (state.cwd === cwd && serial > state.floor && (!state.id || serial > subscriptionSerial(state.id))) state.id = id;
    } catch (error) {
      if (currentEvidence.get(client) === evidence) currentEvidence.delete(client);
      if (state.cwd === cwd && !state.id && !letGo(error)) state.error = error instanceof Error ? error.message : 'Git status is unavailable.';
    }
  }
  return { status: state.status, error: state.error };
}
/**
 * BranchToolbarBranchSelector's `branchStatusQuery.refresh()` after a branch action: the stream on
 * `cwd` subscribes again. The last status stays until the new snapshot, as a refreshing query keeps
 * its data (the strip shows the switched name meanwhile, composer-controls-branch.ts).
 */
export function restartVcsStatus(client: T3Client, cwd: string): void {
  const state = states.get(client);
  if (!state || !cwd || state.cwd !== cwd) return;
  currentEvidence.delete(client);
  state.id = ''; state.tried = false; state.floor = state.maxSeen;
}
/** Whether the stream follows `cwd` now (its status may still be on its way). */
export function vcsStreamFollows(client: T3Client, cwd: string): boolean { return !!cwd && states.get(client)?.cwd === cwd; }
/** r7-handoff: the streamed status for `cwd` when the card follows (or recently followed) it, without asking. */
export function peekVcsStatus(client: T3Client, cwd: string): Obj | null { const state = states.get(client); return !state || !cwd ? null : state.cwd === cwd ? state.status ?? state.recent.get(cwd) ?? null : state.recent.get(cwd) ?? null; }

const focusSeen = new WeakMap<T3Client, boolean>();
/**
 * GitActionsControl's window listeners: `focus`, and `visibilitychange` to visible, ask
 * vcs.refreshStatus for the card's workspace. `focused` is the window having the focus
 * while visible (exactPage()); a rise asks once. The reference's 250 ms debounce merges
 * a focus and a visibility change that come together, which here are one answer's
 * arguments already (a data module has no timer, X19).
 */
export async function refreshVcsOnFocus(client: T3Client, native: Native, cwd: string, focused: boolean): Promise<boolean> {
  const previous = focusSeen.get(client);
  focusSeen.set(client, focused);
  if (!focused || previous !== false || !cwd) return false;
  try { await client.restAccess(native).request('vcs.refreshStatus', { cwd }); return true; }
  catch (error) { if (letGo(error)) throw error; return false; }
}
