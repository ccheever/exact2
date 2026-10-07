// @ref llp/1106.005-composer-and-transcript.decision.md#new-task-ownership
import { mobileComposerTargetRequire, mobileComposerTargetWriteText, mobileComposerTargetPersist } from './composer-target';
import { ClientError } from './shared/protocol';
import type { T3Client } from './shared/client';
import { composerOps } from './shared/client-ops-composer';
import { letGo } from './shared/let-go';
import type { Files, Native } from './shared/protocol';
import { mobileVoiceObserveDraft } from './voice-data';

/** Apply the shared input reducer to its current owner before any promise can change focus.
 * Native preferences writes are serialized by T3Transport.queue. Do not cache a resource
 * answer's promise or handles here: an abandoned answer cannot gate a later keystroke.
 */
export async function mobileDraftChanged(client: T3Client, value: string, native: Native, storage: Files, expectedOwner = '') {
  let message = '';
  try {
    // Calling the async reducer executes its draft branch synchronously; it does not load,
    // refresh presentation, or select another thread before reading the current draftKey.
    const target = mobileComposerTargetRequire(client, expectedOwner);
    const reduction = target.kind === 'ordinary'
      ? composerOps.call(client, 'draft', '', value, 0, native, storage, { message: '', id: '', value })
      : mobileComposerTargetWriteText(client, target, value);
    if (reduction === false) throw new ClientError('The queued edit is no longer editable.', 'superseded');
    mobileVoiceObserveDraft(client);
    await reduction;
    await mobileComposerTargetPersist(client, target, native, storage);
  } catch (error) {
    if (letGo(error)) throw error;
    message = error instanceof Error ? error.message : 'Could not save the draft.';
  } finally { client.revision++; }
  return { revision: client.revision, message };
}
