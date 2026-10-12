// Ports of T3 Code 1e2ecbd975 (MIT; see LICENSE-T3) tests:
// apps/web/src/components/chat/composerProviderState.test.tsx (getComposerProviderState,
// withImplicitFastModeDefault, trait controls fastMode display), packages/shared/src/
// model.test.ts (applyClaudePromptEffortPrefix, resolvePromptInjectedEffort) and
// TraitsPicker.test.ts ("still renders the prompt-controlled ultrathink label").
import { describe, expect, it } from 'bun:test';
import type { Obj } from './domain';
import { applyClaudePromptEffortPrefix, composerProviderState, currentDescriptors, outgoingPrompt, promptInjectionState,
  resolvePromptInjectedEffort, traitChoice, ultrathinkTraits, withImplicitFastModeDefault } from './composer-provider-state';

const MODEL = 'test-model';
function select(id: string, options: Array<{ id: string; label: string; isDefault?: boolean }>, injected?: string[]): Obj {
  const defaultId = options.find(option => option.isDefault)?.id;
  return { id, label: id, type: 'select', options, ...(defaultId ? { currentValue: defaultId } : {}), ...(injected?.length ? { promptInjectedValues: injected } : {}) };
}
const boolean = (id: string, currentValue?: boolean): Obj => ({ id, label: id, type: 'boolean', ...(typeof currentValue === 'boolean' ? { currentValue } : {}) });
const modelWith = (descriptors: Obj[]): Obj[] => [{ slug: MODEL, name: MODEL, isCustom: false, capabilities: { optionDescriptors: descriptors } }];
const selections = (...entries: Array<[string, string | boolean]>) => entries.map(([id, value]) => ({ id, value }));
const EFFORT = select('effort', [{ id: 'medium', label: 'Medium' }, { id: 'high', label: 'High', isDefault: true }, { id: 'ultrathink', label: 'Ultrathink' }], ['ultrathink']);

describe('composerProviderState', () => {
  it('derives a stable prompt injection state for ordinary prompt edits', () => {
    expect(promptInjectionState('Investigate this failure')).toBe('none');
    expect(promptInjectionState('Ultrathink:\nInvestigate this failure')).toBe('ultrathink');
  });
  it('uses descriptor defaults for display without dispatching them as overrides', () => {
    expect(composerProviderState({ driver: 'codex', model: MODEL, models: modelWith([select('effort', [{ id: 'low', label: 'Low' }, { id: 'high', label: 'High', isDefault: true }])]),
      options: undefined, planModeEnabled: true })).toEqual({ promptEffort: 'high', dispatchOptions: undefined, ultrathink: false });
  });
  it('lets selections override defaults and propagates them through dispatch', () => {
    expect(composerProviderState({ driver: 'codex', model: MODEL, models: modelWith([select('effort', [{ id: 'low', label: 'Low' }, { id: 'high', label: 'High', isDefault: true }]), boolean('fastMode')]),
      options: selections(['effort', 'low'], ['fastMode', true]), planModeEnabled: true })).toEqual({ promptEffort: 'low', dispatchOptions: selections(['effort', 'low'], ['fastMode', true]), ultrathink: false });
  });
  it('preserves selections that match defaults so deepMerge can overwrite prior state', () => {
    expect(composerProviderState({ driver: 'codex', model: MODEL, models: modelWith([select('effort', [{ id: 'high', label: 'High', isDefault: true }]), boolean('fastMode')]),
      options: selections(['effort', 'high'], ['fastMode', false]), planModeEnabled: true }).dispatchOptions).toEqual(selections(['effort', 'high'], ['fastMode', false]));
  });
  it('drops selections for descriptors the model does not declare', () => {
    expect(composerProviderState({ driver: 'codex', model: MODEL, models: modelWith([boolean('thinking')]), options: selections(['effort', 'max'], ['thinking', false]), planModeEnabled: true }))
      .toEqual({ promptEffort: null, dispatchOptions: selections(['thinking', false]), ultrathink: false });
  });
  it('drops the plan agent from dispatch when legacy plan mode is disabled', () => {
    const models = modelWith([select('agent', [{ id: 'build', label: 'Build', isDefault: true }, { id: 'plan', label: 'Plan' }])]);
    expect(composerProviderState({ driver: 'opencode', model: MODEL, models, options: selections(['agent', 'plan']), planModeEnabled: false }).dispatchOptions).toEqual(selections(['agent', 'build']));
  });
  it('preserves explicit options when the selected OpenCode model is absent from the catalog', () => {
    expect(composerProviderState({ driver: 'opencode', model: 'opencode/kimi-k3', models: [], options: selections(['variant', 'max'], ['agent', 'plan']), planModeEnabled: false }).dispatchOptions)
      .toEqual(selections(['variant', 'max']));
  });
  it('adds the ultrathink frame when the prompt triggers a promptInjectedValues descriptor', () => {
    expect(composerProviderState({ driver: 'codex', model: MODEL, models: modelWith([EFFORT]), options: selections(['effort', 'medium']), prompt: 'Ultrathink:\nInvestigate this failure', planModeEnabled: true }))
      .toEqual({ promptEffort: 'medium', dispatchOptions: selections(['effort', 'medium']), ultrathink: true });
  });
  it('does not add the frame when the descriptor has no promptInjectedValues', () => {
    expect(composerProviderState({ driver: 'codex', model: MODEL, models: modelWith([select('effort', [{ id: 'high', label: 'High', isDefault: true }])]), options: undefined,
      prompt: 'Ultrathink:\nInvestigate this failure', planModeEnabled: true }).ultrathink).toBe(false);
  });
  it('defaults fastMode to false when the provider reports true but the user has not selected it', () => {
    expect(composerProviderState({ driver: 'cursor', model: MODEL, models: modelWith([boolean('fastMode', true)]), options: undefined, planModeEnabled: true }).dispatchOptions).toEqual(selections(['fastMode', false]));
  });
  it('keeps explicit fastMode true when the user selected Fast', () => {
    expect(composerProviderState({ driver: 'cursor', model: MODEL, models: modelWith([boolean('fastMode', true)]), options: selections(['fastMode', true]), planModeEnabled: true }).dispatchOptions).toEqual(selections(['fastMode', true]));
  });
  it('keeps explicit fastMode false when the user selected Normal', () => {
    expect(composerProviderState({ driver: 'cursor', model: MODEL, models: modelWith([boolean('fastMode', true)]), options: selections(['fastMode', false]), planModeEnabled: true }).dispatchOptions).toEqual(selections(['fastMode', false]));
  });
});

describe('withImplicitFastModeDefault', () => {
  it('injects fastMode false only when the model exposes fastMode and no selection exists', () => {
    expect(withImplicitFastModeDefault([boolean('fastMode', true)], undefined)).toEqual(selections(['fastMode', false]));
    expect(withImplicitFastModeDefault([boolean('fastMode', true)], selections(['fastMode', true]))).toEqual(selections(['fastMode', true]));
  });
  it('does not add fastMode when the model does not expose it', () => {
    expect(withImplicitFastModeDefault([boolean('thinking', true)], undefined)).toBeUndefined();
  });
  it('resolves traits fastMode to Normal when the provider defaults to true without a user selection', () => {
    const descriptors = [boolean('fastMode', true)];
    expect(currentDescriptors(descriptors, withImplicitFastModeDefault(descriptors, undefined))[0]!.currentValue).toBe(false);
  });
});

describe('applyClaudePromptEffortPrefix', () => {
  it('keeps slash commands intact when ultrathink is selected', () => {
    expect(applyClaudePromptEffortPrefix('/compact', 'ultrathink')).toBe('/compact');
    expect(applyClaudePromptEffortPrefix(' /compact keep recent errors ', 'ultrathink')).toBe('/compact keep recent errors');
    expect(applyClaudePromptEffortPrefix(' /review src/model.ts ', 'ultrathink')).toBe('/review src/model.ts');
    expect(applyClaudePromptEffortPrefix('/security-review', 'ultrathink')).toBe('/security-review');
    expect(applyClaudePromptEffortPrefix('/plugin:skill run', 'ultrathink')).toBe('/plugin:skill run');
    expect(applyClaudePromptEffortPrefix('/deploy.prod to staging', 'ultrathink')).toBe('/deploy.prod to staging');
  });
  it('still adds the ultrathink prefix to ordinary prompts', () => {
    expect(applyClaudePromptEffortPrefix('Investigate this failure', 'ultrathink')).toBe('Ultrathink:\nInvestigate this failure');
    expect(applyClaudePromptEffortPrefix('/home/theo/app.ts crashed on load', 'ultrathink')).toBe('Ultrathink:\n/home/theo/app.ts crashed on load');
  });
  it('adds the prefix once', () => {
    expect(applyClaudePromptEffortPrefix('Ultrathink:\nInvestigate', 'ultrathink')).toBe('Ultrathink:\nInvestigate');
    expect(applyClaudePromptEffortPrefix('Investigate', 'high')).toBe('Investigate');
  });
});

describe('resolvePromptInjectedEffort', () => {
  it('returns an effort only when a select descriptor injects it', () => {
    expect(resolvePromptInjectedEffort([EFFORT], 'ultrathink')).toBe('ultrathink');
    expect(resolvePromptInjectedEffort([EFFORT], 'high')).toBeNull();
    expect(resolvePromptInjectedEffort([select('contextWindow', [{ id: '1m', label: '1M' }]), EFFORT], ' ultrathink ')).toBe('ultrathink');
    expect(resolvePromptInjectedEffort([EFFORT], '')).toBeNull();
  });
});

describe('the ultrathink traits flow (TraitsPicker handleSelectChange)', () => {
  it('rewrites the prompt and stores nothing when the injected effort is picked', () => {
    expect(traitChoice([EFFORT], '', 'effort', 'ultrathink')).toEqual({ prompt: 'Ultrathink:\n', store: false });
    expect(traitChoice([EFFORT], 'Fix the bug', 'effort', 'ultrathink')).toEqual({ prompt: 'Ultrathink:\nFix the bug', store: false });
  });
  it('locks the primary effort while "ultrathink" is in the body text', () => {
    expect(ultrathinkTraits([EFFORT], 'please ultrathink about it')).toEqual({ primaryId: 'effort', controlled: true, inBody: true });
    expect(traitChoice([EFFORT], 'please ultrathink about it', 'effort', 'medium')).toEqual({ prompt: null, store: false });
  });
  it('strips the prefix when another effort is chosen while the prompt controls it', () => {
    expect(ultrathinkTraits([EFFORT], 'Ultrathink:\nFix it')).toEqual({ primaryId: 'effort', controlled: true, inBody: false });
    expect(traitChoice([EFFORT], 'Ultrathink:\nFix it', 'effort', 'medium')).toEqual({ prompt: 'Fix it', store: true });
  });
  it('leaves other descriptors alone', () => {
    expect(traitChoice([EFFORT, boolean('thinking')], 'please ultrathink', 'thinking', 'true')).toEqual({ prompt: null, store: true });
  });
});

describe('outgoingPrompt (formatOutgoingPrompt)', () => {
  const input = { driver: 'claudeAgent', model: MODEL, models: modelWith([EFFORT]), planModeEnabled: true };
  it('prefixes an injected effort that is the stored choice only once', () => {
    // A stored "ultrathink" resolves to the default (it is prompt-injected), so only the prompt prefix carries it.
    expect(outgoingPrompt({ ...input, options: selections(['effort', 'ultrathink']) }, 'Fix it')).toBe('Fix it');
    expect(outgoingPrompt({ ...input, options: undefined }, 'Ultrathink:\nFix it')).toBe('Ultrathink:\nFix it');
    expect(outgoingPrompt({ ...input, options: undefined }, '/compact')).toBe('/compact');
  });
});
