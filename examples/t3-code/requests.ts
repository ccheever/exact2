// Pending approvals and questions, adapted from T3 Code (MIT); see LICENSE-T3.
// Sources: packages/client-runtime/src/state/threadRequests.ts, apps/web/src/pendingUserInput.ts,
// apps/web/src/components/chat/ComposerPendingApproval{Panel,Actions}.tsx,
// ComposerPendingUserInputPanel.tsx, ComposerPrimaryActions.tsx and
// packages/client-runtime/src/work-log/userInput.ts. The server owns every answer.
import { arr, obj, str, type Obj } from './domain';
import { ClientError } from './protocol';
import type { T3Client } from './client';
import { queuedEdit } from './composer-controls-queue';

/** One draft per question (`request::question`) and one per request (its question index). */
export type RequestDraft = { selected: string[]; custom: string; index: number };
type Question = { id: string; header: string; question: string; multiSelect: boolean; allowCustom: boolean; options: { label: string; value: string; description: string }[] };
type PendingApproval = { requestId: string; kind: string; createdAt: string; detail: string; appName: string; options: Obj[]; live: boolean };
type PendingInput = { requestId: string; createdAt: string; questions: Question[]; resumable: boolean; dismissible: boolean };

export const DEFAULT_APPROVAL_OPTIONS = [
  { decision: 'cancel', label: 'Cancel' },
  { decision: 'decline', label: 'Decline' },
  { decision: 'acceptForSession', label: 'Always allow this session' },
  { decision: 'accept', label: 'Approve' },
];
const NOT_RESUMABLE = 'Provider process is gone — interrupt or restart the run to respond.';

function approvalLabel(kind: string): string {
  return kind === 'mcp-elicitation' ? 'App access approval' : kind === 'command' ? 'Command approval'
    : kind === 'file-read' ? 'File read approval' : kind === 'permission' ? 'App permission approval' : 'File change approval';
}
function question(value: Obj): Question {
  return { id: str(value.id), header: str(value.header), question: str(value.question), multiSelect: value.multiSelect === true,
    allowCustom: value.allowCustomAnswer !== false,
    options: arr(value.options).map(option => ({ label: str(option.label), value: typeof option.value === 'string' ? option.value : str(option.label), description: str(option.description) })) };
}

/** Joins pending request entities to their request items, oldest first (threadRequests.ts). */
export function pendingRequests(projection: Obj): { approvals: PendingApproval[]; inputs: PendingInput[] } {
  const approvals: PendingApproval[] = [], inputs: PendingInput[] = [];
  const items = arr(projection.turnItems);
  for (const request of arr(projection.runtimeRequests)) {
    if (request.status !== 'pending') continue;
    const id = str(request.id), capability = str(obj(request.responseCapability).type);
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
    approvals.push({ requestId: id, kind, createdAt: str(request.createdAt), detail: str(approval.prompt), appName: str(approval.appName),
      options: Array.isArray(approval.options) ? arr(approval.options) : DEFAULT_APPROVAL_OPTIONS, live: capability === 'live' });
  }
  const byTime = (a: { createdAt: string }, b: { createdAt: string }) => a.createdAt.localeCompare(b.createdAt);
  return { approvals: approvals.sort(byTime), inputs: inputs.sort(byTime) };
}

function draft(client: T3Client, key: string): RequestDraft { return client.answers[key] ?? { selected: [], custom: '', index: 0 }; }
/** resolvePendingUserInputAnswer: a non-empty custom answer outranks selected options. */
export function resolveAnswer(question: Question, answer: RequestDraft | undefined): string | string[] | null {
  const custom = question.allowCustom ? answer?.custom.trim() ?? '' : '';
  if (custom) return custom;
  const selected = [...new Set(answer?.selected ?? [])].filter(value => question.options.some(option => option.value === value));
  return question.multiSelect ? (selected.length ? selected : null) : selected[0] ?? null;
}
function progress(client: T3Client, input: PendingInput) {
  const index = Math.max(0, Math.min(draft(client, input.requestId).index, input.questions.length - 1));
  const active = input.questions[index];
  const answers: Record<string, string | string[]> = {};
  let complete = input.questions.length > 0;
  for (const entry of input.questions) {
    const resolved = resolveAnswer(entry, client.answers[`${input.requestId}::${entry.id}`]);
    if (resolved === null) complete = false; else answers[entry.id] = resolved;
  }
  const current = active ? client.answers[`${input.requestId}::${active.id}`] : undefined;
  return { index, active, answers, complete, last: index >= input.questions.length - 1,
    canAdvance: !!active && resolveAnswer(active, current) !== null, custom: active?.allowCustom ? current?.custom ?? '' : '' };
}
export function activeInput(client: T3Client) {
  const input = pendingRequests(client.projection).inputs[0];
  return input ? { input, ...progress(client, input) } : undefined;
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

/** The composer shows the first pending approval, else the first pending question (ChatComposer). */
export function requestPresentation(client: T3Client) {
  const { approvals: pendingApprovals } = pendingRequests(client.projection);
  const pending = client.pending;
  const responding = (id: string) => !!pending && str(pending.payload.requestId) === id;
  const approvals = pendingApprovals.slice(0, 1).map(approval => {
    const reason = responseReason(client, approval.live);
    const options = approval.options.map(option => ({ id: str(option.decision), label: str(option.label), warning: str(option.warning),
      primary: option.decision === 'accept' || option.decision === 'decline' }));
    return { id: approval.requestId, label: approvalLabel(approval.kind), appName: approval.appName,
      count: pendingApprovals.length > 1 ? `1/${pendingApprovals.length}` : '', mono: approval.kind !== 'mcp-elicitation',
      detail: approval.live ? approval.detail || approvalLabel(approval.kind) : NOT_RESUMABLE,
      canRespond: !reason, live: approval.live, responding: responding(approval.requestId), responseReason: reason,
      options: options.filter(option => option.primary), more: options.filter(option => !option.primary) };
  });
  const active = approvals.length ? undefined : activeInput(client);
  const questions = active?.active ? [(() => {
    const { input, active: entry, index } = active;
    const reason = responseReason(client, input.resumable);
    const current = client.answers[`${input.requestId}::${entry.id}`];
    const customActive = active.custom.trim().length > 0;
    return { id: `${input.requestId}::${entry.id}`, requestId: input.requestId, header: entry.header, question: entry.question,
      multiSelect: entry.multiSelect, customAllowed: entry.allowCustom, canRespond: !reason, live: input.resumable, responseReason: reason,
      responding: responding(input.requestId), dismissible: input.dismissible, answer: active.custom,
      position: input.questions.length > 1 ? `${index + 1}/${input.questions.length}` : '', index, last: active.last,
      canAdvance: active.canAdvance, complete: active.complete,
      action: responding(input.requestId) ? 'Submitting...' : active.last ? 'Submit' : 'Next',
      options: entry.options.map((option, position) => ({ id: option.value, label: option.label,
        description: option.description !== option.label ? option.description : '',
        selected: !customActive && (current?.selected ?? []).includes(option.value), shortcut: position < 9 ? String(position + 1) : '' })) };
  })()] : [];
  const requestMode = approvals.length ? 'approval' : questions.length ? 'question' : '';
  const edit = queuedEdit(client);
  return { approvals, questions, requestMode, requestKey: questions[0]?.id ?? (edit ? `queued-edit:${edit.runId}` : ''),
    ...(questions.length ? { draft: active!.custom } : {}), ...connectionBanner(client), ...threadErrors(client) };
}

/** ChatView systemComposerBannerItems: "<label> is reconnecting|offline" above any request. */
function connectionBanner(client: T3Client) {
  const label = str(obj(client.config.environment).label).trim() || 'T3 server';
  const known = !!client.environmentId;
  const reconnecting = client.connection === 'reconnecting' || client.connection === 'connecting' && known;
  const offline = known && (client.connection === 'error' || client.connection === 'disconnected');
  return { connectionTitle: reconnecting ? `${label} is reconnecting` : offline ? `${label} is offline` : '',
    connectionVariant: reconnecting ? 'warning' : offline ? client.connection === 'error' ? 'error' : 'warning' : '',
    connectionAction: offline ? 'Reconnect' : '' };
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
  const current = activeInput(client);
  if (!current?.active || pendingRequests(client.projection).approvals.length) return false;
  if (!current.active.allowCustom) throw new ClientError('Choose one of the offered options.');
  if (value.length > 100_000) throw new ClientError('Keep an answer under 100,000 characters.');
  const key = `${current.input.requestId}::${current.active.id}`;
  setDraft(client, key, { custom: value, selected: value.trim() ? [] : draft(client, key).selected });
  return true;
}
/** Selecting replaces the custom answer; its text returns to the thread draft. */
export function chooseOption(client: T3Client, id: string, value: string, carry: (text: string) => void): boolean {
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
