// Source365aa87982 new-task-flow-provider stores only explicitly changed draft settings.
// @ref llp/1109.005-composer-and-transcript.decision.md#settings-ownership
import type { T3Client } from './shared/client';
import { arr, obj, type Obj } from './shared/domain';
import { ClientError, type Files, type Native } from './shared/protocol';
import { mobileNewTaskDraftCurrent, mobileNewTaskDraftChoicesUpdate, mobileNewTaskDraftPersisted,
  type MobileNewTaskChoices } from './mobile-new-task-drafts';

/** These wrappers live only for this command. Its first save includes the explicit
 * choice, while staged picker rows and effective Send preferences leave it alone. */
export function mobileDraftSettingsHandles(client: T3Client, op: string, native: Native | null | undefined, storage: Files) {
  const draft = mobileNewTaskDraftCurrent(client);
  if (!draft || !native || !['provider', 'model', 'model-option', 'runtime', 'interaction'].includes(op)) return { native, storage };
  const origin = client.origin, environmentId = client.environmentId, generation = client.generation;
  const check = () => {
    const current = mobileNewTaskDraftCurrent(client);
    if (client.origin !== origin || client.environmentId !== environmentId || client.generation !== generation
      || client.threadId || client.draftKey !== draft.key || current?.key !== draft.key
      || current.projectId !== draft.projectId || current.origin !== draft.origin)
      throw new ClientError('The draft changed while settings were saving.', 'superseded');
  };
  const scoped: Native = { available: native.available, watch: topic => native.watch(topic), later: async input => {
    check(); const reply = await native.later(input); check(); return reply;
  } };
  const files: Files = { fs: { ...storage.fs, atomicWriteFile: async (path, bytes) => {
    check();
    const choices: MobileNewTaskChoices = {};
    if (['provider', 'model', 'model-option'].includes(op)) {
      choices.providerId = client.providerId; choices.modelId = client.modelId; choices.modelOptions = client.modelOptions;
      if (op !== 'model-option' && arr(client.config.providers).find(provider => provider.instanceId === client.providerId)?.showInteractionModeToggle === false) {
        client.interactionMode = 'default'; choices.interactionMode = 'default';
      }
    } else if (op === 'runtime') choices.runtimeMode = client.runtimeMode;
    else choices.interactionMode = client.interactionMode;
    if (!mobileNewTaskDraftChoicesUpdate(client, draft.key, choices)) throw new ClientError('The draft changed.', 'superseded');
    // MobileDraftClient already encoded this document. Replace only its extension
    // so the initial durable save contains the choice made by this command.
    const document = obj(JSON.parse(new TextDecoder().decode(bytes)));
    document.mobileNewTaskDrafts = mobileNewTaskDraftPersisted(client) as unknown as Obj;
    await storage.fs.atomicWriteFile(path, new TextEncoder().encode(JSON.stringify(document))); check();
  } } };
  return { native: scoped, storage: files };
}
