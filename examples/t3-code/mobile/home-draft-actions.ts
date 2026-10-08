// Pinned365aa87982 usePendingTaskListActions: local open and confirmed optimistic discard.
// @ref llp/1109.004-home-projection.decision.md#decision
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { bridgeReply, nativeFiles, type Files, type Native } from './shared/protocol';
import { obj } from './shared/domain';
import { letGo, letGoAware } from './shared/let-go';
import { homeDraftLocation, homeDraftTitle } from './home-drafts';
import { mobileNewTaskDraftDiscard, mobileNewTaskDraftHasContent, mobileNewTaskDraftPresentation, mobileNewTaskDraftIsPendingKey } from './mobile-new-task-drafts';
import type { HomeActionResult } from './home-actions';
import { mobileNewTaskTransferGuardAcquire, mobileNewTaskTransferGuardRead, mobileNewTaskTransferGuardAssert,
  mobileNewTaskTransferGuardRelease } from './new-task-transfer-guard';

/** Local draft ownership does not require a connected server or orchestration grant. */
export async function mobileHomeDraftAction(requestRoute: string, environmentId: string, threadId: string,
  operation: string, key: string, currentSurface: () => boolean, client: T3Client,
  nativeInput?: Native | null, storageInput?: Files): Promise<HomeActionResult> {
  const result = (message = '', nextLocation = ''): HomeActionResult => ({ revision: client.revision, requestRoute,
    message, alertTitle: message ? operation === 'draft-open' ? 'Could not open draft' : 'Could not discard draft' : '',
    nextLocation, archiveChanged: false, uncertain: false });
  if (!currentSurface() || threadId || !client.preferencesLoaded) return result();
  if (mobileNewTaskDraftIsPendingKey(key)) return result('This saved edit belongs to a pending task. Open that task to review it.');
  const captured = mobileNewTaskDraftPresentation(client, key);
  if (!captured || captured.environmentId !== environmentId || !mobileNewTaskDraftHasContent(client, key)) return result();
  if (operation === 'draft-open') return result('', homeDraftLocation(captured));
  if (operation !== 'draft-discard') return result();
  if (!nativeInput?.available) return result('Open T3 Code on your iPhone or iPad to discard this draft.');
  const lease = mobileNewTaskTransferGuardAcquire(client, key); if (!lease) return result();
  const native = letGoAware(nativeInput), storage = storageInput ?? nativeFiles(native);
  try {
    await mobileNewTaskTransferGuardRead(client, native, lease, currentSurface);
    const title = homeDraftTitle(captured.text, captured.images.length + captured.files.length);
    const reply = await bridgeReply(native, { op: 'mobileAlert', kind: 'discard', title: 'Discard draft?', message: `“${title}” will be removed.` });
    if (!reply.ok) return currentSurface() ? result(reply.error!.message) : result();
    if (obj(reply.value).choice !== 'discard' || !currentSurface()) return result();
    // Includes content as well as revision: late attachment adoption cannot be discarded by an older prompt.
    if (JSON.stringify(mobileNewTaskDraftPresentation(client, key)) !== JSON.stringify(captured))
      return result('This draft changed. Open its menu again to discard it.');
    mobileNewTaskTransferGuardAssert(client, lease);
    if (!mobileNewTaskDraftDiscard(client, key)) return result();
    try { await client.persist(storage); }
    catch (error) {
      if (letGo(error)) throw error;
      return currentSurface() ? result('The draft was removed here, but the change could not be saved. It may return after restarting. Attachment cleanup is queued for a successful save.') : result();
    }
    // Existing release owners persist retry identities before deleting any unreferenced local bytes.
    await client.flushSnapshotReleases(native, storage);
    return result();
  } catch (error) {
    if (letGo(error)) throw error;
    return currentSurface() ? result(error instanceof Error ? error.message : 'The draft could not be discarded.') : result();
  } finally { mobileNewTaskTransferGuardRelease(client, lease); }
}
