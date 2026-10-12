// Pinned365aa87982 PendingUserInputCard and threadActivity: one form, all questions.
// Question attachments and retained native editor lifetime are separate checkpoints.
import type { T3Client, Pending } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { activeRun, ClientError, type Native, type Files } from './shared/protocol';
import { pendingRequests } from './shared/requests';
import { letGo, letGoAware } from './shared/let-go';
import { mobilePendingInputColors } from './design';
import { questionJournalConsumed, questionJournalError, questionJournalHome, questionJournalWrite, questionJournalObserve, questionJournalBlocked, questionRecordForPending, questionRecordForRequest, type QuestionRecord } from './pending-input-journal';

export interface PendingInputRoute { active: boolean; visit: string; url: string; environmentId: string; threadId: string }
export interface PendingInputOption { id: string; label: string; description: string; selected: boolean }
export interface PendingInputQuestion { id: string; header: string; question: string; multiSelect: boolean; customAllowed: boolean; answer: string; options: PendingInputOption[] }
export interface PendingInputView { cardId: string; owner: string; active: boolean; requestId: string; questions: PendingInputQuestion[]; count: number; editable: boolean; complete: boolean; canOperate: boolean; canSubmit: boolean; canDismiss: boolean; dismissible: boolean; canStop: boolean; responding: boolean; reason: string; error: string; colors: ReturnType<typeof mobilePendingInputColors> }
export interface PendingInputResult { revision: number; owner: string; message: string }
interface Draft { selected: string[]; custom: string }
interface Question { id: string; header: string; question: string; multiSelect: boolean; customAllowed: boolean; options: { id: string; label: string; description: string }[] }
interface Request { id: string; signature: string; questions: Question[]; resumable: boolean; dismissible: boolean }
interface Entry { incarnation: number; signature: string; drafts: Map<string, Draft>; error: string; operation: { responding: boolean } | null }
interface State { route: string; serial: number; entries: Map<string, Entry> }
const states = new WeakMap<T3Client, State>();
const pendingOwners = new WeakMap<Pending, Entry>();
const state = (client: T3Client) => {
  let value = states.get(client);
  if (!value) { value = { route: '', serial: 0, entries: new Map() }; states.set(client, value); }
  return value;
};
const origin = (client: T3Client) => client.origin.trim().replace(/\/+$/, '');
const prefix = (client: T3Client) => JSON.stringify([questionJournalHome(client) || origin(client), client.environmentId, client.threadId]);
const key = (client: T3Client, id: string) => `${prefix(client)}:${JSON.stringify(id)}`;
const routeKey = (route: PendingInputRoute) => JSON.stringify(route);
function routeCurrent(client: T3Client, route: PendingInputRoute): boolean {
  if (!route.active || !route.visit || !route.threadId || route.environmentId !== client.environmentId || route.threadId !== client.threadId) return false;
  try {
    const parts = new URL(route.url, 'https://route.invalid').pathname.split('/').filter(Boolean).map(decodeURIComponent);
    return (parts.length === 3 || parts.length === 4 && parts[3] === 'files') && parts[0] === 'threads' && parts[1] === route.environmentId && parts[2] === route.threadId;
  } catch { return false; }
}
function requests(client: T3Client): Request[] {
  return pendingRequests(client.projection).inputs.map(input => {
    const item = arr(client.projection.turnItems).slice().reverse().find(item => item.requestId === input.requestId && item.type === 'user_input_request')!;
    const questions = arr(item.questions).map(question => ({ id: str(question.id), header: str(question.header), question: str(question.question),
      multiSelect: question.multiSelect === true, customAllowed: question.allowCustomAnswer !== false,
      options: arr(question.options).map(option => ({ id: typeof option.value === 'string' ? option.value : str(option.label).trim(), label: str(option.label),
        description: option.description !== option.label ? str(option.description) : '' })) }));
    return { id: input.requestId, resumable: input.resumable, dismissible: input.dismissible, questions,
      signature: JSON.stringify([str(item.id), input.createdAt, input.resumable, input.dismissible, questions]) };
  });
}
function entry(client: T3Client, request: Request): Entry {
  const s = state(client), id = key(client, request.id);
  let value = s.entries.get(id);
  if (!value || value.signature !== request.signature) {
    const saved = questionRecordForRequest(client, request.id, request.signature);
    value = { incarnation: ++s.serial, signature: request.signature, drafts: new Map(Object.entries(saved?.drafts ?? {})), error: '', operation: null }; s.entries.set(id, value);
  }
  return value;
}
const draft = (entry: Entry, id: string): Draft => entry.drafts.get(id) ?? { selected: [], custom: '' };
const pendingHere = (client: T3Client, request: Request, value: Entry) => client.pending?.payload.requestId === request.id
  && (!pendingOwners.has(client.pending) || pendingOwners.get(client.pending) === value);
function selected(question: Question, value: Draft): string[] {
  return [...new Set(value.selected)].filter(id => question.options.some(option => option.id === id));
}
function answer(question: Question, value: Draft): string | string[] | null {
  if (question.customAllowed && value.custom.trim()) return value.custom.trim();
  const values = selected(question, value);
  return question.multiSelect ? values.length ? values : null : values[0] ?? null;
}
function answers(request: Request, value: Entry): Record<string, string | string[]> | null {
  const result: Record<string, string | string[]> = {};
  for (const question of request.questions) {
    const resolved = answer(question, draft(value, question.id));
    if (resolved === null) return null;
    result[question.id] = resolved;
  }
  return result;
}
export const emptyPendingInput = (): PendingInputView => ({ cardId: '', owner: '', active: false, requestId: '', questions: [], count: 0, editable: false, complete: false, canOperate: false, canSubmit: false, canDismiss: false,
  dismissible: false, canStop: false, responding: false, reason: '', error: '', colors: mobilePendingInputColors('light') });
function owner(client: T3Client, route: PendingInputRoute, request: Request): string {
  return JSON.stringify([key(client, request.id), entry(client, request).incarnation, client.generation, routeKey(route), request.signature]);
}
export function mobilePendingInputObserveRoute(client: T3Client, route: PendingInputRoute): string {
  const token = routeKey(route); state(client).route = token; return token;
}
export const mobilePendingInputRouteCurrent = (client: T3Client, token: string) => state(client).route === token;
/** Only authoritative loaded request lists retire drafts. Selecting another thread retains them. */
export function mobilePendingInput(client: T3Client, route: PendingInputRoute, scheme = 'light', palette = 't3-code', observed?: string): PendingInputView {
  const s = state(client);
  if (observed !== undefined && !mobilePendingInputRouteCurrent(client, observed)) return emptyPendingInput();
  if (observed === undefined) mobilePendingInputObserveRoute(client, route);
  if (!routeCurrent(client, route) || !client.thread) return emptyPendingInput();
  questionJournalObserve(client);
  const list = requests(client), live = new Set(list.map(request => key(client, request.id))), scope = `${prefix(client)}:`;
  if (client.threadLive) for (const id of s.entries.keys()) if (id.startsWith(scope) && !live.has(id)) s.entries.delete(id);
  const request = list[0]; if (!request) return emptyPendingInput();
  const value = entry(client, request), responding = value.operation?.responding === true || pendingHere(client, request, value);
  const blocked = questionJournalBlocked(client);
  const editable = request.resumable && !responding;
  return { cardId: JSON.stringify([key(client, request.id), value.incarnation]), owner: owner(client, route, request), active: true, requestId: request.id, count: request.questions.length, editable,
    complete: answers(request, value) !== null, canOperate: client.writable, colors: mobilePendingInputColors(scheme, palette),
    responding, dismissible: request.dismissible, canSubmit: editable && client.writable && !client.pending && !blocked && !value.operation && !client.busy && answers(request, value) !== null,
    canDismiss: request.dismissible && !responding && client.writable && !client.pending && !blocked && !value.operation && !client.busy,
    canStop: !!activeRun(client.projection) && client.writable && !client.pending && !blocked && !value.operation && !client.busy && !responding,
    reason: questionJournalError(client) || (!request.resumable ? 'The provider process for this request is no longer available. Interrupt or restart the run to continue.' : ''), error: value.error,
    questions: request.questions.map(question => {
      const current = draft(value, question.id), custom = question.customAllowed && !!current.custom.trim(), values = selected(question, current);
      return { ...question, answer: question.customAllowed ? current.custom : '', options: question.options.map(option => ({ ...option, selected: !custom && values.includes(option.id) })) };
    }) };
}

/** Existing root command carries its current route visit/URL, never a guessed selected draft. */
export function mobilePendingInputCommand(client: T3Client, native: Native | null | undefined, storage: Files,
  expectedOwner: string, operation: string, text: string, routeToken: string): Promise<PendingInputResult> {
  const route = JSON.parse(state(client).route || 'null') as PendingInputRoute | null;
  if (!route || `${route.visit}\n${route.url}` !== routeToken) return Promise.resolve({ revision: client.revision, owner: expectedOwner, message: '' });
  const colon = operation.indexOf(':');
  return mobilePendingInputAction(client, native, storage, route, expectedOwner,
    colon < 0 ? operation : operation.slice(0, colon), colon < 0 ? '' : operation.slice(colon + 1), text);
}

/** Question-local edits never borrow the ordinary composer, its draft key or its request index. */
export async function mobilePendingInputAction(client: T3Client, nativeInput: Native | null | undefined, storage: Files,
  route: PendingInputRoute, expectedOwner: string, op: string, questionId: string, text: string): Promise<PendingInputResult> {
  const s = state(client), local = client.local, request = requests(client)[0], generation = client.generation, endpoint = origin(client);
  const stale = () => new ClientError('This question request changed.');
  const current = () => client.local === local && !!request && routeCurrent(client, route) && s.route === routeKey(route)
    && requests(client)[0]?.signature === request.signature && requests(client)[0]?.id === request.id
    && owner(client, route, request) === expectedOwner;
  if (!current()) return { revision: client.revision, owner: expectedOwner, message: '' };
  const value = entry(client, request!), result = (message = '') => ({ revision: client.revision, owner: expectedOwner, message: current() ? message : '' });
  try {
    if (value.operation?.responding || pendingHere(client, request!, value)) throw new ClientError('Wait for this response to finish.');
    if (op === 'custom' || op === 'option') {
      if (!request!.resumable) throw new ClientError('This request can no longer be answered.');
      const question = request!.questions.find(question => question.id === questionId); if (!question) throw stale();
      const prior = draft(value, question.id);
      if (op === 'custom') {
        if (!question.customAllowed) throw new ClientError('Choose one of the offered options.');
        value.drafts.set(question.id, { custom: text, selected: text.trim() ? [] : selected(question, prior) });
      } else {
        if (!question.options.some(option => option.id === text)) throw stale();
        const values = selected(question, prior);
        value.drafts.set(question.id, { custom: '', selected: question.multiSelect
          ? values.includes(text) ? values.filter(id => id !== text) : [...values, text] : [text] });
      }
      value.error = ''; client.revision++; return result();
    }
    if (!['submit', 'dismiss', 'stop'].includes(op)) throw new ClientError('This question action is unavailable.');
    if (!nativeInput?.available || !client.writable || client.pending || questionJournalBlocked(client) || value.operation || client.busy) throw new ClientError('This connection cannot submit answers.');
    let payload: Obj;
    if (op === 'submit') {
      const response = answers(request!, value);
      if (!request!.resumable || response === null) throw new ClientError('Answer every question before submitting.');
      payload = { type: 'runtime-request.respond', threadId: client.threadId, requestId: request!.id, answers: response };
    } else if (op === 'dismiss') {
      if (!request!.dismissible) throw new ClientError('This question needs an answer.');
      payload = { type: 'thread.user-input.dismiss', threadId: client.threadId, requestId: request!.id };
    } else {
      const run = activeRun(client.projection); if (!run) throw new ClientError('There is no active turn to stop.');
      payload = { type: 'run.interrupt', threadId: client.threadId, runId: str(run.id), holdQueue: true };
    }
    const operation = { responding: op !== 'stop' }; value.operation = operation; value.error = ''; client.revision++;
    let dispatched = false, pending: Pending | undefined;
    const before = () => {
      const saved = questionRecordForPending(client);
      const consumed = dispatched && saved !== undefined && saved.pending === pending && questionJournalConsumed(client, saved);
      const sameRoute = client.local === local && client.generation === generation && origin(client) === endpoint && routeCurrent(client, route) && s.route === routeKey(route);
      if (!(current() || sameRoute && consumed) || !client.writable || value.operation !== operation || client.busy || client.pending && client.pending !== pending) throw stale();
    };
    const base = letGoAware(nativeInput), native: Native = { available: true, watch(topic) { if (!dispatched) before(); base.watch(topic); },
      async later(raw) {
        if (!dispatched) before();
        const fields = obj(raw);
        if (fields.op === 'request' && fields.method === 'orchestration.dispatchCommand') dispatched = true;
        const reply = await base.later(raw); if (!dispatched) before(); return reply;
      } };
    try {
      const [commandId] = await client.ids(native, 1); before();
      pending = { method: 'orchestration.dispatchCommand', payload: { ...payload, commandId }, description: op === 'stop' ? 'Stop' : op === 'dismiss' ? 'Dismiss question' : 'Answers',
        threadId: route.threadId, text: '', uncertain: false };
      pendingOwners.set(pending, value);
      const record: QuestionRecord = { origin: origin(client), environmentId: route.environmentId, threadId: route.threadId, requestId: request!.id, signature: request!.signature,
        pending, drafts: Object.fromEntries([...value.drafts].map(([id, draft]) => [id, { custom: draft.custom, selected: [...draft.selected] }])), phase: 'preflight' };
      await questionJournalWrite(client, native, storage, record, before, false, Object.fromEntries(request!.questions.map(question => [question.id, JSON.stringify(question)])));
      return result();
    } finally { if (value.operation === operation) value.operation = null; client.revision++; }
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'Could not complete this question action.';
    if (current()) { value.error = message; client.revision++; }
    return result(message);
  }
}

/** Existing Retry row, with current route captured before any asynchronous work. */
export async function mobilePendingInputRetry(client: T3Client, native: Native | null | undefined, storage: Files): Promise<{ revision: number; message: string } | null> {
  const saved = questionRecordForPending(client); if (!saved) return null;
  const s = state(client), local = client.local, generation = client.generation, endpoint = origin(client), token = s.route, route = JSON.parse(token || 'null') as PendingInputRoute | null;
  const current = () => client.local === local && client.generation === generation && !!route && s.route === token && routeCurrent(client, route) && origin(client) === endpoint
    && client.environmentId === saved.environmentId && client.threadId === saved.threadId;
  const request = requests(client)[0], value = request ? entry(client, request) : null;
  const resolved = () => questionJournalConsumed(client, saved);
  const before = () => {
    const cleanup = saved.phase === 'unsent' || saved.phase === 'settled' || resolved();
    if (!current() || !native?.available || !client.writable || client.busy || questionRecordForPending(client) !== saved
      || !cleanup && (request?.id !== saved.requestId || request?.signature !== saved.signature || requests(client)[0]?.signature !== saved.signature))
      throw new ClientError('Open the synchronized question request before retrying its saved answer.');
  };
  const operation = { responding: saved.pending.payload.type !== 'run.interrupt' };
  try {
    before(); if (value?.operation) throw new ClientError('Wait for this response to finish.');
    if (value) value.operation = operation;
    // Authoritative resolution is save-only, never another RPC.
    if (resolved()) saved.phase = 'settled';
    await questionJournalWrite(client, letGoAware(native!), storage, saved, before, true);
    if (value && current()) value.error = '';
    return { revision: ++client.revision, message: '' };
  } catch (error) {
    if (letGo(error)) throw error;
    const message = error instanceof Error ? error.message : 'Could not retry this answer.';
    if (value && current()) value.error = message;
    return { revision: ++client.revision, message: current() ? message : '' };
  } finally { if (value?.operation === operation) value.operation = null; }
}
