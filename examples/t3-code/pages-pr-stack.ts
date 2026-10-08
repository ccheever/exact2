// The pull request panel's host stack (lane "pages", pr-header-actions-and-stacks): the
// `pullRequests.stack` read once the detail says the host keeps stacks, the saved membership the
// thread links carry while it is out, the header's stack menu (n/m, layers top-down, "↳ base",
// Merge stack, Rebase stack) and its confirmation. Sources: T3 Code (MIT, see LICENSE-T3)
// apps/web/src/components/pullRequest/{PullRequestStackMenu,PullRequestStackLayers,PullRequestStackHeader,
// PullRequestStackLayerContent}.tsx, pullRequestStackSnapshot.ts (savedPullRequestStack,
// pullRequestStackView: ported with their tests' names), apps/web/src/state/usePullRequestStack.ts, and the
// panel's stack wiring (PullRequestDetailPanel.tsx stackReference, supportsStackActions,
// isStackedPullRequestBase over vcs.listRefs).
//
// The read belongs to the panel's resource (pages-pr-detail.ts asks `readPanelStack` after the
// detail); the menu's state (its confirmation, the action in flight) lives here per client. Merge-async
// polling is the server's (githubStackActions.ts): one `pullRequests.runAction` answers when GitHub has.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { letGo } from './let-go';
import { isStackedPullRequestBase } from './r6-pr-logic';

export type StackLayer = { number: number; title?: string; isDraft?: boolean; headSha?: string; headBranch: string; state: string };
export type PullRequestStack = { id: string; number: number; url: string; base: string; layers: StackLayer[] };
type Reference = { host?: string; repository: string; number: number };

const time = (value: unknown) => { const parsed = Date.parse(str(value)); return Number.isFinite(parsed) ? parsed : 0; };
const syncedAt = (link: Obj) => time(obj(link.snapshot).syncedAt || link.linkedAt);
/** savedPullRequestStack: saved native membership is enough for navigation, but never supplies action head SHAs. */
export function savedPullRequestStack(links: readonly Obj[], reference: Reference): PullRequestStack | null {
  const host = reference.host?.toLowerCase();
  if (!host) return null;
  const matching = links.filter(link => str(link.host).toLowerCase() === host && str(link.repository).toLowerCase() === reference.repository.toLowerCase());
  const exact = matching.filter(link => num(link.number) === reference.number);
  const candidates = exact.length > 0 ? exact : matching.filter(link => arr(obj(link.stack).layers).some(layer => num(layer.number) === reference.number));
  const newest = [...candidates].sort((a, b) => syncedAt(b) - syncedAt(a))[0];
  const stack = newest?.stack ? obj(newest.stack) : null;
  if (!stack || !arr(stack.layers).some(layer => num(layer.number) === reference.number)) return null;
  return {
    id: str(stack.id), number: num(stack.number), url: str(stack.url), base: str(stack.base),
    layers: arr(stack.layers).map(layer => {
      const snapshot = [...matching.filter(link => num(link.number) === num(layer.number))].sort((a, b) => syncedAt(b) - syncedAt(a))[0]?.snapshot;
      const known = snapshot ? obj(snapshot) : null;
      return { number: num(layer.number), headBranch: str(layer.headBranch), state: str(layer.state, 'open'), ...(known ? { title: str(known.title), isDraft: known.isDraft === true } : {}) };
    }),
  };
}
export type StackQuery = { data: PullRequestStack | null; isSuccess: boolean; isPending: boolean; error: string | null };
/** pullRequestStackView: a fresh absence overrides saved membership; failed refreshes preserve available navigation. */
export function pullRequestStackView(query: StackQuery, saved: PullRequestStack | null) {
  const data = query.isSuccess ? query.data : (query.data ?? saved);
  return {
    data, isFresh: query.isSuccess && !query.isPending,
    notice: data === null ? null : query.error ? 'Stack data may be stale. We couldn’t refresh it.' : !query.isSuccess || query.isPending ? 'Refreshing stack… Showing saved data.' : null,
  };
}
/** PullRequestStack as the server answers it, shape-checked. */
export function decodeStack(value: unknown): PullRequestStack | null {
  const stack = obj(value);
  if (!num(stack.number) || !Array.isArray(stack.layers)) return null;
  return { id: str(stack.id), number: num(stack.number), url: str(stack.url), base: str(stack.base),
    layers: arr(stack.layers).map(layer => ({ number: num(layer.number), headBranch: str(layer.headBranch), state: str(layer.state, 'open'),
      ...(typeof layer.title === 'string' ? { title: layer.title } : {}), ...(typeof layer.isDraft === 'boolean' ? { isDraft: layer.isDraft } : {}),
      ...(str(layer.headSha) ? { headSha: str(layer.headSha) } : {}) })) };
}

// ── The panel's stack read ──────────────────────────────────────────────────

type Entry = StackQuery & { due: boolean; refsFor: string; stacked: boolean };
const entries = new WeakMap<object, Map<string, Entry>>();
const entriesOf = (client: object) => { let map = entries.get(client); if (!map) { map = new Map(); entries.set(client, map); } return map; };
const caps = (client: { config: Obj }) => obj(obj(obj(client.config).environment).capabilities);
const hostOf = (url: string) => { try { return new URL(url).host; } catch { return ''; } };

/** stackReference: only once the detail says the host keeps stacks, on an environment that links thread pull requests. */
export function stackReference(client: { config: Obj }, detail: Obj | null, reference: Obj): Obj | null {
  if (!detail || obj(detail.capabilities).stacks !== true || caps(client).threadPullRequests !== true) return null;
  const host = str(reference.host) || hostOf(str(detail.url));
  return { projectId: str(reference.projectId), ...(host ? { host } : {}), repository: str(reference.repository), number: num(reference.number) };
}
/** supportsStackActions: the host, the pull request and the environment all say so. */
export function supportsStackActions(client: { config: Obj }, detail: Obj | null): boolean {
  const capabilities = obj(detail?.capabilities);
  return caps(client).threadPullRequests === true && capabilities.stacks === true && capabilities.stackActions === true && caps(client).pullRequestStackActions === true;
}
/** The query as it stands for a panel (a pull request nobody asked about yet is pending). */
export function stackQuery(client: object, key: string): StackQuery & { stacked: boolean } {
  const entry = entriesOf(client).get(key);
  return entry ? { data: entry.data, isSuccess: entry.isSuccess, isPending: entry.isPending, error: entry.error, stacked: entry.stacked } : { data: null, isSuccess: false, isPending: true, error: null, stacked: false };
}
/** Read the stack again with the next answer (refreshDetail, Retry, after an action). */
export function markStackDue(client: object, key?: string): void {
  for (const [entryKey, entry] of entriesOf(client)) if (!key || entryKey === key) { entry.due = true; entry.isPending = true; }
}
/**
 * The panel's stack read (usePullRequestStack) and the base branch's default-branch check
 * (isStackedPullRequestBase over vcs.listRefs, limit 2), each when due; a failure keeps what was held.
 */
export async function readPanelStack(client: T3Client, native: Native, key: string, detail: Obj | null, reference: Obj): Promise<void> {
  const map = entriesOf(client);
  let entry = map.get(key);
  const ref = stackReference(client, detail, reference);
  if (!entry) { entry = { data: null, isSuccess: false, isPending: !!ref, error: null, due: true, refsFor: '', stacked: false }; map.set(key, entry); }
  const reads: Promise<void>[] = [];
  if (ref && entry.due) {
    const held = entry;
    reads.push((async () => {
      try {
        held.data = decodeStack(await client.rpc(native, 'pullRequests.stack', ref));
        held.isSuccess = true; held.error = null;
      } catch (error) {
        if (letGo(error)) throw error;
        held.isSuccess = false; held.error = error instanceof Error && error.message ? error.message : 'The stack lookup failed.';
      }
      held.isPending = false; held.due = false;
    })());
  } else if (!ref) { entry.isPending = false; entry.due = false; }
  const cwd = str(detail?.workspaceRoot), refsFor = `${cwd}|${str(detail?.baseBranch)}`;
  if (detail && cwd && entry.refsFor !== refsFor) {
    const held = entry;
    reads.push((async () => {
      try {
        const listed = obj(await client.rpc(native, 'vcs.listRefs', { cwd, includeMatchingRemoteRefs: true, limit: 2 }));
        held.stacked = isStackedPullRequestBase(str(detail.baseBranch), arr(listed.refs).map(entryRef => ({ name: str(entryRef.name), isDefault: entryRef.isDefault === true, isRemote: entryRef.isRemote === true, ...(str(entryRef.remoteName) ? { remoteName: str(entryRef.remoteName) } : {}) })));
      } catch (error) { if (letGo(error)) throw error; held.stacked = false; }
      held.refsFor = refsFor;
    })());
  }
  await Promise.all(reads);
}
export function forgetStacks(client: object): void { entries.delete(client); }

// ── The header's stack menu (PullRequestStackMenu) ──────────────────────────

const STATE_LABELS: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };
const layerState = (layer: StackLayer) => (layer.state === 'open' && layer.isDraft === true ? 'draft' : layer.state);
export type StackLayerView = { key: string; number: number; ref: string; title: string; detail: string; compact: string; state: string; current: boolean };
export function emptyStack() {
  return { shown: false, retryLookup: false, number: 0, label: '', ariaLabel: '', tooltip: '', warn: false, noticeText: '', notice: '', retry: false, layers: [] as StackLayerView[],
    base: '', selectable: false, canMerge: false, mergeDisabled: true, mergeItemLabel: '', canRebase: false, rebaseDisabled: true, note: '', mergeButton: false, mergeTooltip: '',
    dialogOpen: false, dialogTitle: '', dialogDescription: '', dialogLayers: [] as StackLayerView[], dialogConfirm: '', dialogAction: '', pending: false };
}
export type PrStackView = ReturnType<typeof emptyStack>;
type StackUi = { confirm: { key: string; action: 'merge' | 'update-branch' } | null; pending: string };
const uis = new WeakMap<object, StackUi>();
export const stackUi = (client: object): StackUi => { let ui = uis.get(client); if (!ui) { ui = { confirm: null, pending: '' }; uis.set(client, ui); } return ui; };

/** The stack's arithmetic, shared by the menu and the run: which layers a merge or a rebase touches and whether it may. */
export function stackPlan(stack: PullRequestStack, number: number, pending: boolean) {
  const top = stack.layers.at(-1);
  const unmerged = stack.layers.filter(layer => layer.state !== 'merged');
  const hasClosed = unmerged.some(layer => layer.state !== 'open');
  const position = stack.layers.findIndex(layer => layer.number === number) + 1;
  const mergeLayers = stack.layers.slice(0, position).filter(layer => layer.state !== 'merged');
  const selectedLayer = stack.layers[position - 1];
  const mergeHasClosed = mergeLayers.some(layer => layer.state !== 'open');
  const heads = (layers: StackLayer[]) => layers.flatMap(layer => (layer.headSha ? [{ number: layer.number, headSha: layer.headSha }] : []));
  const hasUnknownHead = heads(unmerged).length !== unmerged.length;
  const mergeDisabled = pending || selectedLayer?.state !== 'open' || mergeLayers.some(layer => !layer.headSha) || mergeHasClosed || mergeLayers.length === 0 || mergeLayers.some(layer => layer.isDraft === true);
  const rebaseDisabled = pending || hasUnknownHead || hasClosed || unmerged.length === 0;
  return { top, unmerged, position, mergeLayers, selectedLayer, mergeHasClosed, mergeDisabled, rebaseDisabled, heads };
}
function layerView(layer: StackLayer, reference: Obj, current: number): StackLayerView {
  const state = layerState(layer), label = STATE_LABELS[state] ?? 'Open';
  const ref = JSON.stringify({ projectId: str(reference.projectId), host: str(reference.host), repository: str(reference.repository), number: layer.number });
  return { key: String(layer.number), number: layer.number, ref, title: layer.title || layer.headBranch, detail: `#${layer.number} · ${layer.headBranch} · ${label}`, compact: `#${layer.number} · ${label}`, state, current: layer.number === current };
}
export type StackInput = { key: string; reference: Obj; detail: Obj; query: StackQuery; saved: PullRequestStack | null; supportsStackActions: boolean; canMergeMethod: boolean; mergeMethod: string };
/** PullRequestStackMenu with its trigger, its separate Merge stack button and its confirmation, as the panel header draws them. */
export function presentStack(client: object, input: StackInput): PrStackView {
  const view = emptyStack(), ui = stackUi(client), stackView = pullRequestStackView(input.query, input.saved);
  const stack = stackView.data, number = num(input.reference.number);
  // "Retry stack lookup": no stack to show, stack actions supported, and the lookup failed.
  view.retryLookup = !stack && input.supportsStackActions && !!input.query.error;
  if (!stack) return view;
  const pending = ui.pending === input.key;
  const plan = stackPlan(stack, number, pending);
  const permissions = obj(input.detail.viewerPermissions);
  const onRetry = !!input.query.error;
  Object.assign(view, {
    shown: true, number: stack.number, label: `${plan.position}/${stack.layers.length}`, ariaLabel: `Stack ${stack.number}, layer ${plan.position} of ${stack.layers.length}`,
    tooltip: `View stack #${stack.number}, layer ${plan.position} of ${stack.layers.length}${stackView.notice ? ` · ${stackView.notice}` : ''}`,
    warn: onRetry, notice: stackView.notice ?? '', noticeText: stackView.notice ? (onRetry ? 'May be stale' : 'Refreshing…') : '', retry: onRetry,
    layers: [...stack.layers].reverse().map(layer => layerView(layer, input.reference, number)), base: `↳ ${stack.base}`, selectable: !pending,
    canMerge: stackView.isFresh && input.supportsStackActions && input.canMergeMethod, canRebase: stackView.isFresh && input.supportsStackActions && permissions.stackRebase === true,
    mergeDisabled: plan.mergeDisabled, rebaseDisabled: plan.rebaseDisabled, mergeItemLabel: `Merge stack (${plan.mergeLayers.length})`, pending,
    note: plan.mergeHasClosed || plan.mergeLayers.some(layer => layer.isDraft === true) ? 'Every layer being merged must be open and ready for review.' : '',
  });
  view.mergeButton = view.canMerge && plan.selectedLayer?.state === 'open';
  view.mergeTooltip = `Merge stack through #${number} into ${stack.base} (${plan.mergeLayers.length} ${plan.mergeLayers.length === 1 ? 'pull request' : 'pull requests'})`;
  const confirm = ui.confirm?.key === input.key ? ui.confirm.action : null;
  if (confirm) {
    const layers = confirm === 'merge' ? plan.mergeLayers : plan.unmerged;
    Object.assign(view, {
      dialogOpen: true, dialogAction: confirm,
      dialogTitle: confirm === 'merge' ? `Merge ${plan.mergeLayers.length} pull requests?` : `Rebase ${plan.unmerged.length} pull requests?`,
      dialogDescription: confirm === 'merge'
        ? `Merge #${number} and its unmerged layers below into ${stack.base} using ${input.mergeMethod}. GitHub checks their rules before merging or queueing them and rebases the remaining stack after merging.`
        : `Rebase the remote branches from bottom to top onto ${stack.base}. This rewrites branch history and may restart checks. If a layer fails, earlier updates remain.`,
      dialogLayers: layers.map(layer => layerView(layer, input.reference, number)),
      dialogConfirm: pending ? 'Working…' : confirm === 'merge' ? 'Merge stack' : 'Rebase stack',
    });
  }
  return view;
}
/** A layer opened beside a thread (onSelectPullRequest → rightPanelStore.openPullRequest): the surface target with its URL. */
export function stackLayerTarget(ref: string): string {
  try {
    const parsed = obj(JSON.parse(ref)), host = str(parsed.host, 'github.com'), repository = str(parsed.repository), number = num(parsed.number);
    return JSON.stringify({ projectId: str(parsed.projectId), host: str(parsed.host), repository, number, url: `https://${host}/${repository}/pull/${number}` });
  } catch { return ''; }
}
