// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/client-ops-composer.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
import { omitExpiredTerminalContexts } from './terminal-integrations';
import { terminalOpen } from './terminal-drawer-view'; // terminal-layout: the real terminalOpen
// The composer's client.command() ops (client-ops.ts): the draft and favorite
// models, Send (a new thread's launch, a follow-up, a fan-out), the model,
// option and mode pickers, Stop, approvals and the provider's questions, and
// Retry of a submission T3 may already have taken.
import { projectCloneBlock } from './project-clones-live';
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { forgetDraftThreadId, launchThreadId } from './r7-handoff-thread';
import { composerNow, stagesChanges, stage, rememberModel, rememberOptions, stagedFor, clearStaged, nextTurnCommands, resolveDispatchMode, followUpBehavior, withDispatchMode, planFollowUp, resolvePlanSubmission } from './composer-controls';
import { additiveGesture, fanoutSelections, sendFanout, setFanout, toggleFanout } from './r3-composer-controls-fanout';
import { acknowledgeWoke, lockedProviderReason, applyOptionChoice, backgroundStarted } from './composer-controls-commands';
import { dispatchSelection, promptForSend, ultrathinkChoice } from './composer-ultrathink'; // composer-fidelity G9
import { queuedEdit, saveQueuedEdit } from './composer-controls-queue';
import { fanoutBase, workspaceStrategy } from './composer-controls-branch';
import { isUsageLimitsCommand, usageLimitsOffered, openUsageLimits } from './composer-controls-usage';
import { withMessageContext } from './composer-editor';
import { sendIntent } from './composer-editor-intent';
import { launchTitle } from './composer-editor-title';
import { promptLengthMessage } from './composer-editor-menu';
import { composerFileAttachments } from './composer-editor-files';
import { activeInput, pendingRequests, setCustomAnswer, chooseOption, advanceQuestion, previousQuestion, dismissPayload, approvalPayload } from './requests';
import { threadPhase } from './composer-presentation';
import { autoBalanceSend } from './auto-balance'; // auto-balance
import { obj, str, arr, type Obj } from './domain';
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
      const key = JSON.stringify([id, value]);
      if (!arr(this.config.providers).some(provider => provider.instanceId === id && arr(provider.models).some(model => model.slug === value))) {
        throw new ClientError('That model is no longer advertised by T3.');
      }
      this.local.favoriteModels = this.local.favoriteModels.includes(key)
        ? this.local.favoriteModels.filter(entry => entry !== key) : [...this.local.favoriteModels, key].slice(-200);
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
    if (op === 'send' && pendingRequests(this.projection).approvals.length) throw new ClientError('Resolve this approval request to continue.');
    else if (op === 'send' && activeInput(this)) await submitAnswers.call(this, native, storage, '', value);
    else if (op === 'send') await autoBalanceSend(this, native, () => send.call(this, native, storage, value)); // auto-balance: onSend's guard (a move in flight finishes first)
    else if (op === 'provider' || op === 'model') await changeModel.call(this, native, storage, op, id, value);
    else if (op === 'model-option') await changeModelOption.call(this, native, storage, id, value);
    else if (op === 'runtime' || op === 'interaction') await changeMode.call(this, native, storage, op, value);
    else if (op === 'stop') {
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
async function send(this: T3Client, native: Native, storage: Files, value: string): Promise<void> {
  const selection = { generation: this.generation, environmentId: this.environmentId, origin: this.origin, projectId: this.projectId, threadId: this.threadId, providerId: this.providerId, modelId: this.modelId, options: JSON.stringify(this.modelOptions), runtimeMode: this.runtimeMode, interactionMode: this.interactionMode };
  const assertOwner = () => {
    if (selection.generation !== this.generation || selection.environmentId !== this.environmentId || selection.origin !== this.origin || selection.projectId !== this.projectId || selection.threadId !== this.threadId || selection.providerId !== this.providerId || selection.modelId !== this.modelId || selection.options !== JSON.stringify(this.modelOptions) || selection.runtimeMode !== this.runtimeMode || selection.interactionMode !== this.interactionMode) throw new ClientError('The draft or model changed before sending. Your original draft is preserved.');
  };
  if (queuedEdit(this)) return saveQueuedEdit(this, native, storage, value, () => this.uploadSnapshots(native, storage)); // composer-fidelity G12a: new SnapShot images upload
  // "/usage-limits" is answered locally from the provider snapshots; the agent never sees it.
  if (isUsageLimitsCommand(value || this.draft) && !this.snapshotDrafts.length && usageLimitsOffered(this)) { if (openUsageLimits(this, composerNow(this))) this.local.drafts[this.draftKey] = ''; return; }
  const plan = planFollowUp(this);
  const submission = plan ? resolvePlanSubmission(value || this.draft, plan.markdown) : null;
  const rawText = promptForSend(this, selection.providerId, selection.modelId, JSON.parse(selection.options), submission ? submission.text : value || this.draft);
  const terminalSubmission = omitExpiredTerminalContexts(this, rawText, this.snapshotDrafts.length > 0);
  if (terminalSubmission.empty) return;
  const text = terminalSubmission.text;
  if (!text.trim() && !this.snapshotDrafts.length) throw new ClientError('Write a message or attach an image first.');
  if (promptLengthMessage(text)) throw new ClientError(promptLengthMessage(text));
  if (!this.projectId) throw new ClientError('Choose or add a project first.');
  if (projectCloneBlock(this)) throw new ClientError(`${projectCloneBlock(this)}. Send once the repository is cloned.`); // project-clones-live.ts
  if (!submission || submission.interactionMode === 'plan') this.local.drafts[this.draftKey] = text;
  const gesture = await this.call(native, { op: 'composerSendIntent' }).catch(() => ({}));
  const running = !!selection.threadId && threadPhase(this.projection) === 'running';
  const intent = sendIntent(this.config, gesture, running, !selection.threadId, terminalOpen(this));
  const provider = arr(this.config.providers).find(provider => provider.instanceId === this.providerId);
  if (!provider || !providerAvailable(provider)) throw new ClientError('This provider is unavailable. Configure it in T3 Code.');
  if (!arr(provider.models).some(model => model.slug === this.modelId)) throw new ClientError('Choose one of the models advertised by T3.');
  if (Array.isArray(provider.supportedRuntimeModes) && !provider.supportedRuntimeModes.includes(this.runtimeMode)) {
    throw new ClientError('Choose a permission mode supported by this provider.');
  }
  const attachments = [...await this.uploadSnapshots(native, storage), ...await composerFileAttachments(this, native, text)]; // + folded pastes (composer-editor-files.ts)
  assertOwner();
  const [commandId, messageId, freshThreadId] = await this.ids(native, 3), launchKey = this.draftKey, threadId = selection.threadId ? freshThreadId : launchThreadId(this, launchKey, freshThreadId); // r7-handoff: a draft launches as its own id
  assertOwner();
  if (selection.threadId) {
    const key = this.draftKey, staged = stagedFor(this);
    for (const command of nextTurnCommands(obj(this.projection.thread), staged, submission?.interactionMode ?? '')) {
      const [modeCommandId] = await this.ids(native, 1);
      assertOwner();
      await this.dispatch(native, storage, { ...command, commandId: modeCommandId, threadId: selection.threadId }, 'Change mode', assertOwner);
    }
    const mode = submission ? 'auto' : resolveDispatchMode(running, followUpBehavior(this), intent === 'alternate');
    const payload = withDispatchMode(withMessageContext(this, sendPayload(commandId, selection.threadId, messageId, text, attachments), text), mode,
      dispatchSelection(this, selection.providerId, selection.modelId, JSON.parse(selection.options)));
    if (submission?.interactionMode === 'default' && plan) payload.sourcePlanRef = { threadId: selection.threadId, planId: plan.planId };
    await this.dispatch(native, storage, payload, 'Send', assertOwner);
    clearStaged(this, key);
    if (submission) this.interactionMode = submission.interactionMode;
    await acknowledgeWoke(this, selection.threadId, native);
    // composer.sendAndNewThread: the sent thread keeps running; a fresh new-thread composer opens in its project.
    if (intent === 'background' && this.threadId === selection.threadId) await this.openFreshDraft(native);
  } else if (fanoutSelections(this)) {
    // Several models: one background thread each, in its own worktree (r3-composer-controls-fanout.ts).
    await sendFanout(this, native, storage, { text, attachments, ...fanoutBase(this), runtimeMode: selection.runtimeMode, interactionMode: selection.interactionMode });
  } else {
    const payload = launchPayload(commandId, threadId, messageId, selection.projectId, text,
      dispatchSelection(this, selection.providerId, selection.modelId, JSON.parse(selection.options)), selection.runtimeMode, selection.interactionMode, attachments);
    payload.workspaceStrategy = workspaceStrategy(this);
    payload.title = launchTitle(text, str(this.snapshotDrafts[0]?.name), str(attachments.find(attachment => attachment.type === 'file')?.name)); // composer-editor-title.ts
    const result = await this.write(native, storage, { method: 'orchestration.launchThread', payload: withMessageContext(this, payload, text),
      description: 'Create thread', threadId, text, uncertain: false }, assertOwner);
    forgetDraftThreadId(this, launchKey);
    assertOwner();
    // composer.sendBackground: the thread starts out of view and a fresh draft stays open.
    if (intent === 'background') { backgroundStarted(this, str(result.threadId, threadId)); return; }
    this.threadId = str(result.threadId, threadId);
    await this.openThread(native, this.threadId);
  }
}
async function changeModel(this: T3Client, native: Native, storage: Files, op: string, id: string, instance = ''): Promise<void> {
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
  // A draft's Shift-click (or Shift+Return) adds the model to a multi-model fan-out; a plain pick ends it.
  if (op === 'model' && await additiveGesture(this, native)) { const single = toggleFanout(this, providerId, modelId); if (!single) return; return changeModel.call(this, native, storage, 'model', single.model, single.instanceId); }
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
async function changeModelOption(this: T3Client, native: Native, storage: Files, id: string, value: string): Promise<void> {
  if (ultrathinkChoice(this, id, value)) return; // composer-fidelity G9: an injected effort rewrites the prompt
  const provider = arr(this.config.providers).find(provider => provider.instanceId === this.providerId);
  const model = arr(provider?.models).find(model => model.slug === this.modelId);
  const options = applyOptionChoice(arr(obj(model?.capabilities).optionDescriptors), this.modelOptions, id, value);
  if (this.threadId && provider?.requiresNewThreadForModelChange === true) throw new ClientError('Start a new thread to change this model option.');
  rememberOptions(this, this.providerId, this.modelId, options);
  if (stagesChanges(this)) { stage(this, { options }); return; }
  if (this.threadId) {
    const [commandId] = await this.ids(native, 1);
    await this.dispatch(native, storage, { type: 'thread.model-selection.set', commandId, threadId: this.threadId,
      modelSelection: modelSelection(this.providerId, this.modelId, options) }, 'Change reasoning effort');
  }
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
async function dispatchRequest(this: T3Client, native: Native, storage: Files, payload: Obj, description: string): Promise<void> {
  const [commandId] = await this.ids(native, 1);
  await this.dispatch(native, storage, { ...payload, commandId }, description);
}
async function approve(this: T3Client, native: Native, storage: Files, id: string, decision: string): Promise<void> {
  await dispatchRequest.call(this, native, storage, approvalPayload(this, id, decision), 'Approval');
}
/** A displaced custom answer returns to the thread draft (carryDisplacedCustomAnswerIntoPrompt). */
function carryAnswer(this: T3Client, text: string): void {
  const prompt = this.local.drafts[this.draftKey] || '';
  this.local.drafts[this.draftKey] = prompt.trim() ? `${prompt.trimEnd()}\n\n${text}` : text;
}
/** The composer's Submit/Next and Return advance; the last question answers the provider. */
async function submitAnswers(this: T3Client, native: Native, storage: Files, id: string, custom?: string): Promise<void> {
  if (custom !== undefined && custom !== '') setCustomAnswer(this, custom);
  const requestId = id || activeInput(this)?.input.requestId || '';
  const answers = advanceQuestion(this, requestId);
  if (answers) await dispatchRequest.call(this, native, storage, { type: 'runtime-request.respond', threadId: this.threadId, requestId, answers }, 'Answers');
}
