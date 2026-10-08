// The pull request panel's hand-offs (pr-handoffs-and-quick-actions): "Ask a question", "Explain this
// PR", "Fix findings", the Check out menu ("In a separate worktree", "In this repository"), a
// finding's own Fix on the Summary tab, and Resolve conflicts — the panel's startAsk/startHandoff
// (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: apps/web/src/components/pullRequest/
// PullRequestDetailPanel.tsx writeTaskToComposer, startAsk, startHandoff, askAboutPullRequest,
// explainPullRequest, startCheckout, startFixFinding, startFixFindings, startResolveConflicts and
// the menus that call them). The thread-opening core is r6-pr-actions.ts (checkoutHandoff,
// askInThread), shared with the thread card's Resolve and Fix, so one hand-off runs at a time whatever
// surface pressed it.
//
// Where the task lands (`attachTarget`): beside a thread (the right panel's pull request surface) the
// thread's own composer, written as one edit of the prompt on screen; on the Pull Requests page a
// thread on the pull request's project, which the window then shows (the reply's
// `sidebar:new-thread`, or `pr-handoff-next` before a checkout, app.contract commandCompleted). A value says which surface pressed it:
// "<surface>|<kind>", kind `ask`, `explain`, `findings`, `conflicts`, `checkout:worktree`,
// `checkout:local` or a finding's key (pullRequestFindingKey).
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { replaceComposerPrompt } from './composer-editor';
import { askInThread, beginCheckoutHandoff, prState, writeTaskToDraft, type HandoffTask } from './r6-pr-actions';
import { checksRollup, resolveConflictsPrompt } from './r6-pr-logic';
import {
  buildAskAboutPullRequestHandoff, buildExplainPullRequestHandoff, buildFixFindingHandoff, buildFixFindingsHandoff, contextMetadata, pullRequestFindingKey,
  pullRequestPanelContext, type PullRequestFinding,
} from './pages-pr-handoffs-logic';

/** What the panel's header and Summary show of the hand-offs. */
export type PrHandoffsView = {
  /** Which hand-off is preparing (its key), '' when none: every hand-off control is disabled meanwhile. */
  pending: string;
  /** checkoutRoot: the server knows where the repository is checked out, so it can be checked out again. */
  canCheckout: boolean;
  /** pullRequestPanelContext beside the window's thread: its own pull request hides the Check out menu there. */
  threadOwn: boolean;
};
export const emptyHandoffs = (): PrHandoffsView => ({ pending: '', canCheckout: false, threadOwn: false });

export function presentHandoffs(client: T3Client, detail: Obj | null, reference: { projectId: string; host: string; repository: string; number: number }): PrHandoffsView {
  const thread = arr(client.shell?.threads).find(entry => entry.id === client.threadId) ?? null;
  const surface = { projectId: reference.projectId, ...(reference.host ? { host: reference.host } : {}), repository: reference.repository, number: reference.number };
  const own = client.threadId
    ? pullRequestPanelContext({ projectId: str(thread?.projectId) || client.projectId, pullRequests: thread?.pullRequests,
      linkedPullRequest: thread?.linkedPullRequest ? obj(thread.linkedPullRequest) : null, branchPullRequest: thread?.branchPullRequest ? obj(thread.branchPullRequest) : null }, surface)
    : 'page';
  return { pending: prState(client).handoff, canCheckout: checkoutRoot(client, detail, reference.projectId) !== '', threadOwn: own === 'thread' };
}
/** checkoutRoot: where the server keeps the repository — the detail's, else the project's (while the detail is out). */
function checkoutRoot(client: T3Client, detail: Obj | null, projectId: string): string {
  return str(detail?.workspaceRoot) || str(arr(client.shell?.projects).find(project => project.id === projectId)?.workspaceRoot);
}

/** What a hand-off from the panel reads: the detail on screen, its conversation, and the list's row (for a newer checks rollup). */
export type HandoffContext = { detail: Obj | null; activity: Obj | null; listEntry: Obj | null };

/** PullRequestSummaryTab checksStale: a newer list rollup disagrees with the detail's checks, which are then not handed over. */
function checksStale(detail: Obj, listEntry: Obj | null): boolean {
  const detailState = checksRollup(arr(detail.checks)) || null;
  const listNewer = listEntry && Date.parse(str(listEntry.updatedAt)) > Date.parse(str(detail.updatedAt));
  const latest = listNewer ? (str(listEntry!.checksState) || null) : detailState;
  const state = latest !== 'failing' && arr(detail.checks).some(check => check.status === 'action-required') ? 'pending' : latest;
  return state !== detailState;
}
/** The finding a Fix button names, found again in what the panel shows. */
export function findingFor(key: string, detail: Obj, activity: Obj | null): PullRequestFinding | null {
  const threads = arr(activity?.reviewThreads), comments = arr(activity?.comments);
  const candidates: PullRequestFinding[] = [
    ...threads.map(thread => ({ kind: 'thread' as const, thread })),
    ...comments.map(comment => ({ kind: 'comment' as const, comment })),
    ...arr(detail.checks).map(check => ({ kind: 'check' as const, check })),
  ];
  return candidates.find(finding => pullRequestFindingKey(finding) === key) ?? null;
}

/** writeTaskToComposer into the composer on screen: one edit of its prompt, chips included. */
async function writeToComposer(client: T3Client, native: Native, task: HandoffTask): Promise<void> {
  const prompt = writeTaskToDraft(client, client.draftKey, task);
  try { await replaceComposerPrompt(client, native, prompt); }
  catch (error) { if (letGo(error)) throw error; /* the draft holds it; the composer reads it when it shows */ }
}

/**
 * `pageslocal:pr-act-handoff`: one of the panel's hand-offs. Resolves the reply's message:
 * `sidebar:new-thread` when it opened a thread the window should show.
 */
export async function prHandoffCommand(client: T3Client, native: Native, ctx: HandoffContext, value: string,
  // pr-links-previews-and-routing ("Act on"): a page hand-off for a server other than the focused one runs there (pages-pr-acton.ts).
  elsewhere?: (kind: string, task: HandoffTask | null, mode: 'worktree' | 'local', detail: Obj) => Promise<string>): Promise<string> {
  const bar = value.indexOf('|');
  const surface = bar < 0 ? 'page' : value.slice(0, bar), kind = bar < 0 ? value : value.slice(bar + 1);
  // handoffSummary: the detail, or while it is out the list's row (a checkout and Resolve need only its identity).
  const summary = ctx.detail ?? ctx.listEntry;
  if (!summary) throw new ClientError('The pull request is still loading.');
  const detail: Obj = { ...summary, workspaceRoot: checkoutRoot(client, summary, str(summary.projectId)) };
  if (prState(client).handoff) return '';
  const attach = surface === 'thread';
  const subject = { number: num(detail.number), title: str(detail.title), url: str(detail.url), headBranch: str(detail.headBranch), baseBranch: str(detail.baseBranch) };
  const opened = (thread: boolean) => (thread && !attach ? 'sidebar:new-thread' : '');
  // startAsk: a question needs a thread and nothing else.
  if (kind === 'ask' || kind === 'explain') {
    const task = kind === 'ask' ? buildAskAboutPullRequestHandoff(contextMetadata(detail)) : buildExplainPullRequestHandoff(contextMetadata(detail));
    if (attach) {
      await writeToComposer(client, native, task);
      pushToast(client, { kind: 'success', title: 'Added to the composer', description: task.prompt.length > 0
        ? 'The question is in the composer — read it over, then send.' : 'The pull request is in the composer — type your question, then send.' });
      return '';
    }
    if (elsewhere) return elsewhere(kind, task, 'worktree', detail);
    return opened(await askInThread(client, native, kind, task, str(detail.projectId)));
  }
  // startHandoff: a task the agent needs the branch for, or a checkout that carries nothing.
  let task: HandoffTask | null = null, mode: 'worktree' | 'local' = 'worktree';
  if (kind === 'checkout:worktree' || kind === 'checkout:local') mode = kind === 'checkout:local' ? 'local' : 'worktree';
  else if (kind === 'conflicts') task = { prompt: resolveConflictsPrompt(subject) };
  else if (kind === 'findings') {
    const activity = ctx.activity;
    task = buildFixFindingsHandoff({ ...subject, reviewThreads: arr(activity?.reviewThreads), comments: arr(activity?.comments),
      checks: checksStale(detail, ctx.listEntry) ? [] : arr(detail.checks), commentsTruncated: activity?.commentsTruncated === true });
  } else if (kind.startsWith('finding:')) {
    const finding = findingFor(kind, detail, ctx.activity);
    if (!finding) throw new ClientError('That finding is no longer on the pull request.');
    task = buildFixFindingHandoff({ ...subject, finding });
  } else throw new ClientError(`Unknown hand-off: ${kind}`);
  if (attach && task) {
    await writeToComposer(client, native, task);
    pushToast(client, { kind: 'success', title: 'Added to the composer', description: 'The task is in the composer — read it over, then send.' });
    return '';
  }
  if (elsewhere) return elsewhere(kind, task, mode, detail);
  if (!str(detail.workspaceRoot)) return ''; // checkoutRoot === null: nothing to check out from
  // The thread opens first and the window shows it (`pr-handoff-next`, app.contract commandCompleted), then
  // `pageslocal:pr-act-handoff-run` runs the checkout (r6-pr-actions.ts finishCheckoutHandoff).
  return (await beginCheckoutHandoff(client, native, kind, task, detail, mode)) ? 'pr-handoff-next' : '';
}
