// Pinned365aa87982 queued-run-edit/use-thread-composer-state. Real server protocol,
// dedicated mobile content, native durable acknowledgment; no ordinary finishPending.
// @ref llp/1106.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import { mobileClient, mobileNative } from './client';
import { mobileModelSelectionReady } from './model-availability';
import type { T3Client } from './shared/client';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, nativeFiles, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { queueState } from './shared/composer-controls-queue';
import { mobileSessionGrants } from './environment-detail';
import { mobileQueueCurrent, mobileQueueOwner } from './queue';
import { mobileAdoptRecoveredDraft, mobileRetireRecoveredDraft } from './mobile-draft-recovery';
import { mobileQueuedEditUpload, queuedEditResolvePayload } from './queued-edit-upload';
import { mobileQueuedEditCurrent, mobileQueuedEditLookup, mobileQueuedEditPersist, queuedEditClone, queuedEditEndMemory,
  queuedEditNative, queuedEditOperation, queuedEditRecord, queuedEditRefreshOrigin, mobileQueuedEditOrigin, queuedEditState, queuedEditThreadKey, type MobileQueuedEditSession, type MobileQueuedEditOperation } from './queued-edit-state';

const busy = new WeakSet<T3Client>();
const modelReady = (client: T3Client) => mobileModelSelectionReady(client.config, { instanceId: client.providerId, model: client.modelId });
const unresolved = (operation: MobileQueuedEditOperation) => ['reserved', 'issued', 'uncertain'].includes(operation.state);
const forSession = (owner: string, client: T3Client) => [...queuedEditState(client).operations.values()].find(op => op.owner === owner);
function notice(client: T3Client, message: string) { queuedEditState(client).notice = message; client.revision++; }
function nativeHandle(input: Native | null | undefined): Native {
  if (!input?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to edit queued messages.');
  return letGoAware(mobileNative(input));
}
function errorMessage(error: unknown) { return error instanceof Error ? error.message : 'Could not save the queued message'; }
function adopted(raw: unknown, client: T3Client) {
  const operation = queuedEditOperation(raw);
  if (operation) {
    const state = queuedEditState(client), prior = state.operations.get(operation.operationId);
    if (!prior || prior.revision <= operation.revision) { state.operations.set(operation.operationId, operation); state.operationEpoch++; }
  }
  return operation;
}
export function mobileQueuedEditPresentation(client: T3Client = mobileClient) {
  const edit = mobileQueuedEditCurrent(client), state = queuedEditState(client);
  const pending = [...state.operations.values()].find(op => op.environmentId === client.environmentId && op.origin === mobileQueuedEditOrigin(client)
    && op.payload.threadId === client.threadId && unresolved(op));
  const running = !!edit?.saving || busy.has(client), saving = running || !!pending;
  return { editing: !!edit, saving, owner: edit?.owner ?? '', runId: edit?.runId ?? '', text: edit?.text ?? '',
    canSave: !!edit && !saving && !pending && client.writable && !client.pending && !client.busy
      && !!edit.text.trim() && modelReady(client),
    canCancel: !!edit && !saving && !pending, error: state.notice, uncertain: !!pending,
    canRetry: !!pending && !running && client.writable && !client.pending && !client.busy, pendingId: pending?.operationId ?? '' };
}
/** Queue tickets bind the actual selected visit/run. Begin stores a new unique
 * session before exposing it; reopening a row never resets an existing edit. */
export async function mobileQueuedEditBegin(ticketJSON: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const result = (message = '', dismiss = false) => ({ revision: client.revision, message, dismiss });
  if (busy.has(client) || client.pending || client.busy) return result('Wait for the current operation before editing the queue.');
  let ticket: Obj; try { ticket = obj(JSON.parse(ticketJSON)); } catch { return result('That queued message is no longer available.'); }
  const native = nativeHandle(nativeInput), owner = mobileQueueOwner(client), generation = client.generation;
  const runId = str(ticket.runId), entry = queueState(client.projection).queued.find(row => row.run.id === runId);
  const message = arr(client.projection.messages).find(item => item.id === entry?.run.userMessageId);
  const current = () => mobileQueueOwner(client) === owner && mobileQueueCurrent(str(ticket.owner), str(ticket.visit), client)
    && queueState(client.projection).queued.some(row => row.run.id === runId && row.run.userMessageId === message?.id);
  if (!entry || !message || ticket.owner !== owner || !current()) return result('That queued message already started or was removed.');
  const prior = mobileQueuedEditCurrent(client);
  if (prior?.runId === runId) return result('', true);
  if (prior?.saving || prior && forSession(prior.owner, client) && unresolved(forSession(prior.owner, client)!)) return result('Resolve the queued edit before opening another.');
  busy.add(client); if (prior) queuedEditState(client).sessions.set(prior.owner, { ...prior, saving: true });
  try {
    await queuedEditRefreshOrigin(native, client);
    if (!current()) throw new ClientError('The selected queue changed.', 'superseded');
    const [session] = await client.ids(native, 1);
    if (!current()) throw new ClientError('The selected queue changed.', 'superseded');
    if (prior) {
      await mobileQueuedEditPersist(prior.owner, native, client);
      for (const operation of [...queuedEditState(client).operations.values()]) if (operation.owner === prior.owner) await retireOperation(operation, native, client);
      await queuedEditNative(native, { action: 'cleanup', owner: prior.owner, editorRevision: prior.revision });
      queuedEditEndMemory(prior.owner, client);
      if (!current()) throw new ClientError('The selected queue changed.', 'superseded');
    }
    const record: MobileQueuedEditSession = { owner: JSON.stringify([mobileQueuedEditOrigin(client), client.environmentId, client.threadId, session]),
      draftKey: `${client.environmentId}:${client.threadId}~queued-edit~${runId}`, origin: mobileQueuedEditOrigin(client),
      environmentId: client.environmentId, threadId: client.threadId, projectId: client.projectId, generation, session: session!, revision: 1,
      runId, messageId: str(message.id), text: str(message.text), context: message.context ? queuedEditClone(obj(message.context)) : undefined,
      attachments: [], existingAttachments: queuedEditClone(arr(message.attachments)), saving: false };
    const state = queuedEditState(client); state.sessions.set(record.owner, record);
    try { await mobileQueuedEditPersist(record.owner, native, client); }
    catch (error) { state.sessions.delete(record.owner); throw error; }
    if (!current()) { state.sessions.delete(record.owner); throw new ClientError('The selected queue changed.', 'superseded'); }
    state.active.set(queuedEditThreadKey(record.environmentId, record.threadId), record.owner); notice(client, '');
    return result('', true);
  } catch (error) { if (letGo(error)) throw error; const message = errorMessage(error); notice(client, message); return result(message); }
  finally { if (prior) { const live = mobileQueuedEditLookup(prior.owner, client); if (live) queuedEditState(client).sessions.set(prior.owner, { ...live, saving: false }); } busy.delete(client); client.revision++; }
}
export async function mobileQueuedEditCancel(owner: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const result = (message = '', cancelled = false) => ({ revision: client.revision, message, cancelled });
  const edit = mobileQueuedEditLookup(owner, client), operation = forSession(owner, client);
  if (!edit) return result('That queued edit has ended.');
  if (edit.saving || busy.has(client) || operation && unresolved(operation)) return result('Wait for the queued message to finish saving.');
  busy.add(client); queuedEditState(client).sessions.set(owner, { ...edit, saving: true });
  try {
    await queuedEditNative(nativeInput, { action: 'cleanup', owner, editorRevision: edit.revision,
      ...(operation ? { operationId: operation.operationId, operationRevision: operation.revision } : {}) });
    if (mobileQueuedEditLookup(owner, client)?.revision !== edit.revision) throw new ClientError('The queued edit changed.', 'superseded');
    queuedEditEndMemory(owner, client); if (operation) forgetOperation(operation.operationId, client);
    notice(client, ''); return result('', true);
  } catch (error) { if (letGo(error)) throw error; const message = errorMessage(error); notice(client, message); return result(message); }
  finally { const live = mobileQueuedEditLookup(owner, client); if (live) queuedEditState(client).sessions.set(owner, { ...live, saving: false }); busy.delete(client); client.revision++; }
}
/** Admission permits only the captured editor and exact current queued message.
 * Every native await uses this guard; model/runtime settings remain thread-owned. */
function saveNative(client: T3Client, owner: string, base: Native) {
  const initial = mobileQueuedEditLookup(owner, client);
  if (!initial || mobileQueuedEditCurrent(client)?.owner !== owner) throw new ClientError('That queued edit is not selected.', 'superseded');
  const generation = client.generation;
  const check = () => {
    const edit = mobileQueuedEditLookup(owner, client);
    if (!edit || mobileQueuedEditCurrent(client)?.owner !== owner || client.generation !== generation || client.pending || client.busy) throw new ClientError('The selected queued edit changed.', 'superseded');
    if (!client.writable) throw new ClientError('This connection cannot save queued messages.');
    if (!modelReady(client)) throw new ClientError('Model unavailable. Open model settings.');
    if (!queueState(client.projection).queued.some(row => row.run.id === initial.runId && row.run.userMessageId === initial.messageId)) throw new ClientError('That message already started or was removed.');
  };
  const native: Native = { available: base.available, watch: topic => base.watch(topic), later: async input => {
    check(); const answer = await base.later(input); check(); return answer;
  } };
  check(); return { native, check, generation };
}
export async function mobileQueuedEditSave(owner: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const result = (message = '', saved = false, errorTitle = '') => ({ revision: client.revision, message, saved, errorTitle });
  const edit = mobileQueuedEditLookup(owner, client);
  if (!edit || edit.saving || busy.has(client)) return result('Wait for the queued edit to finish.');
  if (!edit.text.trim()) return result('A queued message cannot be left empty.', false, 'Add a message');
  if (forSession(owner, client) && unresolved(forSession(owner, client)!)) return result('Resolve the previous queued edit before saving again.');
  const base = nativeHandle(nativeInput), scope = saveNative(client, owner, base), state = queuedEditState(client);
  state.sessions.set(owner, { ...edit, saving: true }); busy.add(client); let operation: MobileQueuedEditOperation | null = null;
  try {
    for (const prior of [...state.operations.values()]) if (prior.owner === owner && !unresolved(prior)) await retireOperation(prior, base, client);
    const grants = await client.http(scope.native, '/api/auth/session');
    if (!mobileSessionGrants(grants, 'orchestration:operate')) throw new ClientError('This connection cannot save queued messages.');
    const prepared = await mobileQueuedEditUpload(owner, scope.native, client);
    scope.check(); const captured = mobileQueuedEditLookup(owner, client)!;
    const payload = queuedEditResolvePayload(captured, prepared);
    const [commandId] = await client.ids(scope.native, 1); scope.check();
    await mobileQueuedEditPersist(owner, scope.native, client); scope.check();
    scope.check();
    operation = adopted(await queuedEditNative(base, { action: 'reserve', owner, editorRevision: captured.revision,
      operationId: commandId, expectedEnvironmentId: captured.environmentId, generation: scope.generation,
      method: 'orchestration.dispatchCommand', payload: { type: 'queued-run.edit', commandId, threadId: captured.threadId,
        runId: captured.runId, text: captured.text.trim(), ...payload } }), client);
    if (!operation) throw new ClientError('The queued edit journal did not reserve the operation.');
    scope.check();
    // The native owner records acknowledgment even if the queue run starts or
    // the screen changes while the wire reply arrives. Do not reject its ack.
    const sent = await queuedEditNative(base, { action: 'send', operationId: operation.operationId, revision: operation.revision,
      expectedEnvironmentId: captured.environmentId, generation: scope.generation });
    operation = adopted(sent.operation, client) ?? operation;
    if (operation.state !== 'acknowledged') throw new ClientError('The server has not confirmed the queued edit.', 'QueuedEdit', true);
    await finishAcknowledged(operation, base, client); notice(client, ''); return result('', true);
  } catch (error) {
    // Read only; never send automatically. Native owns outcome after wire entry.
    try { await readOperations(base, client); } catch { /* The existing reservation remains the visible recovery owner. */ }
    for (const prior of [...state.operations.values()]) if (prior.owner === owner && (prior.state === 'reserved' || prior.state === 'rejected')) {
      try { await retireOperation(prior, base, client); } catch { /* Retain a visible reservation if local retirement fails. */ }
    }
    if (letGo(error)) throw error;
    const message = errorMessage(error); notice(client, message); return result(message, false, 'Could not save the queued message');
  } finally {
    const live = mobileQueuedEditLookup(owner, client); if (live) state.sessions.set(owner, { ...live, saving: false });
    busy.delete(client); client.revision++;
  }
}
function forgetOperation(id: string, client: T3Client) { const state = queuedEditState(client); state.operations.delete(id); state.operationEpoch++; }
async function retireOperation(operation: MobileQueuedEditOperation, native: Native, client: T3Client) {
  const edit = mobileQueuedEditLookup(operation.owner, client);
  await queuedEditNative(native, { action: 'retire', owner: operation.owner, editorRevision: edit?.revision ?? operation.editorRevision,
    operationId: operation.operationId, revision: operation.revision });
  forgetOperation(operation.operationId, client);
}
async function readOperations(native: Native, client: T3Client) {
  const state = queuedEditState(client), serial = ++state.readSerial, epoch = state.operationEpoch;
  const reply = await queuedEditNative(native, { action: 'read' });
  if (serial !== state.readSerial || epoch !== state.operationEpoch) return reply;
  const operations = arr(reply.operations).map(queuedEditOperation).filter((op): op is MobileQueuedEditOperation => !!op);
  const ids = new Set(operations.map(op => op.operationId));
  for (const id of state.operations.keys()) if (!ids.has(id)) state.operations.delete(id);
  for (const operation of operations) adopted(operation, client);
  state.hydrated = true; return reply;
}
async function finishAcknowledged(operation: MobileQueuedEditOperation, native: Native, client: T3Client) {
  const state = queuedEditState(client), edit = mobileQueuedEditLookup(operation.owner, client);
  if (edit && edit.revision !== operation.editorRevision) { await retireOperation(operation, native, client); return; }
  await queuedEditNative(native, { action: 'cleanup', owner: operation.owner, editorRevision: operation.editorRevision,
    operationId: operation.operationId, operationRevision: operation.revision });
  if (!edit || mobileQueuedEditLookup(operation.owner, client)?.revision === operation.editorRevision) queuedEditEndMemory(operation.owner, client);
  forgetOperation(operation.operationId, client);
}
export async function mobileQueuedEditRetry(operationId: string, nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  const result = (message = '', saved = false) => ({ revision: client.revision, message, saved });
  if (busy.has(client) || client.pending || client.busy) return result('Wait for the current operation before retrying.');
  const base = nativeHandle(nativeInput); busy.add(client);
  try {
    await queuedEditRefreshOrigin(base, client);
    await readOperations(base, client);
    const operation = queuedEditState(client).operations.get(operationId), generation = client.generation, environmentId = client.environmentId, threadId = client.threadId, origin = client.origin;
    if (!operation || !unresolved(operation) || operation.environmentId !== environmentId || operation.origin !== mobileQueuedEditOrigin(client) || operation.payload.threadId !== client.threadId) throw new ClientError('That queued edit is not selected.', 'superseded');
    const session = await client.http(base, '/api/auth/session');
    if (!mobileSessionGrants(session, 'orchestration:operate') || !client.writable) throw new ClientError('This connection cannot save queued messages.');
    if (client.generation !== generation || client.environmentId !== environmentId || client.threadId !== threadId || client.origin !== origin || client.pending || client.busy) throw new ClientError('The connection changed.', 'superseded');
    const sent = await queuedEditNative(base, { action: 'send', operationId, revision: operation.revision, generation, expectedEnvironmentId: environmentId });
    const acknowledged = adopted(sent.operation, client);
    if (!acknowledged || acknowledged.state !== 'acknowledged') throw new ClientError('The server has not confirmed the queued edit.');
    await finishAcknowledged(acknowledged, base, client); notice(client, ''); return result('', true);
  } catch (error) {
    try { await readOperations(base, client); } catch { /* Keep current pending identity. */ }
    if (letGo(error)) throw error; const message = errorMessage(error); notice(client, message); return result(message);
  } finally { busy.delete(client); client.revision++; }
}
/** Hydration never reopens a persisted editor or starts a server command. */
export async function mobileQueuedEditRefresh(nativeInput: Native | null | undefined, client: T3Client = mobileClient) {
  if (!nativeInput?.available || busy.has(client)) return { revision: client.revision, message: '' };
  const native = nativeHandle(nativeInput), state = queuedEditState(client); busy.add(client);
  try {
    await queuedEditRefreshOrigin(native, client);
    const reply = await readOperations(native, client);
    const completed = new Set<string>();
    for (const operation of [...state.operations.values()]) if (operation.state === 'acknowledged') {
      await finishAcknowledged(operation, native, client);
      if (!state.sessions.has(operation.owner)) completed.add(operation.owner);
    }
    // Orphan records are from a previous process/abandoned Begin. Never reactivate them.
    for (const wrapper of arr(reply.records)) {
      const record = obj(wrapper.record), owner = str(wrapper.owner);
      if (completed.has(owner)) continue;
      if (str(record.recoveryKey)) {
        const recovery = { ...record, saving: false } as unknown as MobileQueuedEditSession;
        state.recoveries.set(recovery.recoveryKey!, recovery);
        if (recovery.origin === mobileQueuedEditOrigin(client) && recovery.environmentId === client.environmentId) await adoptRecovery(recovery, native, client);
        continue;
      }
      if (state.sessions.has(owner) || [...state.operations.values()].some(op => op.owner === owner)) continue;
      await queuedEditNative(native, { action: 'cleanup', owner, editorRevision: Number(wrapper.revision ?? record.revision) });
    }
    for (const operation of [...state.operations.values()]) if (operation.state === 'rejected') await retireOperation(operation, native, client);
    if (arr(reply.releases).length) await queuedEditNative(native, { action: 'release' });
    const edit = mobileQueuedEditCurrent(client);
    if (edit && !edit.saving && !forSession(edit.owner, client) && client.ready
      && !queueState(client.projection).queued.some(row => row.run.id === edit.runId)) await recoverLostRun(edit, native, client);
    return { revision: client.revision, message: '' };
  } catch (error) { if (letGo(error)) throw error; const message = errorMessage(error); notice(client, message); return { revision: client.revision, message }; }
  finally { busy.delete(client); }
}
async function recoverLostRun(edit: MobileQueuedEditSession, native: Native, client: T3Client) {
  const recovery = { ...edit, recoveryKey: queuedEditThreadKey(edit.environmentId, edit.threadId), revision: edit.revision + 1, saving: true };
  const state = queuedEditState(client); state.sessions.set(edit.owner, recovery);
  // First make the transfer intent durable. Neither ordinary adoption nor cleanup
  // is permitted until a restart can recognize this exact recovery owner.
  try {
    await queuedEditNative(native, { action: 'cas', owner: edit.owner, revision: recovery.revision, record: queuedEditRecord(recovery) });
    state.recoveries.set(recovery.recoveryKey, recovery);
    await adoptRecovery(recovery, native, client);
  } finally { const live = mobileQueuedEditLookup(edit.owner, client); if (live) state.sessions.set(edit.owner, { ...live, saving: false }); }
}
async function adoptRecovery(edit: MobileQueuedEditSession, native: Native, client: T3Client) {
  const outcome = mobileAdoptRecoveredDraft(client, edit);
  if (outcome !== 'blocked') await client.persist(nativeFiles(native));
  // Cleanup reads the durably written ordinary attachment refs under the same
  // native lock; transferred bytes survive while abandoned bytes are released.
  await queuedEditNative(native, { action: 'cleanup', owner: edit.owner, editorRevision: edit.revision });
  if (outcome !== 'blocked') { mobileRetireRecoveredDraft(client, edit.owner); await client.persist(nativeFiles(native)); }
  queuedEditEndMemory(edit.owner, client); queuedEditState(client).recoveries.delete(edit.recoveryKey!);
  notice(client, outcome !== 'blocked' ? 'That message already started. Your edit is back in the composer.' : 'That message already started, so the edit was discarded.');
}
