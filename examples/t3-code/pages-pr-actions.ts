// The pull request panel's host actions (lane "pages", pr-header-actions-and-stacks): the header's
// primary control (Merge with its method, Ready for review, "Auto-merge (method)", Resolve conflicts,
// the armed badge, the Merged/Closed badge), the More menu's action group (Merge now, Enable/Disable
// auto-merge, the merge-method radios, Convert to draft, Close, Reopen, Revert changes), the five
// confirmations, the out-of-date base branch's popover (Update branch / Update with rebase),
// "Approve workflows to run", one runner for all ten actions with the reference's toasts and
// failure hints, and the overrides the panel writes onto the Pull Requests list as an action goes.
// Sources: T3 Code (MIT, see LICENSE-T3) apps/web/src/components/pullRequest/PullRequestDetailPanel.tsx
// (perform/finishAction, the confirmation AlertDialog, PullRequestBaseFreshnessWarning, the header and
// menu matrix), pullRequestDetail.logic.ts (ported in r6-pr-logic.ts), pullRequestList.logic.ts
// (pullRequestOverrideAfterAction, applyPullRequestOverrides, settlePullRequestOverrides: ported here
// with their tests' names) and routes/_chat.pull-requests.tsx onActed (the list's "sent"/"done"/"failed").
//
// The menu's choices (a confirmation, a merge method) are chatlocal ops, answered without the command
// gate; an action is a pages command (`pageslocal:pr-act-action`) that marks itself pending and wakes
// the panel's resource (`t3.pr`) so "Merging..." shows while GitHub works, as the reference's
// pendingAction does. React-only behaviour (Base UI focus return, the dialog's open/close
// transition) is the framework's: Escape is the Cancel button's `aria-keyshortcuts`.
import type { T3Client } from './client';
import { num, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { pagesPrefs } from './pages-prefs';
import { composerNow } from './composer-controls';
import { prState, startHandoff } from './r6-pr-actions';
import {
  ACTION_FAILURE, ACTION_SUCCESS, PULL_REQUEST_ACTIONS, PULL_REQUEST_MERGE_METHOD_LABELS, actionHint, actionPayload, allowedMergeMethods, allowsSinglePullRequestMerge,
  pullRequestActionMenuHasGroup, pullRequestActionNeedsHostRefresh, readableFailure, resolveBaseFreshness, resolvePullRequestMergeMethod, resolvePullRequestPrimaryControl,
  type MergeMethod, type PrimaryControl, type UpdateMethod,
} from './r6-pr-logic';
import { emptyStack, markStackDue, presentStack, savedPullRequestStack, stackPlan, stackQuery, stackUi, supportsStackActions, type PrStackView } from './pages-pr-stack';

// ── List overrides (pullRequestList.logic.ts) ───────────────────────────────

/** What a row should say the moment an action is sent, before any host has answered. */
export type ListOverride = { state: string; isDraft?: boolean; updatedAt: string; token: number; at: number };
export function pullRequestOverrideAfterAction(entry: { state?: unknown; isDraft?: unknown }, action: string, now: Date, token: number): ListOverride | null {
  const stamp = { updatedAt: now.toISOString(), token, at: now.getTime() };
  switch (action) {
    case 'close': return { state: 'closed', ...stamp };
    case 'reopen': return { state: 'open', ...stamp };
    case 'merge': return { state: 'merged', ...stamp };
    case 'draft': return { state: str(entry.state, 'open'), isDraft: true, ...stamp };
    case 'ready': return { state: str(entry.state, 'open'), isDraft: false, ...stamp };
    default: return null;
  }
}
/** applyPullRequestOverrides: the rows with their pending answers written over them, and the ones the list's state filter no longer holds dropped. */
export function applyPullRequestOverrides<Entry extends Obj>(entries: readonly Entry[], overrides: ReadonlyMap<string, ListOverride>, keyOf: (entry: Entry) => string, state: string): readonly Entry[] {
  if (overrides.size === 0) return entries;
  const out: Entry[] = [];
  for (const entry of entries) {
    const override = overrides.get(keyOf(entry));
    if (override === undefined) { out.push(entry); continue; }
    if (state !== 'all' && override.state !== state) continue;
    out.push({ ...entry, ...override });
  }
  return out;
}
/** How long a read that disagrees is taken for a stale one rather than for news. */
export const PULL_REQUEST_OVERRIDE_TRUST_MS = 60_000;
/** settlePullRequestOverrides: dropped once an answer agrees, or disagrees a minute later; absence confirms nothing. */
export function settlePullRequestOverrides<Entry extends Obj>(overrides: ReadonlyMap<string, ListOverride>, answered: readonly Entry[], keyOf: (entry: Entry) => string, now: number): ReadonlyMap<string, ListOverride> {
  if (overrides.size === 0) return overrides;
  const byKey = new Map(answered.map(entry => [keyOf(entry), entry]));
  const kept = new Map<string, ListOverride>();
  for (const [key, override] of overrides) {
    const row = byKey.get(key);
    if (row === undefined) { kept.set(key, override); continue; }
    const agrees = row.state === override.state && (override.isDraft === undefined || row.isDraft === override.isDraft);
    const outranked = now - override.at > PULL_REQUEST_OVERRIDE_TRUST_MS;
    if (!agrees && !outranked) kept.set(key, override);
  }
  return kept.size === overrides.size ? overrides : kept;
}

// ── Per-client state ────────────────────────────────────────────────────────

export type ConfirmAction = 'merge' | 'close' | 'enable-auto-merge' | 'revert' | 'approve-workflows';
const CONFIRMS: readonly ConfirmAction[] = ['merge', 'close', 'enable-auto-merge', 'revert', 'approve-workflows'];
type State = {
  /** pendingAction: which action is in flight; every control is disabled while any runs. */
  pending: { key: string; action: string } | null;
  confirm: { key: string; action: ConfirmAction } | null;
  /** mergeMethodSelection, per pull request; and the armed method last seen (it becomes the selection). */
  methods: Map<string, MergeMethod>; armed: Map<string, string>;
  overrides: Map<string, ListOverride>; token: number; detailTokens: Map<string, number | null>;
  /** The list's relist request (refreshListAndStats), bumped after an action that changed more than a state. */
  relist: number;
};
const states = new WeakMap<object, State>();
export function actionState(client: object): State {
  let state = states.get(client);
  if (!state) { state = { pending: null, confirm: null, methods: new Map(), armed: new Map(), overrides: new Map(), token: 0, detailTokens: new Map(), relist: 0 }; states.set(client, state); }
  return state;
}
const MERGE_METHODS: readonly MergeMethod[] = ['merge', 'squash', 'rebase'];
const asMethod = (value: unknown): MergeMethod | null => (MERGE_METHODS.includes(value as MergeMethod) ? value as MergeMethod : null);
/** useUiStateStore pullRequestMergeMethod: the method last chosen on this device ("merge" until one is). */
export function lastMergeMethod(client: { local: object }): MergeMethod {
  return asMethod((pagesPrefs(client) as { mergeMethod?: unknown }).mergeMethod) ?? 'merge';
}
/** usePullRequestDefaultMergeMethodResolver: the project's Default merge method, or the environment's ("Last selected" is none). */
export function projectDefaultMergeMethod(settings: Obj, projectId: string): MergeMethod | undefined {
  const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
  const value = Object.prototype.hasOwnProperty.call(override, 'pullRequestMergeMethod') ? override.pullRequestMergeMethod : settings.pullRequestMergeMethod;
  return asMethod(value) ?? undefined;
}
const names = (value: unknown): string[] => (Array.isArray(value) ? value.map(String) : []);
const can = (detail: Obj, action: string) => names(obj(detail.capabilities).actions).includes(action) && names(obj(detail.viewerPermissions).actions).includes(action);

/** The panel the menu speaks for: its key, its reference, the detail on screen and how to read it again. */
export type PanelContext = { key: string; selected: string; reference: Obj; detail: Obj | null; listEntry: Obj | null; refresh: (fromHost: boolean) => void };

/** The merge method the panel would use now (resolvePullRequestMergeMethod over the allowed ones). */
export function selectedMethod(client: T3Client, ctx: { key: string; detail: Obj | null; reference: Obj }): MergeMethod {
  const state = actionState(client), detail = ctx.detail ?? {};
  const armed = asMethod(detail.autoMergeMethod);
  // useEffect([autoMergeMethod, pullRequestKey]): an armed method the host reports becomes the selection.
  if (armed && state.armed.get(ctx.key) !== armed) { state.armed.set(ctx.key, armed); state.methods.set(ctx.key, armed); }
  return resolvePullRequestMergeMethod(allowedMergeMethods(detail), state.methods.get(ctx.key) ?? null, projectDefaultMergeMethod(obj(client.config.settings), str(ctx.reference.projectId)), lastMergeMethod(client));
}

// ── The header and menu as the panel draws them ─────────────────────────────

export function emptyActions() {
  return {
    primary: '', primaryLabel: '', primaryDisabled: false, armedLabel: '', armedTip: '', stateLabel: '', stateKey: '',
    refreshing: false, moreLabel: 'More pull request actions', pending: false, open: false,
    draftToggle: '', mergeNow: false, autoMerge: '', methods: [] as { key: string; method: string; label: string; selected: boolean }[], methodsSeparator: false, groupSeparator: false,
    closeItem: false, reopenItem: false, revertItem: false,
    freshness: false, freshnessSummary: '', freshnessMethods: [] as { key: string; value: string; label: string }[], stacked: false, baseTip: '',
    approveWorkflows: false, approveLabel: 'Approve workflows to run',
    dialogOpen: false, dialogTitle: '', dialogDescription: '', dialogConfirm: '', dialogDestructive: false, dialogValue: '',
    stack: emptyStack(),
  };
}
export type PrActionsView = ReturnType<typeof emptyActions>;
type PresentInput = PanelContext & { checksState: string | null; checksStale: boolean; refreshing: boolean; threadLinks: Obj[] };

export function presentActions(client: T3Client, input: PresentInput): PrActionsView {
  const view = emptyActions(), detail = input.detail;
  view.refreshing = input.refreshing;
  view.moreLabel = input.refreshing ? 'Refreshing pull request' : 'More pull request actions';
  if (!detail) return view;
  const state = actionState(client), pendingAction = state.pending?.key === input.key ? state.pending.action : '';
  const actionPending = state.pending !== null;
  const allowed = allowedMergeMethods(detail), method = selectedMethod(client, input), methodLabel = PULL_REQUEST_MERGE_METHOD_LABELS[method];
  const open = detail.state === 'open', draft = detail.isDraft === true, conflicting = open && detail.mergeability === 'conflicting';
  const autoMergeArmed = open && detail.autoMergeEnabled === true;
  const armedMethod = asMethod(detail.autoMergeMethod);
  const armedLabel = armedMethod ? `Auto-merge (${PULL_REQUEST_MERGE_METHOD_LABELS[armedMethod].toLowerCase()})` : 'Auto-merge';
  const workflows = open ? num(detail.workflowApprovalsRequired) : 0;
  // The stack: the single-pull-request merge waits for its discovery where stack actions exist.
  const query = stackQuery(client, input.key), stacks = supportsStackActions(client, detail);
  const saved = savedPullRequestStack(input.threadLinks, { host: str(input.reference.host), repository: str(input.reference.repository), number: num(input.reference.number) });
  const hasStack = (query.isSuccess ? query.data : (query.data ?? saved)) !== null;
  const single = allowsSinglePullRequestMerge({ supportsStackActions: stacks, hasStack, stackPending: !query.isSuccess || query.isPending, stackError: query.error });
  const primary: PrimaryControl = resolvePullRequestPrimaryControl({ state: str(detail.state), isDraft: draft, mergeability: str(detail.mergeability), checksState: input.checksState,
    autoMergeEnabled: typeof detail.autoMergeEnabled === 'boolean' ? detail.autoMergeEnabled : undefined, hasMergeMethod: allowed.length > 0,
    canMerge: single && can(detail, 'merge'), canMarkReady: can(detail, 'ready'), canEnableAutoMerge: single && can(detail, 'enable-auto-merge') });
  view.pending = actionPending; view.open = open;
  view.primary = primary ?? '';
  const handoff = prState(client).handoff;
  if (primary === 'resolve') { view.primaryLabel = handoff === 'conflicts' ? 'Preparing...' : 'Resolve conflicts'; view.primaryDisabled = handoff !== '' || !str(detail.workspaceRoot); }
  else if (primary === 'ready') { view.primaryLabel = 'Ready for review'; view.primaryDisabled = actionPending; }
  else if (primary === 'enable-auto-merge') { view.primaryLabel = pendingAction === 'enable-auto-merge' ? 'Enabling...' : `Auto-merge (${methodLabel.toLowerCase()})`; view.primaryDisabled = actionPending; }
  else if (primary === 'merge') { view.primaryLabel = pendingAction === 'merge' ? 'Merging...' : methodLabel; view.primaryDisabled = actionPending; }
  else if (primary === 'auto-merge-armed') view.primaryLabel = armedLabel;
  else if (primary === 'merged' || primary === 'closed') { view.stateKey = primary; view.stateLabel = primary === 'merged' ? 'Merged' : 'Closed'; }
  // Said where the Merge button is: the merge is already asked for, and the host is holding it.
  if (autoMergeArmed && primary !== 'auto-merge-armed') view.armedLabel = armedLabel;
  view.armedTip = `${armedLabel}: the host will merge this on its own once its requirements are met`;
  // The menu's action group (showsDraftToggle, showsAutoMerge, showsMergeNow, showsMergeMethods).
  const showsDraftToggle = open && can(detail, draft ? 'ready' : 'draft') && !(draft && primary === 'ready');
  const showsAutoMerge = single && open && ((autoMergeArmed && can(detail, 'disable-auto-merge'))
    || (!autoMergeArmed && primary !== 'enable-auto-merge' && !draft && !conflicting && can(detail, 'enable-auto-merge') && allowed.length > 0));
  const showsMergeNow = single && open && (primary === 'enable-auto-merge' || primary === 'auto-merge-armed') && can(detail, 'merge') && !draft && !conflicting && allowed.length > 0;
  const showsMergeMethods = open && can(detail, 'merge') && !draft && !conflicting && allowed.length > 1;
  view.draftToggle = showsDraftToggle ? (draft ? 'ready' : 'draft') : '';
  view.mergeNow = showsMergeNow;
  view.autoMerge = autoMergeArmed && can(detail, 'disable-auto-merge') ? 'disable' : showsAutoMerge ? 'enable' : '';
  if (showsMergeMethods) view.methods = allowed.map(item => ({ key: item, method: item, label: PULL_REQUEST_MERGE_METHOD_LABELS[item], selected: item === method }));
  view.methodsSeparator = showsMergeMethods && (showsDraftToggle || showsMergeNow || showsAutoMerge);
  view.groupSeparator = open && pullRequestActionMenuHasGroup(showsDraftToggle, showsAutoMerge || showsMergeNow, showsMergeMethods);
  view.closeItem = open && can(detail, 'close');
  view.reopenItem = detail.state === 'closed' && can(detail, 'reopen');
  view.revertItem = detail.state === 'merged' && can(detail, 'revert');
  // The base branch: the out-of-date mark and its popover, else the plain name (stacked on another branch, or not).
  const freshness = resolveBaseFreshness(detail), base = str(detail.baseBranch);
  view.stacked = query.stacked;
  view.baseTip = query.stacked ? `Stacked on ${base}` : base;
  if (freshness) {
    const behind = freshness.behindBy === null ? '' : ` by ${freshness.behindBy.toLocaleString('en-US')} ${freshness.behindBy === 1 ? 'commit' : 'commits'}`;
    view.freshness = true; view.freshnessSummary = `This branch is out-of-date with ${base}${behind}.`;
    view.freshnessMethods = freshness.methods.map(item => ({ key: item, value: `update-branch:${item}`, label: item === 'rebase' ? 'Update with rebase' : 'Update branch' }));
  }
  view.approveWorkflows = workflows > 0 && !input.checksStale && can(detail, 'approve-workflows');
  view.approveLabel = pendingAction === 'approve-workflows' ? 'Approving...' : 'Approve workflows to run';
  // The confirmation AlertDialog.
  const confirm = state.confirm?.key === input.key ? state.confirm.action : null;
  if (confirm) {
    const number = num(detail.number, num(input.reference.number));
    const text: Record<ConfirmAction, [string, string, string]> = {
      merge: ['Merge pull request?', `This merges #${number} using ${method}.`, methodLabel],
      'enable-auto-merge': ['Enable auto-merge?', `This merges #${number} using ${method} as soon as the host considers it ready, which may be immediately.`, 'Enable auto-merge'],
      revert: ['Revert these changes?', `This opens a new pull request that reverses the changes merged by #${number}.`, 'Create revert PR'],
      'approve-workflows': ['Approve workflows to run?', `This allows ${workflows} ${workflows === 1 ? 'workflow' : 'workflows'} from #${number} to run. Review the code and workflow changes first.`, 'Approve and run'],
      close: ['Close pull request?', `This closes #${number} without merging it.`, 'Close'],
    };
    const [title, description, label] = text[confirm];
    Object.assign(view, { dialogOpen: true, dialogTitle: title, dialogDescription: description, dialogConfirm: label, dialogDestructive: confirm === 'close',
      dialogValue: confirm === 'merge' || confirm === 'enable-auto-merge' ? `${confirm}:${method}` : confirm });
  }
  view.stack = presentStack(client, { key: input.key, reference: input.reference, detail, query, saved, supportsStackActions: stacks, canMergeMethod: can(detail, 'merge') && allowed.length > 0, mergeMethod: method }) as PrStackView;
  return view;
}

// ── Commands ────────────────────────────────────────────────────────────────

/** The panel's resource reads again now, so a state a command set before waiting is drawn while it waits. */
async function wakePanel(native: Native): Promise<void> {
  try { await native.later({ op: 'r10Wake', topic: 't3.pr' }); } catch (error) { if (letGo(error)) throw error; }
}
const entryKeyOf = (entry: Obj) => `${str(entry.host)}:${str(entry.repository)}#${num(entry.number)}`;
/**
 * onActed (routes/_chat.pull-requests.tsx): an action that only moves a row's state is written onto
 * the row as it is sent (a merge once the host has done it) and taken back if the host refuses;
 * the rest have the list read again once they are done.
 */
export function noteActed(client: object, listEntry: Obj | null, action: string | undefined, phase: 'sent' | 'done' | 'failed'): void {
  const state = actionState(client);
  // The reader's clock: the newest wall time a snapshot saw (a data source reads no clock of its own).
  const now = new Date(composerNow(client));
  const stateOnly = action !== undefined && listEntry !== null && pullRequestOverrideAfterAction(listEntry, action, now, 0) !== null;
  if (stateOnly) {
    const key = entryKeyOf(listEntry!);
    const write = () => { const token = ++state.token; const override = pullRequestOverrideAfterAction(listEntry!, action!, now, token); if (!override) return null; state.overrides.set(key, override); return token; };
    if (phase === 'sent' && action !== 'merge') state.detailTokens.set(key, write());
    if (phase === 'failed' && action !== 'merge') { const token = state.detailTokens.get(key) ?? null; if (token !== null && state.overrides.get(key)?.token === token) state.overrides.delete(key); }
    if (phase !== 'sent') state.detailTokens.delete(key);
    if (phase === 'done' && action === 'merge') { write(); state.relist++; }
    return;
  }
  if (phase === 'done') state.relist++;
}
/** The list's rows with the panel's overrides over them (applyPullRequestOverrides), for pages-prs.ts. */
export function overrideListEntries(client: object, entries: Obj[], listState: string): Obj[] {
  const overrides = actionState(client).overrides;
  if (!overrides.size) return entries;
  const strip = new Map([...overrides].map(([key, { state, isDraft, updatedAt }]) => [key, { state, ...(isDraft === undefined ? {} : { isDraft }), updatedAt } as unknown as ListOverride]));
  return [...applyPullRequestOverrides(entries, strip, entryKeyOf, listState)];
}
/** A whole-list answer settles the overrides it agrees with (settlePullRequestOverrides). */
export function settleListOverrides(client: object, answered: Obj[], now: number): void {
  const state = actionState(client);
  state.overrides = new Map(settlePullRequestOverrides(state.overrides, answered, entryKeyOf, now));
}
export const listRelist = (client: object) => actionState(client).relist;

/** perform / finishAction: one action at a time, the reference's toasts, the right re-read, the list told. */
export async function performAction(client: T3Client, native: Native, ctx: PanelContext, action: string, method?: MergeMethod, updateMethod?: UpdateMethod): Promise<string> {
  const state = actionState(client);
  if (state.pending) return '';
  if (!(PULL_REQUEST_ACTIONS as readonly string[]).includes(action)) throw new ClientError(`Unknown pull request action: ${action}`);
  state.pending = { key: ctx.key, action };
  if (state.confirm?.key === ctx.key) state.confirm = null;
  noteActed(client, ctx.listEntry, action, 'sent');
  try {
    await wakePanel(native);
    await client.rpc(native, 'pullRequests.runAction', actionPayload(ctx.reference, action, method, updateMethod ? { updateMethod } : {}), true);
  } catch (error) {
    state.pending = null;
    if (letGo(error)) throw error;
    // The host's own sentence, because it is the only thing that says why; the hint for what was asked.
    pushToast(client, { kind: 'error', title: ACTION_FAILURE[action] ?? 'Could not update this pull request', description: readableFailure(error, actionHint(action, updateMethod)) });
    noteActed(client, ctx.listEntry, action, 'failed');
    return error instanceof Error ? error.message : 'The host refused it.';
  }
  state.pending = null;
  pushToast(client, { kind: 'success', title: ACTION_SUCCESS[action] ?? 'Pull request updated' });
  // An update moves the head and approved workflows are data the detail omits: around the server's cache.
  ctx.refresh(pullRequestActionNeedsHostRefresh(action));
  markStackDue(client, ctx.key);
  noteActed(client, ctx.listEntry, action, 'done');
  return '';
}

/** PullRequestStackMenu run: the confirmed stack action against the selected layer (merge) or the top (rebase). */
export async function runStackAction(client: T3Client, native: Native, ctx: PanelContext): Promise<string> {
  const ui = stackUi(client), confirm = ui.confirm?.key === ctx.key ? ui.confirm.action : null;
  const query = stackQuery(client, ctx.key), detail = ctx.detail;
  const stack = query.isSuccess ? query.data : null;
  if (!confirm || !stack || !detail || ui.pending) return '';
  const plan = stackPlan(stack, num(ctx.reference.number), false);
  const supported = supportsStackActions(client, detail), fresh = query.isSuccess && !query.isPending;
  const allowedMerge = fresh && supported && can(detail, 'merge') && allowedMergeMethods(detail).length > 0;
  const allowedRebase = fresh && supported && obj(detail.viewerPermissions).stackRebase === true;
  if (confirm === 'merge' ? !allowedMerge || plan.mergeDisabled : !allowedRebase || plan.rebaseDisabled) return '';
  const target = confirm === 'merge' ? plan.selectedLayer : plan.top;
  if (!target?.headSha) return '';
  const heads = plan.heads(confirm === 'merge' ? plan.mergeLayers : plan.unmerged);
  ui.pending = ctx.key;
  let failure: unknown = null;
  try {
    await wakePanel(native);
    await client.rpc(native, 'pullRequests.runAction', actionPayload({ ...ctx.reference, number: target.number }, confirm, confirm === 'merge' ? selectedMethod(client, ctx) : undefined,
      { stackNumber: stack.number, expectedStackHeads: heads, ...(confirm === 'update-branch' ? { updateMethod: 'rebase' as const } : {}) }), true);
  } catch (error) {
    if (letGo(error)) { ui.pending = ''; throw error; }
    failure = error;
  }
  ui.pending = ''; ui.confirm = null;
  ctx.refresh(false); markStackDue(client, ctx.key); noteActed(client, ctx.listEntry, undefined, 'done');
  if (failure !== null) {
    // String(failure): the error's name and its message, as the reference's toast prints it.
    const kind = failure instanceof ClientError && failure.kind ? failure.kind : failure instanceof Error ? failure.name : '';
    const message = failure instanceof Error ? failure.message : String(failure);
    pushToast(client, { kind: 'error', title: 'Stack operation did not complete', description: kind ? `${kind}: ${message}` : message });
    return message;
  }
  pushToast(client, confirm === 'merge' ? { kind: 'success', title: 'Stack merge request completed', description: 'GitHub merged the stack or added it to its merge queue.' } : { kind: 'success', title: 'Stack rebased' });
  return '';
}

/** `pageslocal:pr-act-*` that act on the host: `action` ("merge:squash", "update-branch:rebase", "close"), `stack-run`, `handoff`. */
export async function prActionCommand(client: T3Client, native: Native, op: string, ctx: PanelContext, value: string): Promise<string> {
  if (op === 'action') {
    const [action = '', extra = ''] = value.split(':');
    if (action === 'merge' || action === 'enable-auto-merge') return performAction(client, native, ctx, action, asMethod(extra) ?? selectedMethod(client, ctx));
    if (action === 'update-branch') return performAction(client, native, ctx, action, undefined, extra === 'rebase' ? 'rebase' : extra === 'merge' ? 'merge' : undefined);
    return performAction(client, native, ctx, action);
  }
  if (op === 'stack-run') return runStackAction(client, native, ctx);
  if (op === 'handoff' && value === 'conflicts') {
    if (!ctx.detail) throw new ClientError('The pull request is still loading.');
    return startHandoff(client, native, 'conflicts', ctx.detail);
  }
  throw new ClientError(`Unknown pull request action: ${op}`);
}
export const isActionOp = (op: string) => op === 'action' || op === 'stack-run' || op === 'handoff';

/** `chatlocal:pr-ui-*`: the menu's choices, answered without the command gate. */
export function prActionUi(client: T3Client, op: string, ctx: { key: string; detail: Obj | null; reference: Obj }, value: string): string {
  const state = actionState(client);
  if (op === 'ask') {
    if (!CONFIRMS.includes(value as ConfirmAction)) throw new ClientError(`Unknown confirmation: ${value}`);
    if (!state.pending) state.confirm = { key: ctx.key, action: value as ConfirmAction };
    return '';
  }
  if (op === 'cancel') { if (state.confirm?.key === ctx.key) state.confirm = null; return ''; }
  if (op === 'method') {
    const method = asMethod(value);
    if (!method) throw new ClientError(`Unknown merge method: ${value}`);
    state.methods.set(ctx.key, method);
    (pagesPrefs(client) as { mergeMethod?: string }).mergeMethod = method; // setLastSelectedMergeMethod
    return '';
  }
  const ui = stackUi(client);
  if (op === 'stack-ask') { if ((value === 'merge' || value === 'update-branch') && !ui.pending) ui.confirm = { key: ctx.key, action: value }; return ''; }
  if (op === 'stack-cancel') { if (!ui.pending && ui.confirm?.key === ctx.key) ui.confirm = null; return ''; }
  if (op === 'stack-retry') { markStackDue(client, ctx.key); return ''; }
  throw new ClientError(`Unknown pull request control: ${op}`);
}
