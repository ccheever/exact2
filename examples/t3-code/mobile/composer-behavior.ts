import { mobileNewTaskDraftIsPendingKey, mobileNewTaskDraftCurrent, mobileNewTaskDraftLookup } from './mobile-new-task-drafts';
import { mobileComposerTarget, mobileComposerTargetCurrent } from './composer-target';
// Pinned365aa87982 ComposerTextView key commands and followUpBehavior.ts.
// @ref llp/1109.005-composer-and-transcript.decision.md#settings-ownership
import type { T3Client } from './shared/client';
import { arr, obj } from './shared/domain';
import { stage } from './shared/composer-controls';
import { bridgeReply, ClientError, type Native, type Files } from './shared/protocol';
import { queueState } from './shared/composer-controls-queue';
import { threadPhase } from './shared/composer-presentation';
import { letGo } from './shared/let-go';
import { normalizeMobilePreferences } from './settings-preferences';

/** The mobile preference file is authoritative; this is only the shared reducer's input. */
export function applyMobileComposerBehavior(client: T3Client, input: unknown) {
  const preferences = normalizeMobilePreferences(input);
  client.local.clientSettings.followUpBehavior = preferences.followUpBehavior;
  client.local.deviceSettings.planModeEnabled = preferences.planModeEnabled;
}

/** The authored mobile key/button action already resolved its chord. Do not apply
 * desktop server bindings, which can turn Command-Return into a background launch.
 * This native response is local input attribution, not a server result.
 */
export function mobileSubmissionNative(native: Native, alternate: boolean, afterPresentation?: () => void, assertOwner?: () => void): Native {
  let dispatched = false;
  return { available: native.available, watch: topic => native.watch(topic), later: async input => {
    const request = obj(input);
    if (!dispatched) assertOwner?.();
    if (request.op === 'composerSendIntent') return { ok: true, generation: request.generation ?? 0,
      value: { source: 'mobile', modifiers: alternate ? 'meta' : '', ageMs: 0 } };
    // Once the authored write starts, shared pending reconciliation owns its outcome.
    // A successful launch intentionally changes the selected thread afterward.
    if (request.op === 'request' && ['orchestration.dispatchCommand', 'orchestration.launchThread'].includes(String(request.method))) dispatched = true;
    const response = await native.later(input);
    if (!dispatched) assertOwner?.();
    if (request.op === 'devicePresentation') afterPresentation?.();
    return response;
  } };
}

export async function mobileSend(client: T3Client, alternate: boolean, native: Native, storage: Files) {
  const identity = () => JSON.stringify([client.generation, client.threadEpoch, client.origin, client.environmentId, client.projectId,
    client.threadId, mobileComposerTarget(client).owner, client.draftKey, client.providerId, client.modelId, client.modelOptions, client.runtimeMode, client.interactionMode]);
  const owner = identity(), target = mobileComposerTarget(client), draft = mobileNewTaskDraftCurrent(client);
  const storedChoices = JSON.stringify(draft?.choices), originalMode = client.interactionMode;
  const model = JSON.stringify([client.providerId, client.modelId, client.modelOptions, client.runtimeMode]);
  let projectedBuild = false;
  const assertOwner = () => { if (owner !== identity()) throw new ClientError('The draft or model changed before the message could be sent.', 'superseded'); };
  try {
    if (mobileNewTaskDraftIsPendingKey(client.draftKey)) throw new ClientError('Save these changes from the pending task editor.');
    if (mobileComposerTarget(client).kind !== 'ordinary') throw new ClientError('Save the queued edit from its composer.');
    const reply = await bridgeReply(native, { op: 'mobilePreferences' });
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
    assertOwner();
    // Admit effective Build only after the captured author/draft survived the preference read.
    // Hydration itself never changes server mode. The shared sender applies this staged choice.
    const preferences = normalizeMobilePreferences(reply.value);
    const provider = arr(client.config.providers).find(value => value.instanceId === client.providerId);
    if (!preferences.planModeEnabled || provider?.showInteractionModeToggle === false) {
      if (client.threadId) stage(client, { interactionMode: 'default' });
      else { projectedBuild = client.interactionMode !== 'default'; client.interactionMode = 'default'; }
    }
    const admittedOwner = identity();
    const assertAdmittedOwner = () => { if (admittedOwner !== identity()) throw new ClientError('The draft or model changed before the message could be sent.', 'superseded'); };
    const canSteer = queueState(client.projection).canSteer;
    const apply = () => {
      applyMobileComposerBehavior(client, reply.value);
      // Pinned mobile collapses unsupported steering into a normal queued follow-up.
      if (threadPhase(client.projection) === 'running' && !canSteer) client.local.clientSettings.followUpBehavior = 'queue';
    };
    apply();
    return await client.command('send', '', '', 0, mobileSubmissionNative(native, alternate && canSteer, apply, assertAdmittedOwner), storage);
  } catch (error) {
    if (letGo(error)) throw error;
    return { revision: client.revision, message: error instanceof Error ? error.message : 'Could not send the message.' };
  } finally {
    // Source derives Build for this submission without erasing stored Plan intent.
    // A successful launch removes its record; a changed owner keeps its own modes.
    if (projectedBuild && draft && !client.threadId && mobileComposerTargetCurrent(client, target)
      && JSON.stringify(mobileNewTaskDraftLookup(client, draft.key)?.choices) === storedChoices
      && JSON.stringify([client.providerId, client.modelId, client.modelOptions, client.runtimeMode]) === model
      && client.interactionMode === 'default') client.interactionMode = originalMode;
  }
}
