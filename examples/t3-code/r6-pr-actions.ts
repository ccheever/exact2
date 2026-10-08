// Lane r6-pr: what the details card's pull request row does against the host
// (MIT reference, see LICENSE-T3, upstream f90b77d809):
// - ThreadDetailsPrRow's detail and checks reads (useEnvironmentQuery, one read
//   in flight per reference, a 10-minute refresh while the pull request is open);
// - usePullRequestActionRunner: Ready and Merge through `pullRequests.runAction`,
//   the toasts every surface says the same way, a fresh read after success;
// - usePullRequestHandoffs.startHandoff: Resolve and Fix open a thread on the
//   project, check the pull request out with `git.preparePullRequestThread`
//   (a worktree), point the draft at that checkout and leave the task in its
//   composer for the reader to send;
// - the Merge AlertDialog ("Merge pull request?") and the checks popover's
//   "Show all".
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { dismissToast, pushToast } from './toast';
import { ensureDraftThreadId, withHandoffThread } from './r7-handoff-thread'; // lane r7-handoff
import { ACTION_FAILURE, ACTION_HINT, ACTION_SUCCESS, actionPayload, fixChecksPrompt, handoffPrompt, preparePayload, readableFailure, resolveConflictsPrompt, type MergeMethod } from './r6-pr-logic';
import { letGo } from './let-go';

type Entry = { at: number; checksAt: number; detail: Obj | null; error: string; reading: boolean };
type State = {
  details: Map<string, Entry>;
  /** usePullRequestActionRunner.actionPending, per reference. */
  acting: Set<string>;
  /** usePullRequestHandoffs.handoff: one at a time, whatever surface pressed it. */
  handoff: string;
  /** ChecksBody's showAll, per row. */
  showAll: Set<string>;
  /** The Merge AlertDialog: the reference it asks about and the method it names. */
  confirm: { reference: Obj; number: number; method: MergeMethod } | null;
  /** lastHandoffPromptByDraft. */
  lastHandoff: Map<string, string>;
};
const states = new WeakMap<T3Client, State>();
export function prState(client: T3Client): State {
  let state = states.get(client);
  if (!state) { state = { details: new Map(), acting: new Set(), handoff: '', showAll: new Set(), confirm: null, lastHandoff: new Map() }; states.set(client, state); }
  return state;
}
const caps = (client: T3Client): Obj => obj(obj(obj(client.config).environment).capabilities);
/** A PullRequestRef as sent: an empty host is the project's own and is left out. */
export function normalRef(reference: Obj): Obj {
  const host = str(reference.host);
  return { projectId: str(reference.projectId), ...(host ? { host } : {}), repository: str(reference.repository), number: num(reference.number) };
}
export const refKey = (client: T3Client, reference: Obj) => JSON.stringify([client.environmentId, normalRef(reference)]);

const REFRESH_MS = 10 * 60_000; // useLiveRefresh(…, { intervalMs: 10 * 60_000 }) while open
const RETRY_MS = 30_000;
/**
 * pullRequestEnvironment.detail merged with .checks where the environment reports them. Each answer
 * reads for itself and shares only resolved values. Since exact2 f96641ddd an answer awaiting another
 * answer's fetch waits for it while that answer lives, but a let-go answer's native replies are still
 * dropped (#109), so a shared in-flight promise could hang every answer that awaits it. An entry whose
 * read is still out (or was dropped with its answer) is not fresh, so the next answer reads again; the
 * latest read to finish is the one kept.
 */
export async function readDetail(client: T3Client, native: Native, reference: Obj | null, now: number): Promise<Obj | null> {
  if (!reference || num(reference.number) <= 0 || !str(reference.repository)) return null;
  const state = prState(client), key = refKey(client, reference), cached = state.details.get(key);
  if (cached && !cached.reading) {
    const open = cached.detail?.state === 'open';
    const fresh = cached.error ? !now || now - cached.at < RETRY_MS : !open || !now || now - cached.at < REFRESH_MS;
    // usePullRequestChecksRefresh: the checks alone, every 45 s while runs are pending (or none reported), else 60 s.
    if (fresh && open && now && cached.detail && caps(client).pullRequestChecks === true) {
      const checks = arr(cached.detail.checks), busy = !checks.length || checks.some(check => check.status === 'pending' || check.status === 'action-required');
      if (now - cached.checksAt >= (busy ? 45_000 : 60_000)) {
        cached.checksAt = now;
        const update = await client.rpc(native, 'pullRequests.checks', normalRef(reference)).catch(() => null);
        if (update && Array.isArray(obj(update).checks) && cached.detail) cached.detail = { ...cached.detail, ...obj(update) };
        return cached.detail;
      }
    }
    if (fresh) return cached.detail;
  }
  const entry: Entry = { at: now, checksAt: now, detail: cached?.detail ?? null, error: '', reading: true };
  state.details.set(key, entry);
  try {
    const ref = normalRef(reference);
    let merged = obj(await client.rpc(native, 'pullRequests.detail', { ...ref, allowStale: false }));
    if (caps(client).pullRequestChecks === true && num(merged.number) > 0) {
      const checks = await client.rpc(native, 'pullRequests.checks', ref).catch((error: unknown) => { if (letGo(error)) throw error; return null; });
      if (checks && typeof checks === 'object' && Array.isArray(obj(checks).checks)) merged = { ...merged, ...obj(checks) };
    }
    entry.detail = num(merged.number) > 0 ? merged : null;
  } catch (error) {
    // real-github-lane: an answer let go mid-read (Exact replaced it, e.g. for a new revision while
    // GitHub was still answering) settles nothing. Storing it as a finished read with no detail and
    // no error made every later answer treat "nothing" as fresh, so the row never split.
    if (letGo(error)) {
      if (state.details.get(key) === entry) { if (cached) state.details.set(key, cached); else state.details.delete(key); }
      throw error;
    }
    entry.error = error instanceof Error ? error.message : String(error);
  }
  entry.reading = false;
  state.details.set(key, entry);
  return entry.detail;
}
/** The detail already read for a reference, without asking (a tab icon is drawn synchronously). */
export function peekDetail(client: T3Client, reference: Obj): Obj | null { return prState(client).details.get(refKey(client, reference))?.detail ?? null; }
export function forgetDetails(client: T3Client, reference?: Obj): void {
  const state = prState(client);
  if (reference) state.details.delete(refKey(client, reference)); else state.details.clear();
}

/** usePullRequestActionRunner.perform: one host action, toasted the reference's way, then a fresh read. */
export async function performAction(client: T3Client, native: Native, reference: Obj, action: 'merge' | 'ready', method?: MergeMethod): Promise<string> {
  const state = prState(client), key = refKey(client, reference);
  if (state.acting.has(key)) return '';
  state.acting.add(key);
  try {
    await client.rpc(native, 'pullRequests.runAction', actionPayload(reference, action, action === 'merge' ? method ?? 'merge' : undefined), true);
  } catch (error) {
    if (letGo(error)) throw error;
    // The host's own sentence, because it is the only thing that says why.
    pushToast(client, { kind: 'error', title: ACTION_FAILURE[action]!, description: readableFailure(error, ACTION_HINT[action]!) });
    return '';
  } finally { state.acting.delete(key); }
  pushToast(client, { kind: 'success', title: ACTION_SUCCESS[action]! });
  forgetDetails(client, reference);
  return '';
}

export const isActing = (client: T3Client, reference: Obj | null) => !!reference && prState(client).acting.has(refKey(client, reference));

/** The Merge AlertDialog opens on the row's Merge and closes on Cancel, Escape or its own Merge. */
export function askMerge(client: T3Client, reference: Obj, number: number, method: MergeMethod): void { prState(client).confirm = { reference: normalRef(reference), number, method }; }
export function closeMerge(client: T3Client): void { prState(client).confirm = null; }
export type MergeAsk = { open: boolean; number: number; description: string; target: string; pending: boolean };
export function mergeAsk(client: T3Client): MergeAsk {
  const confirm = prState(client).confirm;
  if (!confirm) return { open: false, number: 0, description: '', target: '', pending: false };
  return { open: true, number: confirm.number, description: `This merges #${confirm.number} using ${confirm.method}.`, target: JSON.stringify({ ...confirm.reference, method: confirm.method }), pending: isActing(client, confirm.reference) };
}
export async function confirmMerge(client: T3Client, native: Native, target: string): Promise<string> {
  const parsed = obj(JSON.parse(target || '{}'));
  const method = (['merge', 'squash', 'rebase'].includes(str(parsed.method)) ? str(parsed.method) : 'merge') as MergeMethod;
  closeMerge(client);
  return performAction(client, native, parsed, 'merge', method);
}

export function toggleShowAll(client: T3Client, rowKey: string): void {
  const set = prState(client).showAll;
  if (set.has(rowKey)) set.delete(rowKey); else set.add(rowKey);
}

/**
 * usePullRequestHandoffs.startHandoff: open a thread on the pull request's project, check the
 * pull request out into its own worktree, point the thread at it, and leave the task in its
 * composer for the reader to read over and send. Nothing is sent.
 */
export async function startHandoff(client: T3Client, native: Native, kind: 'conflicts' | 'findings', detail: Obj): Promise<string> {
  const state = prState(client);
  if (state.handoff) return '';
  const task = kind === 'conflicts'
    ? resolveConflictsPrompt({ number: num(detail.number), url: str(detail.url), headBranch: str(detail.headBranch), baseBranch: str(detail.baseBranch) })
    : fixChecksPrompt({ number: num(detail.number), title: str(detail.title), url: str(detail.url), headBranch: str(detail.headBranch), baseBranch: str(detail.baseBranch), checks: arr(detail.checks) });
  state.handoff = kind;
  const toastKey = 'pr-handoff';
  const loading = pushToast(client, { kind: 'loading', title: 'Preparing the pull request checkout...', key: toastKey });
  try {
    const projectId = str(detail.projectId);
    if (!client.shell.projects.some(project => project.id === projectId)) {
      pushToast(client, { kind: 'error', title: 'Could not open a thread for the checkout', description: 'Try again from the project, or open a thread first.', key: toastKey });
      return '';
    }
    await client.openProjectDraft(native, projectId);
    const draftKey = client.draftKey;
    // r7-handoff: the thread is opened (its id allocated) before the checkout, so the server runs the setup script for it.
    let threadId = '';
    try { threadId = await ensureDraftThreadId(client, native, draftKey); }
    catch {
      pushToast(client, { kind: 'error', title: 'Could not open a thread for the checkout', description: 'Try again from the project, or open a thread first.', key: toastKey });
      return '';
    }
    let prepared: Obj;
    try { prepared = obj(await client.rpc(native, 'git.preparePullRequestThread', withHandoffThread(preparePayload(detail), threadId), true)); }
    catch (error) {
      if (letGo(error)) { dismissToast(client, loading); throw error; } // let-go.ts: no failure toast
      const description = error instanceof Error ? error.message : '';
      pushToast(client, { kind: 'error', title: 'Could not prepare the pull request checkout', ...(description ? { description } : {}), key: toastKey });
      return '';
    }
    const branch = str(prepared.branch), worktreePath = str(prepared.worktreePath);
    if (client.draftKey !== draftKey || !branch) {
      pushToast(client, { kind: 'error', title: 'Checked out, but the thread stayed where it was',
        description: `The checkout is ready on \`${branch}\`. Point a thread at it from the branch picker, then ask again.`, key: toastKey });
      return '';
    }
    // newThread(projectRef, { branch, worktreePath, envMode }): the same draft, now on the checkout.
    (client.local.composerControls.contexts ??= {})[draftKey] = { envMode: worktreePath ? 'worktree' : 'local', branch, worktreePath };
    client.local.drafts[draftKey] = handoffPrompt({ prompt: client.local.drafts[draftKey] ?? '', lastHandoffPrompt: state.lastHandoff.get(draftKey) }, task);
    state.lastHandoff.set(draftKey, task);
    if (prepared.isOnPullRequestHead === false) {
      pushToast(client, { kind: 'warning', title: 'Checked out, but not on the latest commits', key: toastKey,
        description: "The checkout could not be moved onto the pull request's latest commits, so the code there is older than the pull request. Uncommitted work or local commits keep it where it is." });
    } else pushToast(client, { kind: 'success', title: 'Checkout ready', description: 'The task is in the composer — read it over, then send.', key: toastKey });
    return '';
  } finally { state.handoff = ''; }
}
