// @ref llp/1109.005-composer-and-transcript.decision.md#retained-editor-preparation-for-terminal-recovery
import type { MobileDraftClient } from './mobile-draft-recovery';
import { obj, str } from './shared/domain';
import { ClientError, type Native, type Files } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';
import { mobileOutboxTransferCanonical as canonical } from './mobile-outbox-transfer-model';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import { mobileNewTaskDraftBoundKey } from './mobile-new-task-drafts';
import { mobilePendingTaskEditorsSnapshot, mobilePendingTaskEditorKey, type MobilePendingTaskExpected } from './mobile-pending-task-state';
import { mobilePendingTaskEditorSaveForRecovery, mobilePendingTaskEditorFinishRecovery } from './mobile-pending-task-editor';
import { mobileOutboxDeliveryStatus, mobileOutboxDeliveryRecover } from './mobile-outbox-delivery';
import { mobileOutboxDraftHandoffStatus, mobileOutboxDraftHandoffDecode } from './mobile-outbox-draft-handoff';
import { mobileOutboxDraftRecover, type MobileOutboxDraftRecoveryResult } from './mobile-outbox-draft-recovery';

const busy = new WeakSet<MobileDraftClient>();
/** One explicit, unbound editor operation. Native completion is checked before
 * any new save, including recovery after removal has already deleted the row. */
export async function mobilePendingTaskRecover(client: MobileDraftClient, handle: Native, storage: Files,
  input: { expected: MobilePendingTaskExpected; current(): boolean }): Promise<MobileOutboxDraftRecoveryResult> {
  const expected = JSON.parse(JSON.stringify(input.expected)) as MobilePendingTaskExpected, owner = expected.owner;
  const retained = (reason: string, draftKey = ''): MobileOutboxDraftRecoveryResult => ({ status: 'retained', reason, draftKey });
  if (busy.has(client)) return retained('This editor is already being recovered.');
  busy.add(client);
  const selection = () => canonical([client.origin, client.environmentId, client.generation, client.projectId, client.threadId, client.threadEpoch, client.draftKey]);
  const initial = selection(), ownerKey = mobilePendingTaskEditorKey(owner);
  const check = () => {
    if (!input.current() || selection() !== initial || mobileQueuedEditOrigin(client) !== owner.origin || client.environmentId !== owner.environmentId)
      throw new ClientError('The pending task recovery route changed.', 'superseded');
    if (!client.preferencesLoaded || !mobilePendingTaskEditorsSnapshot(client).ready || mobileNewTaskDraftBoundKey(client))
      throw new ClientError('Close the active editor before recovering its saved draft.');
  };
  const marker = () => mobilePendingTaskEditorsSnapshot(client).markers.find(item => mobilePendingTaskEditorKey(item.owner) === ownerKey);
  try {
    check(); const base = letGoAware(handle);
    const native: Native = { available: base.available, watch(topic) { check(); base.watch(topic); }, async later(request) {
      check(); const reply = await base.later(request); check(); return reply;
    } };
    let live = marker();
    if (live && (live.session !== expected.session || live.revision !== expected.revision)) return retained('The saved editor changed. Reopen its current recovery status.');
    let terminal = await mobileOutboxDeliveryStatus(native, owner.commandId);
    if (!terminal.operation || mobilePendingTaskEditorKey(terminal.operation.record) !== ownerKey)
      return retained('The original task has no matching terminal receipt. Keep its saved editor.');
    if (!terminal.durable) terminal = await mobileOutboxDeliveryRecover(native, terminal.operation);
    const receipt = terminal.operation!, completed = await mobileOutboxDraftHandoffStatus(native, receipt);
    const raw = obj(obj(client.local).mobileOutboxDraftHandoffs)[owner.commandId];
    const proof = completed.handoff ?? (raw === undefined ? null : mobileOutboxDraftHandoffDecode(raw, receipt));
    const source = proof && obj(obj(obj(proof.draft.recovered)[`outbox:${JSON.stringify([owner.origin, owner.environmentId, owner.commandId])}`]).pendingEditor);
    if (proof && (source?.session !== expected.session || source.draftKey !== `new-task:pending-${owner.messageId}` || !str(source.fingerprint)))
      return retained('The saved destination belongs to a different editor session.');
    let fingerprint = source ? str(source.fingerprint) : '';
    if (!proof) {
      if (!live) return retained('The original saved editor is missing.');
      const saved = await mobilePendingTaskEditorSaveForRecovery(client, native, storage, { owner, expected: live, current: input.current });
      check();
      if (saved.status !== 'recovery-saved' || !saved.marker || !saved.fingerprint) return retained(saved.reason || 'Keep the saved editor until its original send is resolved.');
      live = saved.marker; fingerprint = saved.fingerprint;
    }
    const recovered = await mobileOutboxDraftRecover(client, native, storage, { owner, current: input.current,
      ...(live ? { editor: { expected: live, fingerprint } } : {}) });
    check();
    if (recovered.status !== 'recovered' || !recovered.handoff) return recovered;
    if (!live) return recovered;
    const finished = await mobilePendingTaskEditorFinishRecovery(client, native, storage, { owner, expected: live,
      fingerprint, handoff: recovered.handoff, current: input.current });
    check();
    return finished.status === 'finished' ? recovered : retained(finished.reason || 'The destination is safe. Newer editor content remains saved.', recovered.draftKey);
  } catch (error) {
    if (letGo(error)) throw error;
    return retained(error instanceof Error ? error.message : String(error));
  } finally { busy.delete(client); }
}
