// The Pull Requests page's row extras (pr-handoffs-and-quick-actions): the Shift quick actions
// (PullRequestSpeedActions: Reopen, Close + Ready for review, Close + Merge, shown while Shift alone is
// held), the checks popover (PullRequestChecksPopover on a listing row: the rollup's headline, then the
// checks behind it read when it opens) and the stack popover (PullRequestStackPopover: "n/m", the
// layers read when it opens). Sources: T3 Code 1e2ecbd975 (MIT, see LICENSE-T3)
// apps/web/src/components/pullRequest/{PullRequestSpeedActions,PullRequestChecksPopover,
// PullRequestStackPopover,PullRequestStackLayers,PullRequestRow}.tsx, usePullRequestActions.ts
// (usePullRequestActionRunner's toasts), routes/_chat.pull-requests.tsx (speedMode, onSpeedAction).
//
// Speed mode is the native flags monitor's (T3Sidebar.swift `sidebarSpeedMode`: Shift alone, not while
// a text field or the composer has the focus, cleared when the app resigns), which wakes this page's
// resource (`t3.pr`) when it changes. A quick action runs as the panel's actions do (`pageslocal:pr-act-quick`,
// one command at a time), and a row's popovers read only once opened (`pageslocal:pr-quick-*`).
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { peekDetail, readDetail } from './r6-pr-actions';
import { ACTION_FAILURE, ACTION_HINT, CHECKS_HEADLINE, actionPayload, allowedMergeMethods, checksList, readableFailure, resolvePullRequestMergeMethod, type MergeMethod } from './r6-pr-logic';
import { actionState, lastMergeMethod, noteActed, projectDefaultMergeMethod } from './pages-pr-actions';
import { decodeStack, layerView, pullRequestStackView, savedPullRequestStack, type PullRequestStack, type StackLayerView } from './pages-pr-stack';

// ── State ───────────────────────────────────────────────────────────────────

type StackRead = { data: PullRequestStack | null; error: string | null; isSuccess: boolean; isPending: boolean };
type QuickState = {
  /** usePullRequestActionRunner.actionPending, per row (its entry key). */
  pending: Set<string>;
  /** The row whose checks popover is open (its ref), and its "Show all". */
  checks: string; showAll: boolean;
  /** The row whose stack popover is open, and what each row's stack read said. */
  stack: string; stacks: Map<string, StackRead>;
  speedMode: boolean;
  /** Popover reads shown as loading once before they are asked, so the loading line draws first. */
  shown: Set<string>;
};
const states = new WeakMap<object, QuickState>();
function quickState(client: object): QuickState {
  let state = states.get(client);
  if (!state) { state = { pending: new Set(), checks: '', showAll: false, stack: '', stacks: new Map(), speedMode: false, shown: new Set() }; states.set(client, state); }
  return state;
}
const entryKey = (entry: Obj) => `${str(entry.host)}:${str(entry.repository)}#${num(entry.number)}`;
const referenceOf = (entry: Obj): Obj => ({ projectId: str(entry.projectId), ...(str(entry.host) ? { host: str(entry.host) } : {}), repository: str(entry.repository), number: num(entry.number) });
const refOf = (entry: Obj) => JSON.stringify({ projectId: str(entry.projectId), host: str(entry.host), repository: str(entry.repository), number: num(entry.number) });

// ── Speed actions (PullRequestSpeedActions) ─────────────────────────────────

const ACTIONS: Record<string, { label: string; icon: string }> = {
  close: { label: 'Close', icon: 'git-pull-request-closed' }, merge: { label: 'Merge', icon: 'git-merge' },
  ready: { label: 'Ready for review', icon: 'git-pull-request-arrow' }, reopen: { label: 'Reopen', icon: 'git-pull-request-arrow' },
};
export type PrSpeedAction = { key: string; action: string; label: string; icon: string; destructive: boolean; disabled: boolean; tooltip: string; aria: string; value: string };
/** The row's quick actions: none on a merged or non-GitHub row; Reopen on a closed one, Close and Ready for review on a draft, else Close and Merge. */
export function speedActions(entry: Obj, pending: boolean): PrSpeedAction[] {
  if (entry.state === 'merged' || str(entry.provider, 'github') !== 'github') return [];
  const actions = entry.state === 'closed' ? ['reopen'] : entry.isDraft === true ? ['close', 'ready'] : ['close', 'merge'];
  const stacked = entry.stack !== undefined && entry.stack !== null;
  return actions.map(action => {
    const { label, icon } = ACTIONS[action]!;
    return { key: action, action, label, icon, destructive: action === 'close', disabled: pending || (action === 'merge' && stacked),
      tooltip: action === 'merge' && stacked ? 'Open this pull request to merge its stack' : `${label} immediately`, aria: `${label} #${num(entry.number)}`,
      value: JSON.stringify({ ref: refOf(entry), action }) };
  });
}
/** What a row adds to PullRequestRow: its quick actions, its stack trigger and its checks trigger's name. */
export function quickRow(client: object, entry: Obj) {
  const pending = quickState(client).pending.has(entryKey(entry));
  const stack = obj(entry.stack), member = num(stack.number) > 0;
  const position = num(stack.position), size = num(stack.size), number = num(stack.number);
  return {
    speed: speedActions(entry, pending), speedPending: pending,
    stackLabel: member ? `${position}/${size}` : '', stackAria: member ? `Stack ${number}, layer ${position} of ${size}` : '',
    stackTip: member ? `View stack #${number}, layer ${position} of ${size}` : '',
    // The checks popover's headline (pullRequestChecksStatePresentation's label).
    checksHeadline: CHECKS_HEADLINE[str(entry.checksState)] ?? 'No checks reported',
  };
}

/**
 * resolveMergeMethod: a merge from the list reads the detail around the cache first, and refuses
 * what the row cannot know — a pull request that is no longer open, a draft, one this viewer may not
 * merge, one in a host stack, a repository with no merge method allowed.
 */
async function resolveMergeMethod(client: T3Client, native: Native, entry: Obj): Promise<MergeMethod> {
  const reference = referenceOf(entry);
  const detail = obj(await client.rpc(native, 'pullRequests.detail', { ...reference, allowStale: false }));
  const names = (value: unknown) => (Array.isArray(value) ? value.map(String) : []);
  if (detail.state !== 'open' || detail.isDraft === true || !names(obj(detail.capabilities).actions).includes('merge') || !names(obj(detail.viewerPermissions).actions).includes('merge')) {
    throw new Error('This pull request cannot be merged.');
  }
  if (obj(detail.capabilities).stackActions === true && (await client.rpc(native, 'pullRequests.stack', reference)) !== null) throw new Error('Open this pull request to merge its stack.');
  const allowed = allowedMergeMethods(detail);
  if (!allowed.length) throw new Error('No merge method is available for this repository.');
  return resolvePullRequestMergeMethod(allowed, null, projectDefaultMergeMethod(obj(client.config.settings), str(entry.projectId)), lastMergeMethod(client));
}
/** usePullRequestActionRunner's success words, which a quick action says (the panel's are its own). */
const SPEED_SUCCESS: Record<string, string> = { merge: 'Merge requested', ready: 'Marked ready for review', close: 'Pull request closed', reopen: 'Pull request reopened' };

/**
 * `pageslocal:pr-act-quick`: one quick action on one row, no confirmation (PullRequestSpeedActions
 * perform). Success writes the row's new state over it (not a merge: the host's next read says so),
 * re-reads the list and the panel; failure says the host's sentence under the action's title.
 */
export async function runQuickAction(client: T3Client, native: Native, value: string, listEntry: (ref: string) => Obj | null, refreshPanels: () => void): Promise<string> {
  let parsed: Obj;
  try { parsed = obj(JSON.parse(value)); } catch { throw new ClientError('Unknown quick action.'); }
  const action = str(parsed.action), entry = listEntry(str(parsed.ref));
  if (!ACTIONS[action]) throw new ClientError(`Unknown quick action: ${action}`);
  if (!entry) throw new ClientError('That pull request is no longer in the list.');
  const state = quickState(client), key = entryKey(entry);
  if (state.pending.has(key)) return '';
  state.pending.add(key);
  try {
    try { await native.later({ op: 'r10Wake', topic: 't3.pr' }); } catch (error) { if (letGo(error)) throw error; }
    const method = action === 'merge' ? await resolveMergeMethod(client, native, entry) : undefined;
    await client.rpc(native, 'pullRequests.runAction', actionPayload(referenceOf(entry), action, method), true);
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: ACTION_FAILURE[action]!, description: readableFailure(error, ACTION_HINT[action]!) });
    return '';
  } finally { state.pending.delete(key); }
  pushToast(client, { kind: 'success', title: SPEED_SUCCESS[action]! });
  // onSpeedAction: some hosts accept a merge before it completes, so the next read declares it merged.
  if (action !== 'merge') { noteActed(client, entry, action, 'sent'); noteActed(client, entry, action, 'done'); }
  actionState(client).relist++;
  refreshPanels();
  return '';
}

// ── Speed mode ──────────────────────────────────────────────────────────────

/** The native monitor's speed mode (Shift alone), read on every answer of the page; false without a module. */
export async function readSpeedMode(client: T3Client, native: Native): Promise<boolean> {
  try { quickState(client).speedMode = obj(await client.restAccess(native).call({ op: 'sidebarSpeedMode' })).speedMode === true; }
  catch (error) { if (letGo(error)) throw error; quickState(client).speedMode = false; }
  return quickState(client).speedMode;
}

// ── Popovers ────────────────────────────────────────────────────────────────

export type PrRowCheck = { key: string; name: string; status: string; label: string; url: string; description: string };
export function emptyChecksPopover() {
  return { ref: '', headline: '', loading: false, error: '', empty: false, rows: [] as PrRowCheck[], collapsible: false, expanded: false };
}
export function emptyStackPopover() {
  return { ref: '', number: 0, label: '', notice: '', noticeText: '', retry: false, layers: [] as StackLayerView[], base: '', message: '' };
}
export type PrChecksPopover = ReturnType<typeof emptyChecksPopover>;
export type PrStackPopover = ReturnType<typeof emptyStackPopover>;

/** `pageslocal:pr-quick-*`: a row's checks or stack popover opened ("" closes), "Show all", "Retry stack refresh". */
export function prQuickLocal(client: object, op: string, value: string): string {
  const state = quickState(client);
  if (op === 'checks') { if (state.checks !== value) state.showAll = false; state.checks = value; return ''; }
  if (op === 'checks-all') { state.showAll = !state.showAll; return ''; }
  if (op === 'stack') { if (value && state.stack !== value) state.stacks.delete(value); state.stack = value; return ''; }
  if (op === 'stack-retry') { const read = state.stacks.get(value); if (read) { read.isPending = true; read.isSuccess = false; } return ''; }
  throw new ClientError(`Unknown pull request control: quick-${op}`);
}

/**
 * The open popovers' bodies, reading what they need: the detail behind a row's checks (LazyChecksBody,
 * the panel's own cached read) and the row's stack (usePullRequestStack, with the thread links'
 * saved membership while it reads). A read still due shows its loading line first and wakes the page.
 */
export async function quickPopovers(client: T3Client, native: Native | null, entries: Obj[], now: number) {
  const state = quickState(client), checks = emptyChecksPopover(), stack = emptyStackPopover();
  let wake = false;
  const checksEntry = state.checks ? entries.find(entry => refOf(entry) === state.checks) ?? null : null;
  if (checksEntry) {
    checks.ref = state.checks; checks.headline = CHECKS_HEADLINE[str(checksEntry.checksState)] ?? 'No checks reported';
    const read = native ? await readCachedDetail(client, native, checksEntry, now) : { detail: null, due: false, error: '' };
    if (read.error) checks.error = read.error;
    else if (read.due) { checks.loading = true; wake = true; }
    // LazyChecksBody: a finished read with nothing in it says there is nothing.
    else if (!read.detail) checks.empty = true;
    else {
      const all = arr(read.detail.checks);
      checks.empty = all.length === 0;
      const list = checksList(all, state.showAll);
      checks.rows = list.rows; checks.collapsible = list.collapsible; checks.expanded = state.showAll;
    }
  }
  const stackEntry = state.stack ? entries.find(entry => refOf(entry) === state.stack) ?? null : null;
  if (stackEntry) {
    const membership = obj(stackEntry.stack), reference = referenceOf(stackEntry);
    let read = state.stacks.get(state.stack);
    if (!read) { read = { data: null, error: null, isSuccess: false, isPending: true }; state.stacks.set(state.stack, read); }
    if (read.isPending && native) {
      if (!state.shown.has(`stack:${state.stack}`)) { state.shown.add(`stack:${state.stack}`); wake = true; }
      else {
        state.shown.delete(`stack:${state.stack}`);
        try { read.data = decodeStack(await client.rpc(native, 'pullRequests.stack', reference)); read.isSuccess = true; read.error = null; }
        catch (error) { if (letGo(error)) throw error; read.isSuccess = false; read.error = error instanceof Error && error.message ? error.message : 'The stack lookup failed.'; }
        read.isPending = false;
      }
    }
    const saved = savedPullRequestStack(client.shell.threads.flatMap(thread => arr(thread.pullRequests)), { host: str(stackEntry.host), repository: str(stackEntry.repository), number: num(stackEntry.number) });
    const view = pullRequestStackView(read, saved), data = view.data;
    stack.ref = state.stack;
    if (data) {
      Object.assign(stack, { number: data.number, label: `Stack #${data.number}`, notice: view.notice ?? '', noticeText: view.notice ? (read.error ? 'May be stale' : 'Refreshing…') : '', retry: !!read.error,
        layers: [...data.layers].reverse().map(layer => layerView(layer, { ...reference, host: str(stackEntry.host) }, num(stackEntry.number))), base: `↳ ${data.base}` });
    } else {
      Object.assign(stack, { number: num(membership.number), label: `Stack #${num(membership.number)}`, message: read.error ?? (read.isPending ? 'Loading stack…' : 'This pull request is no longer in a stack.') });
    }
  }
  if (wake && native) { try { await native.later({ op: 'r10Wake', topic: 't3.pr' }); } catch (error) { if (letGo(error)) throw error; } }
  return { checks, stack };
}
/** The detail behind a row's checks: what the panel's read already holds, else one read (shown as loading first). */
async function readCachedDetail(client: T3Client, native: Native, entry: Obj, now: number): Promise<{ detail: Obj | null; due: boolean; error: string }> {
  const asked = quickState(client).shown, key = `checks:${refOf(entry)}`;
  if (!asked.has(key) && !peekDetail(client, referenceOf(entry))) { asked.add(key); return { detail: null, due: true, error: '' }; }
  try { return { detail: await readDetail(client, native, referenceOf(entry), now), due: false, error: '' }; }
  catch (error) { if (letGo(error)) throw error; return { detail: null, due: false, error: error instanceof Error ? error.message : 'The checks could not be read.' }; }
}
