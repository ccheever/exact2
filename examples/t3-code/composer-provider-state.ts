// The composer's provider state: the Claude "ultrathink" prompt flow, the
// implicit Fast-mode default and the options a send dispatches (task
// composer-fidelity, G9), adapted from T3 Code 1e2ecbd975 (MIT); see LICENSE-T3.
// Sources: apps/web/src/components/chat/composerProviderState.tsx
// (getComposerPromptInjectionState, withImplicitFastModeDefault,
// getComposerProviderState), packages/shared/src/model.ts
// (isClaudeUltrathinkPrompt, applyClaudePromptEffortPrefix,
// resolvePromptInjectedEffort, getProviderOptionDescriptors,
// buildExplicitProviderOptionSelectionsFromDescriptors), apps/web/src/providerModels.ts
// (getProviderModelCapabilities, withoutPlanAgentOption), TraitsPicker.tsx
// (getSelectedTraits' ultrathink flags, handleSelectChange) and ChatView.tsx
// (formatOutgoingPrompt).
// Changes: no React; descriptors and selections are the clone's plain records;
// the frame's class names become one `ultrathink` flag the Contract view reads;
// the descriptor's current value folds the stored choice through the clone's
// `resolvedCurrent` (r3-composer-controls-model.ts, withDescriptorCurrentValue).
import { arr, obj, str, type Obj } from './domain';
import { resolveSelectableModel, resolvedCurrent } from './r3-composer-controls-model';

export const ULTRATHINK_PROMPT_PREFIX = 'Ultrathink:\n';

/** isClaudeUltrathinkPrompt: the word "ultrathink" anywhere, any case. */
export function isClaudeUltrathinkPrompt(text: unknown): boolean {
  return typeof text === 'string' && /\bultrathink\b/i.test(text);
}
export type PromptInjectionState = 'none' | 'ultrathink';
export function promptInjectionState(prompt: string): PromptInjectionState {
  return isClaudeUltrathinkPrompt(prompt) ? 'ultrathink' : 'none';
}

/**
 * applyClaudePromptEffortPrefix: the injected effort leads the prompt once. A
 * slash command (any first token without a second slash) keeps no prefix, so
 * Claude still runs it; an absolute path is prose and is prefixed.
 */
export function applyClaudePromptEffortPrefix(text: string, effort: string | null | undefined): string {
  const trimmed = text.trim();
  if (!trimmed) return trimmed;
  if (effort !== 'ultrathink' || /^\/[^\s/]+(?:\s|$)/u.test(trimmed)) return trimmed;
  if (trimmed.startsWith('Ultrathink:')) return trimmed;
  return `${ULTRATHINK_PROMPT_PREFIX}${trimmed}`;
}

const injectedValues = (descriptor: Obj | undefined): string[] => Array.isArray(descriptor?.promptInjectedValues)
  ? (descriptor.promptInjectedValues as unknown[]).filter((value): value is string => typeof value === 'string') : [];

/** resolvePromptInjectedEffort: the effort when any select descriptor injects it into the prompt, else null. */
export function resolvePromptInjectedEffort(descriptors: Obj[], rawEffort: string | null | undefined): string | null {
  const trimmed = typeof rawEffort === 'string' ? rawEffort.trim() : '';
  if (!trimmed) return null;
  return descriptors.some(descriptor => descriptor.type === 'select' && injectedValues(descriptor).includes(trimmed)) ? trimmed : null;
}

/** withoutPlanAgentOption: with plan mode off the "plan" agent is neither selectable nor dispatched. */
export function modelDescriptors(model: Obj | undefined, planModeEnabled: boolean): Obj[] {
  const descriptors = arr(obj(model?.capabilities).optionDescriptors);
  if (planModeEnabled) return descriptors;
  return descriptors.flatMap(descriptor => {
    if (descriptor.type !== 'select' || descriptor.id !== 'agent') return [descriptor];
    const options = arr(descriptor.options).filter(option => option.id !== 'plan');
    if (!options.length) return [];
    const currentValue = descriptor.currentValue && options.some(option => option.id === descriptor.currentValue)
      ? str(descriptor.currentValue) : str(options.find(option => option.isDefault === true)?.id, str(options[0]?.id));
    return [{ ...descriptor, options, ...(currentValue ? { currentValue } : {}) }];
  });
}

/**
 * withImplicitFastModeDefault: a provider may report Fast as its default; the
 * clone treats Fast as chosen only when the user chose it, so a model with a
 * boolean `fastMode` descriptor and no choice dispatches `fastMode: false`.
 */
export function withImplicitFastModeDefault(descriptors: Obj[], options: Obj[] | null | undefined): Obj[] | undefined {
  if (options?.some(selection => selection.id === 'fastMode')) return options;
  if (!descriptors.some(descriptor => descriptor.type === 'boolean' && descriptor.id === 'fastMode')) return options ?? undefined;
  return [...(options ?? []), { id: 'fastMode', value: false }];
}

/** getProviderOptionDescriptors: each descriptor with its current value (the stored choice, else its default). */
export function currentDescriptors(descriptors: Obj[], selections: Obj[] | undefined): Obj[] {
  return descriptors.map(descriptor => {
    const value = resolvedCurrent(descriptor, selections ?? []);
    const next: Obj = { ...descriptor };
    if (value === undefined || value === '') delete next.currentValue; else next.currentValue = value;
    return next;
  });
}

/** getProviderOptionCurrentValue without a selection context: a boolean's value, a select's value or its default. */
function currentValue(descriptor: Obj): unknown {
  if (descriptor.type === 'boolean') return descriptor.currentValue;
  if (descriptor.currentValue) return descriptor.currentValue;
  return arr(descriptor.options).find(option => option.isDefault === true)?.id;
}

/** buildExplicitProviderOptionSelectionsFromDescriptors: only the ids the user chose, normalized by the descriptors. */
export function explicitSelections(descriptors: Obj[], selections: Obj[] | undefined): Obj[] | undefined {
  if (!selections?.length) return undefined;
  const explicit = new Set(selections.map(selection => str(selection.id)));
  const normalized: Obj[] = [];
  for (const descriptor of descriptors) {
    const value = currentValue(descriptor);
    if ((typeof value === 'string' || typeof value === 'boolean') && explicit.has(str(descriptor.id))) normalized.push({ id: str(descriptor.id), value });
  }
  return normalized.length ? normalized : undefined;
}

export type ComposerProviderState = { promptEffort: string | null; dispatchOptions: Obj[] | undefined; ultrathink: boolean };

/**
 * getComposerProviderState: the primary select descriptor's effort, the
 * options a send dispatches, and whether the prompt drives the ultrathink
 * frame. An OpenCode model the catalog no longer lists keeps its saved options.
 */
export function composerProviderState(input: { driver: string; model: string; models: Obj[]; options: Obj[] | null | undefined;
  prompt?: string; planModeEnabled: boolean }): ComposerProviderState {
  const { driver, models, options, planModeEnabled } = input;
  const slug = resolveSelectableModel(driver, input.model, models);
  if (driver === 'opencode' && !models.some(candidate => candidate.slug === (input.model ?? '').trim())) {
    const kept = options?.filter(option => planModeEnabled || option.id !== 'agent' || option.value !== 'plan');
    return { promptEffort: null, dispatchOptions: kept && kept.length ? kept : undefined, ultrathink: false };
  }
  const base = modelDescriptors(models.find(candidate => candidate.slug === slug), planModeEnabled);
  const selections = withImplicitFastModeDefault(base, options);
  const descriptors = currentDescriptors(base, selections);
  const primary = descriptors.find(descriptor => descriptor.type === 'select');
  const value = primary ? currentValue(primary) : undefined;
  return { promptEffort: typeof value === 'string' ? value : null, dispatchOptions: explicitSelections(descriptors, selections),
    ultrathink: injectedValues(primary).length > 0 && promptInjectionState(input.prompt ?? '') === 'ultrathink' };
}

/** formatOutgoingPrompt: the text a send dispatches (ultrathink prefixed once; the attachment-only text is the caller's). */
export function outgoingPrompt(input: { driver: string; model: string; models: Obj[]; options: Obj[] | null | undefined; planModeEnabled: boolean }, text: string): string {
  const state = composerProviderState({ ...input, prompt: text });
  const slug = resolveSelectableModel(input.driver, input.model, input.models);
  const descriptors = arr(obj(input.models.find(candidate => candidate.slug === slug)?.capabilities).optionDescriptors);
  return applyClaudePromptEffortPrefix(text, resolvePromptInjectedEffort(descriptors, state.promptEffort));
}

/**
 * getSelectedTraits' prompt flags: the prompt controls the primary effort
 * (shown as "Ultrathink"), and "ultrathink" in the body (not only the prefix)
 * locks the primary effort's rows.
 */
export function ultrathinkTraits(descriptors: Obj[], prompt: string) {
  const primary = descriptors.find(descriptor => descriptor.type === 'select');
  const controlled = injectedValues(primary).length > 0 && isClaudeUltrathinkPrompt(prompt);
  const inBody = controlled && isClaudeUltrathinkPrompt(prompt.replace(/^Ultrathink:\s*/i, ''));
  return { primaryId: str(primary?.id), controlled, inBody };
}

export const ULTRATHINK_LOCKED_MESSAGE = 'Your prompt contains "ultrathink" in the text. Remove it to change this option.';

/**
 * TraitsMenuContent handleSelectChange: an injected value rewrites the prompt
 * to start with "Ultrathink:" and stores no option; with "ultrathink" in the
 * body the primary effort does not change; choosing another effort while the
 * prompt controls it strips the prefix and stores the choice.
 */
export function traitChoice(descriptors: Obj[], prompt: string, id: string, value: string): { prompt: string | null; store: boolean } {
  const descriptor = descriptors.find(entry => entry.id === id);
  const flags = ultrathinkTraits(descriptors, prompt);
  if (!value) return { prompt: null, store: false };
  if (descriptor?.type === 'select' && injectedValues(descriptor).includes(value)) {
    return { prompt: prompt.trim().length === 0 ? ULTRATHINK_PROMPT_PREFIX : applyClaudePromptEffortPrefix(prompt, 'ultrathink'), store: false };
  }
  if (flags.inBody && id === flags.primaryId) return { prompt: null, store: false };
  if (flags.controlled && id === flags.primaryId) return { prompt: prompt.replace(/^Ultrathink:\s*/i, ''), store: true };
  return { prompt: null, store: true };
}
