// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r5-panels-pr.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r5-panels: the thread's pull request beside the thread (MIT reference,
// see LICENSE-T3, upstream f90b77d809):
// - the details card's rows under the branch select (components/chat/
//   ThreadDetailsPrRows.tsx, ThreadDetailsPrRow.tsx, BranchToolbarBranchSelector
//   panel mode: displayedPr / prNumber / prUrl / panelPrLabel), "Show N more"
//   for the thread's other linked pull requests;
// - where a pull request link opens (lib/openPullRequestLink.ts useOpenPrLink /
//   useOpenChangeRequestLink): a project on the link's host makes it the right
//   panel's "pull-request" surface, anything else stays an ordinary link;
// - that surface's reference and tab (rightPanelStore.ts pullRequestSurface,
//   RightPanelTabs.tsx surfaceTitle "#N" and PullRequestSurfaceIcon).
// The surface body is the Pull Requests page's detail panel over the window's
// one `prDetail` resource (pages-pr-detail.ts), fed this surface's reference.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { currentPullRequestLink, visiblePullRequests, pullRequestPanelTarget } from './shell-pr';
import { listLines, resolveChains } from './r4-surfaces-prs';
import { parseChangeRequestUrl, findProjectForChangeRequest, findProjectOnChangeRequestHost } from './palette-linkpr';
import { repositorySelector } from './composer-editor-menu';
import { readDetail, peekDetail, forgetDetails, mergeAsk, askMerge, confirmMerge, performAction, startHandoff, closeMerge, toggleShowAll } from './r6-pr-actions'; // lane r6-pr
import { rowExtra, emptyExtra, type RowExtra } from './r6-pr-row'; // lane r6-pr
import type { MergeAsk } from './r6-pr-actions';

export type PrTarget = { projectId: string; host: string; repository: string; number: number; url: string };
export type PrRow = {
  key: string; label: string; tooltip: string; icon: string; state: string; tinted: boolean; target: string; url: string; aria: string;
  split: boolean; checks: string; checksCount: string; checksAria: string; action: string; actionTarget: string; actionLabel: string; actionTooltip: string; actionDisabled: boolean;
  r6: RowExtra; // lane r6-pr: the hand-offs, Merge, the checks popover and the tooltip card
};
export type PrRowsView = { show: boolean; rows: PrRow[]; rest: PrRow[]; moreLabel: string; expanded: boolean; merge: MergeAsk };
/** ThreadDetailsPrRows' expanded state, per thread so a scheme rebuild of the card keeps it. */
const expandedThreads = new WeakMap<T3Client, Set<string>>();
export function togglePrRows(client: T3Client): void {
  let set = expandedThreads.get(client);
  if (!set) { set = new Set(); expandedThreads.set(client, set); }
  if (set.has(client.draftKey)) set.delete(client.draftKey); else set.add(client.draftKey);
}

const caps = (client: T3Client): Obj => obj(obj(obj(client.config).environment).capabilities);
const snapshotOf = (link: Obj): Obj | null => (link.snapshot && typeof link.snapshot === 'object' ? obj(link.snapshot) : null);
export const linkKey = (link: { host?: unknown; repository?: unknown; number?: unknown }) => `${str(link.host).toLowerCase()}/${str(link.repository).toLowerCase()}#${num(link.number)}`;

/** resolveChangeRequestPresentation(provider).shortName: GitLab says MR, every other host PR. */
export function shortName(kind: string): string { return kind === 'gitlab' ? 'MR' : kind === '' || ['github', 'forgejo', 'azure-devops', 'bitbucket'].includes(kind) ? 'PR' : 'change request'; }
/** linkedPullRequestSnapshotStatus: the provider a linked URL implies. */
export function providerOfUrl(url: string): string {
  return url.includes('/-/merge_requests/') ? 'gitlab' : url.includes('/pullrequest/') ? 'azure-devops' : url.includes('/pull-requests/') ? 'bitbucket' : url.includes('/pulls/') ? 'forgejo' : 'github';
}
const STATE_LABEL: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };
/** resolvePullRequestState: an open draft reads as a draft. */
export const lifecycle = (state: string, isDraft: boolean) => (state === 'open' && isDraft ? 'draft' : ['open', 'closed', 'merged'].includes(state) ? state : 'open');
/** PULL_REQUEST_STATE_PRESENTATION's glyph names (pullRequestIcons.tsx). */
export const lifecycleIcon = (key: string) => key === 'draft' ? 'git-pull-request-draft' : key === 'closed' ? 'git-pull-request-closed' : key === 'merged' ? 'git-merge' : 'git-pull-request-arrow';

type Pr = { number: number; url: string; title: string; state: string; isDraft: boolean; provider: string };
/** prStatusIndicator(pr, provider).tooltip: "PR #72 - Open: Title". */
export function statusTooltip(pr: Pr): string { return `${shortName(pr.provider)} #${pr.number} - ${STATE_LABEL[lifecycle(pr.state, pr.isDraft)]}: ${pr.title}`; }
function snapshotPr(link: Obj): Pr | null {
  const snapshot = snapshotOf(link);
  return snapshot ? { number: num(link.number), url: str(link.url), title: str(snapshot.title), state: str(snapshot.state, 'open'), isDraft: snapshot.isDraft === true, provider: providerOfUrl(str(link.url)) } : null;
}
function vcsPr(value: unknown, provider: unknown): Pr | null {
  const pr = value && typeof value === 'object' ? obj(value) : null;
  return pr && num(pr.number) > 0 ? { number: num(pr.number), url: str(pr.url), title: str(pr.title), state: str(pr.state, 'open'), isDraft: pr.isDraft === true, provider: str(obj(provider).kind, 'github') } : null;
}

/**
 * useOpenPrLink without a modifier: the project this environment reads the link through (its own
 * repository, else, where links are many-per-thread, any project on the link's host). Null keeps
 * the link an ordinary one.
 */
export function prTarget(client: T3Client, url: string): PrTarget | null {
  const parsed = parseChangeRequestUrl(url), capabilities = caps(client);
  if (!parsed || capabilities.pullRequests !== true) return null;
  const projects = client.shell.projects, many = capabilities.threadPullRequests === true;
  const project = findProjectForChangeRequest(projects, parsed) ?? (many ? findProjectOnChangeRequestHost(projects, parsed) : undefined);
  if (!project) return null;
  return { projectId: str(project.id), host: many ? (parsed.authority ?? parsed.host) : '', repository: many ? parsed.repository : repositorySelector(obj(project.repositoryIdentity)) || parsed.repository, number: parsed.number, url };
}
/** threadPullRequestPanelTarget as the chooser's "Pull request" (P) opens it: available where the environment reads pull requests. */
export function threadPrTarget(client: T3Client): PrTarget | null {
  const thread = client.shell.threads.find(entry => entry.id === client.threadId);
  if (!thread || caps(client).pullRequests !== true) return null;
  const target = pullRequestPanelTarget(thread);
  if (!target || num(target.number) <= 0 || !str(target.repository)) return null;
  const host = str((target as Obj).host) || (caps(client).threadPullRequests === true ? parseChangeRequestUrl(str(target.url))?.host ?? '' : '');
  return { projectId: str(target.projectId) || str(thread.projectId), host, repository: str(target.repository), number: num(target.number), url: str(target.url) };
}
/** The Pull Requests page's selection string (pages-pr-detail.ts parseSelection) for a target. */
export const selectionOf = (target: PrTarget) => JSON.stringify({ projectId: target.projectId, host: target.host, repository: target.repository, number: target.number });
/** pullRequestSurfaceId: the reference lives in the id, so several pull requests stay peer tabs. */
export const prSurfaceId = (target: PrTarget) =>
  `pull-request:${encodeURIComponent(target.projectId)}:${target.host ? `${encodeURIComponent(target.host.toLowerCase())}:` : ''}${encodeURIComponent(target.repository)}:${target.number}`;

/** PullRequestSurfaceIcon: the linked snapshot's lifecycle glyph in its tone, else the muted pull request glyph. */
export function prTabIcon(client: T3Client, target: PrTarget): { icon: string; state: string } {
  if (caps(client).threadPullRequests === true && target.host) {
    let newest: Obj | null = null;
    for (const thread of client.shell.threads) for (const link of visiblePullRequests(thread.pullRequests)) {
      if (linkKey(link) !== linkKey(target)) continue;
      if (!newest || str(snapshotOf(link)?.syncedAt) > str(snapshotOf(newest)?.syncedAt)) newest = link;
    }
    const snapshot = newest ? snapshotOf(newest) : null;
    if (snapshot) { const key = lifecycle(str(snapshot.state, 'open'), snapshot.isDraft === true); return { icon: lifecycleIcon(key), state: key }; }
  }
  // lane r6-pr: without a linked snapshot the tab follows the loaded detail (newestPullRequestSummary(detail, …)).
  const detail = caps(client).pullRequests === true ? peekDetail(client, tabReference(client, target)) : null;
  if (detail) { const key = lifecycle(str(detail.state, 'open'), detail.isDraft === true); return { icon: lifecycleIcon(key), state: key }; }
  return { icon: 'git-pull-request-arrow', state: '' };
}
/** PullRequestSurfaceIcon's detail read: the surface's project, its host where links are many-per-thread. */
const tabReference = (client: T3Client, target: PrTarget): Obj => ({ projectId: target.projectId, host: caps(client).threadPullRequests === true ? target.host : '', repository: target.repository, number: target.number });
/** Reads the detail behind every pull request tab that has no linked snapshot, so its icon can take the state. */
export async function prefetchPrTabs(client: T3Client, native: Native, targets: PrTarget[], now: number): Promise<void> {
  if (caps(client).pullRequests !== true) return;
  for (const target of targets) if (prTabIcon(client, target).state === '') await readDetail(client, native, tabReference(client, target), now);
}

// ── The host's detail behind the card's row (ThreadDetailsPrRow detailQuery + checksQuery): lane r6-pr's shared reads ──
export function forgetRowDetail(client: T3Client): void { forgetDetails(client); }

// classifyPullRequestChecks, pullRequestChecksState and resolveThreadPanelPullRequestAction live in r6-pr-logic.ts.
export { checksState, checksRollup, rowAction } from './r6-pr-logic';

function simpleRow(client: T3Client, number: number, url: string, label: string, pr: Pr | null): PrRow {
  const target = prTarget(client, url);
  return {
    key: `pr-${number}-${url}`, label, tooltip: pr ? statusTooltip(pr) : `Pull request #${number}`,
    // ChangeRequestStatusIcon(state) without isDraft: the glyph follows the lifecycle, the tone the draft-aware status.
    icon: pr ? lifecycleIcon(lifecycle(pr.state, false)) : 'git-pull-request-arrow', state: pr ? lifecycle(pr.state, pr.isDraft) : '', tinted: !!pr,
    target: target ? JSON.stringify(target) : '', url, aria: url || 'Open pull request',
    split: false, checks: '', checksCount: '', checksAria: '', action: '', actionTarget: '', actionLabel: '', actionTooltip: '', actionDisabled: false, r6: emptyExtra(),
  };
}
/**
 * The card's pull request rows (panel display mode of BranchToolbarBranchSelector): the thread's
 * current link, else its branch's pull request, as "#N[: title]"; with the host's detail the row
 * splits into its state glyph, the checks and the one action worth taking.
 */
export async function prRowsView(client: T3Client, native: Native, input: { status: Obj | null; threadId: string; now: number; projectId: string; rightGap?: number; inline?: boolean }): Promise<PrRowsView> {
  const none: PrRowsView = emptyPrRows();
  const thread = client.shell.threads.find(entry => entry.id === input.threadId);
  const capabilities = caps(client), many = capabilities.threadPullRequests === true;
  const links = arr(thread?.pullRequests);
  const current = many ? currentPullRequestLink(links) : null;
  const linkedPr = current ? snapshotPr(current) : null;
  // branchPr: the status read's pull request when it reports the thread's own branch.
  // lane r6-pr: a draft reads its own branch (a hand-off's checkout) the way a thread reads its own.
  const branch = thread ? str(thread.branch) : str(client.local.composerControls?.contexts?.[client.draftKey]?.branch);
  const branchPr = branch && input.status && str(input.status.refName) === branch ? vcsPr(input.status.pr, input.status.sourceControlProvider) : null;
  const displayed = linkedPr ?? (current === null ? branchPr : null);
  const number = current ? num(current.number) : displayed?.number ?? 0, url = current ? str(current.url) : displayed?.url ?? '';
  if (!number || !url) return none;
  const label = `#${number}${displayed?.title.trim() ? `: ${displayed.title}` : ''}`;
  const main = simpleRow(client, number, url, label, displayed);
  // ThreadDetailsPrRow's reference: read through the thread's project on the host.
  const project = client.shell.projects.find(entry => entry.id === (thread ? str(thread.projectId) : input.projectId));
  const repository = repositorySelector(obj(project?.repositoryIdentity));
  const reference = capabilities.pullRequests === true && project
    ? current ? { projectId: str(project.id), host: str(current.host), repository: str(current.repository), number }
      : repository ? { projectId: str(project.id), repository, number } : null
    : null;
  const detail = native?.available ? await readDetail(client, native, reference, input.now) : null;
  if (detail && reference) {
    const key = lifecycle(str(detail.state, 'open'), detail.isDraft === true);
    // lane r6-pr: the split row's checks, trailing action and tooltip card (r6-pr-row.ts).
    Object.assign(main, { icon: lifecycleIcon(key), state: key, tinted: true, split: true },
      rowExtra(client, detail, reference, { key: main.key, lifecycleKey: key, rightGap: input.rightGap ?? 0, inline: input.inline === true }));
  }
  // ThreadDetailsPrRows: the other visible links, in the list's order, behind "Show N more".
  const rest = current === null ? [] : listLines(resolveChains(links)).map(line => line.link).filter(link => linkKey(link) !== linkKey(current))
    .map(link => { const pr = snapshotPr(link), snapshot = snapshotOf(link); return simpleRow(client, num(link.number), str(link.url), `#${num(link.number)}${snapshot ? `: ${str(snapshot.title)}` : ''}`, pr); });
  const expanded = rest.length > 0 && expandedThreads.get(client)?.has(client.draftKey) === true;
  return { show: true, rows: [main], rest: expanded ? rest : [], moreLabel: rest.length ? (expanded ? 'Show less' : `Show ${rest.length} more`) : '', expanded, merge: mergeAsk(client) };
}
// A declaration, so shell-details' module-level empty view can call it through the import cycle.
export function emptyPrRows(): PrRowsView { return { show: false, rows: [], rest: [], moreLabel: '', expanded: false, merge: { open: false, number: 0, description: '', target: '', pending: false } }; }

/**
 * `shell:surface-r5-pr-*` (lane r6-pr): Ready and the confirmed Merge run against the host; Resolve
 * and Fix hand the pull request to a new thread. `id` is the row's reference (Merge adds its method).
 */
export async function prRowCommand(client: T3Client, native: Native, op: string, id: string): Promise<string> {
  const target = obj(JSON.parse(id || '{}'));
  if (op === 'ready') return performAction(client, native, target, 'ready');
  if (op === 'merge') return confirmMerge(client, native, id);
  if (op === 'resolve' || op === 'fix') {
    const detail = await readDetail(client, native, target, 0);
    if (!detail) return '';
    return startHandoff(client, native, op === 'resolve' ? 'conflicts' : 'findings', detail);
  }
  return '';
}
/** `surface-r5-pr-*` without the command gate (lane r6-pr): the Merge dialog and the checks popover's "Show all". */
export function prRowLocal(client: T3Client, op: string, id: string, value: string): boolean {
  if (op === 'merge-ask') { const target = obj(JSON.parse(id || '{}')); askMerge(client, target, num(target.number), (value || 'merge') as 'merge'); return true; }
  if (op === 'merge-cancel') { closeMerge(client); return true; }
  if (op === 'checks-all') { toggleShowAll(client, id); return true; }
  return false;
}

/**
 * resolveThreadPullRequestBadge + resolveThreadPullRequestBadgePresentation for the sidebar row
 * (shared/threadPullRequests.ts, ThreadStatusIndicators.tsx): one link is its number behind its
 * snapshot's glyph (muted until a snapshot arrives); several unrelated links fold into "+N" in
 * their aggregate state; one chain of several is a stack of N.
 */
export function sidebarPrBadge(thread: Obj): { badge: string; stacked: boolean; badgeIcon: string; badgeState: string } {
  const visible = visiblePullRequests(thread.pullRequests);
  if (!visible.length) return { badge: '', stacked: false, badgeIcon: '', badgeState: '' };
  const states = visible.map(link => str(snapshotOf(link)?.state, 'open'));
  const aggregate = visible.every(link => snapshotOf(link)?.state === 'open' && snapshotOf(link)?.isDraft === true) ? 'draft'
    : states.includes('open') ? 'open' : states.every(state => state === 'merged') ? 'merged' : 'closed';
  if (visible.length > 1 && resolveChains(visible).length === 1) return { badge: String(visible.length), stacked: true, badgeIcon: 'layers', badgeState: aggregate };
  if (visible.length > 1) return { badge: `+${visible.length}`, stacked: false, badgeIcon: lifecycleIcon(aggregate), badgeState: aggregate };
  const pr = snapshotPr(visible[0]!);
  return { badge: String(num(visible[0]!.number)), stacked: false, badgeIcon: pr ? lifecycleIcon(lifecycle(pr.state, pr.isDraft)) : 'git-pull-request-arrow', badgeState: pr ? lifecycle(pr.state, pr.isDraft) : '' };
}
