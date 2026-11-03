// A palette thread row's leading status (T3 Code 5d3d78ab93, MIT; see LICENSE-T3): ThreadStatusIndicators.tsx
// ThreadRowLeadingStatus, which CommandPaletteThreadItems.tsx puts before each thread's title, and Sidebar.logic.ts
// resolveThreadStatusPill. The pull request is the thread's current link from its snapshot, else the one the server
// found for its branch (the sidebar's useLinkedThreadPullRequest, sidebar-branch-pr.ts); a current link with no
// snapshot yet is the muted "status pending" glyph. The reference's last fallback, the pull request of the checkout's
// git status when the row is on screen, is not read here: the clone keeps no per-row git status.
import type { T3Client } from '../client/client';
import { num, obj, str, type Obj } from '../client/domain';
import { lifecycle, lifecycleIcon, providerOfUrl, snapshotPr, statusTooltip } from '../panel/pr-surface';
import { terminologyOf } from '../details/git-logic';
import { branchPullRequest } from '../sidebar/sidebar-branch-pr';
import { rowPullRequestLinks } from '../sidebar/search-pull-request';
import { resolveThreadCurrentPullRequestLink } from '../sidebar/thread-reference';
import { backgroundHolds, lastVisited, latestRun, pendingApproval, pendingInput, shellRuntime, unseenCompletion } from '../sidebar/sidebar-model';
import { pendingSubagentRequest } from '../sidebar/subagent-requests';
import { sidebarPrefs } from '../sidebar/sidebar-state';

/** ThreadStatusPill: the label, its text-* tone, its bg-* dot and whether the dot pulses. */
export type ThreadStatusPill = { label: string; ink: string; dot: string; pulse: boolean };
const pill = (label: string, ink: string, dot: string, pulse = false): ThreadStatusPill => ({ label, ink, dot, pulse });
const MUTED = 'light-dark(#71717b, #a1a1a1)';

/** Unopened threads' failures still count as unseen, unlike completions (hasUnseenFailure). */
function unseenFailure(thread: Obj, visited: string | undefined): boolean {
  const run = latestRun(thread);
  if (run?.status !== 'failed') return false;
  if (!visited) return true;
  const completedAt = Date.parse(run.completedAt ?? ''), seen = Date.parse(visited);
  return Number.isNaN(completedAt) || Number.isNaN(seen) || completedAt > seen;
}

/** session-logic.ts isLatestRunSettled. */
function latestRunSettled(thread: Obj): boolean {
  const run = latestRun(thread);
  if (run === null || ['preparing', 'queued', 'starting', 'running', 'waiting'].includes(run.status)) return false;
  return shellRuntime(thread)?.activeRunId !== str(thread.latestRunId);
}

/** resolveThreadStatusPill, over the raw thread shell and its last visit. */
export function threadStatusPill(thread: Obj, visited: string | undefined): ThreadStatusPill | null {
  const subagent = pendingSubagentRequest(thread), runtime = shellRuntime(thread)?.status;
  if (pendingApproval(thread) || subagent === 'approval') return pill('Pending Approval', 'light-dark(#e17100, #ffd236e6)', 'light-dark(#fe9a00, #ffd236e6)');
  if (pendingInput(thread) || subagent === 'user_input') return pill('Awaiting Input', 'light-dark(#4f39f6, #a3b3ffe6)', 'light-dark(#615fff, #a3b3ffe6)');
  if (Number(thread.pendingGuestMessageCount ?? 0) > 0) return pill('Guest Request', 'light-dark(#c800de, #f4a8ff)', '#e12afb');
  const blocked = thread.blockedOnThreadWait === true;
  if ((runtime === 'running' && !blocked) || runtime === 'waiting') return pill('Working', 'light-dark(#0084d1, #74d4ffcc)', 'light-dark(#00a6f4, #74d4ffcc)', true);
  if (runtime === 'preparing' || runtime === 'starting' || runtime === 'queued') return pill('Connecting', 'light-dark(#0084d1, #74d4ffcc)', 'light-dark(#00a6f4, #74d4ffcc)', true);
  if ((runtime === 'running' && blocked) || backgroundHolds(thread)) return pill('Waiting', MUTED, MUTED);
  if (unseenFailure(thread, visited)) return pill('Failed', 'light-dark(#e7000b, #ffa2a2e6)', 'light-dark(#fb2c36, #ffa2a2e6)');
  if (!pendingInput(thread) && thread.interactionMode === 'plan' && latestRunSettled(thread) && thread.hasActionableProposedPlan === true) {
    return pill('Plan Ready', 'light-dark(#7f22fe, #c4b4ffe6)', 'light-dark(#8e51ff, #c4b4ffe6)');
  }
  if (unseenCompletion(thread, visited)) return pill('Completed', 'light-dark(#009966, #5ee9b5e6)', 'light-dark(#00bc7d, #5ee9b5e6)');
  return null;
}

/** The row's leading fields: the pull request's glyph, lifecycle state and tooltip, a pending link's label, the status pill. */
export type LeadingStatus = { leadPrIcon: string; leadPrState: string; leadPrLabel: string; leadPending: string; leadStatus: string; leadStatusInk: string; leadStatusDot: string; leadStatusPulse: boolean };

/** ThreadRowLeadingStatus for a thread of the environment whose server config is `config`. */
export function leadingStatus(client: T3Client, thread: Obj, config: Obj): LeadingStatus {
  const supportsLinks = obj(obj(config.environment).capabilities).threadPullRequests === true;
  const current = supportsLinks ? resolveThreadCurrentPullRequestLink(rowPullRequestLinks(client, thread)) : null;
  const pr = current ? snapshotPr(current) : branchPullRequest(client, thread);
  const state = pr ? lifecycle(pr.state, pr.isDraft) : '';
  const pending = !pr && current ? `${terminologyOf(providerOfUrl(str(current.url))).shortLabel} #${num(current.number)}, status pending` : '';
  const status = threadStatusPill(thread, lastVisited(thread, sidebarPrefs(client).visited[str(thread.id)]));
  return { leadPrIcon: pr ? lifecycleIcon(state) : '', leadPrState: state, leadPrLabel: pr ? statusTooltip(pr) : '', leadPending: pending,
    leadStatus: status?.label ?? '', leadStatusInk: status?.ink ?? '', leadStatusDot: status?.dot ?? '', leadStatusPulse: status?.pulse ?? false };
}
