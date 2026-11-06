// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r7-handoff-strip.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r7-handoff: the composer context strip's pull request badge (MIT reference, see
// LICENSE-T3, upstream f90b77d809): BranchToolbarBranchSelector in toolbar mode renders
// ThreadPullRequestBadgeControl (ThreadStatusIndicators.tsx) as a ComposerControl size="xs"
// just before the branch trigger. What it shows follows the selector:
// - the thread's current linked pull request (resolveThreadCurrentPullRequestLink) and its
//   snapshot, else, when the thread has no current link, the branch status's pull request
//   (resolveBranchToolbarPrBranch: only while the thread's branch is the resolved one and the
//   status reports that branch), so a draft handed a pull request's checkout shows it too;
// - resolveThreadPullRequestBadge folds several links into "+N" (unrelated) or a stack of N;
// - resolveThreadPullRequestBadgePresentation: the state glyph and number in the state's tone,
//   "PR #N, status pending" until a state is known; a single pull request is a link to it
//   (useOpenPrLink: the right panel's Pull request surface for a project on its host), a stack
//   or several links open the thread's Pull requests tab;
// - the tooltip (glass, w-80, side top) lists the visible links, or the one pull request.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import { currentPullRequestLink, visiblePullRequests } from './shell-pr';
import { listLines, resolveChains } from './r4-surfaces-prs';
import { lifecycle, lifecycleIcon, prTarget, providerOfUrl, shortName, sidebarPrBadge } from './r5-panels-pr';
import { peekVcsStatus } from './shell-vcs';

export type StripPrTip = { key: string; number: string; title: string; icon: string; state: string; url: string; target: string; depth: number; stack: string };
export type StripPr = { prShow: boolean; prText: string; prIcon: string; prState: string; prLabel: string; prList: boolean; prUrl: string; prTarget: string; prTips: StripPrTip[] };
export const NO_STRIP_PR: StripPr = { prShow: false, prText: '', prIcon: '', prState: '', prLabel: '', prList: false, prUrl: '', prTarget: '', prTips: [] };

type Pr = { number: number; url: string; title: string; state: string; isDraft: boolean; provider: string };
const caps = (client: T3Client): Obj => obj(obj(obj(client.config).environment).capabilities);
const snapshotOf = (link: Obj): Obj | null => (link.snapshot && typeof link.snapshot === 'object' ? obj(link.snapshot) : null);
/** linkedPullRequestSnapshotStatus. */
function snapshotPr(link: Obj): Pr | null {
  const snapshot = snapshotOf(link);
  return snapshot ? { number: num(link.number), url: str(link.url), title: str(snapshot.title), state: str(snapshot.state, 'open'), isDraft: snapshot.isDraft === true, provider: providerOfUrl(str(link.url)) } : null;
}
/** VcsStatusResult.pr with the status's source control provider. */
function statusPr(status: Obj | null): Pr | null {
  const pr = status && status.pr && typeof status.pr === 'object' ? obj(status.pr) : null;
  return pr && num(pr.number) > 0 ? { number: num(pr.number), url: str(pr.url), title: str(pr.title), state: str(pr.state, 'open'), isDraft: pr.isDraft === true, provider: str(obj(status!.sourceControlProvider).kind, 'github') } : null;
}
const STATE_LABEL: Record<string, string> = { open: 'Open', draft: 'Draft', closed: 'Closed', merged: 'Merged' };

/**
 * The badge for the strip. `threadBranch` is the thread's (or draft's) branch, `branch` the strip's
 * resolved branch, `status` the strip's repository status for `cwd` (the card's live stream wins).
 */
export function stripPr(client: T3Client, input: { cwd: string; threadBranch: string; branch: string; status: Obj | null }): StripPr {
  const thread = client.threadId ? client.shell.threads.find(entry => entry.id === client.threadId) ?? null : null;
  const many = caps(client).threadPullRequests === true;
  const links = thread ? arr(thread.pullRequests) : [];
  const current = many ? currentPullRequestLink(links) : null;
  const status = peekVcsStatus(client, input.cwd) ?? input.status;
  // resolveBranchToolbarPrBranch + branchStatusQuery.data.refName === branchPrBranch.
  const prBranch = input.threadBranch && input.threadBranch === input.branch ? input.threadBranch : '';
  const branchPr = prBranch && status && str(status.refName) === prBranch ? statusPr(status) : null;
  const displayed = (current ? snapshotPr(current) : null) ?? (current === null ? branchPr : null);
  const number = current ? num(current.number) : displayed?.number ?? 0;
  const url = current ? str(current.url) : displayed?.url ?? '';
  const badge = many && thread ? sidebarPrBadge(thread) : null;
  const target = url ? prTarget(client, url) : null;
  const base = { prUrl: url, prTarget: target ? JSON.stringify(target) : '', prTips: tips(client, links, number, url, displayed) };
  if (badge?.stacked) {
    return { ...base, prShow: true, prText: badge.badge, prIcon: 'layers', prState: badge.badgeState, prList: true,
      prLabel: `Stack of ${badge.badge} pull requests, ${STATE_LABEL[badge.badgeState]!.toLowerCase()}` };
  }
  if (!number || !url) return NO_STRIP_PR;
  const key = displayed ? lifecycle(displayed.state, displayed.isDraft) : '';
  const tooltip = displayed ? `${shortName(displayed.provider)} #${displayed.number} - ${STATE_LABEL[key]}: ${displayed.title}` : `PR #${number}, status pending`;
  const others = badge && !badge.stacked && badge.badge.startsWith('+') ? Number(badge.badge.slice(1)) - 1 : 0;
  if (badge && others > 0) {
    return { ...base, prShow: true, prText: badge.badge, prIcon: lifecycleIcon(badge.badgeState), prState: badge.badgeState, prList: true,
      prLabel: `${tooltip}, and ${others} more linked; overall ${STATE_LABEL[badge.badgeState]!.toLowerCase()}` };
  }
  return { ...base, prShow: true, prText: String(number), prIcon: displayed ? lifecycleIcon(key) : 'git-pull-request-arrow', prState: key, prList: false, prLabel: tooltip };
}

/** The badge tooltip: ThreadPullRequestsMiniList over the visible links, else the one pull request. */
function tips(client: T3Client, links: Obj[], number: number, url: string, displayed: Pr | null): StripPrTip[] {
  const visible = visiblePullRequests(links);
  if (visible.length) {
    return listLines(resolveChains(visible)).map(line => {
      const link = obj(line.link), snapshot = snapshotOf(link), key = snapshot ? lifecycle(str(snapshot.state, 'open'), snapshot.isDraft === true) : '';
      const target = prTarget(client, str(link.url)), stack = line.stack ? `${line.stack.kind === 'native' ? 'stack' : 'chain'} · ${line.stack.size}` : '';
      return { key: `${str(link.host)}/${str(link.repository)}#${num(link.number)}`, number: `#${num(link.number)}`, title: snapshot ? str(snapshot.title) : str(link.repository),
        icon: key ? lifecycleIcon(key) : 'git-pull-request-arrow', state: key, url: str(link.url), target: target ? JSON.stringify(target) : '', depth: Math.min(num(line.depth), 3), stack };
    });
  }
  if (!number || !url) return [];
  const key = displayed ? lifecycle(displayed.state, displayed.isDraft) : '', target = prTarget(client, url);
  return [{ key: `#${number}`, number: `#${number}`, title: displayed?.title || `PR #${number}, status pending`, icon: key ? lifecycleIcon(key) : 'git-pull-request-arrow', state: key, url, target: target ? JSON.stringify(target) : '', depth: 0, stack: '' }];
}
