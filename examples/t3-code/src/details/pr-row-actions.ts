// What the details card's pull request row does against the host
// (MIT reference, see LICENSE-T3, upstream 7c3b3b3605ef):
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
import { sharedPullRequestSummary } from '../pull-requests/pages-pr-shared';
import { readerIdle } from '../pull-requests/pages-pr-refresh';
import type { T3Client } from '../client/client';
import { arr, num, obj, str, type Obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { dismissToast, pushToast } from '../shell/toast';
import { ensureDraftThreadId, withHandoffThread } from '../composer/draft-thread-id';
import { contextReferences } from '../composer/composer-editor-menu';
import { rememberReviewCommentRecord } from '../composer/composer-editor';
import { appendInlineContextReference, chipLink, chipRecord, handoffReviewComments, setReviewCommentLinks, stripPullRequestHandoffReferences, type HandoffComment } from '../pull-requests/pages-pr-handoffs-logic';
import { actionFailureHints, actionFailureLabels, actionPayload, actionSuccessLabels, checkoutFailureDescription, checksRefreshIntervalMs, fixChecksPrompt, handoffPrompt, offHeadCheckoutToast, preparePayload, readableFailure, resolveConflictsPrompt, resolvePullRequestDetailChecks, type MergeMethod } from './pr-row-logic';
import { terminologyForUrl, terminologyOf, type Terminology } from './git-logic';
import { letGo } from '../client/let-go';

/** `checksNone`: the host answered the checks with null, which is settled: polling cannot find checks. */
type Entry = { at: number; checksAt: number; detail: Obj | null; error: string; reading: boolean; checksNone?: boolean };
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
  /** A checkout hand-off whose thread is open and whose checkout is still to run (the page shows the thread in between). */
  next: Continuation | null;
};
const states = new WeakMap<T3Client, State>();
export function prState(client: T3Client): State {
  let state = states.get(client);
  if (!state) { state = { details: new Map(), acting: new Set(), handoff: '', showAll: new Set(), confirm: null, lastHandoff: new Map(), next: null }; states.set(client, state); }
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
export async function readDetail(client: T3Client, native: Native, reference: Obj | null, now: number, poll?: { visible: boolean }): Promise<Obj | null> {
  if (!reference || num(reference.number) <= 0 || !str(reference.repository)) return null;
  const state = prState(client), key = refKey(client, reference), cached = state.details.get(key);
  if (cached && !cached.reading) {
    const known = sharedPullRequestSummary(client, reference, cached.detail);
    if (known && cached.detail) cached.detail = { ...cached.detail, ...known };
    const open = cached.detail?.state === 'open';
    const fresh = cached.error ? !now || now - cached.at < RETRY_MS : !open || !now || now - cached.at < REFRESH_MS;
    // usePullRequestChecksRefresh, for the card's row (`poll`): the checks alone on the row's timer (rowChecksPoll), every
    // 45 s while runs are pending (or none reported), else 60 s, in a visible window while the reader is about; the
    // answer, newer than the detail it lands on, takes over only the state and the checks.
    if (fresh && poll?.visible && !readerIdle(client, now) && open && now && cached.detail && !cached.checksNone && caps(client).pullRequestChecks === true) {
      if (now - cached.checksAt >= checksRefreshIntervalMs(arr(cached.detail.checks)) - CHECKS_TICK_SLACK_MS) {
        cached.checksAt = now;
        // A failed read is tried again at the next interval; a null answer ends the polling.
        const update = await client.rpc(native, 'pullRequests.checks', normalRef(reference)).catch((error: unknown) => { if (letGo(error)) throw error; return undefined; });
        if (update === null) cached.checksNone = true;
        else if (update && cached.detail) cached.detail = resolvePullRequestDetailChecks(cached.detail, obj(update), true);
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
      const checks = await client.rpc(native, 'pullRequests.checks', ref).catch((error: unknown) => { if (letGo(error)) throw error; return undefined; });
      if (checks === null) entry.checksNone = true;
      else if (checks && typeof checks === 'object') merged = resolvePullRequestDetailChecks(merged, obj(checks), true) ?? merged;
    }
    entry.detail = num(merged.number) > 0 ? merged : null;
  } catch (error) {
    // An answer let go mid-read (Exact replaced it, e.g. for a new revision while
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
/** How early a timer tick may land and still count as the interval come round. */
const CHECKS_TICK_SLACK_MS = 1_000;
/**
 * The row's checks timer (shell-members.contract `prRowChecksTick`): "pending" (45 s), "settled" (60 s), or "" while the
 * pull request is not open, its host reports no checks or answered them null, the window is hidden or the reader idle.
 */
export function rowChecksPoll(client: T3Client, reference: Obj | null, now: number, visible: boolean): '' | 'pending' | 'settled' {
  const cached = reference ? prState(client).details.get(refKey(client, reference)) : undefined, detail = cached?.detail;
  if (!detail || detail.state !== 'open' || cached!.checksNone || !visible || readerIdle(client, now) || caps(client).pullRequestChecks !== true) return '';
  return checksRefreshIntervalMs(arr(detail.checks)) === 45_000 ? 'pending' : 'settled';
}
/** The detail already read for a reference, without asking (a tab icon is drawn synchronously). */
export function peekDetail(client: T3Client, reference: Obj): Obj | null { return sharedPullRequestSummary(client, reference, prState(client).details.get(refKey(client, reference))?.detail ?? null); }
export function forgetDetails(client: T3Client, reference?: Obj): void {
  const state = prState(client);
  if (reference) state.details.delete(refKey(client, reference)); else state.details.clear();
}

/** ThreadDetailsPrRow's terminology: the host of the detail read, else of the project's repository. */
export function rowTerminology(client: T3Client, reference: Obj, detail: Obj | null = peekDetail(client, reference)): Terminology {
  const project = client.shell.projects.find(entry => entry.id === str(reference.projectId));
  return terminologyOf(str(detail?.provider) || str(obj(project?.repositoryIdentity).provider) || undefined);
}

/** usePullRequestActionRunner.perform: one host action, toasted the reference's way, then a fresh read. */
export async function performAction(client: T3Client, native: Native, reference: Obj, action: 'merge' | 'ready', method?: MergeMethod): Promise<string> {
  const state = prState(client), key = refKey(client, reference), term = rowTerminology(client, reference).singular;
  if (state.acting.has(key)) return '';
  state.acting.add(key);
  try {
    await client.rpc(native, 'pullRequests.runAction', actionPayload(reference, action, action === 'merge' ? method ?? 'merge' : undefined), true);
  } catch (error) {
    if (letGo(error)) throw error;
    // The host's own sentence, because it is the only thing that says why.
    pushToast(client, { kind: 'error', title: actionFailureLabels(term)[action]!, description: readableFailure(error, actionFailureHints(term)[action]!) });
    return '';
  } finally { state.acting.delete(key); }
  pushToast(client, { kind: 'success', title: actionSuccessLabels(term)[action]! });
  forgetDetails(client, reference);
  return '';
}

export const isActing = (client: T3Client, reference: Obj | null) => !!reference && prState(client).acting.has(refKey(client, reference));

/** The Merge AlertDialog opens on the row's Merge and closes on Cancel, Escape or its own Merge. */
export function askMerge(client: T3Client, reference: Obj, number: number, method: MergeMethod): void { prState(client).confirm = { reference: normalRef(reference), number, method }; }
export function closeMerge(client: T3Client): void { prState(client).confirm = null; }
export type MergeAsk = { open: boolean; number: number; title: string; description: string; target: string; pending: boolean };
export function mergeAsk(client: T3Client): MergeAsk {
  const confirm = prState(client).confirm;
  if (!confirm) return { open: false, number: 0, title: '', description: '', target: '', pending: false };
  return { open: true, number: confirm.number, title: `Merge ${rowTerminology(client, confirm.reference).singular}?`, description: `This merges #${confirm.number} using ${confirm.method}.`,
    target: JSON.stringify({ ...confirm.reference, method: confirm.method }), pending: isActing(client, confirm.reference) };
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

// ── Hand-offs (usePullRequestHandoffs, generalized for the panel) ──

/** PullRequestThreadTask: a prompt, and the chips that go with it (pages-pr-handoffs-logic.ts). */
export type HandoffTask = { prompt: string; reviewComments?: HandoffComment[] };
/** The hand-off chips this client wrote, by the context id their links carry (the clone's drafts hold chips as links). */
const handoffChips = new WeakMap<object, Map<string, HandoffComment>>();
const chipsOf = (client: object) => { let map = handoffChips.get(client); if (!map) { map = new Map(); handoffChips.set(client, map); } return map; };
/** The hand-off chips a prompt holds now: the draft's reviewComments as the reference keeps them. */
function draftChips(client: object, prompt: string): HandoffComment[] {
  const known = chipsOf(client);
  return contextReferences(prompt).flatMap(reference => (reference.kind === 'review-comment' && known.has(reference.id) ? [known.get(reference.id)!] : []));
}
/**
 * writeTaskToComposer over a draft: the latest press is the ask — it takes over what an earlier
 * hand-off left, prompt and chips both, and what the reader typed themselves survives. Returns the prompt.
 */
export function writeTaskToDraft(client: T3Client, draftKey: string, task: HandoffTask, commit = true, source = client.local.drafts[draftKey] ?? ''): string {
  const state = prState(client), prompt = source;
  const existing = draftChips(client, prompt), incoming = task.reviewComments ?? [];
  const previousIds = new Set(existing.map(comment => comment.id));
  const repeated = new Set(incoming.filter(comment => previousIds.has(comment.id)).map(comment => comment.id));
  let next = handoffPrompt({ prompt: stripPullRequestHandoffReferences(prompt, existing, repeated), lastHandoffPrompt: state.lastHandoff.get(draftKey) }, task.prompt);
  // Only the hand-off's own sentence is this session's to take back next time.
  if (commit) state.lastHandoff.set(draftKey, task.prompt);
  next = setReviewCommentLinks(next, existing, handoffReviewComments(existing, incoming));
  for (const comment of incoming) if (repeated.has(comment.id)) next = appendInlineContextReference(next, chipLink(comment));
  for (const comment of incoming) {
    const record = chipRecord(comment);
    chipsOf(client).set(str(record.contextId), comment);
    rememberReviewCommentRecord(client, record);
  }
  if (commit) client.local.drafts[draftKey] = next;
  return next;
}
const NO_THREAD = { kind: 'error' as const, title: 'Could not open a thread', description: 'Try again from the project, or open a thread first.' };
/** openThreadWithTask: the project's draft, with the task written into it. False when no thread could be opened. */
async function openThreadWithTask(client: T3Client, native: Native, projectId: string, task: HandoffTask | null): Promise<boolean> {
  if (!client.shell.projects.some(project => project.id === projectId)) return false;
  try { await client.openProjectDraft(native, projectId); } catch (error) { if (letGo(error)) throw error; return false; }
  if (task) writeTaskToDraft(client, client.draftKey, task);
  return true;
}
/** The panel's resource and the shell read again now, so "Opening..." and the loading toast show while the hand-off works. */
async function wakeHandoff(native: Native): Promise<void> {
  for (const topic of ['t3.pr', 't3.notify']) { try { await native.later({ op: 'wake', topic }); } catch (error) { if (letGo(error)) throw error; } }
}

/** The question's toast: the question is in the composer, or (an empty prompt) the change request is, for the reader to ask. */
export const askedDescription = (task: HandoffTask, term: Terminology) => (task.prompt.length > 0
  ? 'The question is in the composer. Read it over, then send.' : `The ${term.singular} is in the composer. Type your question, then send.`);

/** startAsk: a question about the change (`detail`: its project and URL), which needs a thread and nothing else. True when a thread was opened. */
export async function askInThread(client: T3Client, native: Native, kind: string, task: HandoffTask, detail: Obj): Promise<boolean> {
  const projectId = str(detail.projectId);
  const state = prState(client);
  if (state.handoff) return false;
  state.handoff = kind;
  let opened = false;
  try { await wakeHandoff(native); opened = await openThreadWithTask(client, native, projectId, task); }
  finally { state.handoff = ''; }
  if (!opened) { pushToast(client, NO_THREAD); return false; }
  // "Ask" leaves the composer empty on purpose: the chips are what landed. Prompt links hand over a URL alone, so the URL names the host.
  pushToast(client, { kind: 'success', title: 'Asked in a thread', description: askedDescription(task, terminologyForUrl(str(detail.url))) });
  return true;
}

type Continuation = { kind: string; task: HandoffTask | null; detail: Obj; mode: 'worktree' | 'local'; draftKey: string; threadId: string; loading: number };
const TOAST_KEY = 'pr-handoff';
const noThreadForCheckout = (client: T3Client) => { pushToast(client, { kind: 'error', title: 'Could not open a thread for the checkout', description: 'Try again from the project, or open a thread first.', key: TOAST_KEY }); };

/**
 * usePullRequestHandoffs.startHandoff, its first half: the hand-off is held (one at a time), the loading
 * toast shows, and the thread is opened (its id allocated) before the checkout, so the server runs the
 * setup script for it. True when the thread is open and `finishCheckoutHandoff` should run the checkout;
 * the window can show the thread in between, as the reference's `newThread` navigates before it prepares.
 */
export async function beginCheckoutHandoff(client: T3Client, native: Native, kind: string, task: HandoffTask | null, detail: Obj, mode: 'worktree' | 'local' = 'worktree'): Promise<boolean> {
  const state = prState(client);
  if (state.handoff) return false;
  state.handoff = kind;
  // The menu closes on the press, so this is the only thing answering for the checkout; a loading toast never expires.
  const loading = pushToast(client, { kind: 'loading', title: `Preparing the ${terminologyForUrl(str(detail.url)).singular} checkout...`, key: TOAST_KEY });
  let begun = false;
  try {
    await wakeHandoff(native);
    if (!(await openThreadWithTask(client, native, str(detail.projectId), null))) { noThreadForCheckout(client); return false; }
    const draftKey = client.draftKey;
    let threadId = '';
    try { threadId = await ensureDraftThreadId(client, native, draftKey); }
    catch (error) { if (letGo(error)) throw error; noThreadForCheckout(client); return false; }
    state.next = { kind, task, detail, mode, draftKey, threadId, loading };
    begun = true;
    return true;
  } finally { if (!begun) state.handoff = ''; }
}

/**
 * startHandoff's second half: check the pull request out (into its own worktree, or this repository),
 * point the thread at it, and leave the task — if it carries one — in its composer for the reader to read
 * over and send. Nothing is sent.
 */
export async function finishCheckoutHandoff(client: T3Client, native: Native): Promise<string> {
  const state = prState(client), next = state.next;
  if (!next) return '';
  state.next = null;
  const { task, detail, mode, draftKey, threadId, loading } = next;
  const term = terminologyForUrl(str(detail.url));
  try {
    let prepared: Obj;
    try { prepared = obj(await client.rpc(native, 'git.preparePullRequestThread', withHandoffThread(preparePayload(detail, mode), threadId), true)); }
    catch (error) {
      if (letGo(error)) { dismissToast(client, loading); throw error; } // let-go.ts: no failure toast
      // The server says what to do about it (that the branch is already checked out, say), else the hint.
      pushToast(client, { kind: 'error', title: `Could not prepare the ${term.singular} checkout`, description: checkoutFailureDescription(error), key: TOAST_KEY });
      return '';
    }
    const branch = str(prepared.branch), worktreePath = str(prepared.worktreePath);
    if (client.draftKey !== draftKey || !branch) {
      pushToast(client, { kind: 'error', title: 'Checked out, but the thread stayed where it was',
        description: `The checkout is ready on \`${branch}\`. Point a thread at it from the branch picker, then ask again.`, key: TOAST_KEY });
      return '';
    }
    // newThread(projectRef, { branch, worktreePath, envMode }): the same draft, now on the checkout; a local one runs where the repository is.
    (client.local.composerControls.contexts ??= {})[draftKey] = { envMode: worktreePath ? 'worktree' : 'local', branch, worktreePath };
    const stale = prepared.isOnPullRequestHead === false;
    const staleToast = { ...offHeadCheckoutToast(term), key: TOAST_KEY };
    if (!task) {
      pushToast(client, stale ? staleToast : { kind: 'success', title: mode === 'local' ? 'Checked out here' : 'Checked out', key: TOAST_KEY,
        description: mode === 'local' ? `This repository is on the ${term.singular}'s branch, with a thread open on it.` : `The ${term.singular} is in its own worktree, with a thread open on it.` });
      return '';
    }
    writeTaskToDraft(client, draftKey, task);
    pushToast(client, stale ? staleToast : { kind: 'success', title: 'Checkout ready', description: 'The task is in the composer. Read it over, then send.', key: TOAST_KEY });
    return '';
  } finally { state.handoff = ''; }
}

/** startHandoff in one go, where nothing has to show between the halves. True when a thread was opened. */
export async function checkoutHandoff(client: T3Client, native: Native, kind: string, task: HandoffTask | null, detail: Obj, mode: 'worktree' | 'local' = 'worktree'): Promise<boolean> {
  if (!(await beginCheckoutHandoff(client, native, kind, task, detail, mode))) return false;
  await finishCheckoutHandoff(client, native);
  return true;
}

/** The thread card's Resolve and Fix (ThreadDetailsPrRow): the row's own task, checked out into a worktree. */
export async function startHandoff(client: T3Client, native: Native, kind: 'conflicts' | 'findings', detail: Obj): Promise<string> {
  const task = kind === 'conflicts'
    ? resolveConflictsPrompt({ number: num(detail.number), url: str(detail.url), headBranch: str(detail.headBranch), baseBranch: str(detail.baseBranch) })
    : fixChecksPrompt({ number: num(detail.number), title: str(detail.title), url: str(detail.url), headBranch: str(detail.headBranch), baseBranch: str(detail.baseBranch), checks: arr(detail.checks) });
  await checkoutHandoff(client, native, kind, { prompt: task }, detail);
  return '';
}
