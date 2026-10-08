// Pinned365aa87982 use-thread-outbox-drain.ts. Root scheduling is separate.
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import { mobileOutboxConnection, type MobileOutboxConnection } from './mobile-outbox-connection';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { fileStagingLimit } from './shared/composer-editor-files';
import { mobileSessionGrants } from './mobile-grants';
import { mobileModelSelectionUnavailable } from './model-availability';
import { mobileOutboxAcknowledge, mobileOutboxCapture, mobileOutboxConfirmQueued, mobileOutboxRead,
  mobileOutboxSnapshot, type MobileOutboxCapture } from './mobile-outbox';
import { mobileOutboxCreationSendable, mobileOutboxDeliveryAction, mobileOutboxDispatchStep,
  mobileOutboxResolveSettings, type MobileOutboxRecord } from './mobile-outbox-model';
import { mobileOutboxDeliveryComplete, mobileOutboxDeliveryRecover, mobileOutboxDeliveryReserve,
  mobileOutboxDeliveryReserveInline, mobileOutboxDeliverySend, mobileOutboxDeliveryStatus,
  type MobileOutboxDeliveryCompletion, type MobileOutboxDeliveryStatus } from './mobile-outbox-delivery';
import { mobileOutboxInlineLookup, mobileOutboxInlineRecover, mobileOutboxInlineReserve, mobileOutboxInlineSend,
  type MobileOutboxInlineStatus } from './mobile-outbox-inline-delivery';
import { mobileOutboxPrepareAttachments } from './mobile-outbox-upload';
import { mobileOutboxLaunchNativePlan, mobileOutboxMessageNativePlan, mobileOutboxSettingsRequests,
  type MobileOutboxThreadFacts, type MobileOutboxWireOwner } from './mobile-outbox-wire';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';

export interface MobileOutboxForegroundOptions {
  /** Synchronize saved cold receipts before resolving their original command. */
  recover?: boolean;
  /** Explicitly retry this exact final ACK's failed/not-started cleanup. */
  retryCleanupRevision?: number;
}
export interface MobileOutboxForegroundResult {
  status: 'delivered' | 'waiting' | 'retry' | 'recovery-required' | 'edited-after-ack' | 'cleanup-pending';
  reason: string;
  delivery?: MobileOutboxDeliveryStatus;
  inline?: MobileOutboxInlineStatus;
  completion?: MobileOutboxDeliveryCompletion;
}
class Stop extends Error {
  constructor(readonly status: MobileOutboxForegroundResult['status'], reason: string) { super(reason); }
}
const ownerOf = (record: MobileOutboxRecord): MobileOutboxWireOwner => ({ origin: record.origin,
  environmentId: record.environmentId, threadId: record.threadId, messageId: record.messageId, commandId: record.commandId });
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
function stop(status: MobileOutboxForegroundResult['status'], reason: string): never { throw new Stop(status, reason); }

/** One caller-owned pass. Native receipts remain the only retry authority. No
 * timer, retained Native, shared Pending entry, draft restoration or route change. */
export async function mobileOutboxDeliverOne(client: T3Client, handle: Native, input: MobileOutboxWireOwner,
  options: MobileOutboxForegroundOptions = {}): Promise<MobileOutboxForegroundResult> {
  const native = letGoAware(handle), owner = copy(input);
  let connection: MobileOutboxConnection = client, generation = connection.generation, activeOrigin = connection.origin;
  let activeEnvironment = connection.environmentId;
  let delivery: MobileOutboxDeliveryStatus | undefined, inline: MobileOutboxInlineStatus | undefined;
  let completion: MobileOutboxDeliveryCompletion | undefined;
  let capture: MobileOutboxCapture | null = null, configStamp = '', settingsStamp = '';
  const result = (status: MobileOutboxForegroundResult['status'], reason: string): MobileOutboxForegroundResult =>
    ({ status, reason, ...(delivery ? { delivery } : {}), ...(inline ? { inline } : {}), ...(completion ? { completion } : {}) });
  function sameOwner(record: MobileOutboxRecord) {
    if (canonical(ownerOf(record)) !== canonical(owner)) stop('recovery-required', 'The saved command belongs to another pending task.');
  }
  function endpoint() {
    if (connection.generation !== generation || connection.origin !== activeOrigin || connection.environmentId !== activeEnvironment
      || activeEnvironment !== owner.environmentId || connection.connection !== 'connected')
      stop('waiting', 'Connect to this pending task\'s environment before delivery.');
  }
  function row() {
    const snapshot = mobileOutboxSnapshot(client), row = snapshot.rows.find(item => item.record.messageId === owner.messageId);
    if (!snapshot.complete || !snapshot.ownerEpoch) stop('recovery-required', 'Read the complete pending-task inventory before delivery.');
    if (!row) stop('waiting', 'That pending task no longer exists.');
    sameOwner(row.record);
    if (row.held) stop('waiting', 'Finish editing this pending task before delivery.');
    if (row.status !== 'confirmed' || snapshot.outcomes.some(item => item.messageId === owner.messageId
      && ['unknown', 'uncertain'].includes(item.status))) stop('recovery-required', 'Resolve this pending task\'s saved storage outcome.');
    if (capture && canonical(mobileOutboxCapture(client, owner.messageId)) !== canonical(capture))
      stop('waiting', 'The pending task changed during delivery.');
    return row;
  }
  function thread(): MobileOutboxThreadFacts {
    const found = connection.shell.threads.find(item => item.id === owner.threadId);
    if (!found) stop('waiting', 'Refresh this pending task\'s thread before delivery.');
    const selection = obj(found.modelSelection), runtime = str(found.runtimeMode), interaction = str(found.interactionMode, 'default');
    if (!str(selection.instanceId) || !str(selection.model) || !['approval-required', 'auto-accept-edits', 'auto', 'full-access'].includes(runtime)
      || !['default', 'plan'].includes(interaction)) stop('recovery-required', 'The thread settings are incomplete.');
    return { origin: owner.origin, environmentId: owner.environmentId, threadId: owner.threadId,
      modelSelection: copy(selection) as unknown as MobileOutboxThreadFacts['modelSelection'],
      runtimeMode: runtime as MobileOutboxThreadFacts['runtimeMode'], interactionMode: interaction as MobileOutboxThreadFacts['interactionMode'] };
  }
  function settings(record: MobileOutboxRecord) {
    const fallback = record.creation ? { modelSelection: record.modelSelection!, runtimeMode: 'full-access' as const,
      interactionMode: 'default' as const } : thread();
    return mobileOutboxResolveSettings(record, fallback, arr(connection.config.providers).map(provider => ({ instanceId: str(provider.instanceId),
      ...(typeof provider.showInteractionModeToggle === 'boolean' ? { showInteractionModeToggle: provider.showInteractionModeToggle } : {}) })));
  }
  function current() {
    endpoint(); const currentRow = row(), record = currentRow.record;
    if (configStamp && (canonical({ config: connection.config, scopes: connection.scopes }) !== configStamp || !connection.configLive))
      stop('waiting', 'The server settings changed during delivery.');
    const action = mobileOutboxDeliveryAction({ isCreation: !!record.creation,
      threadExists: connection.shell.threads.some(item => item.id === record.threadId), shellStatus: connection.shellLive ? 'live' : 'empty',
      environmentConnected: true, threadBusy: false });
    if (action === 'remove') stop('recovery-required', record.creation
      ? 'This task already exists. Recover its queued draft before removing it.' : 'This thread is gone. Resolve its pending message.');
    if (action !== 'send' || record.creation && !mobileOutboxCreationSendable(record)) stop('waiting', 'This pending task is not ready to send.');
    if (record.creation && !str(connection.shell.projects.find(item => item.id === record.creation!.projectId)?.workspaceRoot)
      && !record.creation.projectCwd) stop(connection.shellLive ? 'recovery-required' : 'waiting', 'Resolve this pending task\'s missing project before delivery.');
    if (settingsStamp && canonical(settings(record)) !== settingsStamp) stop('waiting', 'The message settings changed during delivery.');
    return currentRow;
  }
  function prime(payload?: Obj) {
    const record = current().record;
    if (!connection.configLive) stop('retry', 'Refresh the server configuration before sending.');
    const resolved = settings(record);
    if (mobileModelSelectionUnavailable(connection.config, copy(resolved.modelSelection) as unknown as Obj))
      stop('recovery-required', 'Antigravity model unavailable. Choose another model before delivery.');
    if (payload && (canonical(payload.modelSelection) !== canonical(resolved.modelSelection)
      || record.creation && (payload.runtimeMode !== resolved.runtimeMode || payload.interactionMode !== resolved.interactionMode)))
      stop('recovery-required', 'The reserved command uses earlier message settings. Resolve it before delivery.');
    configStamp = canonical({ config: connection.config, scopes: connection.scopes });
    settingsStamp = canonical(resolved);
  }
  async function access(check: () => void) {
    endpoint(); check();
    const status = await connection.call(native, { op: 'status' }, generation); endpoint(); check();
    if (status.environmentId !== owner.environmentId || status.origin !== activeOrigin
      || (str(status.homeOrigin) || activeOrigin) !== owner.origin) stop('waiting', 'The native connection belongs to another environment.');
    const session = await connection.http(native, '/api/auth/session', generation); endpoint(); check();
    if (!mobileSessionGrants(session, 'orchestration:operate')) stop('waiting', 'This connection cannot send messages.');
  }
  async function durableDelivery(saved: MobileOutboxDeliveryStatus) {
    delivery = saved;
    if (saved.operation && !saved.durable) {
      if (!options.recover) stop('recovery-required', 'Recover this saved command before delivery.');
      delivery = await mobileOutboxDeliveryRecover(native, saved.operation);
    }
    return delivery;
  }
  async function finish(saved: MobileOutboxDeliveryStatus) {
    const receipt = (await durableDelivery(saved)).operation!;
    let retry: number | undefined;
    if (options.retryCleanupRevision !== undefined) {
      if (options.retryCleanupRevision !== receipt.revision || !receipt.cleanup
        || !['failed', 'not-started'].includes(receipt.cleanup.phase))
        stop('cleanup-pending', 'Resolve the exact saved cleanup outcome before retrying.');
      if (receipt.cleanup.outcome && !await mobileOutboxAcknowledge(client, native, owner.messageId, str(receipt.cleanup.mutationId)))
        stop('cleanup-pending', 'Acknowledge the saved cleanup outcome before retrying.');
      retry = options.retryCleanupRevision;
    }
    completion = await mobileOutboxDeliveryComplete(client, native, receipt, retry);
    delivery = { operation: completion.operation, durable: completion.durable };
    return result(completion.cleanup === 'removed' ? 'delivered' : completion.cleanup === 'edited' ? 'edited-after-ack' : 'cleanup-pending',
      completion.cleanup === 'removed' ? '' : 'The server accepted the original message. Its saved cleanup still needs resolution.');
  }
  function resumeGuard() {
    endpoint();
    const snapshot = mobileOutboxSnapshot(client);
    if (!snapshot.complete) stop('recovery-required', 'Read the complete pending-task inventory before retrying.');
    if (snapshot.rows.some(item => item.record.messageId === owner.messageId && item.held)) stop('waiting', 'Finish editing this pending task before retrying.');
  }
  async function sendFinal(saved: MobileOutboxDeliveryStatus, fresh = false): Promise<MobileOutboxForegroundResult> {
    delivery = await durableDelivery(saved); const receipt = delivery.operation!;
    if (receipt.state === 'acknowledged') return finish(delivery);
    if (receipt.state === 'rejected') return result('recovery-required', 'Resolve the rejected message before restoring its draft.');
    if (fresh || receipt.state === 'reserved') {
      prime(receipt.payload);
      const currentRow = current();
      if (currentRow.token !== receipt.rowToken || currentRow.nativeRevision !== receipt.rowRevision
        || canonical(currentRow.record) !== canonical(receipt.record)) return result('recovery-required', 'The reserved message changed before issue.');
    }
    await access(fresh || receipt.state === 'reserved' ? () => { current(); } : resumeGuard);
    delivery = await mobileOutboxDeliverySend(client, native, receipt, false, connection);
    if (delivery.operation!.state === 'acknowledged') return finish(delivery);
    return result(delivery.operation!.state === 'rejected' ? 'recovery-required' : 'retry', 'Resolve the saved command outcome before another attempt.');
  }
  async function sendInline(saved: MobileOutboxInlineStatus, retiredFinal?: number): Promise<MobileOutboxForegroundResult> {
    inline = saved;
    if (!inline.durable) {
      if (!options.recover) return result('recovery-required', 'Recover the saved image request before delivery.');
      inline = await mobileOutboxInlineRecover(native, inline.operation!);
    }
    const receipt = inline.operation!;
    if (receipt.state !== 'acknowledged') {
      if (receipt.state === 'reserved') {
        prime(receipt.template.commandTemplate.payload);
        const currentRow = current();
        if (currentRow.token !== receipt.rowToken || currentRow.nativeRevision !== receipt.rowRevision)
          return result('recovery-required', 'The captured image request changed before issue.');
      }
      await access(receipt.state === 'reserved' ? () => { current(); } : resumeGuard);
      inline = await mobileOutboxInlineSend(client, native, receipt, receipt.state === 'rejected', connection);
      if (inline.operation!.state !== 'acknowledged') return result('retry', 'Resolve the saved image request before another attempt.');
    }
    const acknowledged = inline.operation!;
    prime(acknowledged.template.commandTemplate.payload);
    const currentRow = current();
    if (currentRow.token !== acknowledged.rowToken || currentRow.nativeRevision !== acknowledged.rowRevision
      || canonical(currentRow.record) !== canonical(acknowledged.record))
      return result('recovery-required', 'The images were saved for an earlier version of this pending task.');
    await access(() => { current(); });
    delivery = await mobileOutboxDeliveryReserveInline(client, native, acknowledged, retiredFinal, connection);
    return sendFinal(delivery, true);
  }
  try {
    if (!await mobileOutboxRead(client, native)) return result('recovery-required', 'Read the complete pending-task inventory before delivery.');
    const ids = [owner.commandId, `${owner.commandId}:runtime-mode`, `${owner.commandId}:interaction-mode`];
    const saved: MobileOutboxDeliveryStatus[] = [];
    for (const id of ids) {
      const status = await mobileOutboxDeliveryStatus(native, id);
      if (status.operation) sameOwner(status.operation.record);
      saved.push(status);
    }
    const found = await mobileOutboxInlineLookup(native, owner);
    inline = found.operations.find(item => item.operation!.state !== 'retired');
    let final = saved[0]!;
    if (options.retryCleanupRevision !== undefined && (final.operation?.state !== 'acknowledged'
      || final.operation.revision !== options.retryCleanupRevision))
      return result('cleanup-pending', 'Choose the exact saved final acknowledgement before retrying cleanup.');
    // Terminal journal recovery does not need any live endpoint.
    if (final.operation && ['acknowledged', 'rejected'].includes(final.operation.state)) return await sendFinal(final);
    connection = await mobileOutboxConnection(client, native, owner);
    generation = connection.generation; activeOrigin = connection.origin; activeEnvironment = connection.environmentId;
    if (final.operation && final.operation.state !== 'retired') return await sendFinal(final);
    if (final.operation) final = await durableDelivery(final);
    // The shell may already show a settings command whose ACK was lost. Resolve
    // that saved attempt even when no settings delta is required anymore.
    for (let index = 1; index <= 2; index++) {
      let stage = saved[index]!;
      if (!stage.operation || !['acknowledged', 'issued', 'uncertain'].includes(stage.operation.state)) continue;
      stage = await durableDelivery(stage);
      if (stage.operation!.state !== 'acknowledged') {
        await access(resumeGuard);
        const sent = await mobileOutboxDeliverySend(client, native, stage.operation!, false, connection); delivery = sent; stage = sent;
        if (sent.operation!.state !== 'acknowledged') return result('retry', 'Resolve the saved settings outcome before another attempt.');
      }
      const settled = await mobileOutboxDeliveryComplete(client, native, stage.operation!);
      saved[index] = { operation: settled.operation, durable: settled.durable }; delivery = saved[index];
    }
    // An ACK is never independently retired or replaced, even if its row was edited.
    if (inline) return await sendInline(inline, final.operation?.revision);
    const initial = current(); capture = mobileOutboxCapture(client, owner.messageId);
    if (!capture) return result('waiting', 'That pending task no longer exists.');
    prime();
    const limit = fileStagingLimit(obj(obj(connection.config.environment).capabilities));
    const step = mobileOutboxDispatchStep({ deliveryAction: 'send', fileAttachments: initial.record.attachments.filter(item => item.kind === 'file'),
      serverConfig: { maxFileUploadBytes: limit || undefined } });
    if (step.step === 'restore') return result('recovery-required', step.reason);
    await access(() => { current(); });
    if (!await mobileOutboxConfirmQueued(client, native, capture)) return result('waiting', 'The pending task changed before delivery.');
    current();
    if (initial.record.creation) {
      if (saved.slice(1).some(item => item.operation && item.operation.state !== 'retired'))
        return result('recovery-required', 'A new task cannot adopt existing-thread settings commands.');
    } else for (let index = 1; index <= 2; index++) {
      const record = current().record;
      const requests = mobileOutboxSettingsRequests(record, { ...owner, config: connection.config, attachments: [] }, thread());
      if (requests.status !== 'ready') return result('recovery-required', requests.status === 'blocked' ? requests.reason : 'Refresh the thread settings.');
      const request = requests.value.find(item => item.payload.commandId === ids[index]);
      let stage = saved[index]!;
      if (stage.operation && stage.operation.state !== 'retired') {
        delivery = stage;
        if (request && (stage.operation.method !== request.method || canonical(stage.operation.payload) !== canonical(request.payload)))
          return result('recovery-required', 'These settings already have a different saved command with the same identity.');
        if (!request && stage.operation.state !== 'acknowledged')
          return result('recovery-required', 'Resolve the saved settings attempt before following the current thread settings.');
        stage = await durableDelivery(stage); current();
      } else if (request) {
        if (stage.operation) { stage = await durableDelivery(stage); current(); }
        stage = await mobileOutboxDeliveryReserve(client, native, capture, request, stage.operation?.revision, connection);
        stage = await durableDelivery(stage); current();
      } else continue;
      if (stage.operation!.state !== 'acknowledged') {
        const sent = await mobileOutboxDeliverySend(client, native, stage.operation!, stage.operation!.state === 'rejected', connection);
        delivery = sent; stage = sent; current();
        if (sent.operation!.state !== 'acknowledged') return result('retry', 'Retry the saved settings command before preparing attachments.');
      }
      await mobileOutboxDeliveryComplete(client, native, stage.operation!); current();
    }
    const prepared = await mobileOutboxPrepareAttachments(client, native, capture, connection);
    if (prepared.status !== 'ready') return result(prepared.status === 'abandoned' ? 'waiting' : 'recovery-required', prepared.reason);
    capture = prepared.capture; const record = current().record;
    const facts = { origin: owner.origin, environmentId: owner.environmentId, config: connection.config, attachments: prepared.attachments };
    let branch = '';
    if (record.creation?.workspaceMode === 'worktree') {
      const [id] = await client.ids(native, 1); current();
      const token = id!.toLowerCase().replace(/[^0-9a-f]/g, '').slice(0, 8);
      if (token.length !== 8) return result('recovery-required', 'Could not allocate a worktree branch.');
      branch = `t3/${token}`;
    }
    let plan = record.creation ? mobileOutboxLaunchNativePlan(record, facts, branch)
      : mobileOutboxMessageNativePlan(record, facts, thread(), null);
    if (plan.status === 'needs-projection') {
      const projection = await connection.request(native, 'orchestration.getThreadProjection', { threadId: owner.threadId }, generation);
      current(); plan = mobileOutboxMessageNativePlan(record, facts, thread(), projection);
    }
    if (plan.status === 'blocked' || plan.status === 'needs-projection')
      return result('recovery-required', plan.status === 'blocked' ? plan.reason : 'Refresh this pending task\'s thread projection.');
    current();
    if (plan.status === 'needs-inline-reservation') {
      const [id] = await client.ids(native, 1); current();
      inline = await mobileOutboxInlineReserve(client, native, capture, id!, plan.value, connection);
      return await sendInline(inline, final.operation?.revision);
    }
    delivery = await mobileOutboxDeliveryReserve(client, native, capture, plan.value, final.operation?.revision, connection);
    return await sendFinal(delivery, true);
  } catch (error) {
    if (letGo(error)) throw error;
    return result(error instanceof Stop ? error.status : error instanceof ClientError && error.kind === 'stale' ? 'waiting' : 'recovery-required',
      error instanceof Error ? error.message : String(error));
  }
}
