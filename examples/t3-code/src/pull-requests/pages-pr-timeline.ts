// The pull request Timeline tab's model (MIT reference, see LICENSE-T3, T3 Code
// 1e2ecbd975 apps/web/src/components/pullRequest/PullRequestTimelineTab.tsx, its review threads at
// 74541bab17, and PullRequestDetailPanel.tsx's tab bar): buildPullRequestTimeline's events in the reader's order,
// folded by groupPullRequestTimelineConversations (consecutive plain remarks are one "N comments"
// section; commits, lifecycle, verdict rows and review threads, a finding with its replies, stand alone), and the bar's comment, commit and
// approval counts ("-" when the read failed, "…" while it runs). Commit rows are plain here: the
// Code tab they open is pages-pr-code.ts.
import { arr, num, obj, str, type Obj } from '../client/domain';
import { capitalizeTerm, terminologyOf } from '../details/git-logic';
import { relativeLabel } from './pages-prs';
import { moreRepliesText, noRemarkWrites, type PrReactionPill, type RemarkWrites } from './pages-pr-summary';
import {
  buildPullRequestTimeline, groupPullRequestTimelineConversations, isPullRequestVerdictStale, latestPullRequestReviewOutcomes, newestPullRequestCommitAt, pullRequestCommentCount,
  pullRequestReviewOutcome, pullRequestReviewOutcomeLabel, pullRequestReviewOutcomeStaleLabel, type PullRequestTimelineEvent,
} from './pages-pr-logic';

/** A remark's writes on the timeline: the pencil where this reader may rewrite it, and its reactions. */
type Writes = { canEdit: boolean; raw: string; kind: string; saving: boolean; savedSerial: number; reactions: PrReactionPill[]; reacted: string[]; canReact: boolean };
export type PrTimelineCard = { key: string; id: string; author: string; title: string; stateLabel: string; age: string; path: string; url: string; bodyId: string; body: string; markdown: boolean } & Writes;
export type PrTimelineRow = {
  key: string; kind: string; id: string; author: string; avatar: string; initial: string; hasActor: boolean; label: string; age: string; path: string; url: string;
  bodyId: string; outcome: string; outcomeLabel: string; stale: boolean; staleLabel: string; sha: string; headline: string; additions: string; deletions: string;
  count: string; authorsLine: string; cards: PrTimelineCard[]; reactions: PrReactionPill[]; reacted: string[]; canReact: boolean;
  /**
   * A review thread (ReviewThreadEvent): `cards` is its finding then its replies; the line it is on, its marks, the folded
   * summary's reply count, PullRequestMoreReplies' line, and whether it opens expanded (neither resolved nor dismissed).
   */
  location: string; resolved: boolean; dismissed: boolean; outdated: boolean; replyCount: string; moreReplies: string; expanded: boolean;
};
const writesOf = (writes: RemarkWrites, canReact: boolean): Writes => ({ canEdit: writes.canEdit, raw: writes.raw, kind: writes.kind, saving: writes.saving, savedSerial: writes.savedSerial, reactions: writes.pills, reacted: writes.reacted, canReact });
export type PrTimelineView = ReturnType<typeof emptyTimeline>;
export function emptyTimeline() {
  return { newest: [] as PrTimelineRow[], oldest: [] as PrTimelineRow[], empty: true, comments: '…', commentsAria: '', commits: '…', commitsAria: '', approvals: 0, approvalsText: '' };
}
const plural = (count: number, one: string, many: string) => `${count.toLocaleString('en-US')} ${count === 1 ? one : many}`;
/** friendlyReviewState: "CHANGES_REQUESTED" and "changes-requested" read "Changes requested". */
const friendly = (value: string) => value.toLowerCase().split('_').join(' ').split('-').join(' ').replace(/^\w/u, letter => letter.toUpperCase());
const who = (actor: Obj | null) => { const login = str(actor?.login, 'ghost'); return { author: login, avatar: str(actor?.avatarUrl), initial: login.slice(0, 1).toUpperCase(), hasActor: actor !== null }; };
const bodyIdOf = (event: PullRequestTimelineEvent) => (event.markdown && event.body ? `pr-comment:${event.id}` : '');
const blank = (key: string, kind: string): PrTimelineRow => ({ key, kind, id: '', author: '', avatar: '', initial: '', hasActor: false, label: '', age: '', path: '', url: '', bodyId: '',
  outcome: '', outcomeLabel: '', stale: false, staleLabel: '', sha: '', headline: '', additions: '', deletions: '', count: '', authorsLine: '', cards: [], reactions: [], reacted: [], canReact: false,
  location: '', resolved: false, dismissed: false, outdated: false, replyCount: '', moreReplies: '', expanded: false });

type RemarkOf = (event: PullRequestTimelineEvent) => Writes;
function rows(events: PullRequestTimelineEvent[], newestCommitAt: string | null, scope: string, now: number, remarkOf: RemarkOf, term: string): PrTimelineRow[] {
  // ConversationCard; `showPath` false inside a review thread, whose header names the line.
  const cardOf = (event: PullRequestTimelineEvent, showPath = true): PrTimelineCard => ({ key: `${scope}:${event.id}`, id: event.id, author: str(event.actor?.login, 'ghost'), title: event.title,
    stateLabel: event.reviewState ? friendly(event.reviewState) : '', age: relativeLabel(event.at, now), path: showPath ? event.path ?? '' : '', url: event.url ?? '',
    bodyId: bodyIdOf(event), body: event.markdown ? '' : event.body ?? '', markdown: event.markdown, ...remarkOf(event) });
  return groupPullRequestTimelineConversations(events).map(row => {
    if (row.kind === 'comments') {
      const actors = [...new Map(row.events.filter(event => event.actor !== null).map(event => [str(event.actor!.login), event.actor!])).values()];
      const first = row.events[0]!;
      return { ...blank(`comments:${first.id}`, 'comments'), ...who(actors[0] ?? null), hasActor: actors.length > 0, count: plural(row.events.length, 'comment', 'comments'),
        authorsLine: `${plural(actors.length, 'author', 'authors')} · ${relativeLabel(first.at, now)}`, cards: row.events.map(event => cardOf(event)) };
    }
    const event = row.event, base = { ...blank(event.id, event.kind), age: relativeLabel(event.at, now) };
    if (event.thread !== undefined) {
      // Keyed by its resolution too, so a thread resolved or dismissed elsewhere folds again (ReviewThreadEvent's key).
      const thread = event.thread, dismissed = pullRequestReviewOutcome(event.reviewState) === 'dismissed', count = thread.replies.length + thread.moreReplies;
      return { ...base, key: `thread:${scope}:${event.id}:${thread.isResolved}:${dismissed}`, kind: 'thread', id: event.id, ...who(event.actor), url: thread.url ?? '',
        location: thread.line === null ? thread.path : `${thread.path}:${thread.line}`, resolved: thread.isResolved, dismissed, outdated: thread.isOutdated,
        replyCount: count > 0 ? `${count.toLocaleString('en-US')} ${count === 1 ? 'reply' : 'replies'}` : '', moreReplies: moreRepliesText(thread), expanded: !thread.isResolved && !dismissed,
        cards: [cardOf(event, false), ...thread.replies.map(reply => cardOf(reply, false))] };
    }
    if (event.kind === 'commit') {
      const counted = event.additions !== null && event.deletions !== null && (event.additions > 0 || event.deletions > 0);
      // The whole oid, which the row's press opens in the Code tab (CommitEvent onOpen(event.id)).
      return { ...base, ...who(event.commitAuthors[0] ?? null), id: event.id, sha: event.id.slice(0, 7), headline: event.body ?? 'Untitled commit',
        additions: counted ? `+${event.additions!.toLocaleString('en-US')}` : '', deletions: counted ? `-${event.deletions!.toLocaleString('en-US')}` : '' };
    }
    const outcome = pullRequestReviewOutcome(event.reviewState);
    if (outcome !== null) {
      const stale = isPullRequestVerdictStale(event.at, newestCommitAt);
      const writes = remarkOf(event);
      return { ...base, kind: 'verdict', id: event.id, ...who(event.actor), path: event.path ?? '', url: event.url ?? '', bodyId: bodyIdOf(event), outcome,
        outcomeLabel: pullRequestReviewOutcomeLabel(outcome), stale, staleLabel: pullRequestReviewOutcomeStaleLabel(outcome), reactions: writes.reactions, reacted: writes.reacted, canReact: writes.canReact };
    }
    // LifecycleEvent: the host's term ("Merge request merged").
    const label = `${capitalizeTerm(term)} ${event.kind === 'opened' ? 'opened' : event.kind === 'merged' ? 'merged' : 'closed'}`;
    return { ...base, kind: event.kind, ...who(event.actor), label };
  });
}

export type TimelineInput = { detail: Obj; activity: Obj | null; activityPending: boolean; activityError: string; now: number; remark?: (comment: Obj) => RemarkWrites };
export function presentTimeline(input: TimelineInput): PrTimelineView {
  const view = emptyTimeline(), activity = input.activity, detail = input.detail;
  const comments = arr(activity?.comments), commits = arr(activity?.commits);
  const events = buildPullRequestTimeline({ provider: detail.provider, createdAt: detail.createdAt, author: activity?.author ?? detail.author, commits, comments, reviewThreads: arr(activity?.reviewThreads), mergedAt: detail.mergedAt, closedAt: detail.closedAt });
  const term = terminologyOf(str(detail.provider) || undefined).singular;
  const newestCommitAt = newestPullRequestCommitAt(commits), scope = `${str(detail.projectId)}#${num(detail.number)}`;
  // An event carries the remark behind it (a thread-only reply's normalised copy included) for its pencil and reactions.
  const remark = input.remark ?? noRemarkWrites, canReact = obj(detail.capabilities).reactions === true;
  const remarkOf: RemarkOf = event => writesOf(remark(event.comment ?? { id: event.id }), canReact);
  view.newest = rows(events, newestCommitAt, scope, input.now, remarkOf, term);
  view.oldest = rows([...events].reverse(), newestCommitAt, scope, input.now, remarkOf, term);
  view.empty = events.length === 0;
  const count = pullRequestCommentCount(activity);
  view.comments = input.activityError ? '-' : input.activityPending ? '…' : count.toLocaleString('en-US');
  view.commentsAria = input.activityError ? 'Comments unavailable' : plural(count, 'comment', 'comments');
  view.commits = input.activityError ? '-' : input.activityPending ? '…' : commits.length.toLocaleString('en-US');
  view.commitsAria = input.activityError ? 'Commits unavailable' : plural(commits.length, 'commit', 'commits');
  // Approvals that still stand, and none from a conversation this page holds only the recent end of.
  view.approvals = activity && activity.commentsTruncated !== true
    ? latestPullRequestReviewOutcomes(comments, commits).filter(entry => entry.outcome === 'approved' && !entry.stale).length : 0;
  view.approvalsText = view.approvals === 1 ? 'approval' : 'approvals';
  return view;
}
