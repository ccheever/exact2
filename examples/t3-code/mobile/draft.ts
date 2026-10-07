// @ref llp/1106.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { composerOps } from './shared/client-ops-composer';
import { letGo } from './shared/let-go';
import type { Files, Native } from './shared/protocol';
import { mobileVoiceObserveDraft } from './voice-data';

/** Apply the shared input reducer to its current owner before any promise can change focus.
 * Native preferences writes are serialized by T3Transport.queue. Do not cache a resource
 * answer's promise or handles here: an abandoned answer cannot gate a later keystroke.
 */
export async function mobileDraftChanged(client: T3Client, value: string, native: Native, storage: Files) {
  let message = '';
  try {
    // Calling the async reducer executes its draft branch synchronously; it does not load,
    // refresh presentation, or select another thread before reading the current draftKey.
    const reduction = composerOps.call(client, 'draft', '', value, 0, native, storage, { message: '', id: '', value });
    mobileVoiceObserveDraft(client);
    await reduction;
    await client.persist(storage);
  } catch (error) {
    if (letGo(error)) throw error;
    message = error instanceof Error ? error.message : 'Could not save the draft.';
  } finally { client.revision++; }
  return { revision: client.revision, message };
}
