// The pull request detail panel (lane "pages"): pullRequests.detail and
// .activity read for the selected row, presented as PullRequestDetailPanel's
// header and Summary/Timeline tabs, and its writes (runAction, update,
// requestReviewers, setLabels). Sources: T3 Code (MIT, see LICENSE-T3)
// apps/web/src/components/pullRequest/{PullRequestDetailPanel,
// PullRequestSummaryTab,PullRequestTimelineTab,PullRequestGhosts,pullRequestPresentation}.tsx.
//
// Reads (pr-conversation-and-refresh). The detail and the activity are two reads, as the
// reference's two queries are: the panel shows the detail ghost (seeded by the list's row, or
// the last detail kept in t3-code.json) until the detail lands, then the conversation and
// timeline ghosts until the activity lands, and a failed activity read is "Could not load pull
// request activity" with Retry. An answer that has something new to show returns it at once and
// wakes the resource (`t3.pr`, R10Connect.swift) for the read that follows: the runner lets the
// reply land and asks once more (LLP 1016.002 D4). A refresh — the server's announcement
// (pages-pr-refresh.ts), Refresh, the live-refresh interval or an arrival — reads while the last
// detail stays on screen; the activity reads again when the server announced a change, on
// Refresh, or when the detail's `updatedAt` moved (shouldRefreshPullRequestActivity).
import { diffSchemeOf, markdownEnv, type ChipView } from './r4-timeline-chips';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import { relativeLabel, stateKey, conflictLabel, labelChip, listEntryFor } from './pages-prs';
import type { T3Client } from './client';
import { letGo } from './let-go';
import { peekDetail } from './r6-pr-actions';
import { CHECKS_HEADLINE } from './r6-pr-logic';
import {
  LIVE_REFRESH_IDLE_AFTER_MS, LIVE_REFRESH_INTERVAL_MS, readPullRequestDetailSnapshot, resolveDisplayedPullRequestDetail, resolvePullRequestReferenceHost,
  shouldRefreshOnArrival, shouldRefreshOnInterval, shouldRefreshPullRequestActivity, writePullRequestDetailSnapshot, type PullRequestDetailSnapshotRef,
} from './pages-pr-logic';
import { holdPullRequestRefreshes, lastInteraction, noteViewRefreshed, pullRequestRefreshEpoch, snapshotStorage, viewRefreshedAt } from './pages-pr-refresh';
import { conversationBodies, emptySummary, presentSummary } from './pages-pr-summary';
import { emptyTimeline, presentTimeline } from './pages-pr-timeline';

export type PrSelection = { projectId: string; host: string; repository: string; number: number };
/** The row key the list wears, parsed back into the reference a read needs. */
export function parseSelection(value: string): PrSelection | null {
  try {
    const parsed = obj(JSON.parse(value));
    const number = num(parsed.number), repository = str(parsed.repository), projectId = str(parsed.projectId);
    return number > 0 && repository && projectId ? { projectId, host: str(parsed.host), repository, number } : null;
  } catch { return null; }
}
export const selectionRef = (selection: PrSelection): Obj => ({ projectId: selection.projectId, ...(selection.host ? { host: selection.host } : {}), repository: selection.repository, number: selection.number });

const STATE_LABELS: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };
const ACTION_DONE: Record<string, string> = { merge: 'Pull request merged', ready: 'Marked ready for review', draft: 'Converted to draft', close: 'Pull request closed', reopen: 'Pull request reopened', 'update-branch': 'Branch updated with the base branch' };
const ACTION_FAILED: Record<string, string> = { merge: 'Could not merge this pull request', ready: 'Could not mark this ready for review', draft: 'Could not convert this to a draft', close: 'Could not close this pull request', reopen: 'Could not reopen this pull request', 'update-branch': 'Could not update this branch' };
const count = (value: number) => String(Math.round(value)).replace(/\B(?=(\d{3})+(?!\d))/g, ',');

/** summarizePullRequestChecks: GitHub's own headline for the rollup. */
export function summarizeChecks(checks: Obj[]): string {
  if (!checks.length) return 'No checks reported';
  const failed = checks.filter(check => check.status === 'failure' || check.status === 'cancelled').length;
  const action = checks.filter(check => check.status === 'action-required').length;
  const pending = checks.filter(check => check.status === 'pending').length;
  const passed = checks.filter(check => check.status === 'success').length;
  if (failed) return `${failed} of ${checks.length} failing`;
  if (action) return `${action} ${action === 1 ? 'check' : 'checks'} awaiting action`;
  if (pending) return `${pending} of ${checks.length} running`;
  return passed === checks.length ? 'All checks passed' : `${passed} of ${checks.length} passing`;
}
export function checksTone(checks: Obj[]): string {
  if (!checks.length) return '';
  const statuses = new Set(checks.map(check => str(check.status)));
  if (statuses.has('failure') || statuses.has('cancelled')) return 'failing';
  if (statuses.has('pending') || statuses.has('action-required')) return 'pending';
  return statuses.has('success') ? 'passing' : '';
}
const person = (value: unknown) => { const actor = obj(value), login = str(actor.login, 'ghost'); return { key: login.toLowerCase(), login, avatar: str(actor.avatarUrl), initial: login.slice(0, 1).toUpperCase() }; };
const labelColor = (color: unknown) => { const hex = str(color).trim().replace(/^#/, ''); return /^[0-9a-fA-F]{6}$/.test(hex) ? `#${hex}` : ''; };

/** PullRequestDetailGhost's seed: what the list's row (or a kept detail) already says. */
function emptyGhost() {
  return { seeded: false, title: '', repository: '', url: '', state: 'open', author: '', avatar: '', initial: '', updated: '', baseBranch: '', headBranch: '',
    files: '', additions: '', deletions: '', checksLabel: '', checksTone: '', labelsKnown: false, labels: [] as { key: string; name: string; background: string; ink: string }[] };
}
export function emptyDetail() {
  return {
    open: false, loading: false, phase: 'ghost', error: '', errorTitle: '', githubUrl: '', copiedCheckout: 0, copiedBranch: 0, ref: '', number: 0, numberLabel: '', title: '', repository: '', url: '', state: 'open', stateLabel: 'Open', conflict: '',
    author: '', authorAvatar: '', authorInitial: '', updated: '', checkoutCommand: '', baseBranch: '', headBranch: '', files: '', additions: '', deletions: '',
    checksSummary: '', checksTone: '', reviewers: [] as { key: string; login: string; avatar: string; initial: string }[],
    labels: [] as { key: string; name: string; background: string; ink: string }[],
    bodies: [] as { id: string; kind: string; title: string; body: string }[], hasBody: false, bodyId: '',
    commentCount: 0, commentsLabel: 'Comments (0)', activityPending: false, activityError: '',
    ghost: emptyGhost(), summary: emptySummary(), timeline: emptyTimeline(),
    canEdit: false, canClose: false, canReopen: false, canDraft: false, canReady: false, canMerge: false, canUpdateBranch: false, canReview: false, canLabel: false,
    projectId: '', host: '', hostName: 'GitHub', linkMenu: '', code: [] as { id: string; code: string; icon: string; tokens: { id: string; text: string; cls: string }[] }[],
    // r4-timeline: Settings → Appearance code font, size and word wrap for the Markdown.
    md: { codeFont: 'ui-monospace', codeSize: 13, wrap: true, chips: [] as ChipView[], runCommands: [] as string[] }, diffScheme: 'red-green',
  };
}
export type PrDetailView = ReturnType<typeof emptyDetail>;

// ── The panel's reads ───────────────────────────────────────────────────────

/** The topic this resource watches; R10Connect.swift `r10Wake` announces it. */
export const PR_WAKE_TOPIC = 't3.pr';
type Panel = {
  key: string; reference: PullRequestDetailSnapshotRef; cached: Obj | null;
  detail: Obj | null; detailError: unknown; detailDue: boolean; invalidate: boolean;
  activity: Obj | null; activityError: string; activityDue: boolean;
  refresh: number; epoch: number; facts: { visible: boolean; focused: boolean };
};
const panels = new WeakMap<object, Map<string, Panel>>();
/** What the last view this resource returned showed (`key|detail|activity`), so a new state is shown before the read after it. */
const shown = new WeakMap<object, string>();
const panelsOf = (client: object) => { let map = panels.get(client); if (!map) { map = new Map(); panels.set(client, map); } return map; };
const phaseOf = (panel: Panel) => {
  const detail = (panel.detail ?? panel.cached) ? 'content' : panel.detailError ? 'error' : 'ghost';
  return `${panel.key}|${detail}|${panel.activity ? 'content' : panel.activityError ? 'error' : 'ghost'}`;
};
const due = (panel: Panel) => panel.detailDue || (panel.activityDue && !!(panel.detail ?? panel.cached));
const messageOf = (error: unknown) => (error instanceof Error && error.message.trim() ? error.message : 'The environment request failed.');

export type DetailInput = { selected: string; refresh: number; now: number; visible?: boolean; focused?: boolean };
export async function pullRequestDetail(client: T3Client, native: Native | null | undefined, input: DetailInput, storage?: Files): Promise<PrDetailView> {
  const view = emptyDetail();
  view.md = markdownEnv(client); view.diffScheme = diffSchemeOf(client);
  const selection = parseSelection(input.selected);
  const live = !!native?.available && client.ready;
  if (!selection) {
    shown.delete(client);
    if (live) await holdPullRequestRefreshes(client, native!, 'detail', false);
    return view;
  }
  view.open = true; view.ref = input.selected;
  const project = client.shell.projects.find(item => item.id === selection.projectId);
  const reference = resolvePullRequestReferenceHost({ projectId: selection.projectId, ...(selection.host ? { host: selection.host } : {}), repository: selection.repository, number: selection.number }, obj(project?.repositoryIdentity));
  const listEntry = listEntryFor(client, selection);
  const key = JSON.stringify([client.environmentId, selectionRef(selection)]);
  let panel = panelsOf(client).get(key);
  if (!live) return present(view, panel ?? null, selection, listEntry, input.now, client);
  native!.watch?.(PR_WAKE_TOPIC);
  await holdPullRequestRefreshes(client, native!, 'detail', true);
  const viewKey = `pull-request:${key}`, facts = { visible: input.visible !== false, focused: input.focused !== false };
  const arriving = !shown.get(client)?.startsWith(`${key}|`);
  if (!panel) {
    const kept = readPullRequestDetailSnapshot(snapshotStorage(client.local ? client : { local: {} }), client.environmentId, reference);
    panel = { key, reference, cached: kept ?? resolveDisplayedPullRequestDetail({ live: null, cached: peekDetail(client, selectionRef(selection)), reference }),
      detail: null, detailError: null, detailDue: true, invalidate: false, activity: null, activityError: '', activityDue: true,
      refresh: input.refresh, epoch: pullRequestRefreshEpoch(client), facts };
    panelsOf(client).set(key, panel);
  } else {
    // Refresh (the menu's, Retry, "Check details are out of date."): around the server's cache.
    if (input.refresh !== panel.refresh) { panel.refresh = input.refresh; panel.detailDue = panel.activityDue = panel.invalidate = true; }
    // pullRequests.subscribeRefreshes: the server announced a change; detail and activity read again.
    const epoch = pullRequestRefreshEpoch(client);
    if (epoch !== panel.epoch) { panel.epoch = epoch; panel.detailDue = panel.activityDue = true; }
    // useLiveRefresh's window listeners: `focus`, and `visibilitychange` to visible, are arrivals.
    const shownAgain = (facts.visible && !panel.facts.visible) || (facts.focused && !panel.facts.focused);
    panel.facts = facts;
    await liveRefresh(client, native!, panel, viewKey, input, arriving || shownAgain);
  }
  if (viewRefreshedAt(client, viewKey) === undefined) noteViewRefreshed(client, viewKey, input.now); // the mount's own read fills it in
  // Something new to show first: show it, and ask again for the read that follows.
  if (due(panel) && phaseOf(panel) !== shown.get(client)) return wake(client, native!, present(view, panel, selection, listEntry, input.now, client), panel);
  const ref = { ...selectionRef(selection) };
  if (panel.detailDue) await readDetail(client, native!, panel, ref, storage);
  if (due(panel) && phaseOf(panel) !== shown.get(client)) return wake(client, native!, present(view, panel, selection, listEntry, input.now, client), panel);
  if (panel.activityDue && (panel.detail ?? panel.cached)) await readActivity(client, native!, panel, ref);
  shown.set(client, phaseOf(panel));
  return present(view, panel, selection, listEntry, input.now, client);
}
async function wake(client: T3Client, native: Native, view: PrDetailView, panel: Panel): Promise<PrDetailView> {
  shown.set(client, phaseOf(panel));
  try { await native.later({ op: 'r10Wake', topic: PR_WAKE_TOPIC }); } catch (error) { if (letGo(error)) throw error; }
  return view;
}
/** useLiveRefresh: an arrival (a reopened view, the window shown or focused again) and the 5-minute interval read the detail. */
async function liveRefresh(client: T3Client, native: Native, panel: Panel, viewKey: string, input: DetailInput, arrival: boolean): Promise<void> {
  if (panel.detailDue) return;
  const visible = input.visible !== false, now = input.now, lastRefreshedAt = viewRefreshedAt(client, viewKey);
  const interval = lastRefreshedAt !== undefined && now - lastRefreshedAt >= LIVE_REFRESH_INTERVAL_MS;
  if (!arrival && !interval) return;
  const interacted = (await lastInteraction(native, now)) ?? now;
  const read = arrival ? now - interacted < LIVE_REFRESH_IDLE_AFTER_MS && shouldRefreshOnArrival({ visible, now, lastRefreshedAt })
    : shouldRefreshOnInterval({ visible, now, lastRefreshedAt: lastRefreshedAt!, lastInteractedAt: interacted });
  if (read) { noteViewRefreshed(client, viewKey, now); panel.detailDue = true; }
}
async function readDetail(client: T3Client, native: Native, panel: Panel, ref: Obj, storage: Files | undefined): Promise<void> {
  try {
    if (panel.invalidate) await client.rpc(native, 'pullRequests.invalidate', { reference: ref }).catch((error: unknown) => { if (letGo(error)) throw error; return {}; });
    const detail = obj(await client.rpc(native, 'pullRequests.detail', ref));
    if (panel.detail && shouldRefreshPullRequestActivity({ key: panel.key, updatedAt: str(panel.detail.updatedAt) }, { key: panel.key, updatedAt: str(detail.updatedAt) })) panel.activityDue = true;
    panel.detail = detail; panel.detailError = null;
    const store = snapshotStorage(client.local ? client : { local: {} });
    writePullRequestDetailSnapshot(store, client.environmentId, panel.reference, detail);
    if (store.changed && storage) await client.savePreferences(storage);
  } catch (error) {
    if (letGo(error)) throw error; // still due: the next answer reads
    panel.detailError = error;
  }
  panel.detailDue = false; panel.invalidate = false;
  // An announcement that landed while this read was out is answered by it (the stream's first value among them).
  panel.epoch = pullRequestRefreshEpoch(client);
}
async function readActivity(client: T3Client, native: Native, panel: Panel, ref: Obj): Promise<void> {
  try {
    panel.activity = obj(await client.rpc(native, 'pullRequests.activity', ref));
    panel.activityError = '';
  } catch (error) {
    if (letGo(error)) throw error;
    // A refresh that fails keeps the conversation it had; only a first read shows the failure.
    if (!panel.activity) panel.activityError = messageOf(error);
  }
  panel.activityDue = false;
}
/** The panel as it stands: the ghost, the unavailable state, or the detail with its conversation. */
function present(view: PrDetailView, panel: Panel | null, selection: PrSelection, listEntry: Obj | null, now: number, client: T3Client): PrDetailView {
  Object.assign(view, { number: selection.number, numberLabel: `#${selection.number}`, repository: selection.repository });
  const display = panel?.detail ?? panel?.cached ?? null;
  if (!display) {
    if (panel?.detailError) {
      Object.assign(view, { phase: 'error' }, unavailable(selection, { error: messageOf(panel.detailError), notFound: isPullRequestNotFound(panel.detailError) }, client.shell.projects.find(project => project.id === selection.projectId)));
    } else {
      view.phase = 'ghost'; view.loading = true; view.ghost = ghostOf(listEntry, now);
      // loadingPullRequestCheckoutCommand: a GitHub reference already names its checkout.
      const identity = obj(client.shell.projects.find(project => project.id === selection.projectId)?.repositoryIdentity);
      if (identity.provider === 'github' || selection.host.toLowerCase() === 'github.com') view.checkoutCommand = `gh pr checkout ${selection.number}`;
    }
    return view;
  }
  const activityPending = !panel!.activity && !panel!.activityError;
  presentDetail(view, display, panel!.activity, now, { activityPending, activityError: panel!.activity ? '' : panel!.activityError, listEntry });
  view.copiedCheckout = copyNonce(client, view.checkoutCommand); view.copiedBranch = copyNonce(client, view.headBranch);
  return view;
}
/** PullRequestDetailGhost: the list row's identity and summary stay; the rest are bars. */
export function ghostOf(entry: Obj | null, now: number): ReturnType<typeof emptyGhost> {
  const ghost = emptyGhost();
  if (!entry) return ghost;
  const author = person(entry.author), measured = num(entry.additions) + num(entry.deletions) > 0;
  const checks = str(entry.checksState);
  return { ...ghost, seeded: true, title: str(entry.title), repository: str(entry.repository), url: str(entry.url), state: stateKey(entry), author: author.login, avatar: author.avatar, initial: author.initial,
    updated: `updated ${relativeLabel(entry.updatedAt, now)}`, baseBranch: str(entry.baseBranch), headBranch: str(entry.headBranch),
    additions: measured ? `+${count(num(entry.additions))}` : '', deletions: measured ? `-${count(num(entry.deletions))}` : '',
    // Passing list rollups can omit workflows awaiting approval; wait for the detail to claim success.
    checksLabel: checks === 'failing' ? 'Some checks were not successful' : checks === 'pending' ? "Some checks haven't completed yet" : '', checksTone: checks === 'failing' || checks === 'pending' ? checks : '',
    labelsKnown: Array.isArray(entry.labels), labels: arr(entry.labels).map(label => labelChip(str(label.name), labelColor(label.color))) };
}
/** pages:pr-act-activity-retry: the activity unavailable state's Retry (the Summary's also reads the detail). */
export function retryActivity(client: { environmentId: string }, selected: string, which: string): void {
  const selection = parseSelection(selected);
  const panel = selection && panelsOf(client).get(JSON.stringify([client.environmentId, selectionRef(selection)]));
  if (!panel) return;
  panel.activityError = ''; panel.activityDue = true;
  if (which === 'summary') panel.detailDue = true; // refreshDetail: the detail, the activity (and the stack)
}
/** After a write: read the detail and the conversation again, keeping what is shown meanwhile. */
function stale(client: object): void { for (const panel of panelsOf(client).values()) { panel.detailDue = true; panel.activityDue = true; } }

/** The last in-place copy per client: the value and a nonce that restarts the "Copied" swap. */
const copies = new WeakMap<object, { value: string; nonce: number }>();
export function noteCopy(client: object, value: string): void { copies.set(client, { value, nonce: (copies.get(client)?.nonce ?? 0) + 1 }); }
export function copyNonce(client: object, value: string): number { const copy = copies.get(client); return copy && value && copy.value === value ? copy.nonce : 0; }

export type PresentOptions = { activityPending?: boolean; activityError?: string; listEntry?: Obj | null };
export function presentDetail(view: PrDetailView, detail: Obj, activity: Obj | null, now: number, options: PresentOptions = {}): PrDetailView {
  const author = person(activity?.author ?? detail.author), state = stateKey(detail), checks = arr(detail.checks);
  const permissions = obj(detail.viewerPermissions), capabilities = obj(detail.capabilities);
  const key = `${str(detail.repository)}#${num(detail.number)}`;
  Object.assign(view, {
    phase: 'content', loading: false, number: num(detail.number), numberLabel: `#${num(detail.number)}`, title: str(detail.title), repository: str(detail.repository),
    url: str(detail.url), state, stateLabel: STATE_LABELS[state] ?? 'Open', conflict: conflictLabel(detail), author: author.login, authorAvatar: author.avatar, authorInitial: author.initial,
    updated: `updated ${relativeLabel(detail.updatedAt, now)}`,
    checkoutCommand: str(detail.provider, 'github') === 'github' ? `gh pr checkout ${num(detail.number)}` : '', baseBranch: str(detail.baseBranch), headBranch: str(detail.headBranch),
    files: `${count(num(detail.changedFiles))} ${num(detail.changedFiles) === 1 ? 'file' : 'files'}`, additions: `+${count(num(detail.additions))}`, deletions: `-${count(num(detail.deletions))}`,
    checksSummary: summarizeChecks(checks), checksTone: checksTone(checks), projectId: str(detail.projectId), host: hostOf(str(detail.url)),
    hostName: HOST_NAMES[str(detail.provider)] ?? 'GitHub',
    linkMenu: `${str(detail.provider)} ${str(detail.url)}`, // context-menu-gaps: the number's right-click
    activityPending: options.activityPending === true, activityError: options.activityError ?? '',
  });
  view.reviewers = arr(activity?.reviewers ?? detail.reviewers).map(person).map(({ key, login, avatar, initial }) => ({ key, login, avatar, initial }));
  view.labels = arr(detail.labels).map(label => labelChip(str(label.name), labelColor(label.color)));
  view.hasBody = str(detail.body).trim().length > 0;
  view.bodyId = `pr-body:${key}`;
  view.bodies = [...(view.hasBody ? [{ id: view.bodyId, kind: 'assistant', title: '', body: str(detail.body) }] : []), ...conversationBodies(activity)];
  view.commentCount = num(activity?.commentCount, arr(activity?.comments).length);
  view.commentsLabel = `Comments (${view.commentCount})`;
  const shared = { detail, activity, activityPending: view.activityPending, activityError: view.activityError, now };
  view.summary = presentSummary({ ...shared, listEntry: options.listEntry ?? null });
  view.timeline = presentTimeline(shared);
  // A newer rollup than the detail's checks: its headline, not a count the detail cannot back.
  if (view.summary.checksStale) { view.checksSummary = view.summary.checksState ? CHECKS_HEADLINE[view.summary.checksState] ?? 'No checks reported' : 'No checks reported'; view.checksTone = view.summary.checksState; }
  // A control belongs on the page only where the host can do it and this viewer may ask (PullRequestViewerPermissions).
  const names = (value: unknown) => (Array.isArray(value) ? value : []).filter((name): name is string => typeof name === 'string');
  const offered = (action: string) => names(capabilities.actions).includes(action) && names(permissions.actions).includes(action);
  const open = detail.state === 'open';
  view.canEdit = obj(capabilities.edit).changeRequest !== false;
  view.canClose = open && offered('close');
  view.canReopen = detail.state === 'closed' && offered('reopen');
  view.canDraft = open && detail.isDraft !== true && offered('draft');
  view.canReady = open && detail.isDraft === true && offered('ready');
  view.canMerge = open && detail.isDraft !== true && detail.mergeability !== 'conflicting' && offered('merge');
  view.canUpdateBranch = open && detail.baseComparison === 'behind' && offered('update-branch');
  view.canReview = obj(capabilities.reviewers).request === true && permissions.requestReviewers === true;
  view.canLabel = capabilities.labels !== false && permissions.labels !== false;
  return view;
}
/** PullRequestOperationError{reason:"not-found"}: GitHub's not-found, Bitbucket's 404. */
export function isPullRequestNotFound(error: unknown): boolean {
  return error instanceof ClientError && error.kind === 'PullRequestOperationError' && error.reason === 'not-found';
}
/** PullRequestsUnavailableState for a detail that never loaded: its title, text, and GitHub link. */
export function unavailable(selection: PrSelection, cached: { error: string; notFound: boolean }, project: Obj | undefined) {
  return {
    errorTitle: cached.notFound ? `Pull request #${selection.number} not found` : 'Could not load pull requests',
    error: cached.notFound ? "It may be an issue rather than a pull request, or this account can't see it." : cached.error,
    githubUrl: gitHubPullRequestBrowserUrl(obj(project?.repositoryIdentity), selection.repository, selection.number),
  };
}
/** changeRequestUrl.gitHubPullRequestBrowserUrl: only a GitHub identity, an owner/name path and a positive number. */
export function gitHubPullRequestBrowserUrl(identity: Obj, repository: string, number: number): string {
  if (identity.provider !== 'github' || !Number.isSafeInteger(number) || number < 1) return '';
  const path = repository.split('/');
  if (path.length !== 2 || path.some(segment => !segment || segment === '.' || segment === '..')) return '';
  let origin = '';
  try { const remote = new URL(str(obj(identity.locator).remoteUrl).trim()); if (remote.protocol === 'http:' || remote.protocol === 'https:') origin = remote.origin; } catch { /* SCP-style remotes use the canonical key */ }
  const hostname = str(identity.canonicalKey).split('/')[0];
  if (!origin && !hostname) return '';
  try { const url = new URL(origin || `https://${hostname}`); url.pathname = `/${path.join('/')}/pull/${number}`; return url.toString(); } catch { return ''; }
}
const HOST_NAMES: Record<string, string> = { github: 'GitHub', gitlab: 'GitLab', bitbucket: 'Bitbucket', 'azure-devops': 'Azure DevOps', forgejo: 'Forgejo' };
const hostOf = (url: string) => { try { return new URL(url).host; } catch { return ''; } };

/** pages:pr-* writes, each against the selected pull request on its host. */
export async function prCommand(client: T3Client, native: Native, op: string, selected: string, value: string): Promise<string> {
  if (op === 'activity-retry') { retryActivity(client, selected, value); return ''; }
  const selection = parseSelection(selected);
  if (!selection) throw new ClientError('Choose a pull request first.');
  const ref = selectionRef(selection);
  const label = `#${selection.number}`;
  try {
    if (op === 'action') {
      if (!ACTION_DONE[value]) throw new ClientError(`Unknown pull request action: ${value}`);
      await client.rpc(native, 'pullRequests.runAction', { ...ref, action: value }, true);
      pushToast(client, { kind: 'success', title: ACTION_DONE[value]!, description: label });
    } else if (op === 'title') {
      const title = value.trim();
      if (!title) throw new ClientError('A pull request needs a title.');
      await client.rpc(native, 'pullRequests.update', { ...ref, title: title.slice(0, 1024) }, true);
      pushToast(client, { kind: 'success', title: 'Title updated', description: label });
    } else if (op === 'label' || op === 'unlabel') {
      await client.rpc(native, 'pullRequests.setLabels', { ...ref, labels: [value], applied: op === 'label' }, true);
    } else if (op === 'request-review' || op === 'remove-review') {
      const reviewer = obj(JSON.parse(value));
      await client.rpc(native, 'pullRequests.requestReviewers', { ...ref, reviewers: [{ id: str(reviewer.id), kind: str(reviewer.kind, 'user') }], requested: op === 'request-review' }, true);
    } else if (op === 'comment') {
      if (!value.trim()) throw new ClientError('Write a comment first.');
      await client.rpc(native, 'pullRequests.comment', { ...ref, body: value.slice(0, 65_536) }, true);
      pushToast(client, { kind: 'success', title: 'Comment posted', description: label });
    } else throw new ClientError(`Unknown pull request action: ${op}`);
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'The host refused it.';
    pushToast(client, { kind: 'error', title: op === 'action' ? ACTION_FAILED[value] ?? 'Could not update this pull request' : 'Could not update this pull request', description: message });
    return message;
  }
  stale(client);
  return '';
}

type Candidates = { key: string; reviewers: Obj[]; labels: Obj[]; error: string };
const candidates = new WeakMap<object, Candidates>();
/** Reviewer and label candidates, read when their menu opens (not with every detail). */
export async function prCandidates(client: T3Client, native: Native | null | undefined, selected: string, which: string) {
  const selection = parseSelection(selected);
  const empty = { reviewers: [] as { key: string; id: string; kind: string; login: string; avatar: string; initial: string; requested: boolean }[], labels: [] as { key: string; name: string; color: string; applied: boolean; description: string }[], loading: false, error: '' };
  if (!selection || !which || !native?.available || !client.ready) return empty;
  const key = JSON.stringify([selection, which]);
  let cached = candidates.get(client);
  if (!cached || cached.key !== key) {
    cached = { key, reviewers: [], labels: [], error: '' };
    try {
      if (which === 'reviewers') cached.reviewers = arr((await client.rpc(native, 'pullRequests.reviewerCandidates', selectionRef(selection))).candidates);
      else cached.labels = arr((await client.rpc(native, 'pullRequests.labelCandidates', selectionRef(selection))).candidates);
    } catch (error) { if (letGo(error)) throw error; cached.error = error instanceof Error ? error.message : 'Could not read the candidates.'; }
    candidates.set(client, cached);
  }
  empty.error = cached.error;
  empty.reviewers = cached.reviewers.map(candidate => { const by = person(candidate); return { key: str(candidate.id), id: str(candidate.id), kind: str(candidate.kind, 'user'), login: by.login, avatar: by.avatar, initial: by.initial, requested: candidate.isRequested === true }; });
  empty.labels = cached.labels.map(candidate => ({ key: str(candidate.name), name: str(candidate.name), color: labelColor(candidate.color), applied: candidate.isApplied === true, description: str(candidate.description) }));
  return empty;
}
export function forgetCandidates(client: object): void { candidates.delete(client); }
