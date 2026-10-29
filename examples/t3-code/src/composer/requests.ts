// Pending approvals and questions, adapted from T3 Code (MIT); see LICENSE-T3.
// Sources at 74541bab17: packages/client-runtime/src/state/threadRequests.ts, apps/web/src/pendingUserInput.ts,
// apps/web/src/components/chat/ComposerPendingApproval{Panel,Actions}.tsx,
// ComposerPendingUserInputPanel.tsx, ComposerPrimaryActions.tsx, chat/usePendingThreadRequests.ts,
// chat/ChatComposer.tsx (questionAttachmentTarget, questionTakesSnapShots) and
// packages/client-runtime/src/work-log/userInput.ts. The server owns every answer.
import { arr, obj, str, type Obj } from '../client/domain';
import { ClientError } from '../client/protocol';
import type { T3Client } from '../client/client';
import { queuedEdit } from './composer-controls-queue';
import { uploadBlock } from './composer-uploads';
import { promptRewriteKey } from './composer-ultrathink';
import { heldRequestIds } from './composer-typing-guard';
import { markdownEnv, messageChips } from '../timeline/message-chips';
import { withFileLinkMenus } from '../timeline/file-link-menu';
import { relabelThreadLinks } from '../timeline/thread-links';
import { renderCodexInlineDirectivesAsMarkdown } from '../highlight/codex-directives.js';
import { markdownThreadGlyphs } from '../timeline/markdown-thread-glyphs';
import { answerSubmitting, attachmentsStaging, questionDraftKey, questionDraftPrefix, releaseQuestionDrafts } from './question-attachments';

/** One draft per question (`request::question`) and one per request (its question index). */
export type RequestDraft = { selected: string[]; custom: string; index: number };
type Question = { initialAnswer?: string; id: string; header: string; question: string; multiSelect: boolean; allowCustom: boolean; options: { label: string; value: string; description: string }[] };
type PendingApproval = { requestId: string; kind: string; createdAt: string; detail: string; reason: string; fullAccessOverride: string; appName: string;
  options: Obj[]; live: boolean; supportsFeedback: boolean };
type PendingInput = { requestId: string; createdAt: string; questions: Question[]; resumable: boolean; dismissible: boolean };

export const DEFAULT_APPROVAL_OPTIONS = [
  { decision: 'cancel', label: 'Cancel' },
  { decision: 'decline', label: 'Decline' },
  { decision: 'acceptForSession', label: 'Always allow this session' },
  { decision: 'accept', label: 'Approve' },
];
const NOT_RESUMABLE = 'Provider process is gone. Interrupt or restart the run to respond.';

/** fullAccessOverrideExplanation: why an approval reached the user although the thread runs with full access. */
function fullAccessOverrideExplanation(override: string): string {
  return override === 'ask_rule'
    ? "Full access skips approvals, but an ask rule in the agent's own settings matches this action, so it still asks. Remove or narrow that rule to stop these prompts."
    : "Full access skips approvals, but the agent still stops for a few safety checks of its own that no setting can skip, such as a delete whose path it can't verify. T3 Code can't turn these off, so check the request and approve it if it's what you expect.";
}

function approvalLabel(kind: string): string {
  return kind === 'mcp-elicitation' ? 'App access approval' : kind === 'command' ? 'Command approval'
    : kind === 'file-read' ? 'File read approval' : kind === 'permission' ? 'App permission approval' : 'File change approval';
}
function question(value: Obj): Question {
  return { id: str(value.id), header: str(value.header), question: str(value.question), multiSelect: value.multiSelect === true,
    allowCustom: value.allowCustomAnswer !== false, ...(typeof value.initialAnswer === 'string' ? { initialAnswer: value.initialAnswer } : {}),
    options: arr(value.options).map(option => ({ label: str(option.label), value: typeof option.value === 'string' ? option.value : str(option.label), description: str(option.description) })) };
}

/** Joins pending request entities to their request items, oldest first (threadRequests.ts). */
export function pendingRequests(projection: Obj): { approvals: PendingApproval[]; inputs: PendingInput[] } {
  const approvals: PendingApproval[] = [], inputs: PendingInput[] = [];
  const items = arr(projection.turnItems);
  for (const request of arr(projection.runtimeRequests)) {
    if (request.status !== 'pending') continue;
    const id = str(request.id), response = obj(request.responseCapability), capability = str(response.type);
    const item = items.slice().reverse().find(candidate => candidate.requestId === id);
    const kind = str(request.kind) || (item?.type === 'user_input_request' ? 'user_input' : 'approval');
    if (kind === 'user_input') {
      if (item?.type !== 'user_input_request') continue;
      inputs.push({ requestId: id, createdAt: str(request.createdAt), questions: arr(item.questions).map(question),
        resumable: capability !== 'not_resumable', dismissible: item.responseMode === 'message' || capability === 'message' });
      continue;
    }
    if (kind === 'auth_refresh' || kind === 'dynamic_tool_call') continue;
    const approval = item?.type === 'approval_request' ? item : {};
    approvals.push({ requestId: id, kind, createdAt: str(request.createdAt), detail: str(approval.prompt), reason: str(approval.reason),
      fullAccessOverride: str(approval.fullAccessOverride), appName: str(approval.appName),
      options: Array.isArray(approval.options) ? arr(approval.options) : DEFAULT_APPROVAL_OPTIONS, live: capability === 'live',
      supportsFeedback: capability === 'live' && response.supportsFeedback === true });
  }
  const byTime = (a: { createdAt: string }, b: { createdAt: string }) => a.createdAt.localeCompare(b.createdAt);
  return { approvals: approvals.sort(byTime), inputs: inputs.sort(byTime) };
}

function draft(client: T3Client, key: string): RequestDraft { return client.answers[key] ?? { selected: [], custom: '', index: 0 }; }
/** A question's attachments: its image count, and whether they keep it unanswered (a server that takes none, or files still staging). */
type QuestionAttachments = { count: number; blocked: boolean };
/**
 * resolvePendingUserInputAnswer: a non-empty custom answer outranks selected options; with neither, attachments alone
 * answer a question that takes a custom answer, as an empty one. Attachments the server cannot take answer nothing.
 */
export function resolveAnswer(question: Question, answer: RequestDraft | undefined, attachments: QuestionAttachments = { count: 0, blocked: false }): string | string[] | null {
  if (attachments.blocked) return null;
  const custom = !question.allowCustom || answer?.custom === undefined ? null
    : question.initialAnswer !== undefined ? answer.custom : answer.custom.trim() || null;
  if (custom !== null && custom.length > 0) return custom;
  const selected = [...new Set(answer?.selected ?? [])].filter(value => question.options.some(option => option.value === value));
  const fallback = custom ?? (question.allowCustom && attachments.count > 0 ? '' : null);
  return question.multiSelect ? (selected.length ? selected : fallback) : selected[0] ?? fallback;
}
const questionAttachmentsSupported = (client: T3Client) => obj(obj(client.config.environment).capabilities).questionAttachments === true;
function questionAttachments(client: T3Client, requestId: string, questionId: string): QuestionAttachments {
  if (!client.threadId) return { count: 0, blocked: false };
  const key = questionDraftKey(client.environmentId, client.threadId, requestId, questionId), count = (client.local.snapshotDrafts[key] ?? []).length;
  return { count, blocked: (count > 0 && !questionAttachmentsSupported(client)) || attachmentsStaging(client, key) > 0 };
}
/** onRespondToUserInput's first check: a question of the composer's request is still staging files. */
export function questionAttachmentsStaging(client: T3Client): boolean {
  const current = client.threadId ? composerInput(client) : undefined;
  return !!current && current.input.questions.some(question =>
    attachmentsStaging(client, questionDraftKey(client.environmentId, client.threadId, current.input.requestId, question.id)) > 0);
}
/** activePendingIsResponding: the answer is being uploaded or sent. */
const respondingTo = (client: T3Client, requestId: string) => str(client.pending?.payload.requestId) === requestId || answerSubmitting(client, requestId);
function progress(client: T3Client, input: PendingInput) {
  for (const question of input.questions) {
    const key = `${input.requestId}::${question.id}`;
    if (question.allowCustom && question.initialAnswer !== undefined && client.answers[key] === undefined)
      client.answers[key] = { selected: [], custom: question.initialAnswer, index: 0 };
  }
  const index = Math.max(0, Math.min(draft(client, input.requestId).index, input.questions.length - 1));
  const active = input.questions[index];
  const answers: Record<string, string | string[]> = {};
  let complete = input.questions.length > 0;
  for (const entry of input.questions) {
    const resolved = resolveAnswer(entry, client.answers[`${input.requestId}::${entry.id}`], questionAttachments(client, input.requestId, entry.id));
    if (resolved === null) complete = false; else answers[entry.id] = resolved;
  }
  const current = active ? client.answers[`${input.requestId}::${active.id}`] : undefined;
  return { index, active, answers, complete, last: index >= input.questions.length - 1,
    canAdvance: !!active && resolveAnswer(active, current, questionAttachments(client, input.requestId, active.id)) !== null, custom: active?.allowCustom ? current?.custom ?? '' : '' };
}
export function activeInput(client: T3Client) {
  const input = pendingRequests(client.projection).inputs[0];
  return input ? { input, ...progress(client, input) } : undefined;
}
/**
 * The requests that take the composer over: a first approval or question that arrived while the user typed
 * their draft is held (composer-typing-guard.ts), and the composer stays theirs until typing stops.
 */
export function composerRequests(client: T3Client): { approvals: PendingApproval[]; inputs: PendingInput[] } {
  const pending = pendingRequests(client.projection);
  const held = heldRequestIds(client, client.draftKey, [...pending.approvals.map(entry => entry.requestId), ...pending.inputs.map(entry => entry.requestId)]);
  return { approvals: pending.approvals.filter(entry => !held.has(entry.requestId)), inputs: pending.inputs.filter(entry => !held.has(entry.requestId)) };
}
/**
 * declinePendingApprovals (threadRequests.ts): a message sent while approvals are pending redirects the agent, so
 * every approval it replaces is declined first; one still pending would block the agent. A request the provider can
 * no longer answer has nothing to decline. A refused decline stops at that approval and is rethrown.
 */
export async function declinePendingApprovals(approvals: readonly PendingApproval[], respond: (requestId: string) => Promise<void>): Promise<void> {
  for (const approval of approvals) if (approval.live) await respond(approval.requestId);
}
/** The question the composer answers, unless it is held or an approval shows. */
export function composerInput(client: T3Client) {
  const requests = composerRequests(client);
  // A shown approval comes first: the composer is the thread's draft, which a send redirects with.
  if (requests.approvals.length) return undefined;
  const input = requests.inputs[0];
  return input ? { input, ...progress(client, input) } : undefined;
}

/** questionAttachmentTarget: the draft of the question the composer answers, whose shelf the composer shows. */
export function questionAttachmentKey(client: T3Client): string | null {
  const current = client.threadId ? composerInput(client) : undefined;
  return current?.active ? questionDraftKey(client.environmentId, client.threadId, current.input.requestId, current.active.id) : null;
}
/**
 * questionTakesSnapShots: the draft of the composer's question while it takes a SnapShot: the server takes question
 * attachments, the question takes a custom answer and its answer is not being submitted.
 */
export function openQuestionDraft(client: T3Client): { key: string } | null {
  const current = client.threadId ? composerInput(client) : undefined;
  if (!current?.active || !current.active.allowCustom || !questionAttachmentsSupported(client)) return null;
  if (respondingTo(client, current.input.requestId)) return null;
  return { key: questionDraftKey(client.environmentId, client.threadId, current.input.requestId, current.active.id) };
}

function responseReason(client: T3Client, resumable: boolean): string {
  if (!resumable) return NOT_RESUMABLE;
  if (client.connection !== 'connected') return 'Reconnect to respond to this request.';
  if (!client.ready) return 'Wait for synchronization to finish before responding.';
  if (!client.scopes.includes('orchestration:operate')) return 'This connection does not have permission to respond.';
  if (!client.writable) return 'This server does not support responses from this client.';
  if (client.pending || client.busy) return 'Wait for the current submission to finish before responding.';
  return '';
}

/** canOperateThread: without orchestration:operate a request card shows with its answers and decisions disabled. */
const operable = (client: T3Client) => client.scopes.includes('orchestration:operate');

/**
 * pruneResolvedUserInputDrafts: the open thread's answer drafts and question attachment drafts for requests no longer
 * pending go. A thread whose detail has not arrived keeps them: missing detail is not resolution.
 */
function pruneAnswers(client: T3Client): void {
  if (!client.threadId || str(obj(client.projection.thread).id) !== client.threadId) return;
  const inputs = pendingRequests(client.projection).inputs, answers = client.answers;
  const pending = new Set(inputs.map(input => input.requestId));
  for (const key of Object.keys(answers)) if (!pending.has(key.split('::')[0]!)) delete answers[key];
  const { environmentId, threadId } = client;
  const retained = new Set(inputs.flatMap(input => input.questions.map(question => questionDraftKey(environmentId, threadId, input.requestId, question.id))));
  releaseQuestionDrafts(client.local.snapshotDrafts, client.local.snapshotReleases, questionDraftPrefix(environmentId, threadId), retained);
}
/** The composer shows the first pending approval, else the first pending question (ChatComposer). */
export function requestPresentation(client: T3Client) {
  pruneAnswers(client);
  const { approvals: pendingApprovals } = composerRequests(client);
  const pending = client.pending;
  const responding = (id: string) => (!!pending && str(pending.payload.requestId) === id) || answerSubmitting(client, id);
  const approvals = pendingApprovals.slice(0, 1).map(approval => {
    const reason = responseReason(client, approval.live);
    const options = approval.options.map(option => ({ id: str(option.decision), label: str(option.label), warning: str(option.warning),
      primary: option.decision === 'accept' || option.decision === 'decline' }));
    return { id: approval.requestId, label: approvalLabel(approval.kind), appName: approval.appName,
      count: pendingApprovals.length > 1 ? `1/${pendingApprovals.length}` : '', mono: approval.kind !== 'mcp-elicitation',
      detail: approval.live ? approval.detail || approvalLabel(approval.kind) : NOT_RESUMABLE, reason: approval.reason,
      whyAsked: approval.fullAccessOverride ? fullAccessOverrideExplanation(approval.fullAccessOverride) : '', feedback: approval.supportsFeedback,
      canRespond: !reason, operable: operable(client), live: approval.live, responding: responding(approval.requestId), responseReason: reason,
      options: options.filter(option => option.primary), more: options.filter(option => !option.primary) };
  });
  const active = approvals.length ? undefined : composerInput(client);
  const questions = active?.active ? [(() => {
    const { input, active: entry, index } = active;
    const reason = responseReason(client, input.resumable);
    const current = client.answers[`${input.requestId}::${entry.id}`];
    const customActive = (entry.initialAnswer !== undefined ? active.custom : active.custom.trim()).length > 0;
    const text = relabelThreadLinks(renderCodexInlineDirectivesAsMarkdown(entry.question), client.shell.threads), root = str(client.shell.projects.find(project => project.id === client.projectId)?.workspaceRoot);
    return { id: `${input.requestId}::${entry.id}`, requestId: input.requestId, header: entry.header, question: text,
      multiSelect: entry.multiSelect, customAllowed: entry.allowCustom, canRespond: !reason, operable: operable(client), live: input.resumable, responseReason: reason,
      responding: responding(input.requestId), dismissible: input.dismissible, answer: active.custom,
      position: input.questions.length > 1 ? `${index + 1}/${input.questions.length}` : '', index, last: active.last,
      canAdvance: active.canAdvance, complete: active.complete, uploading: uploadBlock(client) !== '',
      action: responding(input.requestId) ? 'Submitting...' : active.last ? 'Submit' : 'Next', md: { ...markdownEnv(client), chips: withFileLinkMenus(client, messageChips({ text }, root, client.shell.threads)), threadGlyphs: markdownThreadGlyphs(client, text) },
      options: entry.options.map((option, position) => ({ id: option.value, label: option.label,
        description: option.description !== option.label ? option.description : '',
        selected: !customActive && (current?.selected ?? []).includes(option.value), shortcut: position < 9 ? String(position + 1) : '' })) };
  })()] : [];
  const requestMode = approvals.length ? 'approval' : questions.length ? 'question' : '';
  const edit = queuedEdit(client);
  return { approvals, questions, requestMode, requestKey: (questions[0]?.id ?? (edit ? `queued-edit:${edit.runId}` : '')) + promptRewriteKey(client), // a client-side prompt rewrite shows
    ...(questions.length ? { draft: active!.custom } : {}), ...threadErrors(client) };
}

// Session-scoped dismissals, keyed by thread and message (ThreadErrorBanner.tsx).
const dismissedErrors = new Set<string>();
/** latestRootProviderFailure + threadErrorSummary over the latest unheld run. */
export function serverThreadError(projection: Obj): string {
  const thread = obj(projection.thread);
  const session = arr(projection.providerSessions).filter(entry => entry.providerInstanceId === thread.providerInstanceId).pop();
  const runs = arr(projection.runs).filter(run => !(run.status === 'queued' && run.queueHeld === true));
  const latest = runs.reduce<Obj | undefined>((best, run) => !best || Number(run.ordinal) > Number(best.ordinal) ? run : best, undefined);
  let failure = '';
  if (latest?.status === 'failed') {
    const errors = arr(projection.turnItems).filter(item => item.type === 'error' && item.status === 'failed' && item.runId === latest.id
      && (latest.rootNodeId === undefined || item.nodeId === latest.rootNodeId));
    const newest = errors.sort((a, b) => str(a.updatedAt).localeCompare(str(b.updatedAt))).pop();
    failure = str(obj(newest?.failure).message);
  }
  return str(session?.lastError) || failure;
}
function threadErrors(client: T3Client) {
  const server = client.threadId ? serverThreadError(client.projection) : '';
  const visible = server && !dismissedErrors.has(`${client.threadId}\u0000${server}`) ? server : '';
  return { error: client.error || visible, clientError: client.error };
}
export function dismissThreadError(client: T3Client): void {
  const server = client.threadId ? serverThreadError(client.projection) : '';
  if (server) dismissedErrors.add(`${client.threadId}\u0000${server}`);
}

function requireInput(client: T3Client, requestId: string) {
  const current = activeInput(client);
  if (!current || current.input.requestId !== requestId || !current.active) throw new ClientError('This request has already been resolved.');
  if (!current.input.resumable) throw new ClientError(NOT_RESUMABLE);
  return current;
}
function setDraft(client: T3Client, key: string, next: Partial<RequestDraft>): void {
  client.answers[key] = { ...draft(client, key), ...next };
}
/** setPendingUserInputCustomAnswer: typing in the composer answers the active question. */
export function setCustomAnswer(client: T3Client, value: string): boolean {
  const current = composerInput(client);
  if (!current?.active || composerRequests(client).approvals.length) return false;
  if (!current.active.allowCustom) throw new ClientError('Choose one of the offered options.');
  if (value.length > 100_000) throw new ClientError('Keep an answer under 100,000 characters.');
  const key = `${current.input.requestId}::${current.active.id}`;
  setDraft(client, key, { custom: value, selected: current.active.initialAnswer !== undefined || value.trim() ? [] : draft(client, key).selected });
  return true;
}
/** Selecting replaces the custom answer; its text returns to the thread draft. */
export function chooseOption(client: T3Client, id: string, value: string, carry: (text: string) => void): boolean {
  if (!operable(client)) return false; // handleOptionSelection: a disabled card ignores the pick
  const separator = id.indexOf('::');
  const current = requireInput(client, id.slice(0, separator));
  if (current.active!.id !== id.slice(separator + 2)) throw new ClientError('That question is no longer active.');
  if (!current.active!.options.some(option => option.value === value)) throw new ClientError('That option is no longer available.');
  const before = draft(client, id), displaced = before.custom.trim();
  if (displaced) carry(displaced);
  if (current.active!.multiSelect) {
    const selected = before.selected.includes(value) ? before.selected.filter(entry => entry !== value) : [...before.selected, value];
    setDraft(client, id, { selected, custom: '' });
    return false;
  }
  setDraft(client, id, { selected: [value], custom: '' });
  return true;
}
/** onAdvanceActivePendingUserInput: the last question submits every resolved answer. */
export function advanceQuestion(client: T3Client, requestId = activeInput(client)?.input.requestId ?? ''): Record<string, string | string[]> | null {
  const current = requireInput(client, requestId);
  if (!current.last) {
    if (!current.canAdvance) throw new ClientError(`Answer “${current.active!.header || current.active!.question}” first.`);
    setDraft(client, requestId, { index: current.index + 1 });
    return null;
  }
  if (!current.complete) {
    const missing = current.input.questions.find(entry => resolveAnswer(entry, client.answers[`${requestId}::${entry.id}`]) === null);
    throw new ClientError(`Answer “${missing?.header || missing?.question || 'every question'}” first.`);
  }
  return current.answers;
}
export function previousQuestion(client: T3Client): void {
  const current = activeInput(client);
  if (current) setDraft(client, current.input.requestId, { index: Math.max(0, current.index - 1) });
}
export function dismissPayload(client: T3Client, requestId: string): Obj {
  const current = requireInput(client, requestId);
  if (!current.input.dismissible) throw new ClientError('This question needs an answer.');
  return { type: 'thread.user-input.dismiss', threadId: client.threadId, requestId };
}
/** Only provider-advertised decisions are sent; defaults apply when none are advertised. */
export function approvalPayload(client: T3Client, requestId: string, decision: string): Obj {
  const approval = pendingRequests(client.projection).approvals.find(entry => entry.requestId === requestId);
  if (!approval) throw new ClientError('This request has already been resolved.');
  if (!approval.live) throw new ClientError('This approval is no longer live.');
  if (!approval.options.some(option => option.decision === decision)) throw new ClientError('Choose one of the approval options offered by the provider.');
  return { type: 'runtime-request.respond', threadId: client.threadId, requestId, decision };
}

function answerText(value: unknown): string {
  if (typeof value === 'string') return value;
  if (Array.isArray(value)) return value.map(answerText).filter(Boolean).join(', ');
  return value && typeof value === 'object' ? answerText((value as Obj).answers) : '';
}
/** Work-log label and detail for request rows (session-logic.ts, work-log/userInput.ts). */
export function requestActivity(item: Obj, projection: Obj): { label: string; body: string } {
  // workEntryDisplayLabel: a compact detail (the prompt) outranks the generic heading.
  if (item.type === 'approval_request') return { label: str(item.prompt).replace(/\s+/g, ' ').trim() || str(item.title).trim() || 'Approval requested', body: str(item.prompt) || str(item.requestKind) };
  const request = arr(projection.runtimeRequests).find(entry => entry.id === item.requestId);
  const answered = obj(item.questionAnswer).answers !== undefined ? obj(item.questionAnswer)
    : request?.status === 'resolved' && request.answers !== undefined
      ? { answers: request.answers, questionTextById: Object.fromEntries(arr(item.questions).map(entry => [str(entry.id), str(entry.question)])) } : undefined;
  if (!answered) return { label: str(item.title).trim() || 'Input requested', body: arr(item.questions).map(entry => str(entry.question)).join('\n\n') };
  const texts = obj(answered.questionTextById), answers = obj(answered.answers);
  const heading = Object.values(texts).map(text => str(text).replace(/\s+/g, ' ').trim()).filter(Boolean).join(' · ');
  const preview = Object.values(answers).map(answerText).filter(Boolean).join(' · ').replace(/\s+/g, ' ').trim();
  const ids = [...new Set([...Object.keys(texts), ...Object.keys(answers)])];
  return { label: [heading || str(item.title).trim() || 'Answered questions', preview].filter(Boolean).join('  '),
    body: ids.map(id => [str(texts[id]), answerText(answers[id]) ? `  ${answerText(answers[id])}` : ''].filter(Boolean).join('\n')).join('\n\n') };
}
