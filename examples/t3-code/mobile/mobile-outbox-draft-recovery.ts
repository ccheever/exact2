// Pinned365aa87982 flush-before-remove recovery, over existing preference/journal owners.
// @ref llp/1109.005-composer-and-transcript.decision.md#terminal-draft-attachment-handoff
import type { MobileDraftClient } from './mobile-draft-recovery';
import { obj, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobilePendingTaskEditorsSnapshot } from './mobile-pending-task-state';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxCapture, mobileOutboxHold, mobileOutboxReleaseHold,
  mobileOutboxPrepareRemoval, mobileOutboxResumeRemoval, mobileOutboxAcknowledge } from './mobile-outbox';
import { mobileOutboxDeliveryStatus, mobileOutboxDeliveryRecover, type MobileOutboxDeliveryReceipt } from './mobile-outbox-delivery';
import { mobileOutboxDraftRecoveryBlocked, mobileOutboxDraftHandoffDecode, mobileOutboxDraftHandoffStatus, mobileOutboxDraftHandoffComplete,
  type MobileOutboxDraftHandoff } from './mobile-outbox-draft-handoff';
import { mobileOutboxRecoveryDraftPrepare, mobileOutboxRecoveryDraftAdopt, mobileOutboxRecoveryDraftCapture,
  mobileOutboxRecoveryDraftRollback } from './mobile-outbox-recovery-draft';

const busy = new WeakSet<MobileDraftClient>();
const copy = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const field = 'mobileOutboxDraftHandoffs';
const identity = (value: MobileOutboxWireOwner) => canonical([value.origin, value.environmentId, value.threadId, value.messageId, value.commandId]);
const contextOwner = (value: MobileOutboxWireOwner) => `outbox:${JSON.stringify([value.origin, value.environmentId, value.commandId])}`;
function saved(client: MobileDraftClient): Obj {
  const value = obj(client.local)[field];
  if (value === undefined) return {};
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new ClientError('Saved draft recovery is unreadable. Keep the original task.', 'retained');
  return value;
}
function set(client: MobileDraftClient, id: string, proof: MobileOutboxDraftHandoff | null) {
  const next = { ...saved(client) }; if (proof) next[id] = copy(proof) as unknown as Obj; else delete next[id];
  Object.assign(client.local, { [field]: next }); client.revision++;
}
export interface MobileOutboxDraftRecoveryResult { status: 'recovered' | 'retained' | 'stale'; draftKey: string; reason: string }
export { mobileOutboxDraftRecoveryBlocked } from './mobile-outbox-draft-handoff';
/** One invocation owns handles; saved proofs own retries. No provider requests. */
export async function mobileOutboxDraftRecover(client: MobileDraftClient, handle: Native | null | undefined, storage: Files,
  input: { owner: MobileOutboxWireOwner; current(): boolean }): Promise<MobileOutboxDraftRecoveryResult> {
  const owner = copy(input.owner), heldBy = `draft-recovery:${identity(owner)}`;
  let key = '';
  let releaseUnpublished: (() => Promise<unknown>) | null = null;
  const reply = (status: MobileOutboxDraftRecoveryResult['status'], reason = '') => ({ status, draftKey: key, reason });
  if (busy.has(client)) return reply('retained', 'Another draft recovery is already running.');
  busy.add(client);
  const initial = canonical([client.origin, client.environmentId, client.generation, client.threadId, client.threadEpoch, client.draftKey]);
  const check = () => {
    if (!input.current() || initial !== canonical([client.origin, client.environmentId, client.generation, client.threadId, client.threadEpoch, client.draftKey])
      || owner.origin !== mobileQueuedEditOrigin(client) || owner.environmentId !== client.environmentId)
      throw new ClientError('The recovery destination changed. Reopen its saved status.', 'superseded');
    const editors = mobilePendingTaskEditorsSnapshot(client);
    if (!client.preferencesLoaded || !editors.ready) throw new ClientError('Read complete saved draft ownership first.');
    if (editors.markers.some(marker => identity(marker.owner) === identity(owner))) throw new ClientError('Finish the saved pending editor before automatic draft recovery.');
  };
  try {
    check(); if (!handle?.available) throw new ClientError('Open T3 Code on iPhone or iPad to recover this draft.');
    const base = letGoAware(handle);
    const native: Native = { available: true, watch(topic) { check(); base.watch(topic); }, async later(request) {
      check(); const result = await base.later(request); check(); return result;
    } };
    const persist = async () => { check(); await client.persist(storage); check(); };
    const read = async () => { if (!await mobileOutboxRead(client, native)) throw new ClientError('Read complete queued task ownership before recovery.'); check(); };
    await read();
    let terminal = await mobileOutboxDeliveryStatus(native, owner.commandId); check();
    if (!terminal.operation || identity(terminal.operation.record) !== identity(owner)) return reply('retained', 'The original terminal receipt is missing or belongs to another task.');
    if (!terminal.durable) { terminal = await mobileOutboxDeliveryRecover(native, terminal.operation); check(); }
    const receipt = terminal.operation!;
    const completed = await mobileOutboxDraftHandoffStatus(native, receipt); check();
    // A lost completion reply is resolved before considering local target content.
    let proof = completed.handoff;
    if (proof) {
      key = proof.draftKey;
      if (!completed.durable) await mobileOutboxDraftHandoffComplete(native, receipt, proof);
    } else {
      const raw = saved(client)[owner.commandId];
      proof = raw === undefined ? null : mobileOutboxDraftHandoffDecode(raw, receipt);
      if (!proof) {
        if (receipt.stage !== 'start-turn' || !(receipt.state === 'rejected' || receipt.state === 'acknowledged'
          && receipt.record.creation && receipt.cleanup?.phase === 'edited'))
          return reply('retained', 'Resolve the original send and its exact cleanup before restoring a draft.');
        if (client.local.pending[owner.environmentId]) return reply('retained', 'Resolve the active send before restoring another draft.');
        const row = mobileOutboxSnapshot(client).rows.find(row => row.record.messageId === owner.messageId);
        if (!row || row.status !== 'confirmed' || row.nativeRevision === null || row.held || identity(row.record) !== identity(owner))
          return reply('retained', 'The queued task changed or has an open editor.');
        const capture = mobileOutboxCapture(client, owner.messageId);
        if (!capture || !await mobileOutboxHold(client, native, capture, heldBy)) return reply('retained', 'The queued task could not be held for recovery.');
        releaseUnpublished = () => mobileOutboxReleaseHold(client, base, owner.messageId, heldBy);
        await read();
        const current = mobileOutboxSnapshot(client).rows.find(row => row.record.messageId === owner.messageId);
        if (!current || current.token !== row.token || current.nativeRevision !== row.nativeRevision || canonical(current.record) !== canonical(row.record))
          throw new ClientError('The queued task changed before recovery.', 'stale');
        const change = mobileOutboxRecoveryDraftPrepare(client, row.record, receipt.state === 'rejected' ? 'rejected' : 'accepted-edits'); key = change.key;
        if (mobileOutboxDraftRecoveryBlocked(client, key)) return reply('retained', 'The destination has an unfinished recovery.');
        const request = mobileOutboxPrepareRemoval(client, owner.messageId, { expectedToken: row.token, expectedRevision: row.nativeRevision });
        // Rollback evidence lives with this existing context owner, not in another store.
        obj(obj(change.after.recovered)[contextOwner(owner)]).recoveryBefore = { draft: change.before, terminals: change.terminalBefore };
        proof = mobileOutboxDraftHandoffDecode({ version: 1, operationId: owner.commandId, receiptRevision: receipt.revision,
          record: row.record, request, draftKey: key, draft: change.after }, receipt);
        if (!mobileOutboxRecoveryDraftAdopt(client, change)) throw new ClientError('The destination changed before its recovered content was adopted.');
        set(client, owner.commandId, proof); releaseUnpublished = null;
      }
      key = proof.draftKey;
      // Persist even on replay: a previous attempt may have failed before admission.
      await persist();
      const capture = mobileOutboxCapture(client, owner.messageId);
      if (capture && !await mobileOutboxHold(client, native, capture, heldBy)) throw new ClientError('The recovery hold could not be restored.');
      const outcome = await mobileOutboxResumeRemoval(client, native, proof.request, heldBy); check();
      if (outcome.status === 'unknown' || outcome.status === 'uncertain') return reply('retained', outcome.message || 'The exact saved removal is still unresolved.');
      if (outcome.status !== 'committed') {
        const previous = obj(obj(obj(proof.draft.recovered)[contextOwner(owner)]).recoveryBefore);
        if (previous.draft && previous.terminals) mobileOutboxRecoveryDraftRollback(client, { key, origin: owner.origin,
          environmentId: owner.environmentId, before: obj(previous.draft), after: proof.draft, terminalBefore: obj(previous.terminals) });
        set(client, owner.commandId, null); await persist();
        await mobileOutboxAcknowledge(client, native, owner.messageId, proof.request.mutationId);
        await mobileOutboxReleaseHold(client, native, owner.messageId, heldBy);
        return reply('stale', outcome.message || 'The queued task changed; newer content has been retained.');
      }
      if (canonical(outcome.removed) !== canonical(proof.record) || outcome.record !== null)
        throw new ClientError('The saved removal does not match this recovery.');
      // Newer typing is certified as it is, never overwritten by the captured merge.
      proof = mobileOutboxDraftHandoffDecode({ ...proof, draft: mobileOutboxRecoveryDraftCapture(client, key) }, receipt);
      set(client, owner.commandId, proof); await persist();
      await mobileOutboxDraftHandoffComplete(native, receipt, proof); check();
    }
    // Native completion is the durable anti-replay authority. Do not re-adopt its draft.
    const marker = obj(obj(obj(client.local).mobileRecoveredDrafts)[contextOwner(owner)]);
    if (marker.key === key) { marker.nativeRetired = true; delete marker.recoveryBefore; }
    set(client, owner.commandId, proof); await persist();
    await read();
    await mobileOutboxAcknowledge(client, native, owner.messageId, proof.request.mutationId);
    const answer = await bridgeReply(native, { op: 'mobileOutbox', action: 'completeRemoval', messageId: owner.messageId, mutationId: proof.request.mutationId });
    if (!answer.ok || obj(answer.value).completed !== true && obj(answer.value).absent !== true) throw new ClientError('The recovered draft is safe, but old attachment cleanup is still pending.');
    await mobileOutboxReleaseHold(client, native, owner.messageId, heldBy);
    await read();
    set(client, owner.commandId, null);
    try { await persist(); } catch (error) { set(client, owner.commandId, proof); throw error; }
    return reply('recovered');
  } catch (error) {
    if (letGo(error)) { releaseUnpublished = null; throw error; }
    return reply('retained', error instanceof Error ? error.message : String(error));
  } finally {
    if (releaseUnpublished) try { await releaseUnpublished(); } catch { /* Exact native hold remains recoverable on restart. */ }
    busy.delete(client);
  }
}
