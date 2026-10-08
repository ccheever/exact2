// The pull request Timeline tab's model (lane "pages"; MIT reference, see LICENSE-T3, T3 Code
// 1e2ecbd975 apps/web/src/components/pullRequest/PullRequestTimelineTab.tsx and
// PullRequestDetailPanel.tsx's tab bar): buildPullRequestTimeline's events in the reader's order,
// folded by groupPullRequestTimelineConversations (consecutive plain remarks are one "N comments"
// section; commits, lifecycle and verdict rows stand alone), and the bar's comment, commit and
// approval counts ("—" when the read failed, "…" while it runs). Commit rows are plain here: the
// Code tab they open belongs to 20261005-pr-code-tab.
import { arr, num, str, type Obj } from './domain';
import { relativeLabel } from './pages-prs';
import { fullDate } from './pages-pr-summary';
import {
  buildPullRequestTimeline, groupPullRequestTimelineConversations, isPullRequestVerdictStale, latestPullRequestReviewOutcomes, newestPullRequestCommitAt,
  pullRequestReviewOutcome, pullRequestReviewOutcomeLabel, pullRequestReviewOutcomeStaleLabel, type PullRequestTimelineEvent,
} from './pages-pr-logic';

export type PrTimelineCard = { key: string; id: string; author: string; title: string; stateLabel: string; age: string; ageTip: string; path: string; url: string; bodyId: string; body: string; markdown: boolean };
export type PrTimelineRow = {
  key: string; kind: string; author: string; avatar: string; initial: string; hasActor: boolean; label: string; age: string; ageTip: string; path: string; url: string;
  bodyId: string; outcome: string; outcomeLabel: string; stale: boolean; staleLabel: string; sha: string; headline: string; additions: string; deletions: string;
  count: string; authorsLine: string; cards: PrTimelineCard[];
};
export type PrTimelineView = ReturnType<typeof emptyTimeline>;
export function emptyTimeline() {
  return { newest: [] as PrTimelineRow[], oldest: [] as PrTimelineRow[], empty: true, comments: '…', commentsAria: '', commits: '…', commitsAria: '', approvals: 0, approvalsText: '' };
}
const plural = (count: number, one: string, many: string) => `${count.toLocaleString('en-US')} ${count === 1 ? one : many}`;
/** friendlyReviewState: "CHANGES_REQUESTED" and "changes-requested" read "Changes requested". */
const friendly = (value: string) => value.toLowerCase().split('_').join(' ').split('-').join(' ').replace(/^\w/u, letter => letter.toUpperCase());
const who = (actor: Obj | null) => { const login = str(actor?.login, 'ghost'); return { author: login, avatar: str(actor?.avatarUrl), initial: login.slice(0, 1).toUpperCase(), hasActor: actor !== null }; };
const bodyIdOf = (event: PullRequestTimelineEvent) => (event.markdown && event.body ? `pr-comment:${event.id}` : '');
const blank = (key: string, kind: string): PrTimelineRow => ({ key, kind, author: '', avatar: '', initial: '', hasActor: false, label: '', age: '', ageTip: '', path: '', url: '', bodyId: '',
  outcome: '', outcomeLabel: '', stale: false, staleLabel: '', sha: '', headline: '', additions: '', deletions: '', count: '', authorsLine: '', cards: [] });

function rows(events: PullRequestTimelineEvent[], newestCommitAt: string | null, scope: string, now: number): PrTimelineRow[] {
  return groupPullRequestTimelineConversations(events).map(row => {
    if (row.kind === 'comments') {
      const actors = [...new Map(row.events.filter(event => event.actor !== null).map(event => [str(event.actor!.login), event.actor!])).values()];
      const first = row.events[0]!;
      return { ...blank(`comments:${first.id}`, 'comments'), ...who(actors[0] ?? null), hasActor: actors.length > 0, count: plural(row.events.length, 'comment', 'comments'),
        authorsLine: `${plural(actors.length, 'author', 'authors')} · ${relativeLabel(first.at, now)}`,
        cards: row.events.map(event => ({ key: `${scope}:${event.id}`, id: event.id, author: str(event.actor?.login, 'ghost'), title: event.title,
          stateLabel: event.reviewState ? friendly(event.reviewState) : '', age: relativeLabel(event.at, now), ageTip: fullDate(event.at), path: event.path ?? '', url: event.url ?? '',
          bodyId: bodyIdOf(event), body: event.markdown ? '' : event.body ?? '', markdown: event.markdown })) };
    }
    const event = row.event, base = { ...blank(event.id, event.kind), age: relativeLabel(event.at, now), ageTip: fullDate(event.at) };
    if (event.kind === 'commit') {
      const counted = event.additions !== null && event.deletions !== null && (event.additions > 0 || event.deletions > 0);
      return { ...base, ...who(event.commitAuthors[0] ?? null), sha: event.id.slice(0, 7), headline: event.body ?? 'Untitled commit',
        additions: counted ? `+${event.additions!.toLocaleString('en-US')}` : '', deletions: counted ? `-${event.deletions!.toLocaleString('en-US')}` : '' };
    }
    const outcome = pullRequestReviewOutcome(event.reviewState);
    if (outcome !== null) {
      const stale = isPullRequestVerdictStale(event.at, newestCommitAt);
      return { ...base, kind: 'verdict', ...who(event.actor), path: event.path ?? '', url: event.url ?? '', bodyId: bodyIdOf(event), outcome,
        outcomeLabel: pullRequestReviewOutcomeLabel(outcome), stale, staleLabel: pullRequestReviewOutcomeStaleLabel(outcome) };
    }
    const label = event.kind === 'opened' ? 'Pull request opened' : event.kind === 'merged' ? 'Pull request merged' : 'Pull request closed';
    return { ...base, kind: event.kind, ...who(event.actor), label };
  });
}

export type TimelineInput = { detail: Obj; activity: Obj | null; activityPending: boolean; activityError: string; now: number };
export function presentTimeline(input: TimelineInput): PrTimelineView {
  const view = emptyTimeline(), activity = input.activity, detail = input.detail;
  const comments = arr(activity?.comments), commits = arr(activity?.commits);
  const events = buildPullRequestTimeline({ createdAt: detail.createdAt, author: activity?.author ?? detail.author, commits, comments, mergedAt: detail.mergedAt, closedAt: detail.closedAt });
  const newestCommitAt = newestPullRequestCommitAt(commits), scope = `${str(detail.projectId)}#${num(detail.number)}`;
  view.newest = rows(events, newestCommitAt, scope, input.now);
  view.oldest = rows([...events].reverse(), newestCommitAt, scope, input.now);
  view.empty = events.length === 0;
  const count = num(activity?.commentCount);
  view.comments = input.activityError ? '—' : input.activityPending ? '…' : count.toLocaleString('en-US');
  view.commentsAria = input.activityError ? 'Comments unavailable' : plural(count, 'comment', 'comments');
  view.commits = input.activityError ? '—' : input.activityPending ? '…' : commits.length.toLocaleString('en-US');
  view.commitsAria = input.activityError ? 'Commits unavailable' : plural(commits.length, 'commit', 'commits');
  // Approvals that still stand, and none from a conversation this page holds only the recent end of.
  view.approvals = activity && activity.commentsTruncated !== true
    ? latestPullRequestReviewOutcomes(comments, commits).filter(entry => entry.outcome === 'approved' && !entry.stale).length : 0;
  view.approvalsText = view.approvals === 1 ? 'approval' : 'approvals';
  return view;
}
