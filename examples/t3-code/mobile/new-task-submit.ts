// Pinned365aa87982 NewTaskDraftScreen.handleStart: local enqueue precedes navigation.
// @ref llp/1109.005-composer-and-transcript.decision.md#durable-draft-capture-transfer
import type { MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskCaptureFacts } from './new-task-capture-facts';
import { mobileNewTaskTransferSubmit, mobileNewTaskTransferResume, type MobileNewTaskTransferResult } from './new-task-transfer';
import { mobileNewTaskDraftPresentation, mobileNewTaskDraftIsPendingKey } from './mobile-new-task-drafts';
import { mobilePrepareNewTaskOutbox } from './mobile-outbox-capture';
import { mobileOutboxSnapshot } from './mobile-outbox';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';
import type { Native, Files } from './shared/protocol';

export interface MobileNewTaskSubmitInput {
  draftKey: string; now: number;
  /** Route/selection ownership, independent of the captured draft's cleanup. */
  current(): boolean;
  retryFailed?: boolean;
}
export interface MobileNewTaskSubmitResult extends MobileNewTaskTransferResult {
  owner: MobileOutboxWireOwner | null;
  threadId: string; messageId: string; commandId: string;
  disposition: 'thread' | 'pending' | 'stay';
  draftRetained: boolean;
}
const destination = (client: MobileDraftClient) => JSON.stringify([client.origin, client.generation,
  client.environmentId, client.projectId, client.threadId, client.threadEpoch]);
function presentation(client: MobileDraftClient, transfer: MobileNewTaskTransferResult, key: string,
  current: boolean, startsNow: boolean): MobileNewTaskSubmitResult {
  const claim = transfer.claim, snapshot = mobileOutboxSnapshot(client);
  // A completed transfer strips its capture. Only the journal's remaining row
  // can then provide its origin; the selected environment cannot reconstruct it.
  const row = claim && snapshot.complete ? snapshot.rows.find(row => row.status === 'confirmed'
    && row.record.messageId === claim.messageId && row.record.threadId === claim.threadId
    && row.record.commandId === claim.commandId) : undefined;
  const record = claim?.record ?? row?.record;
  const owner: MobileOutboxWireOwner | null = record ? { origin: record.origin, environmentId: record.environmentId,
    threadId: record.threadId, messageId: record.messageId, commandId: record.commandId } : null;
  const draftRetained = !!mobileNewTaskDraftPresentation(client, key);
  const canLeave = transfer.status === 'completed' && current && !draftRetained && !!owner;
  return { ...transfer, owner, threadId: claim?.threadId ?? '', messageId: claim?.messageId ?? '', commandId: claim?.commandId ?? '',
    draftRetained, disposition: canLeave ? startsNow && client.connection === 'connected'
      && client.environmentId === owner.environmentId ? 'thread' : 'pending' : 'stay' };
}

/** Local persistence only. Root owns navigation and a later delivery invocation.
 * Recovered transfers prefer the pending list; they never borrow new draft facts. */
export async function mobileNewTaskSubmit(client: MobileDraftClient, native: Native | null | undefined,
  storage: Files, input: MobileNewTaskSubmitInput): Promise<MobileNewTaskSubmitResult> {
  if (mobileNewTaskDraftIsPendingKey(input.draftKey)) return presentation(client, { status: 'blocked', claim: null,
    message: 'Save these changes through the pending task editor.' }, input.draftKey, false, false);
  const stamp = destination(client), capture = mobileNewTaskCaptureFacts(client, input.draftKey, input.current, input.now);
  const initial = mobileNewTaskDraftPresentation(client, input.draftKey);
  let startsNow = false;
  const transfer = await mobileNewTaskTransferSubmit(client, native, storage, { ...capture, retryFailed: input.retryFailed,
    facts() {
      const facts = capture.facts();
      if (initial) {
        const prepared = mobilePrepareNewTaskOutbox(initial, facts);
        startsNow = prepared.status === 'ready' && !prepared.presentation.queuesInsteadOfStarting;
      }
      return facts;
    } });
  return presentation(client, transfer, input.draftKey, input.current() && destination(client) === stamp, startsNow);
}

/** Exact interrupted-transfer recovery uses fresh native lifetime and saved IDs. */
export async function mobileNewTaskSubmitRecover(client: MobileDraftClient, native: Native | null | undefined,
  storage: Files, transferId: string, current: () => boolean, decision?: 'commit' | 'rollback' | 'retry'): Promise<MobileNewTaskSubmitResult> {
  const stamp = destination(client);
  const transfer = await mobileNewTaskTransferResume(client, native, storage, transferId, decision);
  return presentation(client, transfer, transfer.claim?.draftKey ?? '', current() && destination(client) === stamp, false);
}
