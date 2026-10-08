// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/components/settings/
// customModelEditor.logic.test.ts ("customModelEditor.logic", every case, original names; an
// `it.each` is one test per row) and ProviderInstanceCard.test.ts (:20, :50,
// "deriveProviderModelsForDisplay"). Clone cases for the editor session follow.
import { describe, expect, it, test } from 'bun:test';
import type { Obj } from './domain';
import {
  DESCRIPTOR_PRESETS_BY_KIND, customModelEditorOp, customModelEditorView, definitionFromDraft, deriveProviderModelsForDisplay, descriptorFromPreset,
  descriptorsFromCapabilities, draftFromDefinition, readCustomModelEntries, storedCustomModels, toCustomModelSetting, validateDraft,
  type CustomModelDraft, type ModelCapabilities,
} from './custom-model-editor';

const draft = (overrides: Partial<CustomModelDraft>): CustomModelDraft => ({ slug: 'my-model', name: '', descriptors: [], ...overrides });

describe('customModelEditor.logic', () => {
  it('round-trips a definition through the draft, marking the current value as default', () => {
    const definition = definitionFromDraft(draft({ name: ' My Model ', descriptors: [
      { key: 'a', type: 'select', id: 'reasoningEffort', label: 'Reasoning', choices: [{ key: 'a1', id: 'low', label: 'Low', isDefault: false }, { key: 'a2', id: 'high', label: '', isDefault: true }] },
      { key: 'b', type: 'boolean', id: 'fastMode', label: 'Fast Mode', choices: [] },
    ] }));
    expect(definition).toEqual({ slug: 'my-model', name: 'My Model', capabilities: { optionDescriptors: [
      { id: 'reasoningEffort', label: 'Reasoning', type: 'select', options: [{ id: 'low', label: 'Low' }, { id: 'high', label: 'high', isDefault: true }], currentValue: 'high' },
      { id: 'fastMode', label: 'Fast Mode', type: 'boolean' },
    ] } });
    const reopened = draftFromDefinition(definition);
    expect(reopened.name).toBe('My Model');
    expect(reopened.descriptors.map(descriptor => descriptor.id)).toEqual(['reasoningEffort', 'fastMode']);
    expect(reopened.descriptors[0]!.choices.map(choice => choice.isDefault)).toEqual([false, true]);
  });

  it('preserves the current choice when it differs from the built-in default', () => {
    const descriptors = descriptorsFromCapabilities({ optionDescriptors: [{ id: 'effort', label: 'Reasoning', type: 'select', currentValue: 'high',
      options: [{ id: 'low', label: 'Low', isDefault: true }, { id: 'high', label: 'High' }] }] }, 'claudeAgent');
    expect(descriptors[0]!.choices.map(choice => choice.isDefault)).toEqual([false, true]);
    expect(definitionFromDraft(draft({ descriptors })).capabilities?.optionDescriptors?.[0]).toMatchObject({ currentValue: 'high' });
  });

  it("drops prompt-injected choices when copying a built-in's descriptors", () => {
    const [copied] = descriptorsFromCapabilities({ optionDescriptors: [{ id: 'effort', label: 'Reasoning', type: 'select',
      options: [{ id: 'high', label: 'High', isDefault: true }, { id: 'ultrathink', label: 'Ultrathink' }], promptInjectedValues: ['ultrathink'] }] }, 'claudeAgent');
    expect(copied!.choices.map(choice => choice.id)).toEqual(['high']);
  });

  for (const currentValue of [true, false, undefined]) {
    it(`preserves boolean values through copy and edit: ${currentValue}`, () => {
      const capabilities: ModelCapabilities = { optionDescriptors: [{ id: 'thinking', label: 'Thinking', type: 'boolean', ...(currentValue !== undefined ? { currentValue } : {}) }] };
      const copied = definitionFromDraft(draft({ descriptors: descriptorsFromCapabilities(capabilities, 'cursor') }));
      expect(copied.capabilities).toEqual(capabilities);
      const reopened = draftFromDefinition(copied);
      expect(definitionFromDraft({ ...reopened, name: 'Renamed' }).capabilities).toEqual(capabilities);
    });
  }

  it('excludes Claude context choices from presets and copies without changing other providers or authored entries', () => {
    const capabilities: ModelCapabilities = { optionDescriptors: [{ id: 'contextWindow', label: 'Context', type: 'select', options: [{ id: '1m', label: '1M', isDefault: true }] },
      { id: 'thinking', label: 'Thinking', type: 'boolean', currentValue: true }] };
    const copied = definitionFromDraft(draft({ descriptors: descriptorsFromCapabilities(capabilities, 'claudeAgent') }));
    expect(copied.capabilities?.optionDescriptors).toEqual([capabilities.optionDescriptors[1]!]);
    const presets = definitionFromDraft(draft({ descriptors: (DESCRIPTOR_PRESETS_BY_KIND.claudeAgent ?? []).map(descriptorFromPreset) }));
    expect(presets.capabilities?.optionDescriptors?.some(option => option.id === 'contextWindow')).toBe(false);
    const cursorCopy = descriptorsFromCapabilities(capabilities, 'cursor');
    expect(cursorCopy.map(option => option.id)).toEqual(['contextWindow', 'thinking']);
    const authored = { slug: 'custom', name: 'Custom', capabilities };
    expect(definitionFromDraft(draftFromDefinition(authored)).capabilities?.optionDescriptors?.[0]).toMatchObject(capabilities.optionDescriptors[0]!);
  });

  it('preserves choice descriptions when copying, renaming, and saving', () => {
    const capabilities: ModelCapabilities = { optionDescriptors: [{ id: 'effort', label: 'Reasoning', description: 'Choose a reasoning level.', type: 'select',
      options: [{ id: 'high', label: 'High', isDefault: true }, { id: 'ultracode', label: 'Ultracode', description: 'Uses additional reasoning.' }] }] };
    const copied = definitionFromDraft(draft({ descriptors: descriptorsFromCapabilities(capabilities, 'claudeAgent') }));
    const reopened = draftFromDefinition(copied);
    const saved = definitionFromDraft({ ...reopened, name: 'Renamed' });
    expect(saved.capabilities?.optionDescriptors?.[0]).toMatchObject(capabilities.optionDescriptors[0]!);
    expect(copied.capabilities?.optionDescriptors?.[0]).toMatchObject(capabilities.optionDescriptors[0]!);
  });

  for (const [driver, presets] of Object.entries(DESCRIPTOR_PRESETS_BY_KIND)) {
    it(`offers saveable presets for ${driver}`, () => {
      expect(validateDraft(draft({ descriptors: (presets ?? []).map(descriptorFromPreset) }))).toBeNull();
    });
  }

  it('collapses a blank name and no options back to a bare definition', () => {
    expect(definitionFromDraft(draft({ name: '  ' }))).toEqual({ slug: 'my-model', name: 'my-model', capabilities: null });
    expect(draftFromDefinition({ slug: 'x', name: 'x', capabilities: null }).name).toBe('');
  });

  it('rejects duplicate ids, blank ids, and selects without choices', () => {
    const select = (id: string, choices: { id: string }[]) => ({ key: id, type: 'select' as const, id, label: 'Label', choices: choices.map(choice => ({ key: choice.id, label: '', isDefault: false, ...choice })) });
    expect(validateDraft(draft({ descriptors: [select('', [{ id: 'a' }])] }))).toBe('Option 1 needs an id.');
    expect(validateDraft(draft({ descriptors: [select('effort', [{ id: 'a' }]), select('effort', [{ id: 'b' }])] }))).toBe('Option 2: id "effort" is used twice.');
    expect(validateDraft(draft({ descriptors: [select('effort', [])] }))).toBe('Option 1 needs at least one choice.');
    expect(validateDraft(draft({ descriptors: [select('effort', [{ id: 'a' }, { id: 'a' }])] }))).toBe('Option 1: choice "a" is used twice.');
    expect(validateDraft(draft({ descriptors: [select('effort', [{ id: 'a' }])] }))).toBeNull();
  });
});

describe('deriveProviderModelsForDisplay', () => {
  it('uses current config custom models instead of stale live custom rows', () => {
    const liveModels: Obj[] = [{ slug: 'server-model', name: 'Server Model', isCustom: false, capabilities: null },
      { slug: 'removed-custom', name: 'Removed Custom', isCustom: true, capabilities: null }, { slug: 'kept-custom', name: 'Kept Custom', isCustom: true, capabilities: null }];
    expect(deriveProviderModelsForDisplay({ liveModels, customModels: [{ slug: 'kept-custom', name: 'kept-custom', capabilities: null }] }).map(model => model.slug))
      .toEqual(['server-model', 'kept-custom']);
  });

  it("prefers the entry's name and capabilities over the stale live custom row", () => {
    const liveCapabilities = { optionDescriptors: [] };
    const customCapabilities: ModelCapabilities = { optionDescriptors: [{ id: 'reasoningEffort', label: 'Reasoning', type: 'select', options: [{ id: 'high', label: 'High', isDefault: true }], currentValue: 'high' }] };
    const liveModels: Obj[] = [{ slug: 'bare', name: 'bare', isCustom: true, capabilities: liveCapabilities }, { slug: 'named', name: 'named', isCustom: true, capabilities: liveCapabilities }];
    const display = deriveProviderModelsForDisplay({ liveModels, customModels: [{ slug: 'bare', name: 'bare', capabilities: null }, { slug: 'named', name: 'My Model', capabilities: customCapabilities }] });
    // A bare entry keeps the driver default the server filled in.
    expect(display[0]).toEqual({ slug: 'bare', name: 'bare', isCustom: true, capabilities: liveCapabilities });
    expect(display[1]).toEqual({ slug: 'named', name: 'My Model', isCustom: true, capabilities: customCapabilities as unknown as Obj });
  });
});

// --- Clone: the stored form and the editor session -------------------------------------------

test('readCustomModelEntries and toCustomModelSetting keep the reference storage (bare slugs unless named or optioned)', () => {
  const entries = readCustomModelEntries([' a ', { slug: 'a' }, { slug: 'b', name: 'Bee' }, { slug: 'c', capabilities: { optionDescriptors: [{ id: 'x' }] } }, 7, null]);
  expect(entries).toEqual([{ slug: 'a', name: 'a', capabilities: null }, { slug: 'b', name: 'Bee', capabilities: null }, { slug: 'c', name: 'c', capabilities: null }]);
  expect(entries.map(toCustomModelSetting)).toEqual(['a', { slug: 'b', name: 'Bee' }, 'c']);
  expect(storedCustomModels('acpRegistry', entries)).toEqual(['a', 'b', 'c']);
});

test('the editor session: open, type, apply a preset, add and default choices, copy from a built-in, validate and save', () => {
  const owner = {};
  const host = { config: { settings: { providerInstances: { codex_work: { driver: 'codex', config: { customModels: [{ slug: 'gpt-x', name: 'GPT X' }] } } } },
    providers: [{ instanceId: 'codex_work', models: [{ slug: 'gpt-5', name: 'GPT-5', isCustom: false, capabilities: { optionDescriptors: [{ id: 'reasoningEffort', label: 'Reasoning', type: 'select',
      options: [{ id: 'low', label: 'Low' }, { id: 'high', label: 'High', isDefault: true }] }] } }] }] } };
  const live = host.config.providers[0]!.models as Obj[];
  customModelEditorOp(owner, host, 'open', 'codex_work', 'gpt-x', '');
  let view = customModelEditorView(owner, 'codex_work', 'codex', live)[0]!;
  expect([view.slug, view.name, view.empty, view.addPresets.map(preset => preset.label), view.copyFrom.map(model => model.value)]).toEqual(['gpt-x', 'GPT X', true, ['Reasoning', 'Speed'], ['gpt-5']]);
  expect(customModelEditorView(owner, 'other', 'codex', live)).toEqual([]); // another card: the editor closed
  customModelEditorOp(owner, host, 'open', 'codex_work', 'gpt-x', '');
  customModelEditorOp(owner, host, 'add-option', 'codex_work', 'reasoningEffort', '');
  customModelEditorOp(owner, host, 'add-option', 'codex_work', '', '');
  view = customModelEditorView(owner, 'codex_work', 'codex', live)[0]!;
  expect(view.descriptors.map(descriptor => [descriptor.position, descriptor.idSelectLabel, descriptor.label, descriptor.choices.length])).toEqual([['Option 1', 'reasoningEffort', 'Reasoning', 4], ['Option 2', 'Custom…', '', 0]]);
  expect(customModelEditorOp(owner, host, 'save', 'codex_work', '', '')).toBeNull();
  expect(customModelEditorView(owner, 'codex_work', 'codex', live)[0]!.error).toBe('Option 2 needs an id.');
  const custom = view.descriptors[1]!.ref, keyBefore = view.descriptors[1]!.key;
  customModelEditorOp(owner, host, 'text', 'codex_work', `d:${custom}:id`, 'verbosity');
  expect(customModelEditorView(owner, 'codex_work', 'codex', live)[0]!.error).toBe(''); // a change clears the message
  customModelEditorOp(owner, host, 'text', 'codex_work', `d:${custom}:label`, 'Verbosity');
  expect(customModelEditorView(owner, 'codex_work', 'codex', live)[0]!.descriptors[1]!.key).toBe(keyBefore); // typing keeps the row
  customModelEditorOp(owner, host, 'add-choice', 'codex_work', custom, '');
  const choice = customModelEditorView(owner, 'codex_work', 'codex', live)[0]!.descriptors[1]!.choices[0]!;
  customModelEditorOp(owner, host, 'text', 'codex_work', `c:${choice.ref}:id`, 'terse');
  customModelEditorOp(owner, host, 'default', 'codex_work', choice.ref, 'true');
  customModelEditorOp(owner, host, 'text', 'codex_work', 'name', '  Work model ');
  const saved = customModelEditorOp(owner, host, 'save', 'codex_work', '', '')!;
  expect(saved.name).toBe('Work model');
  expect(saved.capabilities?.optionDescriptors.map(descriptor => [descriptor.id, descriptor.currentValue])).toEqual([['reasoningEffort', 'medium'], ['verbosity', 'terse']]);
  customModelEditorOp(owner, host, 'copy-from', 'codex_work', 'gpt-5', '');
  view = customModelEditorView(owner, 'codex_work', 'codex', live)[0]!;
  expect(view.descriptors.map(descriptor => descriptor.id)).toEqual(['reasoningEffort']);
  customModelEditorOp(owner, host, 'preset-id', 'codex_work', view.descriptors[0]!.ref, 'serviceTier');
  const swapped = customModelEditorView(owner, 'codex_work', 'codex', live)[0]!.descriptors[0]!;
  expect([swapped.label, swapped.choices.map(item => item.id), swapped.key !== view.descriptors[0]!.key]).toEqual(['Speed', ['default', 'fast'], true]);
  customModelEditorOp(owner, host, 'cancel', 'codex_work', '', '');
  expect(customModelEditorView(owner, 'codex_work', 'codex', live)).toEqual([]);
});
