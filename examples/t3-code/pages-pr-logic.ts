// The pull request panel's pure rules (lane "pages"), ported with their names from T3 Code
// 1e2ecbd975 (MIT, see LICENSE-T3):
// - apps/web/src/components/pullRequest/pullRequestDetail.logic.ts: shouldRefreshPullRequestActivity,
//   mergePullRequestThreadComments, editPullRequestThreadComment, orderPullRequestComments,
//   pullRequestReviewOutcome, newestPullRequestCommitAt, isPullRequestVerdictStale,
//   latestPullRequestReviewOutcomes, groupPullRequestTimelineConversations, visibleBody,
//   buildPullRequestTimeline, readPullRequestDetailSnapshot, writePullRequestDetailSnapshot,
//   resolveDisplayedPullRequestDetail, resolvePullRequestReferenceHost;
// - pullRequestPresentation.tsx: the review outcome labels (pullRequestReviewOutcomeLabel,
//   pullRequestReviewOutcomeStaleLabel); pullRequestCheckStatusLabel is r6-pr-logic.ts checkStatusLabel;
// - apps/web/src/hooks/useLiveRefresh.ts: the three constants and shouldLiveRefresh,
//   shouldRefreshOnArrival, shouldRefreshOnInterval (the hook's listeners are the panel's
//   arguments here: window visibility and focus, the window clock, and the last interaction).
// The snapshot store is the app's versioned preference file (t3-code.json) instead of
// localStorage: `SnapshotStorage` is the same getItem/setItem pair over a record in it.
import { arr, num, obj, str, type Obj } from './domain';

// ── Live refresh (useLiveRefresh.ts) ────────────────────────────────────────

/** Long enough that alt-tabbing through windows does not become a request per tab stop. */
export const LIVE_REFRESH_MIN_INTERVAL_MS = 10_000;
/** Slow enough to preserve host quota while still updating a view left open. */
export const LIVE_REFRESH_INTERVAL_MS = 5 * 60_000;
/** How long a showing window goes untouched before it stops reading. */
export const LIVE_REFRESH_IDLE_AFTER_MS = 6 * 60_000;

/** Only while the window is actually showing, and not again straight away. */
export function shouldLiveRefresh(input: { visible: boolean; now: number; lastRefreshedAt: number }): boolean {
  return input.visible && input.now - input.lastRefreshedAt >= LIVE_REFRESH_MIN_INTERVAL_MS;
}
/** A view nobody has read this session is left alone: its own first read is already on its way. */
export function shouldRefreshOnArrival(input: { visible: boolean; now: number; lastRefreshedAt: number | undefined }): boolean {
  return input.lastRefreshedAt !== undefined && shouldLiveRefresh({ ...input, lastRefreshedAt: input.lastRefreshedAt });
}
/** The same rule as any other read, plus the reader having been here recently enough. */
export function shouldRefreshOnInterval(input: { visible: boolean; now: number; lastRefreshedAt: number; lastInteractedAt: number }): boolean {
  return input.now - input.lastInteractedAt < LIVE_REFRESH_IDLE_AFTER_MS && shouldLiveRefresh(input);
}

// ── Activity (pullRequestDetail.logic.ts) ───────────────────────────────────

/** Activity changes only when the same host resource reports a newer revision. */
export function shouldRefreshPullRequestActivity(previous: { key: string; updatedAt: string } | null, next: { key: string; updatedAt: string }): boolean {
  return previous !== null && previous.key === next.key && previous.updatedAt !== next.updatedAt;
}
/** Appends fetched pages without replacing fresher comments already in the activity response. */
export function mergePullRequestThreadComments<T extends { readonly id: string }>(base: readonly T[], loaded: readonly T[]): readonly T[] {
  const seen = new Set(base.map(comment => comment.id));
  return [...base, ...loaded.filter(comment => { if (seen.has(comment.id)) return false; seen.add(comment.id); return true; })];
}
export function editPullRequestThreadComment<T extends { readonly id: string; readonly body: string }>(comments: readonly T[], commentId: string, body: string): readonly T[] {
  return comments.map(comment => (comment.id === commentId ? { ...comment, body } : comment));
}
/** Chronological ascending, oldest to newest — reversed for the "newest" reading order. */
export function orderPullRequestComments<T>(comments: readonly T[], order: 'newest' | 'oldest'): readonly T[] {
  return order === 'newest' ? [...comments].reverse() : comments;
}

export type PullRequestReviewOutcome = 'approved' | 'changes-requested' | 'dismissed';
/** GitHub's `CHANGES_REQUESTED` and Bitbucket's `changes_requested` are one verdict; `COMMENTED` is none. */
export function pullRequestReviewOutcome(reviewState: string | null | undefined): PullRequestReviewOutcome | null {
  switch (reviewState?.trim().toLowerCase().split('_').join('-')) {
    case 'approved': return 'approved';
    case 'changes-requested': return 'changes-requested';
    case 'dismissed': return 'dismissed';
    default: return null;
  }
}
/** An instant as a number, because the text is not the order (a UTC offset inverts it). */
const instant = (iso: string) => Date.parse(iso);
/** The newest commit on the branch, which is what a verdict is current against. */
export function newestPullRequestCommitAt(commits: readonly Obj[]): string | null {
  let newest: string | null = null, newestAt = Number.NEGATIVE_INFINITY;
  for (const commit of commits) {
    const at = instant(str(commit.committedDate));
    if (Number.isNaN(at) || at <= newestAt) continue;
    newest = str(commit.committedDate); newestAt = at;
  }
  return newest;
}
/** Whether a verdict was given before the code it was given on (commit dates are the proxy). */
export function isPullRequestVerdictStale(at: string, newestCommitAt: string | null): boolean {
  if (newestCommitAt === null) return false;
  const verdictAt = instant(at), commitAt = instant(newestCommitAt);
  return !Number.isNaN(verdictAt) && !Number.isNaN(commitAt) && verdictAt < commitAt;
}
export type PullRequestReviewOutcomeEntry = { key: string; actor: Obj | null; outcome: PullRequestReviewOutcome; at: string; stale: boolean };
/** One entry per person and only their last word; a dismissal leaves nothing to show. */
export function latestPullRequestReviewOutcomes(comments: readonly Obj[], commits: readonly Obj[] = []): PullRequestReviewOutcomeEntry[] {
  const newestCommitAt = newestPullRequestCommitAt(commits);
  const latest = new Map<string, PullRequestReviewOutcomeEntry>();
  for (const comment of comments) {
    const outcome = pullRequestReviewOutcome(str(comment.reviewState) || null);
    if (outcome === null) continue;
    const author = comment.author && typeof comment.author === 'object' ? obj(comment.author) : null;
    // Two deleted accounts are two reviewers: a review with no author identity stands alone.
    const login = author && str(author.login) ? str(author.login) : `ghost:${str(comment.id)}`;
    const current = latest.get(login);
    if (current !== undefined && instant(current.at) > instant(str(comment.createdAt))) continue;
    latest.set(login, { key: login, actor: author, outcome, at: str(comment.createdAt), stale: isPullRequestVerdictStale(str(comment.createdAt), newestCommitAt) });
  }
  return [...latest.values()].filter(entry => entry.outcome !== 'dismissed');
}

export type PullRequestTimelineEvent = {
  id: string; at: string; kind: 'opened' | 'commit' | 'comment' | 'review' | 'merged' | 'closed'; title: string; body: string | null;
  markdown: boolean; url: string | null; actor: Obj | null; commitAuthors: Obj[]; additions: number | null; deletions: number | null;
  path: string | null; reviewState: string | null; reactions: Obj[];
};
export type PullRequestTimelineRow = { kind: 'event'; event: PullRequestTimelineEvent } | { kind: 'comments'; events: PullRequestTimelineEvent[] };
/** Consecutive comments are one conversation section; commits, lifecycle and verdicts stand alone. */
export function groupPullRequestTimelineConversations(events: readonly PullRequestTimelineEvent[]): PullRequestTimelineRow[] {
  const rows: PullRequestTimelineRow[] = [];
  for (const event of events) {
    if ((event.kind === 'comment' || event.kind === 'review') && pullRequestReviewOutcome(event.reviewState) === null) {
      const last = rows.at(-1);
      if (last?.kind === 'comments') rows[rows.length - 1] = { kind: 'comments', events: [...last.events, event] };
      else rows.push({ kind: 'comments', events: [event] });
    } else rows.push({ kind: 'event', event });
  }
  return rows;
}
/** A body that is nothing but an HTML marker renders empty, so it is no body at all. */
export function visibleBody(body: string): string | null {
  return body.replace(/<!--[\s\S]*?-->/gu, '').trim().length === 0 ? null : body.trim();
}
const actorOf = (value: unknown): Obj | null => (value && typeof value === 'object' && !Array.isArray(value) && str(obj(value).login) ? obj(value) : null);
const lifecycle = (id: string, at: string, kind: 'opened' | 'merged' | 'closed', title: string, actor: Obj | null): PullRequestTimelineEvent =>
  ({ id, at, kind, title, body: null, markdown: false, url: null, actor, commitAuthors: [], additions: null, deletions: null, path: null, reviewState: null, reactions: [] });
/** Creation, commits, comments and the terminal event in one list, newest first; merged wins over closed. */
export function buildPullRequestTimeline(detail: { createdAt?: unknown; author?: unknown; commits?: unknown; comments?: unknown; mergedAt?: unknown; closedAt?: unknown }): PullRequestTimelineEvent[] {
  const mergedAt = str(detail.mergedAt), closedAt = str(detail.closedAt);
  return [
    lifecycle('created', str(detail.createdAt), 'opened', 'opened this pull request', actorOf(detail.author)),
    ...arr(detail.commits).map((commit): PullRequestTimelineEvent => ({
      id: str(commit.oid), at: str(commit.committedDate), kind: 'commit', title: `Commit ${str(commit.oid).slice(0, 7)}`, body: str(commit.messageHeadline) || null,
      markdown: false, url: null, actor: actorOf(arr(commit.authors)[0]), commitAuthors: arr(commit.authors),
      additions: typeof commit.additions === 'number' ? commit.additions : null, deletions: typeof commit.deletions === 'number' ? commit.deletions : null,
      path: null, reviewState: null, reactions: [] })),
    ...arr(detail.comments).map((comment): PullRequestTimelineEvent => ({
      id: str(comment.id), at: str(comment.createdAt), kind: comment.kind === 'review' ? 'review' : 'comment', title: comment.kind === 'review' ? 'reviewed' : 'commented',
      body: visibleBody(str(comment.body)), markdown: true, url: str(comment.url) || null, actor: actorOf(comment.author), commitAuthors: [],
      additions: null, deletions: null, path: str(comment.path) || null, reviewState: str(comment.reviewState) || null, reactions: arr(comment.reactions) })),
    ...(mergedAt ? [lifecycle('merged', mergedAt, 'merged', 'Pull request merged', null)] : []),
    ...(closedAt && !mergedAt ? [lifecycle('closed', closedAt, 'closed', 'Pull request closed', null)] : []),
  ].sort((left, right) => right.at.localeCompare(left.at));
}

// ── Presentation (pullRequestPresentation.tsx) ──────────────────────────────

const OUTCOME_LABELS: Record<PullRequestReviewOutcome, string> = { approved: 'Approved', 'changes-requested': 'Changes requested', dismissed: 'Review dismissed' };
export const pullRequestReviewOutcomeLabel = (outcome: PullRequestReviewOutcome): string => OUTCOME_LABELS[outcome];
/** The same word with when it applied added: commits landed after it. */
export const pullRequestReviewOutcomeStaleLabel = (outcome: PullRequestReviewOutcome): string => `${OUTCOME_LABELS[outcome]} earlier changes`;

// ── Cached detail (readPullRequestDetailSnapshot and friends) ───────────────

export type SnapshotStorage = { getItem(key: string): string | null; setItem(key: string, value: string): void };
export type PullRequestDetailSnapshotRef = { host?: string | undefined; projectId: string; repository: string; number: number };
const snapshotKey = (environmentId: string, reference: PullRequestDetailSnapshotRef) => reference.host
  ? `t3.pullRequests.detail:${JSON.stringify([environmentId, reference.projectId, reference.host.toLowerCase(), reference.repository.toLowerCase(), reference.number])}`
  : `t3.pullRequests.detail:${environmentId}:${reference.projectId}:${reference.repository}#${reference.number}`;
/** PullRequestDetail's schema, as far as a painted tab relies on it. */
function decodeDetailSnapshot(value: unknown): Obj | null {
  const detail = obj(value);
  return str(detail.projectId) && str(detail.repository) && num(detail.number) > 0 && typeof detail.title === 'string' && typeof detail.url === 'string'
    && typeof detail.state === 'string' && typeof detail.headBranch === 'string' && typeof detail.baseBranch === 'string' ? detail : null;
}
/** The last detail answered for this change request, brought back across a relaunch. */
export function readPullRequestDetailSnapshot(storage: SnapshotStorage | undefined, environmentId: string, reference: PullRequestDetailSnapshotRef): Obj | null {
  try {
    const raw = storage?.getItem(snapshotKey(environmentId, reference))
      ?? (reference.host === undefined ? null : storage?.getItem(snapshotKey(environmentId, { ...reference, host: undefined })));
    if (!raw) return null;
    const decoded = decodeDetailSnapshot(JSON.parse(raw));
    return decoded ? resolveDisplayedPullRequestDetail({ live: null, cached: decoded, reference }) : null;
  } catch { return null; }
}
export function writePullRequestDetailSnapshot(storage: SnapshotStorage | undefined, environmentId: string, reference: PullRequestDetailSnapshotRef, detail: Obj): void {
  try { storage?.setItem(snapshotKey(environmentId, reference), JSON.stringify(detail)); } catch { /* the next open waits on the live read */ }
}
/** Live host state wins; a snapshot is only the same change request, never a neighbour's. */
export function resolveDisplayedPullRequestDetail(input: { live: Obj | null; cached: Obj | null; reference: PullRequestDetailSnapshotRef }): Obj | null {
  if (input.live !== null) return input.live;
  const cached = input.cached;
  if (cached === null || cached.projectId !== input.reference.projectId || str(cached.repository).toLowerCase() !== input.reference.repository.toLowerCase() || cached.number !== input.reference.number) return null;
  if (input.reference.host === undefined) return cached;
  try {
    const url = new URL(str(cached.url));
    const host = cached.provider === 'forgejo' ? url.host : url.hostname;
    return (url.protocol === 'https:' || url.protocol === 'http:') && host.toLowerCase() === input.reference.host.toLowerCase() ? cached : null;
  } catch { return null; }
}
/** A GitHub reference without a host takes the identity's: a thread link and the list name the same snapshot. */
export function resolvePullRequestReferenceHost<R extends PullRequestDetailSnapshotRef>(reference: R, identity: Obj | null | undefined): R {
  if (reference.host !== undefined || identity?.provider !== 'github') return reference;
  const host = gitHubHostOf(identity);
  return host ? { ...reference, host } : reference;
}
/** pullRequestHostOf for GitHub: the remote's web authority, else the canonical key's host. */
function gitHubHostOf(identity: Obj): string {
  try { const remote = new URL(str(obj(identity.locator).remoteUrl).trim()); if (remote.protocol === 'http:' || remote.protocol === 'https:') return remote.hostname.toLowerCase(); } catch { /* SCP-style remote */ }
  return (str(identity.canonicalKey).split('/')[0] ?? '').toLowerCase();
}
