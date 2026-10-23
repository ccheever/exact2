import { requestComposerField } from './composer-field-request';
// composer.increaseEffort / composer.decreaseEffort: step the reasoning effort
// one level without opening the traits menu, wrapping at either end. Adapted
// from T3 Code 9c161462c9 (MIT); see LICENSE-T3. Sources:
// apps/web/src/components/chat/TraitsPicker.tsx (getReasoningTraitStep,
// getTraitSelectChange), composerProviderState.tsx (getComposerReasoningStep)
// and ChatComposer.tsx (stepReasoningEffort).
// Changes: a stored step is one option id and value, saved as a traits menu
// pick saves it; the prompt and caret are the native editor's.
import { arr, obj, str, type Obj } from '../client/domain';
import { bridgeReply, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { optionValue, REASONING_SELECT_DESCRIPTOR_IDS, reportedSelection, type Selection } from './composer-controls-model';
import { applyClaudePromptEffortPrefix, currentDescriptors, modelDescriptors, ULTRATHINK_PROMPT_PREFIX, ultrathinkTraits, withImplicitFastModeDefault } from './composer-provider-state';
import { composerSelection } from './composer-provider-selection';
import { fanoutSelections } from './composer-controls-fanout';
import { pendingRequests } from './requests';
import { queuedEdit } from './composer-controls-queue';
import { rewritePrompt } from './composer-ultrathink';

/** A step: the prompt after it, and the option to store (none when only the Ultrathink prefix changes). */
export type ReasoningStep = { prompt: string; option?: { id: string; value: string } };

/**
 * getReasoningTraitStep: the next option of the first reasoning select, from
 * the current one (a value outside the catalog steps from the catalog
 * default, or from the end when there is none). Ultrathink is a prompt
 * prefix, skipped where a slash command refuses it; "ultrathink" in the body
 * locks the effort. `options` carry the composer's implicit defaults.
 */
export function reasoningStep(input: { driver: string; model: string; models: Obj[]; options: Obj[]; reported: Selection | null; prompt: string;
  instanceId: string; planModeEnabled: boolean; direction: 1 | -1 }): ReasoningStep | null {
  const { prompt } = input;
  if (input.driver === 'opencode' && !input.models.some(model => model.slug === input.model.trim())) return null;
  const descriptors = currentDescriptors(modelDescriptors(input.models.find(model => model.slug === input.model), input.planModeEnabled), input.options);
  const flags = ultrathinkTraits(descriptors, prompt);
  if (flags.inBody) return null;
  const descriptor = descriptors.find(entry => entry.type === 'select' && REASONING_SELECT_DESCRIPTOR_IDS.has(str(entry.id)));
  const options = arr(descriptor?.options);
  if (!descriptor || options.length < 2) return null;
  const isPrimary = descriptor.id === flags.primaryId;
  const selection = { instanceId: input.instanceId, model: input.model, options: input.options };
  const stringValue = (value: unknown) => typeof value === 'string' ? value : null;
  const current = isPrimary && flags.controlled ? 'ultrathink' : stringValue(optionValue(descriptor, descriptor.currentValue, selection, input.reported));
  const ordered = input.direction > 0 ? options : [...options].reverse();
  const position = [current, stringValue(optionValue(descriptor, descriptor.currentValue, null, null))]
    .map(value => ordered.findIndex(option => option.id === value)).find(index => index >= 0) ?? -1;
  const injected: unknown[] = Array.isArray(descriptor.promptInjectedValues) ? descriptor.promptInjectedValues : [];
  for (const option of [...ordered.slice(position + 1), ...ordered.slice(0, Math.max(position, 0))]) {
    const value = str(option.id);
    if (!value) return null;
    if (injected.includes(value)) {
      const next = prompt.trim().length === 0 ? ULTRATHINK_PROMPT_PREFIX : applyClaudePromptEffortPrefix(prompt, 'ultrathink');
      // A slash command refuses the Ultrathink prefix, so that level is skipped.
      if (next.startsWith(ULTRATHINK_PROMPT_PREFIX)) return { prompt: next };
      continue;
    }
    return { prompt: isPrimary && flags.controlled ? prompt.replace(/^Ultrathink:\s*/i, '') : prompt, option: { id: str(descriptor.id), value } };
  }
  return null;
}

/** getComposerReasoningStep: the step, with `cursor` kept on the same character when the prefix comes or goes. */
export function composerReasoningStep(input: Parameters<typeof reasoningStep>[0] & { cursor: number }): (ReasoningStep & { cursor: number }) | null {
  const { prompt, cursor } = input;
  const step = reasoningStep(input);
  if (!step) return null;
  if (step.prompt === prompt) return { ...step, cursor };
  // Adding the prefix also trims the prompt, so the kept text starts right after the prefix.
  const keptStart = step.option ? 0 : step.prompt.length - prompt.trim().length;
  const offset = step.option ? step.prompt.length - prompt.length : keptStart - (prompt.length - prompt.trimStart().length);
  return { ...step, cursor: Math.min(step.prompt.length, Math.max(keptStart, cursor + offset)) };
}

/**
 * ChatComposer stepReasoningEffort: nothing while no provider is available, a
 * question is pending, or a queued message is being edited.
 * `store` saves the option as the traits menu does; the prompt edit is one
 * undoable replacement in the editor that keeps the caret.
 */
export async function stepReasoningEffort(client: T3Client, native: Native, direction: 1 | -1, store: (id: string, value: string) => Promise<void>): Promise<void> {
  const chosen = composerSelection(client, !!fanoutSelections(client));
  if (chosen.noProviderAvailable || !chosen.provider) return;
  if (pendingRequests(client.projection).inputs.length || queuedEdit(client)) return;
  const editor = await bridgeReply(native, { op: 'editorState' }).then(reply => reply.ok ? obj(reply.value) : null, () => null);
  const prompt = typeof editor?.text === 'string' ? editor.text : client.draft;
  const models = arr(chosen.provider.models), model = chosen.entry ? chosen.model : client.modelId;
  const descriptors = arr(obj(models.find(entry => entry.slug === model)?.capabilities).optionDescriptors);
  const step = composerReasoningStep({ driver: str(chosen.provider.driver), model, models, options: withImplicitFastModeDefault(descriptors, client.modelOptions) ?? [],
    reported: client.threadId ? reportedSelection(client.projection) : null, prompt, instanceId: chosen.instanceId,
    planModeEnabled: client.local.deviceSettings.planModeEnabled !== false, direction, cursor: typeof editor?.caret === 'number' ? editor.caret : prompt.length });
  if (!step) return;
  if (step.option) await store(step.option.id, step.option.value);
  if (step.prompt === prompt) return;
  if (!editor) { rewritePrompt(client, step.prompt); return; }
  await requestComposerField(native, { op: 'editorEdit', all: true, expect: prompt, text: step.prompt, caret: step.cursor });
}
