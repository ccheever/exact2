// The composer's ultrathink prompt rewrite and the options a send dispatches
// (task composer-fidelity, G9), adapted from T3 Code 1e2ecbd975 (MIT); see
// LICENSE-T3. Sources: apps/web/src/components/chat/TraitsPicker.tsx
// (TraitsMenuContent handleSelectChange), ChatView.tsx (formatOutgoingPrompt and
// the send path's createModelSelection(..., modelOptionsForDispatch)) and
// apps/web/src/modelSelection.ts (the multi-model draft's selections).
// Changes: the prompt lives in the client's local drafts; a rewrite bumps a
// per-client serial that the requests projection appends to `requestKey`, so the
// window drops its typed copy and shows the rewritten draft (no root action).
import { arr, obj, str, type Obj } from './domain';
import { modelSelection } from './protocol';
import { composerProviderState, outgoingPrompt, traitChoice } from './composer-provider-state';

type Source = { config: Obj; providerId: string; modelId: string; draftKey: string; local: { drafts: Record<string, string>; deviceSettings: { planModeEnabled: boolean } } };

const rewrites = new WeakMap<object, number>();
/** The suffix `requestKey` carries after a prompt rewrite ('' before the first). */
export function promptRewriteKey(client: object): string {
  const serial = rewrites.get(client) ?? 0;
  return serial ? `#rewrite:${serial}` : '';
}
/** Replace the draft from the client side; the window shows it in place of its typed copy. */
export function rewritePrompt(client: Source, prompt: string): void {
  client.local.drafts[client.draftKey] = prompt;
  rewrites.set(client, (rewrites.get(client) ?? 0) + 1);
}

function model(client: Pick<Source, 'config'>, providerId: string, modelId: string) {
  const provider = arr(client.config.providers).find(entry => entry.instanceId === providerId);
  return { driver: str(provider?.driver), models: arr(provider?.models), model: arr(provider?.models).find(entry => entry.slug === modelId) };
}

/**
 * A traits pick before it is stored: true when it is fully handled here (the
 * injected effort rewrote the prompt, or the prompt's own "ultrathink" locks
 * the effort), false when the option should be stored as usual.
 */
export function ultrathinkChoice(client: Source, id: string, value: string): boolean {
  const { model: entry } = model(client, client.providerId, client.modelId);
  const descriptors = arr(obj(entry?.capabilities).optionDescriptors);
  const choice = traitChoice(descriptors, client.local.drafts[client.draftKey] ?? '', id, value);
  if (choice.prompt !== null) rewritePrompt(client, choice.prompt);
  return !choice.store;
}

/** createModelSelection(instance, model, modelOptionsForDispatch): explicit choices plus the implicit Fast-mode default. */
export function dispatchSelection(client: Pick<Source, 'config' | 'local'>, providerId: string, modelId: string, options: unknown): Obj {
  const { driver, models } = model(client, providerId, modelId);
  const state = composerProviderState({ driver, model: modelId, models, options: arr(options), planModeEnabled: client.local.deviceSettings.planModeEnabled !== false });
  return modelSelection(providerId, modelId, state.dispatchOptions ?? []);
}

/** formatOutgoingPrompt: an injected effort leads the prompt once; any other text is sent as written. */
export function promptForSend(client: Pick<Source, 'config' | 'local'>, providerId: string, modelId: string, options: unknown, text: string): string {
  const { driver, models } = model(client, providerId, modelId);
  const out = outgoingPrompt({ driver, model: modelId, models, options: arr(options), planModeEnabled: client.local.deviceSettings.planModeEnabled !== false }, text);
  return out === text.trim() ? text : out;
}

/** Whether the draft drives the ultrathink frame (the composer's ring and the model icon's chroma). */
export function ultrathinkFrame(client: Pick<Source, 'config' | 'local' | 'providerId' | 'modelId'> & { modelOptions: Obj[] }, prompt: string): boolean {
  const { driver, models } = model(client, client.providerId, client.modelId);
  return composerProviderState({ driver, model: client.modelId, models, options: client.modelOptions, prompt, planModeEnabled: client.local.deviceSettings.planModeEnabled !== false }).ultrathink;
}
