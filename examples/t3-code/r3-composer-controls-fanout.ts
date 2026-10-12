// Several models for one new thread (lane composer-controls, round 3),
// adapted from T3 Code (MIT); see LICENSE-T3. Sources: ChatComposer.tsx
// (onToggleModel), ProviderModelPicker.tsx (selectedEntries, multipleLabel,
// allModelNames), ModelPickerContent.tsx (Shift-click or Shift+Return adds a
// model), ModelListRow.tsx (the check) and ChatView.tsx (the fan-out send: one
// background thread per model, each in its own worktree from the base branch).
import { arr, obj, str, type Obj } from './domain';
import { ClientError, launchPayload, providerAvailable, type Files, type Native } from './protocol';
import { pushToast } from './toast';
import type { T3Client } from './client';
import { triggerModelName } from './r3-composer-controls-model';
import { dispatchSelection } from './composer-ultrathink';
import { letGo } from './let-go';
import { composerPair } from './composer-provider-selection'; // composer-provider-state-and-details: the seed is the shown model

export type FanoutSelection = { instanceId: string; model: string; options: Obj[] };
// draftFanoutStateAtom: per draft route, in memory.
const fanouts = new WeakMap<object, Map<string, FanoutSelection[]>>();
const routes = (client: T3Client) => { let map = fanouts.get(client); if (!map) { map = new Map(); fanouts.set(client, map); } return map; };

/** requiredWorktreeBootstrap: the server can start a thread straight into a new worktree. */
export function fanoutSupported(client: T3Client): boolean {
  return !client.threadId && obj(obj(client.config.environment).capabilities).requiredWorktreeBootstrap === true;
}
/** The draft's selections when there are several, else null (the single composer selection stands). */
export function fanoutSelections(client: T3Client): FanoutSelection[] | null {
  if (client.threadId) return null;
  const list = routes(client).get(client.draftKey);
  return list && list.length > 1 ? list : null;
}
export function setFanout(client: T3Client, list: FanoutSelection[] | null): void {
  if (list && list.length > 1) routes(client).set(client.draftKey, list); else routes(client).delete(client.draftKey);
}

/**
 * onToggleModel: Shift adds the model to the selection (or takes it out). Two
 * or more become the fan-out; one left becomes the composer's model again.
 * Returns the single model to select, or null when the fan-out holds.
 */
export function toggleFanout(client: T3Client, instanceId: string, model: string): FanoutSelection | null {
  // selectedModelSelection: the instance and model the composer shows and sends (composer-provider-selection.ts), not its raw choice.
  const shown = composerPair(client);
  const single = { instanceId: shown.providerId, model: shown.modelId, options: client.modelOptions.map(option => ({ ...option })) };
  const current = fanoutSelections(client) ?? [single];
  const exists = current.some(selection => selection.instanceId === instanceId && selection.model === model);
  const next = exists ? current.filter(selection => !(selection.instanceId === instanceId && selection.model === model)) : [...current, { instanceId, model, options: [] }];
  if (next.length > 1) { setFanout(client, next); return null; }
  setFanout(client, null);
  return next[0] ?? single;
}

/** A Shift-click or Shift+Return in the picker (the newest pointer or Return press, T3ComposerIntent). */
export async function additiveGesture(client: T3Client, native: Native): Promise<boolean> {
  if (!fanoutSupported(client)) return false;
  const gesture = obj(await client.restAccess(native).call({ op: 'composerSendIntent' }).catch(() => ({})));
  const age = Number(gesture.ageMs);
  return Number.isFinite(age) && age >= 0 && age <= 2000 && /\bshift\b/.test(str(gesture.modifiers));
}

/** The trigger and rows while several models are chosen. */
export function fanoutView(client: T3Client) {
  const list = fanoutSelections(client);
  const providers = arr(client.config.providers);
  const entries = (list ?? []).map(selection => {
    const provider = providers.find(entry => entry.instanceId === selection.instanceId);
    const model = arr(provider?.models).find(entry => entry.slug === selection.model);
    return { key: `${selection.instanceId}:${selection.model}`, driver: str(provider?.driver),
      label: model ? `${triggerModelName(model)}${model.isUnavailable === true ? ' (Unavailable)' : ''}` : selection.model };
  });
  return {
    fanout: !!list,
    // multipleLabel: two names, then "N more"; allModelNames is the accessible name.
    fanoutLabel: !list ? '' : `${entries.slice(0, 2).map(entry => entry.label).join(', ')}${entries.length > 2 ? `, ${entries.length - 2} more` : ''}`,
    fanoutAria: !list ? '' : entries.map(entry => entry.label).join(', ') || 'Choose models',
    fanoutMarks: entries.slice(0, 3).map(entry => ({ key: entry.key, driver: entry.driver })),
    fanoutMore: Math.max(0, entries.length - 3),
    fanoutKeys: entries.map(entry => entry.key),
  };
}

/**
 * The fan-out send: every selected model starts its own background thread in a
 * new worktree from the base branch; failures keep their selections and the
 * prompt for a retry. Returns how many started.
 */
export async function sendFanout(client: T3Client, native: Native, storage: Files, input: { text: string; attachments: Obj[]; branch: string; isRepo: boolean;
  runtimeMode: string; interactionMode: string; startFromOrigin: boolean }): Promise<number> {
  const list = fanoutSelections(client);
  if (!list) return 0;
  if (obj(obj(client.config.environment).capabilities).requiredWorktreeBootstrap !== true) throw new ClientError('Update this server before starting multiple models.');
  if (!input.isRepo || !input.branch) {
    pushToast(client, { kind: 'warning', title: 'Choose models and a base branch', description: 'Multiple models need a new thread in a Git project. Each gets its own worktree.' });
    return 0;
  }
  for (const selection of list) {
    const provider = arr(client.config.providers).find(entry => entry.instanceId === selection.instanceId);
    if (!provider || !providerAvailable(provider) || provider.status !== 'ready') throw new ClientError(`Provider for ${selection.model} is unavailable.`);
  }
  const access = client.restAccess(native), projectId = client.projectId, draftKey = client.draftKey;
  const failed: FanoutSelection[] = [];
  let started = 0;
  client.local.drafts[draftKey] = '';
  for (const selection of list) {
    try {
      const [commandId, messageId, threadId] = await access.ids(3);
      const payload = launchPayload(commandId, threadId, messageId, projectId, input.text, dispatchSelection(client, selection.instanceId, selection.model, selection.options), // composer-fidelity: modelOptionsForDispatch per target
        input.runtimeMode, input.interactionMode, input.attachments);
      payload.workspaceStrategy = { type: 'worktree', baseRef: input.branch, ...(input.startFromOrigin ? { startFromOrigin: true } : {}) };
      await access.write(storage, { method: 'orchestration.launchThread', payload, description: 'Create thread', threadId, text: '', uncertain: false });
      started += 1;
    } catch (error) {
      if (letGo(error)) throw error;
      failed.push(selection);
      pushToast(client, { kind: 'error', title: `Could not start ${selection.model}`, description: error instanceof Error ? error.message : 'Failed to send message.' });
    }
  }
  if (started > 0) pushToast(client, { kind: 'success', title: `Started ${started} ${started === 1 ? 'thread' : 'threads'} in background` });
  // restoreFailedDraft: the failed models stay selected with the prompt back in an untouched composer.
  setFanout(client, failed.length ? failed : null);
  if (failed.length && !client.local.drafts[draftKey]) client.local.drafts[draftKey] = input.text;
  return started;
}
