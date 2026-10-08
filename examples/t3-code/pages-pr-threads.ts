// The Code tab's review conversations (20261005-pr-code-tab): PullRequestReviewAnnotation.tsx's
// ReviewThreadCard and PullRequestCodeTab.tsx's renderThreadCard/runThreadCommand (T3 Code
// 1e2ecbd975, MIT, see LICENSE-T3) — the card's header ("Open · N comments", "outdated", Fix in a
// thread, Resolve/Unresolve), its comments with their reactions and the reader's own pencil, "Load
// more comments" by cursor, and Reply. A thread is the same card on its line or in the list of
// conversations off the diff (pages-pr-threads.contract draws it).
//
// What outlives a view, per change request (kept in the Code tab's state, pages-pr-code.ts): the
// pages loaded past the detail's comments, which page is loading, the write in flight (every
// thread command waits for the one out: the reference's `threadPending`), and the serials that tell
// a card its reply or its edit landed, so it clears the box or closes the editor. A failure keeps
// the typed words and toasts the reference's title.
import type { T3Client } from './client';
import { arr, num, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { relativeLabel } from './pages-prs';
import { fullDate } from './pages-pr-summary';
import { visibleBody } from './pages-pr-logic';
import { canEditPullRequestComment } from './pages-pr-writes-logic';
import { reactionPills, type WriteReference } from './pages-pr-writes';
import { pullRequestFindingKey } from './pages-pr-handoffs-logic';
import { editPullRequestThreadComment, mergePullRequestThreadComments } from './pages-pr-code-logic';
import type { PrReactionPill } from './pages-pr-summary';

export type ThreadState = {
  /** One write at a time across every card (reply, resolve, edit). */
  pending: boolean;
  /** Pages read past the detail's comments, by thread. */
  pages: Map<string, { comments: Obj[]; nextCursor: string | null }>;
  loading: Set<string>;
  replied: Map<string, number>;
  saving: string;
  saved: Map<string, number>;
};
export const emptyThreadState = (): ThreadState => ({ pending: false, pages: new Map(), loading: new Set(), replied: new Map(), saving: '', saved: new Map() });

export type PrThreadComment = {
  id: string; author: string; avatar: string; initial: string; authorTip: string; age: string; ageTip: string; bodyId: string;
  raw: string; canEdit: boolean; saving: boolean; savedSerial: number; reactions: PrReactionPill[]; reacted: string[];
};
export type PrThreadCard = {
  id: string; key: string; resolved: boolean; outdated: boolean; countLabel: string; canReply: boolean; canResolve: boolean; canReact: boolean;
  fix: string; comments: PrThreadComment[]; more: boolean; loadingMore: boolean; pending: boolean; repliedSerial: number;
  /** "Line N" for the list of conversations off the diff; '' when the host anchored it to the file. */
  lineLabel: string; path: string;
};

const named = (actor: Obj) => { const login = str(actor.login, 'ghost'), name = str(actor.name); return name && name !== login ? `${name} (@${login})` : login; };
/** A thread's comments as shown: the detail's, then each loaded page's comment once. */
export function threadComments(state: ThreadState, thread: Obj): Obj[] {
  const page = state.pages.get(str(thread.id));
  return mergePullRequestThreadComments(arr(thread.comments) as (Obj & { id: string })[], (page?.comments ?? []) as (Obj & { id: string })[]);
}
/** The markdown documents the cards render (`pr-thread-comment:<id>`), each comment once. */
export function threadBodies(state: ThreadState, threads: Obj[]): { id: string; kind: string; title: string; body: string }[] {
  const seen = new Set<string>(), out: { id: string; kind: string; title: string; body: string }[] = [];
  for (const thread of threads) for (const comment of threadComments(state, thread)) {
    const id = str(comment.id), body = visibleBody(str(comment.body));
    if (!id || seen.has(id) || body === null) continue;
    seen.add(id); out.push({ id: `pr-thread-comment:${id}`, kind: 'assistant', title: '', body });
  }
  return out;
}

/** Every thread as its card: what this host lets this reader do with it, its comments and their reactions. */
export function presentThreads(client: object, state: ThreadState, reference: WriteReference, detail: Obj, threads: Obj[], now: number): PrThreadCard[] {
  const capabilities = obj(detail.capabilities), review = obj(capabilities.review), viewer = obj(detail.viewerPermissions);
  const canReply = review.reply === true && viewer.comment === true, canResolve = review.resolve === true && viewer.resolve === true, canReact = capabilities.reactions === true;
  return threads.map(thread => {
    const id = str(thread.id), page = state.pages.get(id), comments = threadComments(state, thread);
    const nextCursor = page ? page.nextCursor : (str(thread.nextCommentsCursor) || null);
    const count = num(thread.commentCount, comments.length);
    return {
      // Named with the pull request too: two pull requests can hand out the same thread id.
      id, key: `${reference.projectId}#${reference.number}:${id}`, resolved: thread.isResolved === true, outdated: thread.isOutdated === true,
      countLabel: `${thread.isResolved === true ? 'Resolved' : 'Open'} · ${count} ${count === 1 ? 'comment' : 'comments'}`, canReply, canResolve, canReact,
      fix: pullRequestFindingKey({ kind: 'thread', thread }), more: nextCursor !== null, loadingMore: state.loading.has(id), pending: state.pending, repliedSerial: state.replied.get(id) ?? 0,
      lineLabel: num(thread.line) > 0 ? `Line ${num(thread.line)}` : '', path: str(thread.path),
      comments: comments.map(comment => {
        const actor = obj(comment.author), login = str(actor.login, 'ghost'), commentId = str(comment.id), body = visibleBody(str(comment.body));
        const pills = reactionPills(client, reference, commentId, comment.reactions);
        return {
          id: commentId, author: login, avatar: str(actor.avatarUrl), initial: login.slice(0, 1).toUpperCase(), authorTip: named(actor),
          age: relativeLabel(comment.createdAt, now), ageTip: fullDate(str(comment.createdAt)), bodyId: body === null ? '' : `pr-thread-comment:${commentId}`, raw: str(comment.body),
          // A conversation on a line is made of review comments, whatever the host filed them as.
          canEdit: canEditPullRequestComment(detail, { author: comment.author, kind: 'review-comment' }), saving: state.saving === commentId, savedSerial: state.saved.get(commentId) ?? 0,
          reactions: pills.pills, reacted: pills.reacted,
        };
      }),
    };
  });
}

export type ThreadContext = { client: T3Client; native: Native; state: ThreadState; reference: WriteReference; threads: () => Obj[]; refresh: () => void };
const wake = async (native: Native) => { try { await native.later({ op: 'r10Wake', topic: 't3.pr' }); } catch (error) { if (letGo(error)) throw error; } };
const split = (value: string, count: number) => { const parts: string[] = []; let rest = value; for (let i = 1; i < count; i++) { const bar = rest.indexOf('|'); if (bar < 0) break; parts.push(rest.slice(0, bar)); rest = rest.slice(bar + 1); } parts.push(rest); return parts; };

/** runThreadCommand: one write at a time; a failure toasts its label; success re-reads the detail. */
async function runThreadCommand(ctx: ThreadContext, label: string, run: () => Promise<unknown>): Promise<boolean> {
  if (ctx.state.pending) return false;
  ctx.state.pending = true;
  await wake(ctx.native);
  try { await run(); }
  catch (error) {
    if (letGo(error)) throw error;
    pushToast(ctx.client, { kind: 'error', title: label });
    return false;
  } finally { ctx.state.pending = false; }
  ctx.refresh();
  return true;
}

/** `pageslocal:pr-act-thread-*`: reply, resolve, edit, more. Answers '' (a failure has toasted). */
export async function threadCommand(ctx: ThreadContext, op: string, value: string): Promise<string> {
  const { client, native, state, reference } = ctx;
  if (op === 'reply') {
    const [threadId = '', body = ''] = split(value, 2), text = body.trim();
    if (!threadId || text.length === 0) return '';
    const thread = ctx.threads().find(entry => str(entry.id) === threadId);
    if (await runThreadCommand(ctx, 'Reply could not be posted', () => client.rpc(native, 'pullRequests.replyToThread', { ...reference, threadId, body: text }, true))) {
      // The mutation returns no comment: keep what was loaded and reopen its cursor, so the new reply stays reachable.
      const page = state.pages.get(threadId);
      if (page) state.pages.set(threadId, { ...page, nextCursor: page.nextCursor ?? (str(thread?.nextCommentsCursor) || null) });
      state.replied.set(threadId, (state.replied.get(threadId) ?? 0) + 1);
    }
    return '';
  }
  if (op === 'resolve') {
    const [threadId = '', resolved = ''] = split(value, 2);
    if (threadId) await runThreadCommand(ctx, 'The conversation could not be updated', () => client.rpc(native, 'pullRequests.setThreadResolution', { ...reference, threadId, resolved: resolved === 'true' }, true));
    return '';
  }
  if (op === 'edit') {
    const [threadId = '', commentId = '', body = ''] = split(value, 3);
    if (!commentId || body.trim().length === 0 || state.saving) return '';
    state.saving = commentId;
    try {
      if (await runThreadCommand(ctx, 'The comment could not be saved', () => client.rpc(native, 'pullRequests.updateComment', { ...reference, commentId, kind: 'review-comment', body }, true))) {
        const page = state.pages.get(threadId);
        if (page) state.pages.set(threadId, { ...page, comments: editPullRequestThreadComment(page.comments as (Obj & { id: string; body: string })[], commentId, body) });
        state.saved.set(commentId, (state.saved.get(commentId) ?? 0) + 1);
      }
    } finally { state.saving = ''; }
    return '';
  }
  if (op === 'more') {
    // Only after the reader asks: the page after the last one read, or after the detail's comments.
    const threadId = value, thread = ctx.threads().find(entry => str(entry.id) === threadId), page = state.pages.get(threadId);
    const cursor = page ? page.nextCursor : (str(thread?.nextCommentsCursor) || null);
    if (!threadId || !cursor || state.loading.has(threadId)) return '';
    state.loading.add(threadId);
    await wake(native);
    try {
      const page = obj(await client.rpc(native, 'pullRequests.threadComments', { ...reference, threadId, cursor }));
      const previous = state.pages.get(threadId);
      state.pages.set(threadId, { comments: mergePullRequestThreadComments((previous?.comments ?? []) as (Obj & { id: string })[], arr(page.comments) as (Obj & { id: string })[]), nextCursor: str(page.nextCursor) || null });
    } catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: 'More comments could not be loaded' });
    } finally { state.loading.delete(threadId); }
    return '';
  }
  return '';
}
