// Pinned365aa87982 NewTaskDraftScreen local enqueue; native owns interrupted-write recovery.
// @ref llp/1109.005-composer-and-transcript.decision.md#durable-draft-capture-transfer
import type { MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskSubmitRecover, type MobileNewTaskSubmitResult } from './new-task-submit';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxTransferLookup, mobileOutboxTransferStatus,
  mobileOutboxReleaseFailedTransfer, type MobileOutboxOutcome } from './mobile-outbox';
import { mobileOutboxTransferCanonical as canonical, type MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
import { mobileNewTaskTransferGuardAcquire, mobileNewTaskTransferGuardBusy, mobileNewTaskTransferGuardRelease } from './new-task-transfer-guard';
import { ClientError, type Files, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { obj } from './shared/domain';

export interface MobileNewTaskTransferRecoveryInput { draftKey: string; current(): boolean }
export type MobileNewTaskTransferRecoveryOperation = 'commit' | 'rollback' | 'retry' | 'finish' | 'release';
export interface MobileNewTaskTransferRecoveryItem {
  transferId: string; draftKey: string; threadId: string; messageId: string; commandId: string;
  state: MobileOutboxTransferClaim['state'] | 'unknown'; message: string;
  actions: Array<{ kind: MobileNewTaskTransferRecoveryOperation; label: string; key: string }>;
}
export interface MobileNewTaskTransferRecoveryView {
  revision: number; draftKey: string; complete: boolean; blocksSend: boolean; busy: boolean;
  message: string; items: MobileNewTaskTransferRecoveryItem[];
}
export interface MobileNewTaskTransferRecoveryResult {
  revision: number; message: string; released: boolean; submit: MobileNewTaskSubmitResult | null;
}
interface Evidence {
  claim: MobileOutboxTransferClaim; outcome: MobileOutboxOutcome | null; epoch: string;
  mode: 'pending' | 'durability' | 'terminal' | 'unknown';
}
const labels: Record<MobileNewTaskTransferRecoveryOperation, string> = {
  commit: 'Keep queued task', rollback: 'Keep draft instead', retry: 'Retry saving', finish: 'Finish saving task', release: 'Keep draft',
};
const stale = () => new ClientError('This draft recovery changed. Refresh its saved status.', 'superseded');
function scope(client: MobileDraftClient, input: MobileNewTaskTransferRecoveryInput, handle: Native | null | undefined) {
  const stamp = () => canonical([client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch, client.draftKey]);
  const expected = stamp();
  const check = () => { if (!/^new-task:[\w-]{1,128}$/.test(input.draftKey) || !input.current()
    || client.draftKey !== input.draftKey || expected !== stamp()) throw stale(); };
  check();
  if (!handle?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to recover this task.');
  const base = letGoAware(handle);
  const native: Native = { available: true, watch(topic) { check(); base.watch(topic); }, async later(request) {
    check(); const reply = await base.later(request); check(); return reply;
  } };
  return { native, check };
}
function proof(value: Evidence) {
  const c = value.claim;
  return { transferId: c.transferId, draftKey: c.draftKey, fingerprint: c.fingerprint, messageId: c.messageId,
    threadId: c.threadId, commandId: c.commandId, mutationId: c.mutationId, state: c.state,
    outcomeStatus: value.outcome?.status ?? '', outcomeRevision: value.outcome?.revision ?? null, epoch: value.epoch, mode: value.mode };
}
function operations(value: Evidence): MobileNewTaskTransferRecoveryOperation[] {
  switch (value.claim.state) {
    case 'prepared': return value.mode === 'pending' ? ['commit', 'rollback'] : value.mode === 'durability' ? ['retry'] : [];
    case 'queued': return ['finish'];
    case 'failed': return ['release'];
    case 'completed': case 'released': return [];
  }
}
function evidence(client: MobileDraftClient, claim: MobileOutboxTransferClaim, outcome: MobileOutboxOutcome | null): Evidence {
  const snapshot = mobileOutboxSnapshot(client);
  const marker = snapshot.recovery.find(raw => {
    const item = obj(raw), mutation = obj(item.mutation), transfer = obj(mutation.transfer);
    return item.messageId === claim.messageId && item.state === 'pending' && mutation.mutationId === claim.mutationId
      && mutation.operation === 'enqueue' && transfer.transferId === claim.transferId && transfer.fingerprint === claim.fingerprint
      && transfer.draftKey === claim.draftKey && canonical(mutation.record) === canonical(claim.record);
  });
  const unresolved = snapshot.recovery.some(raw => obj(raw).messageId === claim.messageId);
  return { claim, outcome, epoch: snapshot.ownerEpoch ?? '', mode: claim.state !== 'prepared' ? 'terminal'
    : marker ? 'pending' : unresolved ? 'unknown' : 'durability' };
}
function item(value: Evidence, enabled: boolean): MobileNewTaskTransferRecoveryItem {
  const c = value.claim;
  const message = c.state === 'prepared' ? value.mode === 'pending'
    ? 'Saving this task was interrupted. Keep it queued to send or keep it as a draft.'
    : value.mode === 'unknown' ? 'This interrupted write does not match the saved transfer. Refresh its storage before changing it.'
      : 'The saved local outcome still needs durability confirmation.'
    : c.state === 'queued' ? 'The task is queued. Finish saving it; any newer draft edits will be kept.'
      : c.state === 'failed' ? 'This task has not been queued. Keep the draft to continue editing.'
        : c.state === 'completed' ? 'This captured draft was already queued.' : 'The failed capture was released. Your draft can be submitted again.';
  return { transferId: c.transferId, draftKey: c.draftKey, threadId: c.threadId, messageId: c.messageId, commandId: c.commandId,
    state: c.state, message, actions: enabled ? operations(value).map(kind => ({ kind, label: labels[kind], key: canonical({ ...proof(value), kind }) })) : [] };
}
async function inspect(client: MobileDraftClient, native: Native, input: MobileNewTaskTransferRecoveryInput) {
  const complete = await mobileOutboxRead(client, native);
  const lookup = await mobileOutboxTransferLookup(client, native, input.draftKey);
  const values: Evidence[] = [], unknown: MobileOutboxTransferClaim[] = [];
  for (const saved of lookup.claims) {
    const status = await mobileOutboxTransferStatus(client, native, saved.transferId);
    if (!status.claim) { unknown.push(saved); continue; }
    if (status.claim.draftKey !== input.draftKey) throw new ClientError('This transfer belongs to another draft.');
    values.push(evidence(client, status.claim, status.outcome));
  }
  return { values, unknown, complete: complete && lookup.complete && mobileOutboxSnapshot(client).complete,
    conflicts: values.filter(value => !['completed', 'released'].includes(value.claim.state)).length > 1 };
}
/** Fresh complete inventory and exact status are the only action authority. */
export async function mobileNewTaskTransferRecoveryRead(client: MobileDraftClient, handle: Native | null | undefined,
  input: MobileNewTaskTransferRecoveryInput): Promise<MobileNewTaskTransferRecoveryView> {
  const view = (message: string, complete = false, items: MobileNewTaskTransferRecoveryItem[] = [], blocksSend = true): MobileNewTaskTransferRecoveryView =>
    ({ revision: client.revision, draftKey: input.draftKey, complete, blocksSend, busy: mobileNewTaskTransferGuardBusy(client, input.draftKey), message, items });
  if (!client.preferencesLoaded) return view('Wait for saved drafts to load.');
  try {
    const { native } = scope(client, input, handle), found = await inspect(client, native, input);
    const enabled = found.complete && !found.conflicts && !found.unknown.length && !mobileNewTaskTransferGuardBusy(client, input.draftKey);
    const rows = found.values.map(value => item(value, enabled));
    for (const claim of found.unknown) rows.push({ transferId: claim.transferId, draftKey: claim.draftKey, threadId: claim.threadId,
      messageId: claim.messageId, commandId: claim.commandId, state: 'unknown', message: 'The native transfer outcome could not be found. Refresh its status before changing this draft.', actions: [] });
    return view(found.conflicts ? 'More than one transfer owns this draft. Recover its storage before continuing.'
      : !found.complete || found.unknown.length ? 'Saved task ownership is incomplete. Refresh before choosing a recovery action.' : '',
    found.complete && !found.unknown.length, rows, !enabled || found.values.some(value => !['completed', 'released'].includes(value.claim.state)));
  } catch (error) { if (letGo(error)) throw error; return view(error instanceof Error ? error.message : String(error)); }
}
/** Compact UI boundary. Completed/released history never becomes a permanent
 * pre-Send warning; the coordinator still owns its duplicate fingerprints. */
export function mobileNewTaskTransferRecoveryPresentation(snapshot: MobileNewTaskTransferRecoveryView | null) {
  const active = snapshot?.items.filter(item => !['completed', 'released'].includes(item.state)) ?? [];
  return { visible: !!snapshot && (!snapshot.complete || active.length > 0 || !!snapshot.message),
    message: snapshot?.message || active.map(item => item.message).join('\n'),
    actions: snapshot?.busy ? [] : active.flatMap(item => item.actions.map(({ key, label }) => ({ key, label }))) };
}
/** One action, one Native lifetime. Keys carry exact native identity/state and
 * available revision; stale buttons cannot choose a new recovery interpretation. */
export async function mobileNewTaskTransferRecoveryAction(client: MobileDraftClient, handle: Native | null | undefined,
  storage: Files, input: MobileNewTaskTransferRecoveryInput, key: string): Promise<MobileNewTaskTransferRecoveryResult> {
  const result = (message: string, submit: MobileNewTaskSubmitResult | null = null, released = false): MobileNewTaskTransferRecoveryResult =>
    ({ revision: client.revision, message, released, submit });
  if (!client.preferencesLoaded) return result('Wait for saved drafts to load.');
  try {
    const { native, check } = scope(client, input, handle);
    let requested: Record<string, unknown>;
    try { requested = JSON.parse(key); } catch { return result('Refresh this draft recovery before choosing an action.'); }
    if (!requested || typeof requested !== 'object' || Array.isArray(requested) || requested.draftKey !== input.draftKey)
      return result('This recovery action belongs to another draft.');
    const found = await inspect(client, native, input), chosen = found.values.find(value => value.claim.transferId === requested.transferId);
    if (!found.complete || found.conflicts || found.unknown.length || !chosen || mobileNewTaskTransferGuardBusy(client, input.draftKey))
      return result('Refresh the complete saved transfer status before recovering.');
    const kind = requested.kind as MobileNewTaskTransferRecoveryOperation;
    if (!operations(chosen).includes(kind) || key !== canonical({ ...proof(chosen), kind }))
      return result('The saved recovery state changed. Refresh before choosing again.');
    if (kind === 'release') {
      const lease = mobileNewTaskTransferGuardAcquire(client, input.draftKey);
      if (!lease) return result('This draft transfer is already being recovered.');
      try { check(); const released = await mobileOutboxReleaseFailedTransfer(client, native, chosen.claim);
        return result(released ? '' : 'The failed capture is still retained.', null, released);
      } finally { mobileNewTaskTransferGuardRelease(client, lease); }
    }
    // Resume performs another status read. Before the first mutation, it must
    // still describe the exact action the user selected, not a newer queued ACK.
    let mutating = false;
    const admitted: Native = { available: true, watch: topic => native.watch(topic), async later(request) {
      const r = obj(request);
      if (r.op === 'mobileOutbox' && ['recover', 'completeTransfer'].includes(String(r.action))) mutating = true;
      const reply = await native.later(request);
      if (!mutating && r.op === 'mobileOutbox' && r.action === 'transferStatus') {
        const raw = obj(obj(reply).value), claim = raw.claim as MobileOutboxTransferClaim | null;
        if (!claim || canonical(proof(evidence(client, claim, raw.outcome as MobileOutboxOutcome | null))) !== canonical(proof(chosen))) throw stale();
      }
      return reply;
    } };
    const guarded: Files = { fs: {
      async mkdir(path) { check(); const value = await storage.fs.mkdir(path); check(); return value; },
      async readFile(path) { check(); const value = await storage.fs.readFile(path); check(); return value; },
      async atomicWriteFile(path, bytes) { check(); const value = await storage.fs.atomicWriteFile(path, bytes); check(); return value; },
    } };
    const submitted = await mobileNewTaskSubmitRecover(client, admitted, guarded, chosen.claim.transferId, input.current,
      kind === 'finish' ? undefined : kind);
    // A confirmed rollback is the user's successful choice, although the native
    // enqueue claim is intentionally terminal-failed until explicitly released.
    const rolledBack = kind === 'rollback' && submitted.status === 'failed' && submitted.claim?.state === 'failed';
    return result(rolledBack ? '' : submitted.message, submitted);
  } catch (error) { if (letGo(error)) throw error; return result(error instanceof Error ? error.message : String(error)); }
}
