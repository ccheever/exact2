// Composer commands the footer and banners issue (lane composer-controls),
// adapted from T3 Code (MIT); see LICENSE-T3. Sources: ChatView.tsx (onResume,
// onImplementPlanInNewThread, handleUnsnoozeActiveThread, acknowledgeActiveThreadWoke,
// onProviderModelSelect's provider lock), ChatView.logic.ts (deriveLockedProvider,
// resolveComposerProviderSelection), threadWorkflows.ts (threadSupportsProviderHandoff),
// TraitsPicker.tsx (option choice) and QueuedRunsControl.tsx.
import { arr, obj, str, type Obj } from './domain';
import { ClientError, activeRun, launchPayload, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import { beginQueuedEdit, cancelQueuedEdit, queuedDrop, queueState, removeQueuedEditAttachment } from './composer-controls-queue';
import { branchMenu, selectBranch, setEnvMode } from './composer-controls-branch';
import { attachFiles } from './composer-controls-attach';
import { openUsageLimits, closeUsageLimits, changeLimitRecovery } from './composer-controls-usage';
import type { T3Client } from './client';
import { dismissResumeCompaction } from './r3-composer-controls-resume';
import { dispatchSelection } from './composer-ultrathink'; // composer-fidelity: modelOptionsForDispatch
import { composerNow, clearStaged, nextTurnCommands, planFollowUp, proposedPlanTitle, requireProvider, resumeState, stagedFor,
  PLAN_IMPLEMENTATION_PROMPT_PREFIX, type ComposerControlsPrefs } from './composer-controls';

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
/** threadWokeAt for an unsnoozed timer wake or an early raised hand (threadSettled.ts). */
export function wokeAt(shell: Obj, now: number): string {
  const until = Date.parse(str(shell.snoozedUntil));
  if (!str(shell.snoozedUntil) || !Number.isFinite(until)) return '';
  const run = obj(shell.latestRun), snoozedAt = Date.parse(str(shell.snoozedAt));
  const failed = ['error', 'failed'].includes(str(obj(shell.runtime ?? shell.session).status))
    && (!Number.isFinite(snoozedAt) || Date.parse(str(obj(shell.runtime ?? shell.session).updatedAt)) > snoozedAt);
  const completedAfter = Number.isFinite(snoozedAt) && run.status === 'completed' && Date.parse(str(run.completedAt)) > snoozedAt;
  const raised = !!shell.pendingRuntimeRequest || shell.hasPendingApprovals === true || shell.hasPendingUserInput === true || failed || completedAfter;
  if (raised) return completedAfter ? str(run.completedAt) : str(obj(shell.runtime ?? shell.session).updatedAt) || str(shell.snoozedAt);
  return until <= now ? str(shell.snoozedUntil) : '';
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
      await access.dispatch(storage, { type: 'message.dispatch', commandId, threadId, messageId, text: 'Continue where you left off.', attachments: [],
        createdBy: 'user', creationSource: 'web', manualContinuationOfRunId: runId, dispatchMode: { type: 'start_immediately' } }, 'Resume thread');
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
  if (op === 'compact') {
    // onCompactContext: a standalone "/compact" turn; the draft stays local.
    const projection = requireThread(client), threadId = client.threadId;
    if (activeRun(projection)) throw new ClientError('Compacting is unavailable right now');
    requireProvider(client);
    const staged = stagedFor(client), key = client.draftKey;
    for (const command of nextTurnCommands(obj(projection.thread), staged)) {
      const [commandId] = await access.ids(1);
      await access.dispatch(storage, { ...command, commandId, threadId }, 'Change mode');
    }
    const [commandId, messageId] = await access.ids(2);
    await access.dispatch(storage, { type: 'message.dispatch', commandId, threadId, messageId, text: '/compact', attachments: [], createdBy: 'user', creationSource: 'web',
      modelSelection: dispatchSelection(client, client.providerId, client.modelId, client.modelOptions), deliveryIntent: 'auto', dispatchMode: { type: 'start_immediately' } }, 'Compact context');
    clearStaged(client, key);
    return '';
  }
  if (op === 'limit-resume' || op === 'limit-snooze') { await changeLimitRecovery(client, native, storage, op === 'limit-resume' ? 'resume' : 'snooze'); return ''; }
  if (op === 'branch' || op === 'branch-create') return selectBranch(client, native, storage, value, op === 'branch-create');
  if (op === 'stop-background') {
    // handleStopBackgroundWork → interruptThreadTurn without a run: the latest run owns the background work.
    const projection = requireThread(client), threadId = client.threadId;
    const runs = arr(projection.runs), run = activeRun(projection) ?? runs.reduce<Obj | undefined>((latest, entry) => !latest || Number(entry.ordinal) > Number(latest.ordinal) ? entry : latest, undefined);
    if (!run) throw new ClientError('There is no background work to stop.');
    const [commandId] = await access.ids(1);
    await access.dispatch(storage, { type: 'run.interrupt', commandId, threadId, runId: str(run.id) }, 'Stop background work');
    return '';
  }
  if (op === 'queued-key') {
    // The grip's ↑/↓: move before the previous row, or after the next one (QueuedRunsControl onKeyDown).
    if (value !== 'ArrowUp' && value !== 'ArrowDown') return '';
    const queued = queueState(requireThread(client)).queued.map(entry => str(entry.run.id)), index = queued.indexOf(id);
    if (index < 0 || (value === 'ArrowUp' ? index === 0 : index === queued.length - 1)) return '';
    return composerCommand(client, native, storage, 'queued-reorder', id, value === 'ArrowUp' ? queued[index - 1]! : queued[index + 2] ?? '');
  }
  if (op === 'queued-drop') {
    // The grip's drag (QueuedRunsControl onDrop): a move to the insertion line, unless that is where it already is.
    const move = queuedDrop(client, id);
    return move ? composerCommand(client, native, storage, 'queued-reorder', move.runId, move.before) : '';
  }
  if (op === 'queued-remove' || op === 'queued-steer' || op === 'queued-reorder') {
    const projection = requireThread(client), threadId = client.threadId;
    const run = arr(projection.runs).find(entry => entry.id === id && entry.status === 'queued');
    if (!run) throw new ClientError('That queued message already started or was removed.');
    const [commandId] = await access.ids(1);
    if (op === 'queued-remove') await access.dispatch(storage, { type: 'queued-run.cancel', commandId, threadId, runId: id }, 'Remove queued message');
    else if (op === 'queued-steer') {
      const target = activeRun(projection);
      if (!target) throw new ClientError('There is no active run to steer');
      await access.dispatch(storage, { type: 'queued-message.promote-to-steer', commandId, threadId, queuedRunId: id, targetRunId: str(target.id) }, 'Steer queued message');
    } else await access.dispatch(storage, { type: 'queued-run.reorder', commandId, threadId, runId: id, beforeRunId: value || null }, 'Reorder queued message');
    return '';
  }
  throw new ClientError(`Unknown composer action: ${op}`);
}

/** Local composer state (op names follow `cclocal:`). */
export async function composerLocal(client: T3Client, _native: Native, _storage: Files, op: string, id: string, value: string): Promise<string> {
  if (op === 'queued-edit') { beginQueuedEdit(client, id); return ''; }
  if (op === 'env-mode') { setEnvMode(client, value); return ''; }
  if (op === 'previous-worktree') { setEnvMode(client, 'previous'); return ''; }
  if (op === 'attach') return attachFiles(client, _native);
  if (op === 'branch-menu') return branchMenu(client, _native, value);
  // The compact strip's workspace trigger has no context menu (BranchToolbar's MobileRunContextSelector).
  if (op === 'noop') return '';
  if (op === 'usage-limits') { openUsageLimits(client, composerNow(client)); return ''; }
  if (op === 'usage-limits-dismiss') { closeUsageLimits(client); return ''; }
  if (op === 'queued-cancel') { cancelQueuedEdit(client); return ''; }
  if (op === 'queued-attachment-remove') { removeQueuedEditAttachment(client, id); return ''; } // composer-fidelity G12a
  if (op === 'resume-compaction-dismiss') { dismissResumeCompaction(client, id); return ''; }
  if (op === 'dismiss-woke') { await acknowledgeWoke(client, id || client.threadId, _native, value); return ''; }
  throw new ClientError(`Unknown composer action: ${op}`);
}
