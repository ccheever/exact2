// The pull request panel's writes (lane "pages"; pr-writing-and-metadata): the floating composer
// (a comment, Close/Reopen with comment, a review with its verdict and the line comments the Code
// tab collects), the title, description and comment editors, reactions on remarks, and the
// reviewer and label pickers. Measured against T3 Code 1e2ecbd975 (MIT, see LICENSE-T3)
// apps/web/src/components/pullRequest/{PullRequestComposer,PullRequestCommentForm,
// PullRequestReviewForm,PullRequestMarkdownEditor,PullRequestReactions,PullRequestReviewerPicker,
// PullRequestLabelPicker}.tsx, PullRequestDetailPanel.tsx (performCommentAction, saveTitle),
// PullRequestSummaryTab.tsx (saveBody, commentEditing) and the client's cache patches
// (packages/client-runtime/src/state/pullRequests.ts requestReviewers/setLabels/updateComment).
//
// State that outlives a view: the drafts (pages-pr-writes-logic.ts PullRequestReviewStore and the
// comment form's text), what is in flight, the optimistic reactions and the candidate lists, per
// client and keyed by pullRequestReviewKey. A write marks itself in flight and wakes the panel's
// resource (`t3.pr`), so "Posting..." is drawn while the host answers; its outcome lands with the
// command's reply. Each `…Serial` counts a write that landed, which is how a view built from the
// last answer knows to clear its text or close its editor (pages-pr-compose.contract and
// pages-pr-edit.contract compare the serial they saw with the one that arrives).
import { arr, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { readableFailure } from './r6-pr-logic';
import type { T3Client } from './client';
import type { PrReactionPill } from './pages-pr-summary';
import { actionState, finishAction, type PanelContext } from './pages-pr-actions';
import {
  PULL_REQUEST_REACTION_ORDER, applyPendingPullRequestReactions, canEditPullRequestChangeRequest, canEditPullRequestComment, isReactionContent, pullRequestReactionEmoji,
  pullRequestReactionName, pullRequestReactionTooltip, pullRequestReviewKey, pullRequestReviewStore, reactionsSignature, readReactions, type PullRequestReactionContent,
} from './pages-pr-writes-logic';

export type WriteReference = { projectId: string; host?: string; repository: string; number: number };
type Overlay = { signature: string; values: Map<PullRequestReactionContent, boolean> };
type Candidates = { at: number; loading: boolean; error: string; list: Obj[]; truncated: boolean };
/** The panel's reads that a write patches in place (pages-pr-detail.ts hands them over). */
export type PanelCache = { detail: Obj | null; activity: Obj | null };
type State = {
  comments: Map<string, string>; sent: Map<string, string>; submitting: Map<string, string>; reviewing: Set<string>;
  serials: Map<string, number>; saving: Map<string, string>; previews: Map<string, string>;
  overlays: Map<string, Overlay>; candidates: Map<string, Candidates>; picking: Map<string, string>;
};
const states = new WeakMap<object, State>();
function stateOf(client: object): State {
  let state = states.get(client);
  if (!state) {
    state = { comments: new Map(), sent: new Map(), submitting: new Map(), reviewing: new Set(), serials: new Map(), saving: new Map(), previews: new Map(), overlays: new Map(), candidates: new Map(), picking: new Map() };
    states.set(client, state);
  }
  return state;
}
const serial = (state: State, name: string) => state.serials.get(name) ?? 0;
const bump = (state: State, name: string) => state.serials.set(name, serial(state, name) + 1);
/** Who reads the candidate lists fresh: kept for a minute, as the reference's queries are. */
export const CANDIDATES_FRESH_MS = 60_000;
/** The words a review ends with (PullRequestReviewForm VERDICTS). */
export const VERDICT_SENT: Record<string, string> = { comment: 'Review submitted', approve: 'Pull request approved', 'request-changes': 'Changes requested' };
const VERDICT_ORDER = ['comment', 'approve', 'request-changes'];
const REVIEWER_HINT = 'The host refused it. Check that you have write access on this repository, and that they still have access to it.';
const LABEL_HINT = 'The host refused it. Check that you have triage access on this repository.';

const names = (value: unknown) => (Array.isArray(value) ? value : []).filter((name): name is string => typeof name === 'string');
const personOf = (value: unknown) => { const actor = obj(value), login = str(actor.login, 'ghost'); return { login, name: str(actor.name), avatar: str(actor.avatarUrl), initial: login.slice(0, 1).toUpperCase() }; };
const labelColor = (color: unknown) => { const hex = str(color).trim().replace(/^#/, ''); return /^[0-9a-fA-F]{6}$/.test(hex) ? `#${hex}` : ''; };
/** `a|b|rest`: the first fields of a write's value, then the rest verbatim (a body, a label name). */
function fields(value: string, count: number): string[] {
  const parts: string[] = [];
  let rest = value;
  for (let index = 0; index < count - 1; index++) { const at = rest.indexOf('|'); if (at < 0) { parts.push(rest); rest = ''; continue; } parts.push(rest.slice(0, at)); rest = rest.slice(at + 1); }
  return [...parts, rest];
}

// ── The view ────────────────────────────────────────────────────────────────

export type PrPickRow = { key: string; label: string; detail: string; avatar: string; initial: string; color: string; team: boolean; on: boolean; value: string };
export type PrPicker = { shown: boolean; allowed: boolean; loading: boolean; error: string; truncated: boolean; busy: boolean; rows: PrPickRow[] };
export type PrWrites = ReturnType<typeof emptyWrites>;
const emptyPicker = (): PrPicker => ({ shown: false, allowed: false, loading: false, error: '', truncated: false, busy: false, rows: [] });
export function emptyWrites() {
  return {
    key: '', composer: false, canComment: false, verdicts: [] as string[], followUp: '', commentDraft: '', summary: '', summaryRequired: false, pendingCount: 0,
    submitting: '', actionPending: false, reviewPending: false, closeSerial: 0, commentSerial: 0, reviewSerial: 0,
    canEditChange: false, rawBody: '', titleSaving: false, titleSerial: 0, bodySaving: false, bodySerial: 0,
    reactionChoices: PULL_REQUEST_REACTION_ORDER.map(content => ({ content, emoji: pullRequestReactionEmoji(content), name: pullRequestReactionName(content) })),
    reviewers: emptyPicker(), labels: emptyPicker(),
  };
}
/** The reference a selection names, as the RPCs and the review store take it. */
export function writeReference(selection: { projectId: string; host: string; repository: string; number: number }): WriteReference {
  return { projectId: selection.projectId, ...(selection.host ? { host: selection.host } : {}), repository: selection.repository, number: selection.number };
}
/** The panel's writes as they stand for this pull request (PullRequestComposer, the editors, the pickers). */
export function presentWrites(client: object, reference: WriteReference, detail: Obj): PrWrites {
  const state = stateOf(client), key = pullRequestReviewKey(reference), view = emptyWrites();
  const capabilities = obj(detail.capabilities), permissions = obj(detail.viewerPermissions);
  view.key = key;
  // What is offered is the host's ability and this account's permission together.
  view.canComment = capabilities.comment === true && permissions.comment === true;
  const offered = names(obj(capabilities.review).verdicts), allowed = names(permissions.verdicts);
  view.verdicts = VERDICT_ORDER.filter(verdict => offered.includes(verdict) && allowed.includes(verdict));
  view.composer = view.canComment || view.verdicts.length > 0;
  const actions = names(capabilities.actions), viewerActions = names(permissions.actions);
  view.followUp = detail.state === 'open' && actions.includes('close') && viewerActions.includes('close') ? 'close'
    : detail.state === 'closed' && actions.includes('reopen') && viewerActions.includes('reopen') ? 'reopen' : '';
  view.commentDraft = state.comments.get(key) ?? '';
  const store = pullRequestReviewStore(client);
  view.summary = store.summaries[key] ?? '';
  view.summaryRequired = detail.provider === 'forgejo';
  view.pendingCount = store.pending(key).length;
  view.submitting = state.submitting.get(key) ?? '';
  view.actionPending = actionState(client).pending !== null;
  view.reviewPending = state.reviewing.has(key);
  view.closeSerial = serial(state, `${key}|close`); view.commentSerial = serial(state, `${key}|comment`); view.reviewSerial = serial(state, `${key}|review`);
  view.canEditChange = canEditPullRequestChangeRequest(detail);
  view.rawBody = str(detail.body);
  view.titleSaving = state.saving.get(key) === 'title'; view.titleSerial = serial(state, `${key}|title`);
  view.bodySaving = state.saving.get(key) === 'body'; view.bodySerial = serial(state, `${key}|body`);
  // The pickers: shown where the host takes the change at all; disabled with the reason where this account may not.
  const reviewers = obj(capabilities.reviewers);
  view.reviewers = picker(state, key, 'reviewers', reviewers.request === true && reviewers.listCandidates === true, permissions.requestReviewers === true);
  view.labels = picker(state, key, 'labels', capabilities.labels === true, permissions.labels !== false);
  return view;
}
function picker(state: State, key: string, which: 'reviewers' | 'labels', shown: boolean, allowed: boolean): PrPicker {
  const held = state.candidates.get(`${key}|${which}`), busy = state.picking.get(`${key}|${which}`) ?? '';
  return {
    shown, allowed, loading: !held || held.loading, error: held && !held.loading ? held.error : '', truncated: held?.truncated === true, busy: busy !== '',
    rows: (held?.list ?? []).map(candidate => (which === 'reviewers' ? reviewerRow(candidate) : labelRow(candidate))),
  };
}
function reviewerRow(candidate: Obj): PrPickRow {
  const person = personOf(candidate), kind = str(candidate.kind, 'user'), requested = candidate.isRequested === true;
  return { key: `${kind}:${str(candidate.id)}`, label: person.login, detail: person.name, avatar: person.avatar, initial: person.initial, color: '', team: kind === 'team', on: requested,
    value: `${kind}|${str(candidate.id)}|${requested ? 'remove' : 'request'}|${person.login}` };
}
function labelRow(candidate: Obj): PrPickRow {
  const name = str(candidate.name), applied = candidate.isApplied === true;
  return { key: name, label: name, detail: str(candidate.description), avatar: '', initial: '', color: labelColor(candidate.color), team: false, on: applied, value: `${applied ? 'remove' : 'apply'}|${name}` };
}
/** A remark's reaction pills as drawn: the host's, with this client's presses in flight laid over. */
export function reactionPills(client: object, reference: WriteReference, subjectId: string, value: unknown): { pills: PrReactionPill[]; reacted: string[] } {
  const reactions = readReactions(value), overlay = stateOf(client).overlays.get(`${pullRequestReviewKey(reference)}|${subjectId}`);
  const pending = overlay && overlay.signature === reactionsSignature(reactions) ? overlay.values : new Map<PullRequestReactionContent, boolean>();
  const shown = applyPendingPullRequestReactions(reactions, pending);
  return {
    pills: shown.map(reaction => ({ content: reaction.content, emoji: pullRequestReactionEmoji(reaction.content), count: reaction.count, pressed: reaction.viewerHasReacted,
      label: `${pullRequestReactionName(reaction.content)}, ${reaction.count}`, tooltip: pullRequestReactionTooltip(reaction) })),
    reacted: shown.filter(reaction => reaction.viewerHasReacted).map(reaction => reaction.content),
  };
}
/** What a remark's card needs to be rewritten where it sits (CommentBody's editing). */
export function commentEditing(client: object, reference: WriteReference, detail: Obj, comment: Obj) {
  const state = stateOf(client), key = pullRequestReviewKey(reference), id = str(comment.id);
  return { canEdit: canEditPullRequestComment(detail, comment), raw: str(comment.body), kind: str(comment.kind), saving: state.saving.get(key) === `comment:${id}`, savedSerial: serial(state, `${key}|comment:${id}`) };
}
/** The text an editor previews (PullRequestMarkdownEditor's Preview), as one more markdown document. */
export function previewBodies(client: object, reference: WriteReference): { id: string; kind: string; title: string; body: string }[] {
  const prefix = `${pullRequestReviewKey(reference)}|`;
  return [...stateOf(client).previews].filter(([name, text]) => name.startsWith(prefix) && text.trim().length > 0)
    .map(([name, text]) => ({ id: `pr-preview:${name.slice(prefix.length)}`, kind: 'assistant', title: '', body: text }));
}

// ── The writes ──────────────────────────────────────────────────────────────

async function wakePanel(native: Native): Promise<void> {
  try { await native.later({ op: 'r10Wake', topic: 't3.pr' }); } catch (error) { if (letGo(error)) throw error; }
}
const failureOf = (error: unknown) => { if (letGo(error)) throw error; return error; };
/** `actionContext`: the header's action context (pages-pr-actions.ts), which Close/Reopen with comment run their action through. */
export type WriteContext = { client: T3Client; native: Native; reference: WriteReference; panel: PanelCache | null; refresh: () => void; now: number; actionContext?: () => PanelContext | null };
/**
 * A pages:pr-act-* write of this module, or null for an op it does not own. Returns the form's
 * message ("" when it landed, the host's reason when it did not); every outcome also toasts as the
 * reference does.
 */
export async function prWrite(context: WriteContext, op: string, value: string): Promise<string | null> {
  const { client, reference } = context, state = stateOf(client), key = pullRequestReviewKey(reference);
  switch (op) {
    // The boxes keep their words here when they lose the focus (chatlocal:prw-*, beside the writes'
    // one-at-a-time route): a blur that a press on Comment or Submit review caused can land after
    // that write, so words already sent, or a box being sent, are not kept again.
    case 'draft-comment': if (!state.submitting.has(key) && state.sent.get(`${key}|comment`) !== value) state.comments.set(key, value); return '';
    case 'draft-summary': if (!state.reviewing.has(key) && state.sent.get(`${key}|review`) !== value) pullRequestReviewStore(client).setSummary(key, value); return '';
    case 'preview': { const [editor = '', text = ''] = fields(value, 2); state.previews.set(`${key}|${editor}`, text); return ''; }
    case 'discard-pending': pullRequestReviewStore(client).clear(key); return '';
    case 'post-comment': return postComment(context, value, 'comment');
    case 'comment-close': return postComment(context, value, 'close');
    case 'comment-reopen': return postComment(context, value, 'reopen');
    case 'review': { const [verdict = '', body = ''] = fields(value, 2); return submitReview(context, verdict, body); }
    case 'edit-title': return saveTitle(context, value);
    case 'edit-body': return saveBody(context, value);
    case 'edit-comment': { const [id = '', kind = '', body = ''] = fields(value, 3); return saveComment(context, id, kind, body); }
    case 'react': { const [subjectId = '', content = '', reacted = ''] = fields(value, 3); return react(context, subjectId, content, reacted === 'true'); }
    case 'candidates': return readCandidates(context, value === 'labels' ? 'labels' : 'reviewers');
    case 'reviewer': { const [kind = '', id = '', change = '', login = ''] = fields(value, 4); return requestReviewer(context, { id, kind }, change === 'request', login); }
    case 'label': { const [change = '', name = ''] = fields(value, 2); return setLabel(context, name, change === 'apply'); }
    default: return null;
  }
}

/**
 * PullRequestCommentForm.submit and PullRequestDetailPanel.performCommentAction. The comment form waits while
 * any header action is out (actionPending), and Close/Reopen with comment hold the header's action runner
 * from the start (pendingAction), post the comment, then finish the action through it (finishAction:
 * its toasts and failure hints, its re-read, the list row's state written on as sent and taken back if
 * refused). The comment is durable even if the action is refused: it is read back while the toast explains.
 */
async function postComment(context: WriteContext, text: string, action: 'comment' | 'close' | 'reopen'): Promise<string> {
  const { client, native, reference } = context, state = stateOf(client), key = pullRequestReviewKey(reference), body = text.trim();
  const actions = actionState(client);
  // Locked while posting or while an action runs, and nothing to post without words (the buttons say so too).
  if (body.length === 0 || state.submitting.has(key) || actions.pending) return '';
  const ctx = action === 'comment' ? null : context.actionContext?.() ?? null;
  if (action !== 'comment' && !ctx) return '';
  state.comments.set(key, text); state.submitting.set(key, action);
  if (ctx) actions.pending = { key: ctx.key, action };
  await wakePanel(native);
  try {
    try { await client.rpc(native, 'pullRequests.comment', { ...reference, body }, true); }
    catch (error) {
      if (ctx && actions.pending?.key === ctx.key) actions.pending = null;
      failureOf(error);
      pushToast(client, { kind: 'error', title: 'Could not post the comment' });
      return error instanceof Error ? error.message : 'Could not post the comment';
    }
    // The comment is posted: clear the box and close the composer, whatever the action does.
    state.comments.delete(key); state.sent.set(`${key}|comment`, text); bump(state, `${key}|comment`); bump(state, `${key}|close`);
    if (!ctx) { context.refresh(); return ''; }
    const failure = await finishAction(client, native, ctx, action);
    if (failure) context.refresh();
    return failure;
  } finally { state.submitting.delete(key); }
}

/** PullRequestReviewForm.submit: the verdict, the summary and the pending line comments in one request. */
async function submitReview(context: WriteContext, verdict: string, body: string): Promise<string> {
  const { client, native, reference } = context, state = stateOf(client), key = pullRequestReviewKey(reference), store = pullRequestReviewStore(client);
  if (!VERDICT_SENT[verdict] || state.reviewing.has(key)) return '';
  store.setSummary(key, body);
  const submittedComments = store.pending(key);
  // canSubmit: an approval may be empty; anything else needs a summary or a line comment.
  if (verdict !== 'approve' && body.trim().length === 0 && submittedComments.length === 0) return '';
  state.reviewing.add(key);
  await wakePanel(native);
  try {
    await client.rpc(native, 'pullRequests.submitReview', { ...reference, verdict, body, comments: submittedComments.map(({ id: _id, ...comment }) => comment) }, true);
  } catch (error) {
    failureOf(error);
    // The draft is kept: whatever went wrong, retyping the review is not the answer.
    pushToast(client, { kind: 'error', title: 'The review could not be submitted' });
    return error instanceof Error ? error.message : 'The review could not be submitted';
  } finally { state.reviewing.delete(key); }
  store.removeComments(key, submittedComments.map(comment => comment.id));
  store.clearSummary(key, body); state.sent.set(`${key}|review`, body);
  bump(state, `${key}|review`); bump(state, `${key}|close`);
  pushToast(client, { kind: 'success', title: VERDICT_SENT[verdict]! });
  context.refresh();
  return '';
}

/** PullRequestDetailPanel.saveTitle: an unchanged or empty title sends nothing. */
async function saveTitle(context: WriteContext, next: string): Promise<string> {
  const { client, native, reference, panel } = context, state = stateOf(client), key = pullRequestReviewKey(reference), title = next.trim();
  if (state.saving.get(key) === 'title') return '';
  if (title.length === 0 || title === str(panel?.detail?.title)) { bump(state, `${key}|title`); return ''; }
  state.saving.set(key, 'title');
  await wakePanel(native);
  try { await client.rpc(native, 'pullRequests.update', { ...reference, title }, true); }
  catch (error) {
    // The draft stays open with the words still in it.
    pushToast(client, { kind: 'error', title: 'The title could not be saved', description: readableFailure(failureOf(error), 'The host refused the new title.') });
    return error instanceof Error ? error.message : 'The title could not be saved';
  } finally { if (state.saving.get(key) === 'title') state.saving.delete(key); }
  bump(state, `${key}|title`);
  context.refresh();
  return '';
}

/** PullRequestSummaryTab.saveBody: an empty description is how one is removed. */
async function saveBody(context: WriteContext, body: string): Promise<string> {
  const { client, native, reference } = context, state = stateOf(client), key = pullRequestReviewKey(reference);
  if (state.saving.get(key) === 'body') return '';
  state.saving.set(key, 'body');
  await wakePanel(native);
  try { await client.rpc(native, 'pullRequests.update', { ...reference, body }, true); }
  catch (error) {
    failureOf(error);
    pushToast(client, { kind: 'error', title: 'Could not save the description' });
    return error instanceof Error ? error.message : 'Could not save the description';
  } finally { if (state.saving.get(key) === 'body') state.saving.delete(key); }
  bump(state, `${key}|body`); state.previews.delete(`${key}|body`);
  context.refresh();
  return '';
}

/** CommentBody's onSave: a remark already posted, rewritten by whoever wrote it. */
async function saveComment(context: WriteContext, commentId: string, kind: string, body: string): Promise<string> {
  const { client, native, reference } = context, state = stateOf(client), key = pullRequestReviewKey(reference);
  // A review's own summary is not a kind any host rewrites, which is why no pencil is offered on one.
  // A remark may not be emptied (the Save button says so too).
  if ((kind !== 'issue-comment' && kind !== 'review-comment') || body.trim().length === 0 || state.saving.get(key)?.startsWith('comment:')) return '';
  state.saving.set(key, `comment:${commentId}`);
  await wakePanel(native);
  try { await client.rpc(native, 'pullRequests.updateComment', { ...reference, commentId, kind, body }, true); }
  catch (error) {
    failureOf(error);
    pushToast(client, { kind: 'error', title: 'Could not save the comment' });
    return error instanceof Error ? error.message : 'Could not save the comment';
  } finally { if (state.saving.get(key) === `comment:${commentId}`) state.saving.delete(key); }
  bump(state, `${key}|comment:${commentId}`); state.previews.delete(`${key}|comment:${commentId}`);
  // updateComment's onSuccess: the activity is read again (and the panel refreshes, onRefresh).
  context.refresh();
  return '';
}

/** PullRequestReactionBar.toggle: drawn at once, rolled back with a toast if the host refuses. */
async function react(context: WriteContext, subjectId: string, content: string, reacted: boolean): Promise<string> {
  const { client, native, reference, panel } = context, state = stateOf(client), key = pullRequestReviewKey(reference);
  if (!isReactionContent(content) || !subjectId) return '';
  const name = `${key}|${subjectId}`;
  const comment = arr(panel?.activity?.comments).find(entry => str(entry.id) === subjectId);
  const signature = reactionsSignature(readReactions(comment?.reactions));
  const held = state.overlays.get(name), values = held && held.signature === signature ? held.values : new Map<PullRequestReactionContent, boolean>();
  state.overlays.set(name, { signature, values: new Map([...values, [content, reacted]]) });
  await wakePanel(native);
  try { await client.rpc(native, 'pullRequests.setReaction', { ...reference, subjectId, content, reacted }, true); }
  catch (error) {
    failureOf(error);
    const current = state.overlays.get(name);
    if (current) { const next = new Map(current.values); next.delete(content); state.overlays.set(name, { signature: current.signature, values: next }); }
    pushToast(client, { kind: 'error', title: 'The reaction could not be saved' });
    return error instanceof Error ? error.message : 'The reaction could not be saved';
  }
  context.refresh();
  return '';
}

/** The reviewer and label menus read their candidates when they open, kept for a minute. */
async function readCandidates(context: WriteContext, which: 'reviewers' | 'labels'): Promise<string> {
  const { client, native, reference, now } = context, state = stateOf(client), name = `${pullRequestReviewKey(reference)}|${which}`, held = state.candidates.get(name);
  if (held && (held.loading || (!held.error && now - held.at < CANDIDATES_FRESH_MS))) return '';
  state.candidates.set(name, { at: now, loading: true, error: '', list: held?.list ?? [], truncated: held?.truncated ?? false });
  await wakePanel(native);
  try {
    const answer = obj(await client.rpc(native, which === 'reviewers' ? 'pullRequests.reviewerCandidates' : 'pullRequests.labelCandidates', { ...reference }));
    state.candidates.set(name, { at: now, loading: false, error: '', list: arr(answer.candidates), truncated: answer.truncated === true });
  } catch (error) {
    failureOf(error);
    state.candidates.set(name, { at: now, loading: false, error: error instanceof Error ? error.message : 'The host did not answer.', list: [], truncated: false });
  }
  return '';
}

/** PullRequestReviewerPicker.toggle, then requestReviewers' cache patch (no reread of the host). */
async function requestReviewer(context: WriteContext, reviewer: { id: string; kind: string }, requested: boolean, login: string): Promise<string> {
  const { client, native, reference } = context, state = stateOf(client), name = `${pullRequestReviewKey(reference)}|reviewers`;
  if (state.picking.has(name)) return '';
  state.picking.set(name, `${reviewer.kind}:${reviewer.id}`);
  await wakePanel(native);
  try { await client.rpc(native, 'pullRequests.requestReviewers', { ...reference, reviewers: [{ id: reviewer.id, kind: reviewer.kind || 'user' }], requested }, true); }
  catch (error) {
    pushToast(client, { kind: 'error', title: requested ? `Could not ask ${login} for a review` : `Could not take back the review request to ${login}`, description: readableFailure(failureOf(error), REVIEWER_HINT) });
    return error instanceof Error ? error.message : 'The host refused it.';
  } finally { state.picking.delete(name); }
  const missing = patchReviewers(state.candidates.get(name), context.panel, [reviewer], requested);
  if (missing) context.refresh();
  pushToast(client, { kind: 'success', title: requested ? `Review requested from ${login}` : `Review request to ${login} taken back` });
  return '';
}
/**
 * requestReviewers' onSuccess: the candidates' `isRequested`, the detail's reviewers and the
 * activity's (where a remark keeps a reviewer who already reviewed) change in place. Returns
 * whether a reviewer's identity was missing from the candidates, which only a read can supply.
 */
export function patchReviewers(candidates: Candidates | undefined, panel: PanelCache | null, reviewers: { id: string; kind: string }[], requested: boolean): boolean {
  const selected = (candidates?.list ?? []).filter(candidate => reviewers.some(reviewer => reviewer.id === str(candidate.id) && (reviewer.kind || 'user') === str(candidate.kind, 'user')));
  const missing = selected.length < reviewers.length;
  if (candidates) candidates.list = candidates.list.map(candidate => (selected.includes(candidate) ? { ...candidate, isRequested: requested } : candidate));
  const logins = new Set(selected.map(candidate => str(candidate.login).toLowerCase()));
  const update = (actors: Obj[], keep: (actor: Obj) => boolean = () => false): Obj[] => (requested
    ? [...actors, ...selected.filter(candidate => !actors.some(actor => str(actor.login).toLowerCase() === str(candidate.login).toLowerCase()))
      .map(candidate => ({ login: str(candidate.login), name: candidate.name ?? null, avatarUrl: candidate.avatarUrl ?? null }))]
    : actors.filter(actor => !logins.has(str(actor.login).toLowerCase()) || keep(actor)));
  if (panel?.detail) panel.detail.reviewers = update(arr(panel.detail.reviewers));
  if (panel?.activity && Array.isArray(panel.activity.reviewers)) {
    const comments = arr(panel.activity.comments);
    panel.activity.reviewers = update(arr(panel.activity.reviewers), actor => comments.some(comment => (comment.kind === 'review' || comment.kind === 'review-comment') && str(obj(comment.author).login).toLowerCase() === str(actor.login).toLowerCase()));
  }
  return missing;
}

/** PullRequestLabelPicker.toggle, then setLabels' cache patch. No success toast, as the reference. */
async function setLabel(context: WriteContext, label: string, applied: boolean): Promise<string> {
  const { client, native, reference } = context, state = stateOf(client), name = `${pullRequestReviewKey(reference)}|labels`;
  if (!label.trim() || state.picking.has(name)) return '';
  state.picking.set(name, label);
  await wakePanel(native);
  try { await client.rpc(native, 'pullRequests.setLabels', { ...reference, labels: [label], applied }, true); }
  catch (error) {
    pushToast(client, { kind: 'error', title: applied ? `Could not put ${label} on` : `Could not take ${label} off`, description: readableFailure(failureOf(error), LABEL_HINT) });
    return error instanceof Error ? error.message : 'The host refused it.';
  } finally { state.picking.delete(name); }
  patchLabels(state.candidates.get(name), context.panel, [label], applied);
  return '';
}
/** setLabels' onSuccess: the candidates' `isApplied` and the detail's labels, in place. */
export function patchLabels(candidates: Candidates | undefined, panel: PanelCache | null, labels: string[], applied: boolean): void {
  const chosen = new Set(labels);
  const known = candidates?.list ?? [];
  if (candidates) candidates.list = candidates.list.map(candidate => (chosen.has(str(candidate.name)) ? { ...candidate, isApplied: applied } : candidate));
  if (!panel?.detail) return;
  const current = arr(panel.detail.labels);
  panel.detail.labels = applied
    ? [...current, ...labels.filter(name => !current.some(label => str(label.name) === name)).map(name => ({ name, color: known.find(candidate => str(candidate.name) === name)?.color ?? null }))]
    : current.filter(label => !chosen.has(str(label.name)));
}
/** Test hook: the candidate list a picker holds. */
export function heldCandidates(client: object, reference: WriteReference, which: 'reviewers' | 'labels'): Candidates | undefined {
  return stateOf(client).candidates.get(`${pullRequestReviewKey(reference)}|${which}`);
}
