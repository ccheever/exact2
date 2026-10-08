// The pull request Summary tab's model (lane "pages"; MIT reference, see LICENSE-T3, T3 Code
// 1e2ecbd975 apps/web/src/components/pullRequest/PullRequestSummaryTab.tsx): reviewers with the
// verdict each last gave (a ring, the sr-only words and the "<name> — <outcome>" tooltip), the
// checks with "Check details are out of date." when a newer rollup disagrees, and the
// conversation split as the tab splits it — finished work (a resolved thread's comment, a
// dismissed review), bot comments, and the rest, windowed 10 at a time from the newest. The
// window, the order, the open groups are the tab's own state (pages-pr-summary.contract); this
// model gives every card its place from the newest (`fromEnd`) and both orders.
import { arr, num, obj, str, type Obj } from './domain';
import { relativeLabel } from './pages-prs';
import { currentTimestampLocale } from './timestamp-format';
import { checkStatusLabel, checksRollup } from './r6-pr-logic';
import { latestPullRequestReviewOutcomes, orderPullRequestComments, pullRequestReviewOutcome, pullRequestReviewOutcomeLabel, pullRequestReviewOutcomeStaleLabel, visibleBody } from './pages-pr-logic';

export const COMMENT_PAGE = 10;
export type PrFace = { key: string; login: string; avatar: string; initial: string };
export type PrCommentCard = {
  key: string; id: string; bodyId: string; author: string; avatar: string; initial: string; authorTip: string; age: string; ageTip: string; url: string;
  outcome: string; outcomeLabel: string; stateLabel: string; location: string; outdated: boolean; fromEnd: number; finishedLabel: string; preview: string;
};
export type PrCommentGroup = { label: string; authorsText: string; filesText: string; latest: string; latestTip: string; faces: PrFace[]; more: number };
export type PrReviewerEntry = { key: string; login: string; avatar: string; initial: string; outcome: string; stale: boolean; outcomeText: string; tooltip: string };
export type PrSummaryCheck = { key: string; name: string; status: string; statusLabel: string; url: string };
export type PrSummaryView = ReturnType<typeof emptySummary>;

export function emptySummary() {
  return {
    key: '', activityPending: false, activityError: '', reviewers: [] as PrReviewerEntry[], showLabels: true,
    checksStale: false, checksState: '', checks: [] as PrSummaryCheck[], commentsTitle: 'Comments (0)', truncated: false, truncatedText: '', empty: true,
    activeCount: 0, newest: [] as PrCommentCard[], oldest: [] as PrCommentCard[],
    botCount: 0, botsNewest: [] as PrCommentCard[], botsOldest: [] as PrCommentCard[], bots: emptyGroup(),
    finishedCount: 0, finishedNewest: [] as PrCommentCard[], finishedOldest: [] as PrCommentCard[], finished: emptyGroup(),
  };
}
const emptyGroup = (): PrCommentGroup => ({ label: '', authorsText: '', filesText: '', latest: '', latestTip: '', faces: [], more: 0 });

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

/** The markdown documents the conversation renders: every remark with something visible to say. */
export function conversationBodies(activity: Obj | null): { id: string; kind: string; title: string; body: string }[] {
  return arr(activity?.comments).flatMap(comment => { const body = visibleBody(str(comment.body)); return body === null ? [] : [{ id: `pr-comment:${str(comment.id)}`, kind: 'assistant', title: '', body }]; });
}

function card(comment: Obj, threads: Map<string, Obj>, url: string, now: number, fromEnd: number): PrCommentCard {
  const thread = threads.get(str(comment.id)), person = face(comment.author);
  const outcome = pullRequestReviewOutcome(str(comment.reviewState) || null), body = visibleBody(str(comment.body));
  const path = str(thread?.path) || str(comment.path), line = num(thread?.line);
  return {
    key: `${url}:${str(comment.id)}`, id: str(comment.id), bodyId: body === null ? '' : `pr-comment:${str(comment.id)}`,
    author: person.login, avatar: person.avatar, initial: person.initial, authorTip: named(comment.author),
    age: relativeLabel(comment.createdAt, now), ageTip: `${fullDate(str(comment.createdAt))}${str(comment.url) ? ' · Open comment on host' : ''}`, url: str(comment.url),
    outcome: outcome ?? '', outcomeLabel: outcome ? pullRequestReviewOutcomeLabel(outcome) : '', stateLabel: !outcome && str(comment.reviewState) ? reviewStateLabel(str(comment.reviewState)) : '',
    location: path ? `${path}${thread && line ? `:${line}` : ''}` : '', outdated: thread?.isOutdated === true, fromEnd,
    finishedLabel: thread?.isResolved === true ? 'Resolved' : 'Review dismissed', preview: body === null ? '' : previewText(body),
  };
}
/** CommentGroup's header: the first three faces, how many wrote, across how many files, and when last. */
function group(label: string, comments: Obj[], now: number): PrCommentGroup {
  const authors = [...new Map(comments.map(comment => [str(obj(comment.author).login, 'ghost').toLowerCase(), comment.author])).values()];
  const files = new Set(comments.flatMap(comment => (str(comment.path) ? [str(comment.path)] : []))).size;
  const latest = comments.reduce<string | null>((date, comment) => (date === null || str(comment.createdAt) > date ? str(comment.createdAt) : date), null);
  return { label, authorsText: `${authors.length} ${authors.length === 1 ? 'author' : 'authors'}`, filesText: files > 0 ? `${files} ${files === 1 ? 'file' : 'files'}` : '',
    latest: latest ? relativeLabel(latest, now) : '', latestTip: latest ? fullDate(latest) : '', faces: authors.slice(0, 3).map(face), more: Math.max(0, authors.length - 3) };
}
const both = (cards: PrCommentCard[]) => ({ newest: [...orderPullRequestComments(cards, 'newest')], oldest: cards });

export type SummaryInput = { detail: Obj; activity: Obj | null; activityPending: boolean; activityError: string; now: number; listEntry: Obj | null };
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
  view.checks = arr(detail.checks).map((check, index) => ({ key: `${index}:${str(check.name)}:${str(check.url)}`, name: str(check.name), status: str(check.status, 'neutral'), statusLabel: checkStatusLabel(check), url: str(check.url) }));
  // The conversation, as the tab buckets it.
  view.commentsTitle = `Comments (${num(activity?.commentCount)})`;
  view.truncated = activity?.commentsTruncated === true;
  view.truncatedText = `This conversation is longer than this page reads in one go. The most recent ${comments.length} are here; open it on the host to read the rest.`;
  view.empty = comments.length === 0;
  const threads = new Map(arr(activity?.reviewThreads).flatMap(thread => arr(thread.comments).map(comment => [str(comment.id), thread] as const)));
  const active: Obj[] = [], finished: Obj[] = [], bots: Obj[] = [];
  for (const comment of comments) {
    const done = threads.get(str(comment.id))?.isResolved === true || pullRequestReviewOutcome(str(comment.reviewState) || null) === 'dismissed';
    (done ? finished : isBot(comment.author) ? bots : active).push(comment);
  }
  const cards = (list: Obj[]) => list.map((comment, index) => card(comment, threads, url, now, list.length - 1 - index));
  const activeCards = both(cards(active)), botCards = both(cards(bots)), finishedCards = both(cards(finished));
  view.activeCount = active.length; view.newest = activeCards.newest; view.oldest = activeCards.oldest;
  view.botCount = bots.length; view.botsNewest = botCards.newest; view.botsOldest = botCards.oldest;
  view.bots = group(`${bots.length} bot comment${bots.length === 1 ? '' : 's'}`, bots, now);
  view.finishedCount = finished.length; view.finishedNewest = finishedCards.newest; view.finishedOldest = finishedCards.oldest;
  view.finished = group(`${finished.length} resolved or dismissed comment${finished.length === 1 ? '' : 's'}`, finished, now);
  return view;
}
