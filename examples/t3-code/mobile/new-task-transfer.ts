// Local enqueue completion is separate from server launch and its shared Pending owner.
// @ref llp/1109.005-composer-and-transcript.decision.md#durable-draft-capture-transfer
import type { MobileDraftClient } from './mobile-draft-recovery';
import { mobilePrepareNewTaskOutbox, type MobileOutboxCaptureFacts } from './mobile-outbox-capture';
import { mobileOutboxEncode } from './mobile-outbox-model';
import { mobileNewTaskDraftPresentation } from './mobile-new-task-drafts';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxEnqueueTransfer, mobileOutboxTransferLookup,
  mobileOutboxTransferStatus, mobileOutboxCompleteTransfer, mobileOutboxReleaseFailedTransfer, mobileOutboxRecover } from './mobile-outbox';
import { mobileOutboxTransferCanonical, type MobileOutboxTransferClaim } from './mobile-outbox-transfer-model';
import { mobileOutboxTransferApplyCleanup } from './mobile-outbox-transfer-cleanup';
import { ClientError, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileNewTaskTransferGuardAcquire, mobileNewTaskTransferGuardRelease,
  mobileNewTaskTransferGuardBusy } from './new-task-transfer-guard';

export interface MobileNewTaskTransferInput {
  draftKey: string;
  /** Fresh action clock, supplied by Contract as epochAtZero + now(). */
  now: number;
  /** Resolve only new-admission facts, after existing transfer recovery. */
  prepareFacts?(native: Native): Promise<void>;
  /** Route/selection ownership only. Reads and draft persistence change client.revision. */
  current(): boolean;
  /** Synchronous resolved facts for this full draft. Never prepare a different selected draft here. */
  facts(): MobileOutboxCaptureFacts;
  /** An explicit retry action may retire a known failed enqueue before submitting again. */
  retryFailed?: boolean;
}
export interface MobileNewTaskTransferResult {
  status: 'completed' | 'cleanup-pending' | 'recovery-required' | 'failed' | 'blocked' | 'busy';
  claim: MobileOutboxTransferClaim | null; message: string;
}
const result = (status: MobileNewTaskTransferResult['status'], message = '', claim: MobileOutboxTransferClaim | null = null): MobileNewTaskTransferResult =>
  ({ status, claim, message });
export const mobileNewTaskTransferBusy = mobileNewTaskTransferGuardBusy;
function acquire(client: MobileDraftClient, key: string): (() => void) | null {
  const lease = mobileNewTaskTransferGuardAcquire(client, key); if (!lease) return null;
  client.revision++;
  return () => { mobileNewTaskTransferGuardRelease(client, lease); client.revision++; };
}
function invocation(input: Native | null | undefined) {
  if (!input?.available) throw new ClientError('Open T3 Code on your iPhone or iPad to queue this task.');
  const base = letGoAware(input); let cancelled: unknown;
  const assertLive = () => { if (cancelled) throw cancelled; };
  const native: Native = { available: true, watch: topic => { assertLive(); base.watch(topic); }, async later(request) {
    assertLive();
    try { return await base.later(request); }
    catch (error) { if (letGo(error)) cancelled = error; throw error; }
  } };
  return { native, assertLive };
}
async function readOwner(client: MobileDraftClient, native: Native, assertLive: () => void): Promise<boolean> {
  await mobileOutboxRead(client, native); assertLive();
  return mobileOutboxSnapshot(client).initialized;
}
async function finish(client: MobileDraftClient, native: Native, storage: Files, claim: MobileOutboxTransferClaim): Promise<MobileNewTaskTransferResult> {
  if (claim.state === 'completed') return result('completed', '', claim);
  if (claim.state === 'prepared') return result('recovery-required', 'Resolve the interrupted local enqueue before submitting this draft again.', claim);
  if (claim.state !== 'queued') return result('failed', 'The local enqueue failed. Your draft has been kept.', claim);
  if (mobileOutboxTransferApplyCleanup(client, claim) === 'blocked')
    return result('cleanup-pending', 'The task is queued, but its captured draft cleanup needs attention.', claim);
  try {
    // The marker and cleanup enter the existing preference write together, before this await.
    await client.persist(storage);
    if (await mobileOutboxCompleteTransfer(client, native, claim))
      return result('completed', '', mobileOutboxSnapshot(client).transfers.find(item => item.transferId === claim.transferId) ?? claim);
    return result('cleanup-pending', 'The task is queued. Its draft cleanup still needs to finish.', claim);
  } catch (error) {
    if (letGo(error)) throw error;
    return result('cleanup-pending', error instanceof Error ? error.message : 'Could not finish the captured draft cleanup.', claim);
  }
}
async function freshStatus(client: MobileDraftClient, native: Native, transferId: string) {
  const reply = await mobileOutboxTransferStatus(client, native, transferId);
  if (!reply.claim) throw new ClientError('The saved draft transfer could not be found. Keep the draft until its storage is recovered.');
  return reply.claim;
}
/** One foreground invocation, with a plain per-draft latch. It never sends to a server,
 * navigates, or retains a Promise/native handle for another answer. Preparation is caller-owned. */
export async function mobileNewTaskTransferSubmit(client: MobileDraftClient, nativeInput: Native | null | undefined,
  storage: Files, input: MobileNewTaskTransferInput): Promise<MobileNewTaskTransferResult> {
  if (!client.preferencesLoaded) return result('blocked', 'Wait for saved drafts to load.');
  if (!/^new-task:[\w-]{1,128}$/.test(input.draftKey)) return result('blocked', 'Choose an independent draft.');
  const release = acquire(client, input.draftKey); if (!release) return result('busy', 'This draft is already being submitted.');
  try {
    const { native, assertLive } = invocation(nativeInput);
    const initial = mobileNewTaskDraftPresentation(client, input.draftKey);
    const assertCurrent = () => {
      if (!input.current() || mobileOutboxTransferCanonical(mobileNewTaskDraftPresentation(client, input.draftKey)) !== mobileOutboxTransferCanonical(initial))
        throw new ClientError('The draft changed before it could be queued.', 'superseded');
    };
    if (!await readOwner(client, native, assertLive)) return result('blocked', 'Read the pending-task storage before submitting.');
    // Recovery identity is the full saved key, independent of whatever is selected now.
    let lookup = await mobileOutboxTransferLookup(client, native, input.draftKey); assertLive();
    const active = lookup.claims.filter(claim => !['completed', 'released'].includes(claim.state));
    if (active.length > 1) return result('blocked', 'More than one transfer claims this draft. Recover its storage first.');
    if (active[0]) {
      const claim = await freshStatus(client, native, active[0].transferId); assertLive();
      if (claim.state !== 'failed' || !input.retryFailed) return await finish(client, native, storage, claim);
      assertCurrent();
      if (!await mobileOutboxReleaseFailedTransfer(client, native, claim)) return result('failed', 'The failed enqueue is still retained.', claim);
      lookup = await mobileOutboxTransferLookup(client, native, input.draftKey); assertLive();
      if (lookup.claims.some(item => !['completed', 'released'].includes(item.state))) return result('recovery-required', 'Another transfer now owns this draft.');
    }
    if (!lookup.complete || !mobileOutboxSnapshot(client).complete) return result('blocked', 'Resolve the incomplete pending-task inventory before submitting.');
    assertCurrent();
    if (!initial) return result('blocked', 'That saved draft is no longer available.');
    // Completed fingerprints, unlike failed/released ones, remain duplicate evidence.
    const captured = { version: 1 as const, draft: initial };
    const compared = await mobileOutboxTransferLookup(client, native, input.draftKey, captured); assertLive();
    if (!compared.complete) return result('blocked', 'Resolve the incomplete draft-transfer inventory before submitting.');
    const existing = compared.claims.find(claim => claim.state !== 'released' &&
      (claim.state !== 'completed' || claim.fingerprint === compared.fingerprint));
    if (existing) return await finish(client, native, storage, await freshStatus(client, native, existing.transferId));
    assertCurrent();
    if (input.prepareFacts) {
      const preparing: Native = { available: native.available, watch(topic) { assertCurrent(); native.watch(topic); }, async later(request) {
        assertCurrent(); const reply = await native.later(request); assertLive(); assertCurrent(); return reply;
      } };
      await input.prepareFacts(preparing); assertLive(); assertCurrent();
    }
    const facts = input.facts(), factsKey = mobileOutboxTransferCanonical(facts);
    const assertCapture = () => { assertCurrent(); if (mobileOutboxTransferCanonical(input.facts()) !== factsKey)
      throw new ClientError('The task settings changed before it could be queued.', 'superseded'); };
    const prepared = mobilePrepareNewTaskOutbox(initial, facts);
    if (prepared.status !== 'ready') return result('blocked', prepared.reason);
    const now = input.now;
    if (!Number.isFinite(now) || now <= 0) return result('blocked', 'Wait for the app clock before queuing this draft.');
    await client.persist(storage); assertLive(); assertCapture();
    const [threadId, messageId, commandId] = await client.ids(native, 3); assertLive(); assertCapture();
    if (new Set([threadId, messageId, commandId]).size !== 3) return result('blocked', 'Task identifiers were not unique.');
    const record = mobileOutboxEncode({ ...prepared.record, threadId, messageId, commandId, createdAt: new Date(now).toISOString() });
    const admission = await mobileOutboxEnqueueTransfer(client, native, record, { version: 1, draft: prepared.draftCapture });
    assertLive();
    if (!admission.claim || admission.disposition === 'unknown')
      return result('recovery-required', 'The local enqueue response was interrupted. Recover this draft before retrying.');
    // The native claim can belong to an earlier submit with different allocated IDs.
    return await finish(client, native, storage, admission.claim);
  } catch (error) {
    if (letGo(error)) throw error;
    return result('blocked', error instanceof Error ? error.message : 'Could not queue this draft.');
  } finally { release(); }
}
/** Recovery uses a new invocation and the original IDs. A pending write is never
 * treated as committed merely because the native envelope exists. */
export async function mobileNewTaskTransferResume(client: MobileDraftClient, nativeInput: Native | null | undefined,
  storage: Files, transferId: string, decision?: 'commit' | 'rollback' | 'retry'): Promise<MobileNewTaskTransferResult> {
  if (!client.preferencesLoaded) return result('blocked', 'Wait for saved drafts to load.');
  const { native, assertLive } = invocation(nativeInput);
  if (!await readOwner(client, native, assertLive)) return result('blocked', 'Read the pending-task storage before recovering.');
  let claim = await freshStatus(client, native, transferId); assertLive();
  const release = acquire(client, claim.draftKey); if (!release) return result('busy', 'This draft transfer is already being recovered.', claim);
  try {
    if (claim.state === 'prepared' && decision) {
      await mobileOutboxRecover(client, native, claim.messageId, claim.mutationId, decision); assertLive();
      claim = await freshStatus(client, native, transferId); assertLive();
    }
    return await finish(client, native, storage, claim);
  } finally { release(); }
}
