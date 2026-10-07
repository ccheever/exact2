// Codex `/feedback` in the composer, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/ChatView.tsx (feedbackSubmissionsByThreadKey, feedbackUploading,
// feedbackUploadsInFlightRef, anchoredTimelineMessages :3803-3814, feedbackBannerItems
// :7364-7378, onSend's feedback branch :8653-8706, sendDisabledReason :11163-11178) and
// apps/web/src/components/chat/ComposerFeedback.tsx (feedbackBannerItem).
// Changes: the submissions live here per client and thread key instead of React state. Send
// starts `provider.uploadFeedback` as a detached request (composer-replies.ts) and registers the
// submission, so the banner, the two local rows and the cleared draft show at once and the window
// stays usable while the upload runs; submitCodexFeedback (thread-feedback.ts) is called
// unchanged, its `upload` resolving when the reply arrives. The reply maps onto the reference's result: a typed
// ProviderUploadFeedbackError says its message getter's sentence (the wire has no message), a
// connection change or a let-go answer is an interruption, anything else its own message. Local
// rows are inserted among the transcript's rows by time (session-logic.ts anchoredMessages).
import type { T3Client } from './client';
import type { Message } from './domain';
import { arr, obj, str } from './domain';
import { ClientError, bridgeReply, type Native } from './protocol';
import { pushToast } from './toast';
import { letGo } from './let-go';
import { composerNow } from './composer-controls';
import { messageContext } from './composer-editor';
import { fanoutSelections } from './r3-composer-controls-fanout';
import { settleStaleReplies, startDetached, type DetachedReply } from './composer-replies';
import {
  beginCodexFeedbackSubmission, codexFeedbackMessage, codexFeedbackNotice, commandFailure, commandInterrupted, commandSuccess,
  parseCodexFeedbackCommand, submitCodexFeedback, type CodexFeedbackSubmission, type CommandResult, type ProviderUploadFeedbackResult,
} from './thread-feedback';

type FeedbackState = { byThread: Map<string, CodexFeedbackSubmission[]>; inFlight: Set<string> };
const states = new WeakMap<object, FeedbackState>();
function state(client: object): FeedbackState {
  let value = states.get(client);
  if (!value) states.set(client, value = { byThread: new Map(), inFlight: new Set() });
  return value;
}
/** routeThreadKey: the environment and the server thread. */
const threadKey = (client: T3Client) => `${client.environmentId}:${client.threadId}`;
const submissions = (client: T3Client) => client.threadId ? state(client).byThread.get(threadKey(client)) ?? [] : [];

/** feedbackUploading: this thread's upload is running (Send reads "Sending feedback"). */
export function feedbackUploading(client: T3Client): boolean {
  return submissions(client).some(submission => submission.status === 'uploading');
}
/** feedbackUploadsInFlightRef.has(routeThreadKey): onSend returns before anything else. */
export function feedbackInFlight(client: T3Client): boolean {
  return !!client.threadId && state(client).inFlight.has(threadKey(client));
}

/**
 * The feedback command a Send would upload: the Codex driver, a typed `/feedback [reason]` with
 * no attachments, images or contexts, and not a several-model draft. Null sends as a message.
 */
export function feedbackCommandFor(client: T3Client, text: string): { readonly reason?: string } | null {
  const provider = arr(client.config.providers).find(entry => entry.instanceId === client.providerId);
  if (str(provider?.driver) !== 'codex' || client.snapshotDrafts.length > 0 || messageContext(client, text)) return null;
  const command = parseCodexFeedbackCommand(text);
  return command && !fanoutSelections(client) ? command : null;
}

/**
 * onSend's feedback branch. Without a started Codex thread it warns and keeps the draft;
 * otherwise the draft clears now and the upload starts; its reply settles the submission later.
 */
export async function sendFeedback(client: T3Client, native: Native, text: string, command: { readonly reason?: string }): Promise<string> {
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  const providerThreadId = shell?.activeProviderThreadId ?? obj(client.projection.thread).activeProviderThreadId ?? null;
  if (!client.threadId || !shell || providerThreadId === null || providerThreadId === undefined) {
    pushToast(client, { kind: 'warning', title: 'Start a Codex thread first', description: 'Send a message before you submit feedback.', stacked: true });
    return '';
  }
  const key = threadKey(client), threadId = client.threadId, draftKey = client.draftKey;
  const release = beginCodexFeedbackSubmission(state(client).inFlight, key);
  if (!release) return '';
  let id = '', reply: Promise<DetachedReply>;
  try {
    [id] = await client.ids(native, 1) as [string];
    ({ reply } = await startDetached(client, native, 'provider.uploadFeedback', { threadId, ...command }));
  } catch (error) { release(); throw error; }
  void submitCodexFeedback({
    submission: { id, command: text.trim(), createdAt: new Date(composerNow(client)).toISOString() },
    clearDraft: () => { client.local.drafts[draftKey] = ''; },
    onUpdate: submission => {
      const list = state(client).byThread.get(key) ?? [];
      const found = list.some(entry => entry.id === submission.id);
      state(client).byThread.set(key, found ? list.map(entry => entry.id === submission.id ? submission : entry) : [...list, submission]);
    },
    upload: () => reply.then(result => uploadResult(result, threadId)),
  }).finally(release);
  return '';
}

/** The upload's reply as the atom command's result the reference reads. */
function uploadResult(reply: DetachedReply, threadId: string): CommandResult<ProviderUploadFeedbackResult> {
  if (reply.ok) return commandSuccess({ feedbackId: str(reply.value.feedbackId) });
  const error = reply.error;
  if (reply.interrupted || letGo(error)) return commandInterrupted();
  // ProviderUploadFeedbackError.message is a getter the wire does not carry (provider.ts:132-142).
  if (error.kind === 'ProviderUploadFeedbackError') return commandFailure(new Error(`Failed to upload feedback for thread ${threadId}.`));
  return commandFailure(error);
}

/** feedbackBannerItem for each of this thread's submissions (none for an interrupted one). */
export function feedbackNotices(client: T3Client) {
  settleStaleReplies(client);
  return submissions(client).flatMap(submission => {
    const notice = codexFeedbackNotice(submission);
    if (!notice) return [];
    const uploading = submission.status === 'uploading';
    return [{
      // The status is part of the id: a docked notice whose title changes in place kept drawing the old title natively.
      id: `feedback:${submission.id}:${submission.status}`, variant: submission.status === 'failed' ? 'error' : submission.status === 'sent' ? 'success' : 'info',
      priority: uploading ? 0 : 2, icon: 'message-square', title: notice.title, description: notice.description ?? '',
      action: submission.status === 'sent' ? `cclocal:feedback-copy:${submission.id}` : '', actionLabel: 'Copy ID',
      dismiss: uploading ? '' : 'cclocal:feedback-dismiss', dismissLabel: 'Dismiss feedback notice', dismissId: submission.id,
    }];
  });
}

/** `cclocal:feedback-*`: Copy ID (writeTextToClipboard, an error toast on failure) and the dismiss. */
export async function feedbackLocal(client: T3Client, native: Native, op: string, id: string): Promise<string> {
  const list = submissions(client);
  if (op === 'feedback-dismiss') {
    if (client.threadId) state(client).byThread.set(threadKey(client), list.filter(entry => entry.id !== id));
    return '';
  }
  const submission = list.find(entry => entry.id === op.slice('feedback-copy:'.length));
  if (submission?.status !== 'sent') return '';
  try {
    const reply = await bridgeReply(native, { op: 'copyText', text: submission.feedbackId });
    if (!reply.ok || obj(reply.value).copied === false) throw new ClientError(reply.error?.message || 'An error occurred.');
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Could not copy thread ID', description: error instanceof Error ? error.message : 'An error occurred.' });
  }
  return '';
}

/** anchoredTimelineMessages: the command and its reply as local rows, inserted by time. */
export function withFeedbackRows(client: T3Client, rows: Message[]): Message[] {
  const local = submissions(client).flatMap(submission => submission.status === 'interrupted' ? [] : [codexFeedbackMessage(submission), codexFeedbackMessage(submission, 'assistant')]);
  if (!local.length) return rows;
  const out = [...rows];
  for (const message of local) {
    if (out.some(row => row.id === message.id)) continue;
    const row: Message = message.role === 'user'
      ? { id: message.id, kind: 'user', title: 'You', body: message.text, createdAt: message.createdAt, completed: true, local: true }
      : { id: message.id, kind: 'assistant', title: 'Assistant', body: message.text, createdAt: message.createdAt, completed: true, meta: true, streaming: false, local: true };
    const at = out.findIndex(candidate => !!candidate.createdAt && candidate.createdAt > message.createdAt);
    if (at === -1) out.push(row); else out.splice(at, 0, row);
  }
  return out;
}
