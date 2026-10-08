// The pull request writes' pure rules (lane "pages"; pr-writing-and-metadata), ported with their
// names from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), apps/web/src/components/pullRequest/:
// - pullRequestEditing.logic.ts: canEditPullRequestChangeRequest, canEditPullRequestComment (sameLogin);
// - pullRequestReactions.logic.ts: PULL_REQUEST_REACTION_ORDER, pullRequestReactionEmoji/Name/Tooltip,
//   applyPendingPullRequestReactions;
// - pullRequestReviewStore.ts: the review being written (line comments and the summary), as a plain
//   store per client keyed by pullRequestReviewKey instead of a zustand hook; not persisted, as the
//   reference's is not ("a draft lives only as long as the tab does").
// The comment form's text (PullRequestCommentForm's `body`) is kept beside the summaries, so it
// outlives the popover and a visit to another pull request in the session.
import { arr, obj, str } from './domain';

// ── Editing (pullRequestEditing.logic.ts) ───────────────────────────────────

/** Hosts disagree about the case of a login and none of them treats two casings as two people. */
export function sameLogin(one: string | null | undefined, other: string | null | undefined): boolean {
  if (one == null || other == null) return false;
  return one.trim().toLowerCase() === other.trim().toLowerCase();
}
type EditingSubject = { author?: unknown; capabilities?: unknown; viewer?: unknown; viewerPermissions?: unknown };
const viewerOf = (detail: EditingSubject) => (typeof detail.viewer === 'string' ? detail.viewer : undefined);
const editOf = (detail: EditingSubject) => obj(obj(detail.capabilities).edit);
/**
 * Whether the title and description can be rewritten from here. Beyond the host being able to at
 * all, either the reader wrote the change request or they may merge it — merging is the one action
 * every host here grants with write access and withholds without it.
 */
export function canEditPullRequestChangeRequest(detail: EditingSubject): boolean {
  if (editOf(detail).changeRequest !== true) return false;
  const actions = obj(detail.viewerPermissions).actions;
  return sameLogin(viewerOf(detail), str(obj(detail.author).login) || null) || (Array.isArray(actions) && actions.includes('merge'));
}
/** Whether this remark can be rewritten from here. A review's own summary is left out. */
export function canEditPullRequestComment(detail: EditingSubject, comment: { author?: unknown; kind?: unknown }): boolean {
  if (editOf(detail).comment !== true) return false;
  if (comment.kind !== 'issue-comment' && comment.kind !== 'review-comment') return false;
  const author = comment.author === null || comment.author === undefined ? null : str(obj(comment.author).login) || null;
  return sameLogin(viewerOf(detail), author);
}

// ── Reactions (pullRequestReactions.logic.ts) ───────────────────────────────

export type PullRequestReactionContent = 'thumbs-up' | 'thumbs-down' | 'laugh' | 'hooray' | 'confused' | 'heart' | 'rocket' | 'eyes';
export type PullRequestReaction = { content: PullRequestReactionContent; count: number; actors: string[]; viewerHasReacted: boolean };
/** The picker's order, which is GitHub's: the two verdicts first, then the rest as it lists them. */
export const PULL_REQUEST_REACTION_ORDER: readonly PullRequestReactionContent[] = ['thumbs-up', 'thumbs-down', 'laugh', 'hooray', 'confused', 'heart', 'rocket', 'eyes'];
const REACTION_EMOJI: Record<PullRequestReactionContent, string> = { 'thumbs-up': '👍', 'thumbs-down': '👎', laugh: '😄', hooray: '🎉', confused: '😕', heart: '❤️', rocket: '🚀', eyes: '👀' };
/** The spoken names GitHub uses in its own hover text, which is what a screen reader reads out. */
const REACTION_NAME: Record<PullRequestReactionContent, string> = { 'thumbs-up': 'thumbs up', 'thumbs-down': 'thumbs down', laugh: 'laugh', hooray: 'hooray', confused: 'confused', heart: 'heart', rocket: 'rocket', eyes: 'eyes' };
export const isReactionContent = (value: unknown): value is PullRequestReactionContent => typeof value === 'string' && (PULL_REQUEST_REACTION_ORDER as readonly string[]).includes(value);
export function pullRequestReactionEmoji(content: PullRequestReactionContent): string { return REACTION_EMOJI[content]; }
export function pullRequestReactionName(content: PullRequestReactionContent): string { return REACTION_NAME[content]; }

/** Past three names the sentence stops being readable and starts being a list. */
const NAMED_ACTOR_LIMIT = 3;
function joinNames(parts: readonly string[]): string {
  if (parts.length <= 1) return parts[0] ?? '';
  if (parts.length === 2) return `${parts[0]} and ${parts[1]}`;
  return `${parts.slice(0, -1).join(', ')}, and ${parts.at(-1)}`;
}
/** "others" only alongside somebody named; on its own a count is people, not other people. */
function countRemainder(count: number, named: boolean): string {
  if (named) return `${count} ${count === 1 ? 'other' : 'others'}`;
  return `${count} ${count === 1 ? 'person' : 'people'}`;
}
/**
 * Who reacted, in GitHub's sentence. The viewer reads as "You" and comes first where the host left
 * them room (`actors` shorter than `count`); a host that already names everyone it counts is named
 * as given. Never names more people than `count` claims; the rest are counted.
 */
export function pullRequestReactionTooltip(reaction: PullRequestReaction): string {
  const viewerHasRoom = reaction.actors.length < reaction.count;
  const names = reaction.viewerHasReacted && viewerHasRoom ? ['You', ...reaction.actors] : [...reaction.actors];
  const shown = names.slice(0, Math.min(NAMED_ACTOR_LIMIT, reaction.count));
  const others = Math.max(0, reaction.count - shown.length);
  const parts = [...shown, ...(others > 0 ? [countRemainder(others, shown.length > 0)] : [])];
  return `${joinNames(parts)} reacted with ${pullRequestReactionName(reaction.content)} emoji`;
}
/** The list as it should be drawn while a reaction is still in flight. */
export function applyPendingPullRequestReactions(reactions: readonly PullRequestReaction[], pending: ReadonlyMap<PullRequestReactionContent, boolean>): readonly PullRequestReaction[] {
  if (pending.size === 0) return reactions;
  const byContent = new Map(reactions.map(reaction => [reaction.content, reaction] as const));
  for (const [content, reacted] of pending) {
    const current = byContent.get(content);
    if (current === undefined) {
      if (reacted) byContent.set(content, { content, count: 1, actors: [], viewerHasReacted: true });
      continue;
    }
    if (current.viewerHasReacted === reacted) continue;
    const count = current.count + (reacted ? 1 : -1);
    if (count <= 0) byContent.delete(content);
    else byContent.set(content, { ...current, count, viewerHasReacted: reacted });
  }
  return PULL_REQUEST_REACTION_ORDER.flatMap(content => byContent.get(content) ?? []);
}
/** The host's reactions on a remark, as the contract reads them (unknown contents dropped). */
export function readReactions(value: unknown): PullRequestReaction[] {
  return arr(value).flatMap(entry => (isReactionContent(entry.content) && typeof entry.count === 'number' && entry.count > 0
    ? [{ content: entry.content, count: entry.count, actors: (Array.isArray(entry.actors) ? entry.actors : []).filter((name): name is string => typeof name === 'string'), viewerHasReacted: entry.viewerHasReacted === true }]
    : []));
}
/** What the host last said, so a press in flight is forgotten the moment the real counts land. */
export function reactionsSignature(reactions: readonly PullRequestReaction[]): string {
  return reactions.map(reaction => `${reaction.content}:${reaction.count}:${reaction.viewerHasReacted ? 1 : 0}`).join(' ');
}

// ── The review being written (pullRequestReviewStore.ts) ────────────────────

export type PullRequestReviewPosition = { kind: 'added'; newLine: number } | { kind: 'deleted'; oldLine: number } | { kind: 'context'; oldLine: number; newLine: number; side: string };
export type PendingReviewComment = { id: string; path: string; oldPath?: string; position: PullRequestReviewPosition; body: string };
/** A counter rather than anything derived from the comment: two remarks on one line can be the same. */
let pendingCommentSequence = 0;
export function nextPendingReviewCommentId(): string {
  pendingCommentSequence += 1;
  return `pending-review-comment-${pendingCommentSequence}`;
}
/** A project's thread can review the same repository path and number on different hosts. */
export function pullRequestReviewKey(reference: { projectId: string; host?: string | undefined; repository: string; number: number }): string {
  return JSON.stringify([reference.projectId, reference.host?.toLowerCase() ?? null, reference.repository.toLowerCase(), reference.number]);
}
const EMPTY: readonly PendingReviewComment[] = [];
/** usePullRequestReviewStore's state and actions, one per client. */
export class PullRequestReviewStore {
  drafts: Readonly<Record<string, readonly PendingReviewComment[]>> = {};
  summaries: Readonly<Record<string, string>> = {};
  addComment(key: string, comment: PendingReviewComment): void { this.drafts = { ...this.drafts, [key]: [...(this.drafts[key] ?? EMPTY), comment] }; }
  removeComment(key: string, commentId: string): void { this.keep(key, (this.drafts[key] ?? EMPTY).filter(entry => entry.id !== commentId)); }
  removeComments(key: string, commentIds: readonly string[]): void {
    const submitted = new Set(commentIds);
    this.keep(key, (this.drafts[key] ?? EMPTY).filter(entry => !submitted.has(entry.id)));
  }
  clear(key: string): void { const { [key]: _removed, ...rest } = this.drafts; this.drafts = rest; }
  setSummary(key: string, body: string): void { this.summaries = { ...this.summaries, [key]: body }; }
  /** Only the exact draft the host accepted is removed; a summary revised meanwhile is new work. */
  clearSummary(key: string, submittedBody: string): void {
    if (this.summaries[key] !== submittedBody) return;
    const { [key]: _removed, ...rest } = this.summaries; this.summaries = rest;
  }
  /** The comments a pull request's draft holds (usePendingReviewComments). */
  pending(key: string): readonly PendingReviewComment[] { return this.drafts[key] ?? EMPTY; }
  private keep(key: string, remaining: readonly PendingReviewComment[]): void {
    if (remaining.length > 0) { this.drafts = { ...this.drafts, [key]: remaining }; return; }
    const { [key]: _removed, ...rest } = this.drafts; this.drafts = rest;
  }
}
const stores = new WeakMap<object, PullRequestReviewStore>();
/** The client's review store (the Code tab's line comments fill it; 20261005-pr-code-tab). */
export function pullRequestReviewStore(client: object): PullRequestReviewStore {
  let store = stores.get(client);
  if (!store) { store = new PullRequestReviewStore(); stores.set(client, store); }
  return store;
}
