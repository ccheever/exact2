// The pull request Summary tab's model (MIT reference, see LICENSE-T3, T3 Code
// 74541bab17 apps/web/src/components/pullRequest/PullRequestSummaryTab.tsx): reviewers with the
// verdict each last gave (a ring, the sr-only words and the "<name> — <outcome>" tooltip), the
// checks with "Check details are out of date." when a newer rollup disagrees, and the
// conversation through the comment filter — bot comments and finished work (a resolved thread,
// a dismissed review, drawn collapsed) shown only when the filter asks for them — as loose comments
// and review threads (a finding with its replies under it), windowed 10 conversations at a time
// from the newest. The filter is this device's (commentFilter, kept in the pages
// preferences as the reference keeps it in localStorage); the window and the order are the tab's own
// state (pages-pr-summary.contract); this model gives every card its place from the newest (`fromEnd`)
// and both orders.
import { arr, num, obj, str, type Obj } from '../client/domain';
import { pagesPrefs } from '../pages/pages-prefs';
import { relativeLabel } from './pages-prs';
import { currentTimestampLocale } from '../shared/timestamp-format';
import { checkStatusLabel, checksRollup } from '../details/pr-row-logic';
import {
  filterPullRequestConversationItems, isPullRequestCommentFinished, latestPullRequestReviewOutcomes, orderPullRequestComments, pullRequestCommentCount, pullRequestConversationItemComments,
  pullRequestConversationItems, pullRequestReviewOutcome, pullRequestReviewOutcomeLabel, pullRequestReviewOutcomeStaleLabel, visibleBody, type PullRequestConversationItem,
} from './pages-pr-logic';
import { pullRequestFindingKey, remarkFinding } from './pages-pr-handoffs-logic';

export const COMMENT_PAGE = 10;
export type PrFace = { key: string; login: string; avatar: string; initial: string };
/**
 * What a remark carries for the writes (pages-pr-writes.ts): whether this
 * reader may rewrite it (and its raw words and kind for the editor), whether a save is out and how
 * many have landed, and its reaction pills with this client's presses in flight laid over.
 */
export type PrReactionPill = { content: string; emoji: string; count: number; pressed: boolean; label: string; tooltip: string };
export type RemarkWrites = { canEdit: boolean; raw: string; kind: string; saving: boolean; savedSerial: number; pills: PrReactionPill[]; reacted: string[] };
export const noRemarkWrites = (comment: Obj): RemarkWrites => ({ canEdit: false, raw: str(comment.body), kind: str(comment.kind), saving: false, savedSerial: 0, pills: [], reacted: [] });
export type PrCommentCard = {
  key: string; id: string; bodyId: string; author: string; avatar: string; initial: string; authorTip: string; age: string; ageTip: string; url: string;
  outcome: string; outcomeLabel: string; stateLabel: string; location: string; outdated: boolean; fromEnd: number; finishedLabel: string; preview: string;
  canEdit: boolean; raw: string; kind: string; saving: boolean; savedSerial: number; reactions: PrReactionPill[]; reacted: string[]; canReact: boolean;
  /** The finding its Fix hands over (pullRequestFindingKey), '' for talk and verdicts. */
  fix: string;
  /** Finished work (a resolved thread's comment, a dismissed review): drawn collapsed. */
  finished: boolean;
  /** A reply under its finding: no location and no Fix. */
  reply: boolean;
};
/**
 * One conversation: a loose comment (`replies` empty), or a review thread's finding (`card`) with its replies, the
 * count the collapsed card shows ("N replies", the unread ones counted) and PullRequestMoreReplies' line and link.
 */
export type PrConversation = { key: string; fromEnd: number; finished: boolean; card: PrCommentCard; replies: PrCommentCard[]; replyCount: string; moreReplies: string; moreUrl: string };
export type PrReviewerEntry = { key: string; login: string; avatar: string; initial: string; outcome: string; stale: boolean; outcomeText: string; tooltip: string };
export type PrSummaryCheck = { key: string; name: string; status: string; statusLabel: string; url: string; fix: string };
export type PrSummaryView = ReturnType<typeof emptySummary>;

export function emptySummary() {
  return {
    key: '', activityPending: false, activityError: '', reviewers: [] as PrReviewerEntry[], showLabels: true,
    checksStale: false, checksState: '', checks: [] as PrSummaryCheck[], commentsTitle: 'Comments (0)', truncated: false, truncatedText: '', empty: true,
    activeCount: 0, newest: [] as PrConversation[], oldest: [] as PrConversation[],
    // The filter: what it shows, its trigger's words ("Filter", "3 hidden") and the line for a conversation it hides whole.
    showBots: false, showResolved: false, filterLabel: 'Filter', filteredOutText: '',
  };
}

/**
 * Which kinds of comment the conversation shows. Kept per device rather than per pull request:
 * whether a reader wants bot reports and finished threads in front of them is a habit, not a
 * property of one change.
 */
export type CommentFilter = { bots: boolean; resolved: boolean };
export function commentFilter(client: { local: object }): CommentFilter {
  const saved = pagesPrefs(client).commentFilter;
  return { bots: saved?.bots === true, resolved: saved?.resolved === true };
}
/** `chatlocal:pr-ui-comment-filter`: the menu's checkbox ("bots" or "resolved") flips. */
export function toggleCommentFilter(client: { local: object }, which: string): void {
  if (which !== 'bots' && which !== 'resolved') return;
  const next = commentFilter(client);
  next[which] = !next[which];
  pagesPrefs(client).commentFilter = next;
}

const face = (actor: unknown): PrFace => { const person = obj(actor), login = str(person.login, 'ghost'); return { key: login.toLowerCase(), login, avatar: str(person.avatarUrl), initial: login.slice(0, 1).toUpperCase() }; };
/** PullRequestActorLabel's tooltip: "Name (@login)" where the host gave a different name. */
const named = (actor: unknown) => { const person = obj(actor), login = str(person.login, 'ghost'), name = str(person.name); return name && name !== login ? `${name} (@${login})` : login; };
export const fullDate = (iso: string) => { const at = Date.parse(iso); return Number.isFinite(at) ? new Date(at).toLocaleString(currentTimestampLocale()) : ''; };
/** "CHANGES_REQUESTED" reads as "Changes requested". */
export const reviewStateLabel = (state: string) => { const words = state.toLowerCase().split('_').join(' '); return words.charAt(0).toUpperCase() + words.slice(1); };
const isBot = (actor: unknown) => { const person = obj(actor); return person.isBot === true || str(person.login).endsWith('[bot]'); };
/** CollapsedComment's one-line preview of a finished remark. */
export const previewText = (body: string) => body.replace(/<!--[\s\S]*?-->/gu, '').replace(/^\s*>?\s*\[!\w+\]\s*$/gmu, '').replace(/!?(\[([^\]]+)\])\([^)]*\)/gu, '$2')
  .replace(/^[\s>#*-]+/gmu, '').replace(/[*`]/gu, '').replace(/\s+/g, ' ').trim();

/** The markdown documents the conversation renders: every remark with something visible to say, thread-only replies included. */
export function conversationBodies(activity: Obj | null): { id: string; kind: string; title: string; body: string }[] {
  return pullRequestConversationItems(arr(activity?.comments), arr(activity?.reviewThreads)).flatMap(pullRequestConversationItemComments)
    .flatMap(comment => { const body = visibleBody(str(comment.body)); return body === null ? [] : [{ id: `pr-comment:${str(comment.id)}`, kind: 'assistant', title: '', body }]; });
}

function card(comment: Obj, thread: Obj | undefined, url: string, now: number, fromEnd: number, remark: (comment: Obj) => RemarkWrites, canReact: boolean, reply = false): PrCommentCard {
  const person = face(comment.author), writes = remark(comment);
  const outcome = pullRequestReviewOutcome(str(comment.reviewState) || null), body = visibleBody(str(comment.body));
  const path = str(thread?.path) || str(comment.path), line = num(thread?.line);
  return {
    key: `${url}:${str(comment.id)}`, id: str(comment.id), bodyId: body === null ? '' : `pr-comment:${str(comment.id)}`,
    author: person.login, avatar: person.avatar, initial: person.initial, authorTip: named(comment.author),
    age: relativeLabel(comment.createdAt, now), ageTip: `${fullDate(str(comment.createdAt))}${str(comment.url) ? ' · Open comment on host' : ''}`, url: str(comment.url),
    outcome: outcome ?? '', outcomeLabel: outcome ? pullRequestReviewOutcomeLabel(outcome) : '', stateLabel: !outcome && str(comment.reviewState) ? reviewStateLabel(str(comment.reviewState)) : '',
    location: reply || !path ? '' : `${path}${thread && line ? `:${line}` : ''}`, outdated: !reply && thread?.isOutdated === true, fromEnd,
    finishedLabel: thread?.isResolved === true ? 'Resolved' : 'Review dismissed', preview: body === null ? '' : previewText(body),
    canEdit: writes.canEdit, raw: writes.raw, kind: writes.kind, saving: writes.saving, savedSerial: writes.savedSerial, reactions: writes.pills, reacted: writes.reacted, canReact,
    fix: reply ? '' : (finding => (finding ? pullRequestFindingKey(finding) : ''))(remarkFinding(comment, thread, outcome)),
    finished: isPullRequestCommentFinished(comment, thread), reply,
  };
}
/** PullRequestMoreReplies: "N more replies" (or "More replies" where the host did not count them). */
export const moreRepliesText = (item: { moreReplies: number; hasMoreReplies: boolean }) => !item.hasMoreReplies ? ''
  : item.moreReplies > 0 ? `${item.moreReplies.toLocaleString('en-US')} more ${item.moreReplies === 1 ? 'reply' : 'replies'}` : 'More replies';
function conversation(item: PullRequestConversationItem, url: string, now: number, fromEnd: number, remark: (comment: Obj) => RemarkWrites, canReact: boolean): PrConversation {
  if (item.kind === 'comment') {
    const root = card(item.comment, undefined, url, now, fromEnd, remark, canReact);
    return { key: `${url}:comment:${item.id}`, fromEnd, finished: root.finished, card: root, replies: [], replyCount: '', moreReplies: '', moreUrl: '' };
  }
  const root = card(item.root, item.thread, url, now, fromEnd, remark, canReact), count = item.replies.length + item.moreReplies;
  return { key: `${url}:thread:${item.id}`, fromEnd, finished: root.finished, card: root, replies: item.replies.map(reply => card(reply, undefined, url, now, fromEnd, remark, canReact, true)),
    replyCount: count > 0 ? `${count} ${count === 1 ? 'reply' : 'replies'}` : '', moreReplies: moreRepliesText(item), moreUrl: item.url ?? '' };
}
const both = (items: PrConversation[]) => ({ newest: [...orderPullRequestComments(items, 'newest')], oldest: items });

export type SummaryInput = { detail: Obj; activity: Obj | null; activityPending: boolean; activityError: string; now: number; listEntry: Obj | null; remark?: (comment: Obj) => RemarkWrites; filter?: CommentFilter };
export function presentSummary(input: SummaryInput): PrSummaryView {
  const { detail, activity, now } = input;
  const view = emptySummary(), url = str(detail.url), capabilities = obj(detail.capabilities);
  view.key = url; view.activityPending = input.activityPending; view.activityError = input.activityError;
  const comments = arr(activity?.comments), commits = arr(activity?.commits);
  // Reviewers: the people asked, then anyone who ruled without being asked; each wears their last verdict.
  const outcomes = latestPullRequestReviewOutcomes(comments, commits);
  const byLogin = new Map(outcomes.flatMap(entry => (entry.actor ? [[str(entry.actor.login).toLowerCase(), entry] as const] : [])));
  const asked = arr(activity?.reviewers ?? detail.reviewers);
  const entries = [...asked.map(actor => ({ key: str(actor.login), actor: actor as Obj | null, outcome: byLogin.get(str(actor.login).toLowerCase()) ?? null })),
    ...outcomes.filter(entry => !asked.some(actor => entry.actor !== null && str(actor.login).toLowerCase() === str(entry.actor.login).toLowerCase())).map(entry => ({ key: entry.key, actor: entry.actor, outcome: entry }))];
  view.reviewers = entries.map(entry => {
    const person = face(entry.actor), verdict = entry.outcome;
    const words = verdict ? (verdict.stale ? pullRequestReviewOutcomeStaleLabel(verdict.outcome) : pullRequestReviewOutcomeLabel(verdict.outcome)) : '';
    return { key: entry.key, login: person.login, avatar: person.avatar, initial: person.initial, outcome: verdict?.outcome ?? '', stale: verdict?.stale ?? false,
      outcomeText: words, tooltip: verdict ? `${named(entry.actor)} — ${words}` : named(entry.actor) };
  });
  view.showLabels = arr(detail.labels).length > 0 || capabilities.labels === true;
  // Checks: a newer rollup (the list's row) that disagrees with the detail's checks makes them stale.
  const detailState = checksRollup(arr(detail.checks)) || null;
  const listNewer = input.listEntry && Date.parse(str(input.listEntry.updatedAt)) > Date.parse(str(detail.updatedAt));
  const latestState = listNewer ? (str(input.listEntry!.checksState) || null) : detailState;
  const checksState = latestState !== 'failing' && arr(detail.checks).some(check => check.status === 'action-required') ? 'pending' : latestState;
  view.checksStale = checksState !== detailState; view.checksState = checksState ?? '';
  view.checks = arr(detail.checks).map((check, index) => ({ key: `${index}:${str(check.name)}:${str(check.url)}`, name: str(check.name), status: str(check.status, 'neutral'), statusLabel: checkStatusLabel(check), url: str(check.url),
    // Only where there is something to fix: a failing check (PullRequestSummaryTab's Fix).
    fix: check.status === 'failure' || check.status === 'cancelled' ? pullRequestFindingKey({ kind: 'check', check }) : '' }));
  // The conversation, as the tab buckets it: loose comments and review threads, each thread one item.
  const items = pullRequestConversationItems(comments, arr(activity?.reviewThreads));
  const counted = (list: readonly PullRequestConversationItem[]) => list.reduce((count, item) => count + pullRequestConversationItemComments(item).length, 0);
  const commentCount = counted(items);
  view.commentsTitle = `Comments (${pullRequestCommentCount(activity)})`;
  view.truncated = activity?.commentsTruncated === true;
  view.truncatedText = `This conversation is only partially loaded. ${commentCount} comment${commentCount === 1 ? ' is' : 's are'} loaded; open it on the host to read the rest.`;
  view.empty = items.length === 0;
  const filter = input.filter ?? { bots: false, resolved: false };
  const shown = filterPullRequestConversationItems(items, filter);
  // The one count the filter shows, in comments. Per-kind counts would overlap, since a resolved bot comment
  // is both kinds and needs both shown.
  const hidden = commentCount - counted(shown);
  view.showBots = filter.bots; view.showResolved = filter.resolved;
  view.filterLabel = hidden > 0 ? `${hidden} hidden` : 'Filter';
  view.filteredOutText = shown.length === 0 && hidden > 0 ? (hidden === 1 ? 'The only comment is hidden by the filter.' : `All ${hidden} comments are hidden by the filter.`) : '';
  const remark = input.remark ?? noRemarkWrites, canReact = capabilities.reactions === true;
  // Windowed by recency regardless of display order: `fromEnd` counts back from the newest conversation shown.
  const conversations = both(shown.map((item, index) => conversation(item, url, now, shown.length - 1 - index, remark, canReact)));
  view.activeCount = shown.length; view.newest = conversations.newest; view.oldest = conversations.oldest;
  return view;
}
