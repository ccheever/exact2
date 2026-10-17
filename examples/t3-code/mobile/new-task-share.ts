// Pinned365aa87982 NewTaskDraftScreen: route-owned import, retry and cancellation.
// @ref llp/1109.005-composer-and-transcript.decision.md#incoming-share-inbox
import type { T3Client } from './shared/client';
import { bridgeReply, ClientError, type Files, type Native } from './shared/protocol';
import { mobileIncomingShareImports } from './incoming-share-imports';
import { letGo } from './shared/let-go';
import { obj } from './shared/domain';
import { mobileIncomingShareAdopt } from './incoming-share-adoption';
import { mobileIncomingShareCancel } from './incoming-share-cancellation';
import { mobileIncomingShare, mobileIncomingShareRead, mobileIncomingShareReservation, mobileIncomingShareInboxState } from './incoming-share-inbox';
import { incomingShareSelection, type IncomingShareEntry } from './incoming-share-model';
export interface NewTaskShare { id: string; attempt: number; phase: 'pending' | 'cancel' | 'done' | 'cancelled'; entry: IncomingShareEntry | null; adoptionId?: string }
export function newTaskShare(id: string, client: T3Client): NewTaskShare { return { id, attempt: 0, phase: mobileIncomingShareReservation(client, id)?.phase === 'cancelling' ? 'cancel' : 'pending', entry: mobileIncomingShare(client, id) }; }
export function newTaskSharePending(share?: NewTaskShare) { return !!share && (share.phase === 'pending' || share.phase === 'cancel'); }
export function newTaskShareReady(share: NewTaskShare, client: T3Client) {
  if (!mobileIncomingShareInboxState(client).ready) return false;
  share.entry ??= mobileIncomingShare(client, share.id);
  if (share.phase === 'pending' && mobileIncomingShareReservation(client, share.id)?.phase === 'cancelling') share.phase = 'cancel';
  return share.phase === 'cancel' || !share.entry || incomingShareSelection(share.entry, client.configLive ? client.config : null, []).status === 'ready';
}
/** One attempt per root task. The alert's answer schedules the next attempt;
 * leaving a route retains the native reservation and never starts a retry. */
export async function newTaskShareRun(share: NewTaskShare, client: T3Client, native: Native, storage: Files, current: () => boolean) {
  const assertCurrent = () => { if (!current()) throw new ClientError('The share route changed.', 'superseded'); };
  const alert = async (kind: string, title: string, message: string) => {
    assertCurrent(); const reply = await bridgeReply(native, { op: 'mobileAlert', kind, title, message }); assertCurrent();
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
    return String(obj(reply.value).choice ?? '');
  };
  if (!newTaskSharePending(share)) return;
  share.attempt++;
  share.entry ??= mobileIncomingShare(client, share.id);
  if (share.phase === 'pending' && mobileIncomingShareReservation(client, share.id)?.phase === 'cancelling') share.phase = 'cancel';
  if (!share.entry) {
    share.phase = 'cancelled';
    await alert('info', 'Shared content unavailable', 'The shared content is no longer available. You can continue editing this draft.'); return;
  }
  if (share.phase === 'cancel') {
    let message = '';
    try {
    await mobileIncomingShareRead(client, native); assertCurrent();
    const state = mobileIncomingShareInboxState(client);
    if (state.error) throw new ClientError(state.error);
    const liveEntry = mobileIncomingShare(client, share.id);
    const reservation = liveEntry?.instanceId === share.entry.instanceId ? mobileIncomingShareReservation(client, share.id) : null;
    const receipt = Object.values(mobileIncomingShareImports(client)[client.draftKey] ?? {}).find(item => item.shareId === share.id && item.instanceId === share.entry!.instanceId);
    share.adoptionId = reservation?.adoptionId ?? share.adoptionId ?? receipt?.adoptionId;
    if (!share.adoptionId) { share.phase = 'cancelled'; return; }
    const cancelled = await mobileIncomingShareCancel(client, native, storage, { entry: share.entry, adoptionId: share.adoptionId, current });
    assertCurrent();
    if (cancelled.status === 'cancelled') { share.phase = 'cancelled'; return; }
    message = cancelled.message;
    } catch (error) { if (letGo(error)) throw error; assertCurrent(); message = error instanceof Error ? error.message : 'The shared content could not be restored safely.'; }
    const choice = await alert('share-cancel', 'Could not cancel import', message);
    share.phase = choice === 'retry-import' ? 'pending' : 'cancel'; return;
  }
  const result = await mobileIncomingShareAdopt(client, native, storage, { entry: share.entry, current }); assertCurrent();
  if (result.status === 'pending') return;
  if (result.status === 'imported') {
    share.phase = 'done';
    if (result.warnings.length) await alert('info', 'Some shared content was skipped', result.warnings.join('\n'));
    return;
  }
  const choice = await alert('share-import', 'Could not import shared content', result.message);
  share.phase = choice === 'retry' ? 'pending' : 'cancel';
}
