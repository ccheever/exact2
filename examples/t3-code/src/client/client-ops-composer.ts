import { omitExpiredTerminalContexts } from '../terminal/terminal-integrations';
import { terminalOpen } from '../terminal/terminal-drawer-view'; // the real terminalOpen
// The composer's client.command() ops (client-ops.ts): the draft and favorite
// models, Send (a new thread's launch, a follow-up, a fan-out), the model,
// option and mode pickers, Stop, approvals and the provider's questions, and
// Retry of a submission T3 may already have taken. Send's submission text and
// its redirect past pending approvals follow T3 Code 74541bab17 ChatView.tsx onSend (MIT; see LICENSE-T3).
import { projectCloneBlock } from '../shell/project-clones-live';
import type { PressImages, T3Client } from './client';
import type { OpOut } from './client-ops';
import { forgetDraftThreadId, launchThreadId } from '../composer/draft-thread-id';
import { composerNow, stagesChanges, stage, rememberModel, rememberOptions, stagedFor, clearStaged, nextTurnCommands, resolveDispatchMode, followUpBehavior, withDispatchMode, planFollowUp, resolvePlanSubmission, sendQueuesBehindSetup, isCompacting } from '../composer/composer-controls';
import { fanoutSelections, fanoutSupported, sendFanout, setFanout, toggleFanout } from '../composer/composer-controls-fanout';
import { acknowledgeWoke, lockedProviderReason, applyOptionChoice, backgroundStarted } from '../composer/composer-controls-commands';
import { keepsFullHistory, resumeCompactionTokens, setKeepFullHistory } from '../composer/composer-controls-resume';
import { contextSnapshot } from '../composer/composer-controls-view';
import { dispatchSelection, promptForSend, ultrathinkChoice } from '../composer/composer-ultrathink';
import { stepReasoningEffort } from '../composer/composer-effort-step';
import { queuedEdit, saveQueuedEdit } from '../composer/composer-controls-queue';
import { ATTACHMENT_ONLY_PROMPT } from '../composer/queued-edit-attachments';
import { fanoutBase, serverThreadWorkspace, workspaceStrategy } from '../composer/composer-controls-branch';
import { antigravitySendBlockReason, composerSelection } from '../composer/composer-provider-selection'; // the sent selection
import { isUsageLimitsCommand, usageLimitsOffered, openUsageLimits } from '../composer/composer-controls-usage';
import { feedbackCommandFor, feedbackInFlight, sendFeedback } from '../composer/composer-feedback';
import { messageContext, pressContext, threadContextRecords, withContext } from '../composer/composer-editor';
import { projectPendingUserInputAnswer } from '../timeline/thread-links';
import { holdSubmitting, questionDraftKey } from '../composer/question-attachments';
import { pendingThreadImports } from '../composer/composer-thread-import';
import { annotationAttachedToDraft, annotationSendMarked, takeAnnotationSend } from '../browser/browser-annotation'; // an annotation's send
import { markAnnotationSend } from '../browser/browser-annotation';
import { annotationDispatchCurrent, takeAnnotationDispatch } from '../browser/browser-capture';
import { composerChipFinishCurrent, takeComposerChipFinish } from '../composer/composer-chip-bridge';
import { pressGesture, composerRequiresModifier, sendIntent } from '../composer/composer-editor-intent';
import { launchTitle } from '../composer/composer-editor-title';
import { promptLengthMessage } from '../composer/composer-editor-menu';
import { composerFileAttachments, draftFiles } from '../composer/composer-editor-files';
import { activeInput, pendingRequests, composerInput, composerRequests, setCustomAnswer, chooseOption, advanceQuestion, previousQuestion, dismissPayload, approvalPayload, declinePendingApprovals, questionAttachmentsStaging } from '../composer/requests';
import { pushToast } from '../shell/toast';
import { machineLabel, unreadableThreads } from '../composer/composer-thread-machines';
import { releaseHeldRequests } from '../composer/composer-typing-guard';
import { threadPhase } from '../composer/composer-presentation';
import { refreshNewThreadContext } from '../composer/new-thread-context';
import { autoBalanceSend } from '../composer/auto-balance';
import { deferSend, settleUploads, takeWaitingSend, uploadsSupported, waitingSend } from '../composer/composer-uploads';
import { providersHost, routedNative } from '../providers/providers-scope';
import { toggleModelFavorite } from '../providers/model-preferences';
import { obj, str, arr, type Obj } from './domain';
import { letGo } from './let-go';
import { ClientError, activeRun, providerAvailable, modelSelection, sendPayload,
  launchPayload, type Native, type Files } from './protocol';

/** The draft, favorite models, a pending question's answers, and Retry of an uncertain submission. */
export async function composerOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op === 'draft') {
      if (value.length > 1_000_000) throw new ClientError('Keep a draft under 1,000,000 characters.');
      if (!setCustomAnswer(this, value)) this.local.drafts[this.draftKey] = value;
     } else if (op === 'favorite-model') {
      const tab = id.indexOf('\t'), environmentId = tab < 0 ? this.environmentId : id.slice(0, tab), instanceId = tab < 0 ? id : id.slice(tab + 1);
      const host = tab < 0 && out.providers ? out.providers.host : providersHost(this, environmentId);
      if (!arr(host.config.providers).some(provider => provider.instanceId === instanceId && arr(provider.models).some(model => model.slug === value))) {
        throw new ClientError('That model is no longer advertised by T3.');
      }
      await toggleModelFavorite(host, routedNative(host, native), instanceId, value);
    } else if (op === 'answer') setCustomAnswer(this, value);
    else if (op === 'choice') chooseOption(this, id, value, text => carryAnswer.call(this, text));
    else if (op === 'previous-question') previousQuestion(this);
    else if (op === 'retry') {
      if (!this.writable) throw new ClientError('Wait for a writable connection to synchronize before retrying.');
      if (this.reconcilePending()) resultMessage = 'T3 already accepted the operation.';
      else if (this.pending) {
        const pending = this.pending;
        const result = await this.write(native, storage, { ...pending, uncertain: false });
        if (pending.method === 'orchestration.launchThread') {
          this.threadId = str(result.threadId, str(pending.payload.threadId));
          await this.openThread(native, this.threadId);
        }
      }
    } else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
/** Send, the model and mode pickers, Stop, approvals and the questions' answers. */
export async function composerWrites(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    const annotation = op === 'send-annotation' ? takeAnnotationDispatch(this, Number(id)) : null;
    const citation = op === 'send-citation' ? takeComposerChipFinish(this, Number(id)) : null;
    if (op === 'send-citation') {
      if (!citation?.send || composerInput(this)) return true;
      op = 'send'; id = ''; value = citation.value;
    }
    if (op === 'send-annotation') {
      if (!annotation) return true;
      op = 'send'; id = ''; value = annotation.value;
    }
    // A press kept while its images uploaded, sent now they have landed (composer-uploads.ts): what it named, to its own
    // thread, whichever is shown. It passed the shown composer's checks at the press.
    if (op === 'send-resume') { resultMessage = await resumeSend(this, native, storage, id); return true; }
    if (['send', 'submit-answers', 'advance-question', 'pick-option'].includes(op) && pendingThreadImports(this)) throw new ClientError('Still bringing attached context into this message. Send again once its chip resolves.');
    if ((['submit-answers', 'advance-question', 'pick-option'].includes(op) || (op === 'send' && composerInput(this))) && questionAttachmentsStaging(this)) throw new ClientError('Still attaching files to this answer. Submit again once they appear.');
    // A request held while the user typed (composer-typing-guard.ts) leaves the composer theirs: this send goes to the
    // thread, and sending releases the request. A shown approval leaves it theirs too: the send declines it and
    // redirects the agent (declineForRedirect).
    const answering = op === 'send' && !!composerInput(this);
    if (answering && (annotation !== null || annotationSendMarked(this))) resultMessage = annotationAttachedToDraft(this); // onSend: a direct annotation never answers a question
    else if (answering) await submitAnswers.call(this, native, storage, '', value);
    else if (op === 'send' && waitingSend(this)) { if (annotation) resultMessage = annotationAttachedToDraft(this); } // sendInFlightRef: one send at a time
    else if (op === 'send') { try { await autoBalanceSend(this, native, async () => {
      if (citation && !composerChipFinishCurrent(this, citation)) return;
      if (annotation) { if (!annotationDispatchCurrent(this, annotation)) return; markAnnotationSend(this); }
      // useComposerSend's snapshot: the images the press names go, whatever joins the draft after it.
      resultMessage = (await send.call(this, native, storage, value, id, { ids: this.snapshotDrafts.map(image => str(image.id)), resume: false })) || '';
    }); } finally { releaseHeldRequests(this); } } // onSend's guard (a move in flight finishes first)
    else if (op === 'provider' || op === 'model') await changeModel.call(this, native, storage, op, id, value, n === 1); // n 1: the picker's Shift+click or Shift+Return
    else if (op === 'model-option') await changeModelOption.call(this, native, storage, id, value);
    // composer.increaseEffort / composer.decreaseEffort (keyboard-dispatch.ts): id is the direction.
    else if (op === 'effort-step') await stepReasoningEffort(this, native, id === '-1' ? -1 : 1, (option, choice) => changeModelOption.call(this, native, storage, option, choice, true));
    else if (op === 'runtime' || op === 'interaction') await changeMode.call(this, native, storage, op, value);
    else if (op === 'stop' && this.threadStopOffered) {
      // A paginated projection can omit commands still running from earlier turns, so the server picks the run.
      const [commandId] = await this.ids(native, 1);
      await this.dispatch(native, storage, { type: 'thread.stop', commandId, threadId: this.threadId }, 'Stop');
    } else if (op === 'stop') {
      const run = activeRun(this.projection);
      if (!run) throw new ClientError('There is no active turn to stop.');
      const [commandId] = await this.ids(native, 1);
      await this.dispatch(native, storage, { type: 'run.interrupt', commandId, threadId: this.threadId, runId: str(run.id), holdQueue: true }, 'Stop');
    } else if (op === 'approval') await approve.call(this, native, storage, id, value);
    else if (op === 'submit-answers' || op === 'advance-question') await submitAnswers.call(this, native, storage, id);
    else if (op === 'pick-option') { if (chooseOption(this, id, value, text => carryAnswer.call(this, text))) await submitAnswers.call(this, native, storage, id.split('::')[0]!); }
    else if (op === 'dismiss-question') await dispatchRequest.call(this, native, storage, dismissPayload(this, id), 'Dismiss question');
    else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
async function send(this: T3Client, native: Native, storage: Files, value: string, pressed = '', press?: PressImages): Promise<string | void> {
  const selection = { generation: this.generation, environmentId: this.environmentId, origin: this.origin, projectId: this.projectId, threadId: this.threadId, providerId: this.providerId, modelId: this.modelId, options: JSON.stringify(this.modelOptions), runtimeMode: this.runtimeMode, interactionMode: this.interactionMode };
  const assertOwner = () => {
    if (selection.generation !== this.generation || selection.environmentId !== this.environmentId || selection.origin !== this.origin || selection.projectId !== this.projectId || selection.threadId !== this.threadId || selection.providerId !== this.providerId || selection.modelId !== this.modelId || selection.options !== JSON.stringify(this.modelOptions) || selection.runtimeMode !== this.runtimeMode || selection.interactionMode !== this.interactionMode) throw new ClientError('The draft or model changed before sending. Your original draft is preserved.');
  };
  // What Send was pressed on: the approvals the composer showed, the follow-up behavior they make "steer", and the draft.
  const shownApprovals = composerRequests(this).approvals.map(approval => approval.requestId), followUp = followUpBehavior(this);
  // The first approval shown takes the message as its decline's feedback when its agent can (useComposerSend.ts).
  const feedbackTarget = feedbackApproval(this, shownApprovals[0]);
  const promptAtSend = this.local.drafts[this.draftKey] ?? '';
  // The Compact chip as the composer showed it when Send was pressed (read before the send's own writes make it busy).
  const compactOffered = !!selection.threadId && resumeCompactionTokens(this, composerNow(this), contextSnapshot(this.projection)) !== null;
  // "/usage-limits" is answered locally from the provider snapshots; the agent never sees it.
  if (isUsageLimitsCommand(value || this.draft) && !this.snapshotDrafts.length && usageLimitsOffered(this)) { if (openUsageLimits(this, composerNow(this))) this.local.drafts[this.draftKey] = ''; return; }
  if (feedbackInFlight(this)) return; // feedbackUploadsInFlightRef: this thread's /feedback upload runs (composer-feedback.ts); onSend checks /usage-limits first
  // threadContextsOutsideEnvironment: a draft moved to another machine cannot send a thread only its old machine reads.
  const unreadable = unreadableThreads(this, value || this.draft)[0];
  if (unreadable) {
    pushToast(this, { kind: 'warning', stacked: true, title: 'Attached thread is on another machine',
      description: `This machine's agent can't read "${unreadable.title}". Remove it, or switch back to ${machineLabel(unreadable.environmentId)} to send.` });
    return;
  }
  const edit = queuedEdit(this);
  if (edit) {
    try { return await saveQueuedEdit(this, native, storage, value, () => this.uploadSnapshots(native, storage, this.draftKey, undefined, press)); } // new SnapShot images upload
    catch (error) {
      if (!(press && error instanceof ClientError && error.kind === 'deferred')) throw error;
      const draftKey = this.draftKey, again = { ids: press.ids, resume: true };
      deferSend(this, { draftKey, environmentId: selection.environmentId, imageIds: [...press.ids], resume: async (native, storage) => {
        if (this.draftKey !== draftKey || queuedEdit(this)?.runId !== edit.runId) return notSent(this);
        return send.call(this, native, storage, value, pressed, again);
      } });
      return;
    }
  }
  const feedback = feedbackCommandFor(this, value || this.draft); // Codex /feedback uploads the thread instead of sending a turn
  if (feedback) return sendFeedback(this, native, value || this.draft, feedback);
  const plan = planFollowUp(this);
  const submission = plan ? resolvePlanSubmission(value || this.draft, plan.markdown) : null, annotationSend = takeAnnotationSend(this);
  // The turn runs on the instance and model the composer shows (composer-provider-selection.ts): a stored model the
  // catalog no longer lists sends the instance's default, as deriveEffectiveComposerModelState does.
  const chosen = composerSelection(this, !!fanoutSelections(this));
  const sendProviderId = chosen.instanceId || selection.providerId, sendModelId = chosen.entry ? chosen.model : selection.modelId;
  const rawText = promptForSend(this, sendProviderId, sendModelId, JSON.parse(selection.options), submission ? submission.text : value || this.draft);
  const terminalSubmission = omitExpiredTerminalContexts(this, rawText, this.snapshotDrafts.length > 0);
  if (terminalSubmission.empty) return;
  const text = terminalSubmission.text.trim();
  if (!text && !this.snapshotDrafts.length) throw new ClientError('Write a message or attach an image first.');
  const outgoingText = text || promptForSend(this, sendProviderId, sendModelId, JSON.parse(selection.options), ATTACHMENT_ONLY_PROMPT);
  if (promptLengthMessage(text)) throw new ClientError(promptLengthMessage(text));
  if (!this.projectId) throw new ClientError('Choose or add a project first.');
  await refreshNewThreadContext(this, native);
  assertOwner();
  if (projectCloneBlock(this)) throw new ClientError(`${projectCloneBlock(this)}. Send once the repository is cloned.`); // project-clones-live.ts
  // An edit typed while the context refreshed is the newer draft; it is not overwritten.
  if ((!submission || submission.interactionMode === 'plan') && (this.local.drafts[this.draftKey] ?? '') === promptAtSend) this.local.drafts[this.draftKey] = text;
  const draftKey = this.draftKey, attachmentsAtSend = attachmentIds(this, draftKey);
  // The press names its gesture (app.contract `submit`): a pointer press's modifiers, or a send chord's from the composer.
  const gesture = pressGesture(pressed);
  const running = !!selection.threadId && threadPhase(this.projection) === 'running';
  // An annotation's ⌘↩ sends in the foreground, "auto" (PreviewView onSendAnnotation).
  const intent = annotationSend ? 'foreground' : sendIntent(this.config, gesture, running, !selection.threadId, terminalOpen(this),
    composerRequiresModifier(this.local.deviceSettings.sendShortcut, value || this.draft));
  const provider = arr(this.config.providers).find(provider => provider.instanceId === sendProviderId);
  // onSend's !providerAvailable: no provider entry, or Antigravity's block reason. A listed provider whose probe failed
  // still sends, and the server's failure shows on the thread.
  const blocked = provider && (chosen.entry || fanoutSelections(this)) ? antigravitySendBlockReason(provider, sendModelId) : 'This provider is unavailable. Configure it in T3 Code.';
  if (blocked || !provider) { if (annotationSend) return annotationAttachedToDraft(this); throw new ClientError(blocked); }
  if (Array.isArray(provider.supportedRuntimeModes) && !provider.supportedRuntimeModes.includes(this.runtimeMode)) {
    throw new ClientError('Choose a permission mode supported by this provider.');
  }
  // An unstarted server thread set to New worktree starts in one with its first message (ChatView onSend).
  const setupQueue = sendQueuesBehindSetup(this);
  const serverWorkspace = selection.threadId && !setupQueue ? serverThreadWorkspace(this) : null;
  if (selection.threadId && feedbackTarget && (attachmentsAtSend || messageContext(this, text))) throw new ClientError('Send approval feedback without attachments or added context.');
  // What the press sends (useComposerSend's snapshot), read now: a press kept while its images upload sends it once they
  // land, whichever thread is shown then. Its folded pastes upload first (composer-editor-files.ts), so their records join.
  const files = await composerFileAttachments(this, native, text);
  assertOwner();
  const contextFor = pressContext(this, text), staged = stagedFor(this), fanout = fanoutSelections(this) ? fanoutBase(this) : null;
  const turnCommands = selection.threadId ? nextTurnCommands(obj(this.projection.thread), staged, submission?.interactionMode ?? '') : [];
  const compacting = isCompacting(this), turnSelection = dispatchSelection(this, sendProviderId, sendModelId, JSON.parse(selection.options));
  const strategy = selection.threadId ? null : workspaceStrategy(this);
  /** The send's rest, with the press's images' attachments: `shown`, whether its draft is still the one shown. */
  const finish = async (native: Native, storage: Files, images: Obj[], check: () => void, shown: () => boolean): Promise<string | void> => {
    check();
    const attachments = [...images, ...files], title = launchTitle(text, attachments); // onSend's titleSeed
    const context = contextFor((this.local.snapshotDrafts[draftKey] ?? []).filter(image => images.some(attachment => attachment.id === str(image.uploadId))));
    const [commandId, messageId, freshThreadId] = await this.ids(native, 3), threadId = selection.threadId ? freshThreadId : launchThreadId(this, draftKey, freshThreadId); // a draft launches as its own id
    check();
    // Its thread left meanwhile, the approvals shown are another thread's: the message leaves them alone, and approval
    // feedback is not sent (useComposerSend).
    if (selection.threadId && !shown() && feedbackTarget) return;
    if (selection.threadId && shown() && !(await declineForRedirect.call(this, native, storage, { shownApprovals, feedbackTarget, text, draftKey, prompts: [promptAtSend, value, text],
      attachmentsAtSend, owner: { ...selection, generation: this.generation } }))) return; // a kept press may go on a connection made since
    check();
    if (selection.threadId) {
      for (const command of turnCommands) {
        const [modeCommandId] = await this.ids(native, 1);
        check();
        await this.dispatch(native, storage, { ...command, commandId: modeCommandId, threadId: selection.threadId }, 'Change mode', check);
      }
      if (serverWorkspace) {
        // startThreadTurn with bootstrap.prepareWorktree: launchThread on the existing thread (reuseExistingThread).
        const payload = launchPayload(commandId, selection.threadId, messageId, selection.projectId, outgoingText, turnSelection, selection.runtimeMode,
          submission?.interactionMode || selection.interactionMode, attachments);
        Object.assign(payload, { reuseExistingThread: true, workspaceStrategy: serverWorkspace, title });
        await this.write(native, storage, { method: 'orchestration.launchThread', payload: withContext(payload, context),
          description: 'Send', threadId: selection.threadId, text, uncertain: false }, check);
      } else {
        // A stale Claude session compacts before this message, so the turn does not re-read the old history (the Compact chip,
        // composer-controls-resume.ts). The message queues behind the /compact run, since steering into it is rejected. A
        // typed `/compact`, with or without instructions, is already the compaction.
        const compactFirst = compactOffered && !keepsFullHistory(this, selection.threadId) && !COMPACT_COMMAND.test(text);
        const mode = compactFirst || setupQueue ? 'queue' : submission || annotationSend ? 'auto' : resolveDispatchMode(running, followUp, intent === 'alternate', !compacting);
        if (compactFirst) {
          const [compactCommandId, compactMessageId] = await this.ids(native, 2);
          check();
          // T3 Code's own compaction says why it ran (OrchestrationV2IdleCompaction), so its row reads "Compacting…" and
          // finishes as "Context compacted before sending" (timeline-compaction.ts).
          await this.dispatch(native, storage, { ...withDispatchMode(sendPayload(compactCommandId, selection.threadId, compactMessageId, '/compact', []), 'auto', turnSelection), idleCompaction: 'onReturn' }, 'Compact context', check);
        }
        const payload = withDispatchMode(withContext(sendPayload(commandId, selection.threadId, messageId, outgoingText, attachments, title), context), mode, turnSelection);
        if (submission?.interactionMode === 'default' && plan) payload.sourcePlanRef = { threadId: selection.threadId, planId: plan.planId };
        await this.dispatch(native, storage, payload, 'Send', check);
        setKeepFullHistory(this, false, selection.threadId); // a turn that starts clears the chip's choice; a failed send keeps it
      }
      clearStaged(this, draftKey);
      if (submission && shown()) this.interactionMode = submission.interactionMode;
      await acknowledgeWoke(this, selection.threadId, native);
      // composer.sendAndNewThread: the sent thread keeps running; a fresh new-thread composer opens in its project.
      if (intent === 'background' && this.threadId === selection.threadId) await this.openFreshDraft(native);
    } else if (fanout) {
      // Several models: one background thread each, in its own worktree (composer-controls-fanout.ts), from the draft shown.
      if (!shown()) return notSent(this);
      await sendFanout(this, native, storage, { text: outgoingText, attachments, ...fanout, runtimeMode: selection.runtimeMode, interactionMode: selection.interactionMode });
    } else {
      const payload = launchPayload(commandId, threadId, messageId, selection.projectId, outgoingText, turnSelection, selection.runtimeMode, selection.interactionMode, attachments);
      Object.assign(payload, { workspaceStrategy: strategy, title });
      const result = await this.write(native, storage, { method: 'orchestration.launchThread', payload: withContext(payload, context),
        description: 'Create thread', threadId, text, uncertain: false }, check);
      forgetDraftThreadId(this, draftKey);
      check();
      // composer.sendBackground: the thread starts out of view and a fresh draft stays open. A draft left while its images
      // uploaded stays out of view too (finalizePromotedDraftThreadByRef).
      if (intent === 'background') { backgroundStarted(this, str(result.threadId, threadId)); return; }
      if (!shown()) return;
      this.threadId = str(result.threadId, threadId);
      await this.openThread(native, this.threadId);
    }
  };
  let images: Obj[];
  try { images = await this.uploadSnapshots(native, storage, draftKey, undefined, press); }
  catch (error) {
    if (!(press && error instanceof ClientError && error.kind === 'deferred')) throw error;
    // The machine the press was on is the one it sends to; the root sends it only while that machine is focused.
    const connected = () => { if (selection.environmentId !== this.environmentId || selection.origin !== this.origin) throw new ClientError('The machine changed before sending. Your original draft is preserved.'); };
    const again = { ids: press.ids, resume: true };
    deferSend(this, { draftKey, environmentId: selection.environmentId, imageIds: [...press.ids], resume: async (native, storage) =>
      finish(native, storage, await this.uploadSnapshots(native, storage, draftKey, undefined, again), connected, () => this.draftKey === draftKey) });
    return;
  }
  return finish(native, storage, images, assertOwner, () => true);
}
/**
 * The press kept under `token`, sent now its images have landed. A failure on a thread no longer shown says so in a toast
 * (useComposerSend sets that thread's error), since the shown thread's banner is another's.
 */
async function resumeSend(client: T3Client, native: Native, storage: Files, token: string): Promise<string> {
  const kept = takeWaitingSend(client, token);
  if (!kept) return '';
  try { return (await kept.resume(native, storage)) || ''; }
  catch (error) {
    if (kept.draftKey === client.draftKey || !(error instanceof ClientError) || ['superseded', 'stale'].includes(error.kind)) throw error;
    pushToast(client, { kind: 'error', stacked: true, title: 'Message not sent', description: error.message });
    return '';
  } finally { releaseHeldRequests(client); }
}
/** A kept press that cannot go from another thread (a multi-model start, a queued message's edit): it stays in its draft. */
function notSent(client: T3Client): string {
  pushToast(client, { kind: 'warning', title: 'Message not sent', description: 'Its attachments finished uploading after you left its thread. It is still in that thread\'s draft.' });
  return '';
}
/** COMPACT_COMMAND_PATTERN (ChatView.logic.ts): `/compact` alone or with instructions. */
const COMPACT_COMMAND = /^\/compact(?:\s|$)/i;
/** The draft's images and attached files, in order (draftChangedSinceSend compares both). */
const attachmentIds = (client: T3Client, key: string) => [...(client.local.snapshotDrafts[key] ?? []).map(image => str(image.id)),
  ...draftFiles(client.local).filter(file => file.draftKey === key).map(file => str(file.id))].join('\n');
const LEFT = Symbol('left the thread');
/** derivePendingThreadRequests' `supportsFeedback` (threadRequests.ts): a live request whose agent reads a decline's feedback. */
function feedbackApproval(client: T3Client, requestId: string | undefined): string {
  const request = requestId ? arr(client.projection.runtimeRequests).find(entry => entry.id === requestId) : undefined;
  const capability = obj(request?.responseCapability);
  return request?.status === 'pending' && capability.type === 'live' && capability.supportsFeedback === true ? requestId! : '';
}
/**
 * ChatView onSend's redirect: a message sent while approvals are pending declines the approvals the composer showed,
 * here, after everything else accepted the send, so a local command or a refused draft leaves them alone. A draft
 * edited since Send, an approval already being answered, or one that arrived during the decline keeps the draft unsent
 * with a toast; a refused decline fails the send with its error, the draft kept. Leaving the thread meanwhile ends the
 * send silently: the approvals shown then are another thread's. When the first approval shown takes feedback, the
 * message declines it as that feedback instead (sendApprovalFeedback), and the draft clears once it is sent.
 */
async function declineForRedirect(this: T3Client, native: Native, storage: Files, sent: { shownApprovals: string[]; feedbackTarget: string; text: string; draftKey: string; prompts: string[];
  attachmentsAtSend: string; owner: { generation: number; environmentId: string; origin: string; threadId: string } }): Promise<boolean> {
  const left = () => sent.owner.generation !== this.generation || sent.owner.environmentId !== this.environmentId || sent.owner.origin !== this.origin || sent.owner.threadId !== this.threadId;
  if (left()) return false;
  const approvals = pendingRequests(this.projection).approvals.filter(approval => sent.shownApprovals.includes(approval.requestId));
  if (!approvals.length && !sent.feedbackTarget) return true;
  const changed = () => !sent.prompts.includes(this.local.drafts[sent.draftKey] ?? '') || attachmentIds(this, sent.draftKey) !== sent.attachmentsAtSend;
  const pendingBefore = pendingRequests(this.projection).approvals.map(approval => approval.requestId);
  const answering = this.pending?.payload.type === 'runtime-request.respond' ? str(this.pending.payload.requestId) : '';
  const responding = approvals.some(approval => approval.requestId === answering), changedBefore = changed();
  const stay = () => { if (left()) throw LEFT; };
  if (sent.feedbackTarget) {
    if (changedBefore || responding || !sent.text.trim()) return false;
    const answered = await dispatchRequest.call(this, native, storage, { type: 'runtime-request.respond', threadId: sent.owner.threadId,
      requestId: sent.feedbackTarget, decision: 'decline', feedback: sent.text.trim() }, 'Decline approval', stay).then(() => true, error => { if (letGo(error)) throw error; return false; });
    if (answered && !changed() && !left()) delete this.local.drafts[sent.draftKey];
    return false;
  }
  const declined = !changedBefore && !responding;
  try {
    if (declined) await declinePendingApprovals(approvals, requestId => (stay(), dispatchRequest.call(this, native, storage,
      { type: 'runtime-request.respond', threadId: sent.owner.threadId, requestId, decision: 'decline' }, 'Decline approval', stay)));
  } catch (error) { if (error === LEFT) return false; throw error; }
  if (left()) return false;
  const draftChanged = changedBefore || changed();
  // An approval that arrived during the decline would block the redirect; the draft stays so the next send declines it too.
  const arrived = pendingRequests(this.projection).approvals.some(approval => !pendingBefore.includes(approval.requestId));
  if (declined && !draftChanged && !arrived) return true;
  pushToast(this, { kind: 'info', description: "Send again when it's ready.",
    title: draftChanged ? declined ? "Approval declined. Your message changed, so it wasn't sent." : "Your message changed, so it wasn't sent."
      : responding ? "An approval is already being answered, so your message wasn't sent." : "Another approval arrived, so your message wasn't sent." });
  return false;
}
async function changeModel(this: T3Client, native: Native, storage: Files, op: string, id: string, instance = '', shift = false, toggled = false): Promise<void> {
  const providerId = op === 'provider' ? id : instance || this.providerId;
  const provider = arr(this.config.providers).find(provider => provider.instanceId === providerId);
  if (!provider || !providerAvailable(provider)) throw new ClientError('This provider is unavailable. Configure it in T3 Code.');
  const models = arr(provider.models);
  const modelId = op === 'model' ? id : str(models.find(model => model.isDefault === true)?.slug || models[0]?.slug);
  if (!models.some(model => model.slug === modelId)) throw new ClientError('That model is no longer advertised by T3.');
  if (this.threadId && provider.requiresNewThreadForModelChange === true && (providerId !== this.providerId || modelId !== this.modelId)) {
    throw new ClientError('Start a new thread to change this model.');
  }
  const locked = lockedProviderReason(this, providerId);
  if (locked) throw new ClientError(locked);
  // A draft's Shift-click (or Shift+Return) adds the model to a multi-model fan-out; a plain pick ends it. The picker says
  // so (model-picker.contract: the press's shiftKey, a row's or the search field's Shift+Return). The model a toggle
  // leaves is a plain pick.
  if (op === 'model' && !toggled) {
    if (shift && fanoutSupported(this)) { const single = toggleFanout(this, providerId, modelId); if (!single) return; return changeModel.call(this, native, storage, 'model', single.model, single.instanceId, false, true); }
  }
  setFanout(this, null);
  const remembered = rememberModel(this, providerId, modelId);
  if (stagesChanges(this)) { stage(this, { providerId, modelId, options: remembered }); return; }
  if (this.threadId) {
    const [commandId] = await this.ids(native, 1);
    await this.dispatch(native, storage, { type: 'thread.model-selection.set', commandId, threadId: this.threadId,
      modelSelection: modelSelection(providerId, modelId, remembered) }, 'Change model');
  }
  this.providerId = providerId; this.modelId = modelId;
  this.modelOptions = remembered;
}
/** `promptHandled`: the caller edits the prompt itself (an effort step), so an Ultrathink choice does not rewrite it here. */
async function changeModelOption(this: T3Client, native: Native, storage: Files, id: string, value: string, promptHandled = false): Promise<void> {
  if (!promptHandled && ultrathinkChoice(this, id, value)) return; // an injected effort rewrites the prompt
  // The traits menu shows the model the turn runs on (composer-provider-selection.ts); its option change keeps that model.
  const chosen = composerSelection(this, !!fanoutSelections(this));
  const providerId = chosen.instanceId || this.providerId, modelId = chosen.entry ? chosen.model : this.modelId;
  const provider = arr(this.config.providers).find(provider => provider.instanceId === providerId);
  const model = arr(provider?.models).find(model => model.slug === modelId);
  const options = applyOptionChoice(arr(obj(model?.capabilities).optionDescriptors), this.modelOptions, id, value);
  if (this.threadId && provider?.requiresNewThreadForModelChange === true) throw new ClientError('Start a new thread to change this model option.');
  rememberOptions(this, providerId, modelId, options);
  if (stagesChanges(this)) { stage(this, { providerId, modelId, options }); return; }
  if (this.threadId) {
    const [commandId] = await this.ids(native, 1);
    await this.dispatch(native, storage, { type: 'thread.model-selection.set', commandId, threadId: this.threadId,
      modelSelection: modelSelection(providerId, modelId, options) }, 'Change reasoning effort');
  }
  this.providerId = providerId; this.modelId = modelId;
  this.modelOptions = options;
}
async function changeMode(this: T3Client, native: Native, storage: Files, op: string, value: string): Promise<void> {
  const modes = op === 'runtime' ? ['approval-required', 'auto-accept-edits', 'auto', 'full-access'] : ['default', 'plan'];
  if (!modes.includes(value)) throw new ClientError('That mode is not supported.');
  const provider = arr(this.config.providers).find(provider => provider.instanceId === this.providerId);
  if (op === 'runtime' && Array.isArray(provider?.supportedRuntimeModes) && !provider.supportedRuntimeModes.includes(value)) {
    throw new ClientError('This provider does not support that permission mode.');
  }
  if (op === 'interaction' && value === 'plan' && provider?.showInteractionModeToggle === false) throw new ClientError('This provider does not support plan mode.');
  if (stagesChanges(this)) { stage(this, op === 'runtime' ? { runtimeMode: value } : { interactionMode: value }); return; }
  if (this.threadId) {
    const [commandId] = await this.ids(native, 1);
    const field = op === 'runtime' ? 'runtimeMode' : 'interactionMode';
    await this.dispatch(native, storage, { type: `thread.${op === 'runtime' ? 'runtime' : 'interaction'}-mode.set`, commandId, threadId: this.threadId, [field]: value }, 'Change mode');
  }
  if (op === 'runtime') this.runtimeMode = value; else this.interactionMode = value;
}
async function dispatchRequest(this: T3Client, native: Native, storage: Files, payload: Obj, description: string, assertOwner?: () => void): Promise<void> {
  assertOwner?.();
  const [commandId] = await this.ids(native, 1);
  assertOwner?.();
  await this.dispatch(native, storage, { ...payload, commandId }, description, assertOwner);
}
async function approve(this: T3Client, native: Native, storage: Files, id: string, decision: string): Promise<void> {
  await dispatchRequest.call(this, native, storage, approvalPayload(this, id, decision), 'Approval');
}
/** A displaced custom answer returns to the thread draft (carryDisplacedCustomAnswerIntoPrompt). */
function carryAnswer(this: T3Client, text: string): void {
  const prompt = this.local.drafts[this.draftKey] || '';
  this.local.drafts[this.draftKey] = prompt.trim() ? `${prompt.trimEnd()}\n\n${text}` : text;
}
/**
 * The composer's Submit/Next and Return advance; the last question answers the provider, each question's attachments
 * beside its answer (onRespondToUserInput): its shelf's images, then the files its answer's chips name.
 */
async function submitAnswers(this: T3Client, native: Native, storage: Files, id: string, custom?: string): Promise<void> {
  if (custom !== undefined && custom !== '') setCustomAnswer(this, custom);
  const requestId = id || activeInput(this)?.input.requestId || '';
  const answers = advanceQuestion(this, requestId);
  if (answers) {
    const owner = { generation: this.generation, environmentId: this.environmentId, origin: this.origin, threadId: this.threadId,
      request: JSON.stringify(pendingRequests(this.projection).inputs[0]) };
    const assertOwner = () => {
      if (owner.generation !== this.generation || owner.environmentId !== this.environmentId || owner.origin !== this.origin || owner.threadId !== this.threadId
        || owner.request !== JSON.stringify(pendingRequests(this.projection).inputs[0])) throw new ClientError('The question or thread changed before submitting. Your answers are still available.');
    };
    const projected = Object.fromEntries(Object.entries(answers).map(([id, answer]) => [id, projectPendingUserInputAnswer(answer, threadContextRecords(this, typeof answer === 'string' ? answer : ''))]));
    const attachmentsByQuestionId: Obj = {};
    // onRespondToUserInput: where the server stages uploads, an answer goes with its images' attachments, or not at all.
    if (uploadsSupported(this)) {
      settleUploads(this);
      const drafts = Object.keys(answers).map(question => this.local.snapshotDrafts[questionDraftKey(owner.environmentId, owner.threadId, requestId, question)] ?? []);
      if (drafts.some(images => images.some(image => !str(image.uploadId)))) throw new ClientError('Wait for attachments to finish uploading, or remove failed uploads.');
    }
    // The request is submitting from its first upload: its shelves take no drop or SnapShot and give up no image.
    const release = holdSubmitting(this, requestId);
    try {
      for (const [id, answer] of Object.entries(answers)) {
        assertOwner();
        const images = await this.uploadSnapshots(native, storage, questionDraftKey(owner.environmentId, owner.threadId, requestId, id), assertOwner);
        assertOwner();
        const files = await composerFileAttachments(this, native, typeof answer === 'string' ? answer : '', assertOwner);
        assertOwner();
        if (images.length || files.length) attachmentsByQuestionId[id] = [...images, ...files];
      }
      await dispatchRequest.call(this, native, storage, { type: 'runtime-request.respond', threadId: owner.threadId, requestId, answers: projected,
        ...(Object.keys(attachmentsByQuestionId).length ? { attachmentsByQuestionId } : {}) }, 'Answers', assertOwner);
    } finally { release(); }
  }
}
