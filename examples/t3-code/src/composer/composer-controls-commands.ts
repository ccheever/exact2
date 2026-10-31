// Composer commands the footer and banners issue,
// adapted from T3 Code 9c161462c9 (MIT); see LICENSE-T3. Sources at 9c161462c9: ChatView.tsx (onResume,
// onImplementPlanInNewThread, handleUnsnoozeActiveThread, acknowledgeActiveThreadWoke,
// onProviderModelSelect's provider lock), ChatView.logic.ts (deriveLockedProvider,
// resolveComposerProviderSelection), threadWorkflows.ts (threadSupportsProviderHandoff),
// TraitsPicker.tsx (option choice) and QueuedRunsControl.tsx.
import { providerGoal, presentProviderGoal, providerGoalWorking } from './provider-goal';
import { arr, obj, str, type Obj } from '../client/domain';
import { visiblePullRequests } from '../panel/shell-pr';
import { ClientError, activeRun, launchPayload, type Files, type Native } from '../client/protocol';
import { pushToast } from '../shell/toast';
import { wokeAt as sidebarWokeAt } from '../sidebar/sidebar-model';
import { beginQueuedEdit, cancelQueuedEdit, queueState, removeQueuedEditAttachment, toggleQueueShownInFull } from './composer-controls-queue';
import { copyBranchName, dismissBranchMismatch, restoreThreadBranch, selectBranch, setEnvMode } from './composer-controls-branch';
import { attachIncoming, cancelPreparingImage, type IncomingFiles } from './composer-controls-attach';
import { retryDraftUpload, runDraftUploads } from './composer-uploads';
import { foldPaste, heldPaste } from './composer-editor-files';
import { uuid } from '../transport/transport-wire';
import { queueRowDrop } from '../sidebar/row-drop';
import { openUsageLimits, closeUsageLimits, changeLimitRecovery, dismissFallbackNotice, usageLocal } from './composer-controls-usage';
import { feedbackLocal } from './composer-feedback';
import type { T3Client } from '../client/client';
import { keepsFullHistory, setKeepFullHistory } from './composer-controls-resume';
import { dispatchSelection } from './composer-ultrathink'; // modelOptionsForDispatch
import { composerSelection } from './composer-provider-selection';
import { composerNow, clearStaged, nextTurnCommands, planFollowUp, proposedPlanTitle, requireProvider, resumeState, stagedFor,
  PLAN_IMPLEMENTATION_PROMPT_PREFIX, type ComposerControlsPrefs } from './composer-controls';
import { loadHostResources } from './auto-balance';
import { isProviderSubagent } from './composer-controls-subagent';
import { announce } from '../shell/announce';

/** A select row picks one of its options; a boolean row is On or Off (buildProviderOptionSelectionsFromDescriptors). */
export function applyOptionChoice(descriptors: Obj[], current: Obj[], id: string, value: string): Obj[] {
  const descriptor = descriptors.find(entry => entry.id === id && (entry.type === 'select' || entry.type === 'boolean'));
  if (!descriptor) throw new ClientError('That model option is no longer offered by this provider.');
  if (descriptor.type === 'select') {
    if (!arr(descriptor.options).some(option => option.id === value)) throw new ClientError('That model option is no longer offered by this provider.');
    return [...current.filter(option => option.id !== id), { id, value }];
  }
  if (value !== 'true' && value !== 'false') throw new ClientError('That model option is no longer offered by this provider.');
  return [...current.filter(option => option.id !== id), { id, value: value === 'true' }];
}

// ── Provider lock in started threads ───────────────────────────────────────

/** threadSupportsProviderHandoff: a session that can hand off (or a thread with nothing to continue) is unlocked. */
export function supportsHandoff(projection: Obj): boolean {
  const thread = obj(projection.thread), run = activeRun(projection);
  const providerThreadId = str(run?.providerThreadId) || str(thread.activeProviderThreadId);
  const providerThreads = arr(projection.providerThreads);
  const attached = providerThreads.find(entry => entry.id === providerThreadId)
    ?? providerThreads.find(entry => entry.appThreadId === thread.id && entry.providerSessionId);
  const session = attached ? arr(projection.providerSessions).find(entry => entry.id === attached.providerSessionId) : undefined;
  if (session) return obj(obj(session.capabilities).sessions).supportsProviderSwitchingViaHandoff === true;
  if (run) return false;
  if (thread.historyOrigin === 'v1_import' || !arr(projection.runs).length) return true;
  return providerThreads.some(entry => entry.id === thread.activeProviderThreadId && entry.appThreadId === thread.id
    && entry.providerInstanceId === obj(thread.modelSelection).instanceId && entry.nativeThreadRef != null);
}
/** deriveLockedProvider + the instance's continuation group: a started thread keeps its driver (and account group). */
export function providerLock(client: { threadId?: string; projection?: Obj; config: Obj }): { driver: string; group: string } | null {
  if (!client.threadId || !client.projection) return null;
  const projection = client.projection, thread = obj(projection.thread);
  const started = arr(projection.runs).length > 0 || arr(projection.turnItems).length > 0 || !!thread.activeProviderThreadId;
  if (!started || supportsHandoff(projection)) return null;
  const providers = arr(client.config.providers);
  const instanceId = str(obj(thread.modelSelection).instanceId);
  const instance = providers.find(provider => provider.instanceId === instanceId);
  const driver = str(instance?.driver);
  if (!driver) return null;
  return { driver, group: str(obj(instance?.continuation).groupKey) };
}
export function matchesLock(provider: Obj, lock: { driver: string; group: string } | null): boolean {
  if (!lock) return true;
  if (provider.driver !== lock.driver) return false;
  return !lock.group || str(obj(provider.continuation).groupKey) === lock.group;
}
export function lockedProviderReason(client: T3Client, providerId: string): string {
  const provider = arr(client.config.providers).find(entry => entry.instanceId === providerId);
  return provider && !matchesLock(provider, providerLock(client)) ? 'This thread continues with its current provider. Start a new thread to use another one.' : '';
}

// ── Woke acknowledgment ────────────────────────────────────────────────────

const prefs = (client: T3Client): ComposerControlsPrefs => client.local.composerControls;
/** threadWokeAt for an unsnoozed timer wake or an early raised hand, as the sidebar computes it (threadSettled.ts). */
export function wokeAt(shell: Obj, now: number): string {
  return sidebarWokeAt(shell, now) ?? '';
}
/**
 * useAcknowledgeThreadWoke: a visit at the wake time clears Woke. Servers with
 * visited tracking own the watermark (thread.visit keeps the later value, so
 * every device sees it; failures stay quiet); older servers keep it locally.
 */
export async function acknowledgeWoke(client: T3Client, threadId: string, native?: Native, at = ''): Promise<void> {
  const shell = client.shell.threads.find(thread => thread.id === threadId);
  const woke = at || (shell ? wokeAt(shell, composerNow(client)) : '');
  if (!woke) return;
  if (native && obj(obj(client.config.environment).capabilities).threadVisitedTracking === true) {
    if (!client.writable) return;
    const access = client.restAccess(native);
    // Awaited: a reply that lands after the command ends is dropped and leaves the request pending.
    try { const [commandId] = await access.ids(1); await access.request('orchestration.dispatchCommand', { type: 'thread.visit', commandId, threadId, visitedAt: woke }, true); } catch { /* reportFailure: false */ }
    return;
  }
  prefs(client).wokeSeen[`${client.environmentId}:${threadId}`] = woke;
}
/** resolveThreadLastVisitedAt: a tracking server's watermark (even a mark-unread rewind) wins over the local one. */
export function wokeWatermark(client: T3Client, shell: Obj): string {
  const local = client.local.composerControls.wokeSeen[`${client.environmentId}:${str(shell.id)}`];
  if (shell.lastVisitedAt === undefined) return str(local);
  return str(shell.lastVisitedAt);
}

/** composer.sendBackground: the launched thread stays out of view; a toast opens it. */
export function backgroundStarted(client: T3Client, threadId: string): void {
  pushToast(client, { kind: 'success', title: 'Started in background', timeoutMs: 5000, action: { label: 'Open', op: 'select-thread', id: threadId } });
}

// ── Commands ───────────────────────────────────────────────────────────────

function requireThread(client: T3Client): Obj {
  if (!client.threadId || !client.thread) throw new ClientError('Open a thread first.');
  return client.projection;
}

/** Writes from the composer footer and its banners (op names follow `cc:`). */
export async function composerCommand(client: T3Client, native: Native, storage: Files, op: string, id: string, value: string): Promise<string> {
  const access = client.restAccess(native);
  if (op === 'resume') {
    // onResume: continue an interrupted (or limit-stopped) run, then release a held queue.
    const projection = requireThread(client), threadId = client.threadId;
    const { runId, heldQueue } = resumeState(projection);
    if (!runId && !heldQueue) throw new ClientError('There is nothing to resume.');
    if (runId) {
      const staged = stagedFor(client), key = client.draftKey;
      for (const command of nextTurnCommands(obj(projection.thread), staged)) {
        const [commandId] = await access.ids(1);
        await access.dispatch(storage, { ...command, commandId, threadId }, 'Change mode');
      }
      const [commandId, messageId] = await access.ids(2);
      // Resume continues on the model the composer shows (getSendContext().selectedModelSelection).
      const chosen = composerSelection(client, false);
      const modelSelection = dispatchSelection(client, chosen.instanceId || client.providerId, chosen.entry ? chosen.model : client.modelId, client.modelOptions);
      await access.dispatch(storage, { type: 'message.dispatch', commandId, threadId, messageId, text: 'Continue where you left off.', attachments: [],
        createdBy: 'user', creationSource: 'web', manualContinuationOfRunId: runId, modelSelection,
        dispatchMode: { type: 'start_immediately' } }, 'Resume thread');
      clearStaged(client, key);
    }
    if (heldQueue) {
      const [commandId] = await access.ids(1);
      await access.dispatch(storage, { type: 'queue.resume', commandId, threadId }, 'Resume thread');
    }
    return '';
  }
  if (op === 'implement-new') {
    // onImplementPlanInNewThread: a default-mode thread titled after the plan starts implementing it.
    const plan = planFollowUp(client);
    if (!plan) throw new ClientError('There is no plan ready to implement.');
    requireProvider(client);
    const projection = requireThread(client), source = obj(projection.thread);
    const settings = obj(client.config.settings), overrides = obj(obj(settings.projectSettingsOverrides)[client.projectId]);
    const runtimeMode = str(overrides.defaultRuntimeMode || settings.defaultRuntimeMode, 'approval-required');
    const title = (plan.title ? `Implement ${plan.title}` : 'Implement plan').slice(0, 100);
    const [commandId, messageId, threadId] = await access.ids(3);
    const payload = launchPayload(commandId, threadId, messageId, client.projectId, `${PLAN_IMPLEMENTATION_PROMPT_PREFIX}${plan.markdown.trim()}`,
      dispatchSelection(client, client.providerId, client.modelId, client.modelOptions), runtimeMode, 'default');
    payload.title = title;
    if (str(source.worktreePath)) payload.workspaceStrategy = { type: 'existing_worktree', worktreePath: str(source.worktreePath), ...(str(source.branch) ? { branch: str(source.branch) } : {}) };
    else if (str(source.branch)) payload.workspaceStrategy = { type: 'root', branch: str(source.branch) };
    const result = await access.write(storage, { method: 'orchestration.launchThread', payload, description: 'Create thread', threadId, text: '', uncertain: false });
    client.threadId = str(result.threadId, threadId); client.thread = null;
    return '';
  }
  if (op === 'unsnooze') {
    // handleUnsnoozeActiveThread ("Wake now").
    const threadId = id || client.threadId;
    if (obj(obj(client.config.environment).capabilities).threadSnooze === false) throw new ClientError('This server does not support snoozing threads.');
    const [commandId] = await access.ids(1);
    await access.dispatch(storage, { type: 'thread.unsnooze', commandId, threadId, reason: 'user' }, 'Wake thread');
    return '';
  }
  if (op === 'compact' || op === 'goal-resume' || op === 'goal-clear') {
    // Native commands run as separate turns; the draft and its attachments stay local.
    const projection = requireThread(client), threadId = client.threadId;
    if (op === 'compact' ? !!activeRun(projection) : op === 'goal-resume' && providerGoalWorking(client)) throw new ClientError(op === 'compact' ? 'Compacting is unavailable right now' : 'Wait for the current turn to finish.');
    // A provider's subagent has no composer to send from (its bar replaces it), so its goal line's Clear does nothing.
    if (op !== 'compact' && isProviderSubagent(obj(projection.thread))) return '';
    if (op !== 'compact') {
      const goal = providerGoal(client.shell.threads.find(thread => thread.id === threadId)?.goal);
      if (!goal || op === 'goal-resume' && !presentProviderGoal(goal, false).canResume) throw new ClientError('That goal action is no longer available.');
    }
    requireProvider(client);
    const staged = stagedFor(client), key = client.draftKey;
    for (const command of nextTurnCommands(obj(projection.thread), staged)) {
      const [commandId] = await access.ids(1);
      await access.dispatch(storage, { ...command, commandId, threadId }, 'Change mode');
    }
    const [commandId, messageId] = await access.ids(2);
    const chosen = composerSelection(client);
    await access.dispatch(storage, { type: 'message.dispatch', commandId, threadId, messageId, text: op === 'compact' ? '/compact' : op === 'goal-resume' ? '/goal resume' : '/goal clear', attachments: [], createdBy: 'user', creationSource: 'web',
      modelSelection: dispatchSelection(client, chosen.instanceId, chosen.model, client.modelOptions), deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' } }, op === 'compact' ? 'Compact context' : op === 'goal-resume' ? 'Resume goal' : 'Clear goal');
    clearStaged(client, key);
    return '';
  }
  if (op === 'limit-resume' || op === 'limit-snooze') { await changeLimitRecovery(client, native, storage, op === 'limit-resume' ? 'resume' : 'snooze'); return ''; }
  if (op === 'branch-restore' || op === 'branch-restore-confirmed') return restoreThreadBranch(client, native, storage, op === 'branch-restore-confirmed');
  if (op === 'branch' || op === 'branch-create') return selectBranch(client, native, storage, value, op === 'branch-create');
  if (op === 'stop-background') {
    // interruptThreadTurn: `thread.stop` when the server advertises threadStop; otherwise the active run, else the
    // latest one while background work is pending; with neither, a PR watch can still hold the thread (one that never
    // ran too).
    const projection = requireThread(client), threadId = client.threadId;
    if (client.threadStopOffered) {
      const [commandId] = await access.ids(1);
      await access.dispatch(storage, { type: 'thread.stop', commandId, threadId }, 'Stop background work');
      return '';
    }
    const pending = arr(client.shell?.threads.find(thread => thread.id === threadId)?.pendingBackgroundTasks).length > 0;
    const run = activeRun(projection) ?? (pending ? arr(projection.runs).at(-1) : undefined);
    if (!run) {
      for (const link of visiblePullRequests(obj(projection.thread).pullRequests)) {
        if (link.watch === undefined) continue;
        const [commandId] = await access.ids(1);
        await access.dispatch(storage, { type: 'thread.pull-request.watch', commandId, threadId, host: link.host, repository: link.repository, number: link.number, watching: false }, 'Stop pull request watch');
      }
      return '';
    }
    const [commandId] = await access.ids(1);
    // holdQueue interrupts every provider thread with pending work, a delegated child's too.
    await access.dispatch(storage, { type: 'run.interrupt', commandId, threadId, runId: str(run.id), holdQueue: true }, 'Stop background work');
    return '';
  }
  if (op === 'queued-remove' || op === 'queued-steer') {
    const projection = requireThread(client), threadId = client.threadId;
    const run = arr(projection.runs).find(entry => entry.id === id && entry.status === 'queued');
    if (!run) throw new ClientError('That queued message already started or was removed.');
    const [commandId] = await access.ids(1);
    if (op === 'queued-remove') await access.dispatch(storage, { type: 'queued-run.cancel', commandId, threadId, runId: id }, 'Remove queued message');
    else {
      // sendQueuedMessage: with no active run the message starts a turn; otherwise it steers the active one (or stops
      // its compaction).
      const state = queueState(projection), target = activeRun(projection);
      if (state.canSendNow) await access.dispatch(storage, { type: 'queued-run.start', commandId, threadId, runId: id }, 'Send queued message');
      else if (!target || !state.canSteer) throw new ClientError('There is no active run to steer');
      else await access.dispatch(storage, { type: 'queued-message.promote-to-steer', commandId, threadId, queuedRunId: id, targetRunId: str(target.id) }, 'Steer queued message');
    }
    return '';
  }
  throw new ClientError(`Unknown composer action: ${op}`);
}

/**
 * ModelPickerContent onWindowKeyDown: a tab or a model its shortcut picked is announced, since the focus stays in the
 * search field. ⇧⌘↑/⇧⌘↓ say "Favorites" or the provider's name, ⌘1–9 "Model: <name>"; each replaces the picker's
 * message not yet spoken. `kind` is the shortcut's ("provider" or "model"), `id` what it picked, `value` its provider.
 */
export function announcePickerShortcut(client: T3Client, kind: string, id: string, value: string): void {
  const provider = (instanceId: string) => arr(client.config.providers).find(entry => entry.instanceId === instanceId);
  if (kind === 'provider') announce(client, id === 'favorites' ? 'Favorites' : str(provider(id)?.displayName) || id, 'model-picker');
  if (kind === 'model') announce(client, `Model: ${str(arr(provider(value)?.models).find(model => model.slug === id)?.name) || id}`, 'model-picker');
}

/** Local composer state (op names follow `cclocal:`). */
export async function composerLocal(client: T3Client, _native: Native, _storage: Files, op: string, id: string, value: string): Promise<string> {
  if (op === 'queued-edit') { beginQueuedEdit(client, id); return ''; }
  if (op === 'picker-provider-key' || op === 'picker-model-key') { announcePickerShortcut(client, op === 'picker-provider-key' ? 'provider' : 'model', id, value); return ''; }
  // The root's upload pump (composer-uploads.ts): `value` names the upload resources that failed with no answer.
  if (op === 'uploads') { await runDraftUploads(client, _native, value.split(' ').filter(Boolean).map(Number)); return ''; }
  if (op === 'upload-retry') { retryDraftUpload(client, id); return ''; }
  if (op === 'env-mode') { setEnvMode(client, value); return ''; }
  if (op === 'previous-worktree') { setEnvMode(client, 'previous'); return ''; }
  if (op === 'attach-files') { // a drop, a paste, Attach files or a sidebar row's drop (app.contract attachFiles)
    const incoming = JSON.parse(value) as IncomingFiles;
    // A large text paste the composer held (composer.contract pasted): its record, the field's value and the text.
    if (id === 'fold') { const held = heldPaste(incoming.files); if (held) await foldPaste(client, _native, held.disposition, held, uuid()).catch(() => undefined); return ''; }
    return id === 'row' ? queueRowDrop(client, incoming.target, incoming) : attachIncoming(client, _native, id, incoming);
  }
  if (op === 'branch-mismatch-dismiss') { dismissBranchMismatch(client, id); return ''; }
  if (op === 'branch-copy') return copyBranchName(client, _native, value);
  // The compact strip's workspace trigger has no context menu (BranchToolbar's MobileRunContextSelector).
  if (op === 'noop') return '';
  if (op === 'balance-load') return loadHostResources(client, _native, id, Number(value) || 0); // the machines' host resources
  if (op === 'usage-limits') { openUsageLimits(client, composerNow(client)); return ''; }
  if (op === 'usage-limits-dismiss') { closeUsageLimits(client); return ''; }
  if (op === 'fallback-dismiss') { dismissFallbackNotice(client, id); return ''; } // the fallback model notice, for this window
  // Manage usage and the reset credit confirm (composer-controls-usage.ts), the /feedback banner's
  // Copy ID and dismiss (composer-feedback.ts).
  if (op.startsWith('usage-')) return usageLocal(client, _native, op, id);
  if (op.startsWith('feedback-')) return feedbackLocal(client, _native, op, id);
  if (op === 'queued-cancel') { cancelQueuedEdit(client); return ''; }
  if (op === 'queued-show-all') { toggleQueueShownInFull(client); return ''; } // "Show all N queued" / "Show fewer"
  if (op === 'preparing-image-cancel') { cancelPreparingImage(client, id); return ''; } // a preparing tile's Remove
  if (op === 'queued-attachment-remove') { removeQueuedEditAttachment(client, id); return ''; }
  if (op === 'compact-full') { setKeepFullHistory(client, !keepsFullHistory(client)); return ''; } // the Compact chip's toggle
  if (op === 'dismiss-woke') { await acknowledgeWoke(client, id || client.threadId, _native, value); return ''; }
  throw new ClientError(`Unknown composer action: ${op}`);
}
