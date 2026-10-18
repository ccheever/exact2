import { describe, expect, test } from 'bun:test';
import { reasoningStep, composerReasoningStep, stepReasoningEffort } from '../../src/composer/composer-effort-step';
import { traitsDisplay } from '../../src/composer/composer-controls-model';
import { scoreModel } from '../../src/composer/model-catalog';
import { resolveTextGenerationSelection } from '../../src/settings/text-generation-model';
import { sourceControlRows, scopedPatch } from '../../src/settings/source-control-view';
import { subagent } from '../../src/timeline/timeline-events';
import { keyboardDispatch } from '../../src/shell/keyboard-dispatch';
import { keybindingCommands, keybindingDefaults } from '../../src/settings/keybinding-settings';
import { obj } from '../../src/client/domain';
import { promptRewriteKey } from '../../src/composer/composer-ultrathink';
import { opened, effort, provider } from '../support/composer-controls-fixture';
import { contractComponent } from '../support/contract-action-fixture';

const tier = { id: 'serviceTier', type: 'select', options: [{ id: 'default', label: 'Normal', isDefault: true }, { id: 'priority', label: 'Fast' }, { id: 'flex', label: 'Ultrafast' }] };
const models = [{ slug: 'a', capabilities: { optionDescriptors: [effort, tier] } }];
const base = { driver: 'codex', instanceId: 'codex', model: 'a', models, options: [], reported: null, prompt: 'Read this', planModeEnabled: true, direction: 1 as const };

describe('effort keys', () => {
  test('step, reverse, wrap, and fall back from a reported value outside the catalog', () => {
    expect(reasoningStep(base)?.option).toEqual({ id: 'reasoningEffort', value: 'high' });
    expect(reasoningStep({ ...base, direction: -1 })?.option?.value).toBe('low');
    expect(reasoningStep({ ...base, options: [{ id: 'reasoningEffort', value: 'high' }] })?.option?.value).toBe('low');
    expect(reasoningStep({ ...base, options: [{ id: 'reasoningEffort', value: 'low' }], direction: -1 })?.option?.value).toBe('high');
    expect(reasoningStep({ ...base, reported: { instanceId: 'codex', model: 'a', options: [{ id: 'reasoningEffort', value: 'default' }] } })?.option?.value).toBe('high');
    expect(reasoningStep({ ...base, models: [{ slug: 'a' }] })).toBeNull();
  });
  test('Ultrathink adds and removes the prefix with the caret, skips slash commands and locks body text', () => {
    const ultra = { ...effort, promptInjectedValues: ['ultrathink'], options: [...effort.options, { id: 'ultrathink', label: 'Ultrathink' }] };
    const input = { ...base, driver: 'claudeAgent', models: [{ slug: 'a', capabilities: { optionDescriptors: [ultra] } }], options: [{ id: 'reasoningEffort', value: 'high' }], cursor: 6 };
    expect(composerReasoningStep(input)).toEqual({ prompt: 'Ultrathink:\nRead this', cursor: 18 });
    expect(composerReasoningStep({ ...input, prompt: 'Ultrathink:\nRead this', cursor: 18 })).toEqual({ prompt: 'Read this', cursor: 6, option: { id: 'reasoningEffort', value: 'low' } });
    expect(reasoningStep({ ...input, prompt: '/help' })).toEqual({ prompt: '/help', option: { id: 'reasoningEffort', value: 'low' } });
    expect(reasoningStep({ ...input, prompt: 'Please ultrathink about this' })).toBeNull();
  });
  test('a prompt edit keeps the native caret and a refused edit preserves newer typing', async () => {
    const { client, native } = await opened();
    const ultra = { ...effort, promptInjectedValues: ['ultrathink'], options: [...effort.options, { id: 'ultrathink', label: 'Ultrathink' }] };
    client.config.providers = [{ ...provider('codex'), models: [{ slug: 'model-a', capabilities: { optionDescriptors: [ultra] } }] }];
    client.modelOptions = [{ id: 'reasoningEffort', value: 'high' }];
    client.local.drafts[client.draftKey] = 'Read this';
    const later = native.later.bind(native), edits: unknown[] = [];
    let apply = true;
    native.later = async input => {
      const request = obj(input);
      if (request.op === 'editorState') return native.ok({ text: 'Read this', caret: 6 });
      if (request.op === 'editorEdit') { edits.push(request); return native.ok({ applied: apply }); }
      return later(input);
    };
    await stepReasoningEffort(client, native, 1, async () => {});
    expect(edits[0]).toMatchObject({ expect: 'Read this', text: 'Ultrathink:\nRead this', caret: 18 });
    expect(client.draft).toBe('Ultrathink:\nRead this');
    apply = false;
    client.local.drafts[client.draftKey] = 'Read this plus new typing';
    await stepReasoningEffort(client, native, 1, async () => {});
    expect(client.draft).toBe('Read this plus new typing');
    expect(promptRewriteKey(client)).toBe('');
  });
  test('new commands are unbound until configured, dispatch to the composer and preserve other options', async () => {
    const { client, command } = await opened();
    for (const name of ['composer.increaseEffort', 'composer.decreaseEffort']) {
      expect(keybindingCommands).toContain(name);
      expect(keybindingDefaults.some(binding => binding.command === name)).toBe(false);
    }
    const context = { composerFocus: true, editableFocus: true, turnRunning: false, modelPickerOpen: false, draftThreadRoute: false, modalOpen: false, settingsOpen: false, diffOpen: false };
    expect(keyboardDispatch(client, [], '', '', context).some(item => item.command === 'composer.increaseEffort')).toBe(false);
    client.config.keybindings = [{ command: 'composer.increaseEffort', shortcut: { key: 'i', modKey: true } }];
    expect(keyboardDispatch(client, [], '', '', context).find(item => item.command === 'composer.increaseEffort')).toMatchObject({ kind: 'command', target: 'effort-step', extra: '1' });
    await command('model-option', 'fastMode', 'true');
    await command('effort-step', '1');
    expect(client.modelOptions).toContainEqual({ id: 'reasoningEffort', value: 'high' });
    expect(client.modelOptions).toContainEqual({ id: 'fastMode', value: true });
  });
});

describe('traits and text model settings', () => {
  test('speed follows reasoning in text, including labels around it', () => {
    const context = { id: 'context', type: 'select', options: [{ id: '1m', label: '1M', isDefault: true }] };
    expect(traitsDisplay('codex', [context, effort, tier], [{ id: 'serviceTier', value: 'flex' }])).toEqual({ label: '1M · Medium Ultrafast' });
  });
  test('unlisted text models choose the cheap provider default before its flagship', () => {
    const p = { ...provider('claude', 'claudeAgent'), models: [{ slug: 'opus', isDefault: true }, { slug: 'claude-haiku-5-5', name: 'Haiku' }] };
    expect(resolveTextGenerationSelection({ instanceId: 'claude', model: 'gone' }, [p])?.model?.slug).toBe('claude-haiku-5-5');
    expect(resolveTextGenerationSelection({ instanceId: 'missing', model: 'opus' }, [p])?.model?.slug).toBe('claude-haiku-5-5');
    expect(resolveTextGenerationSelection({ instanceId: 'claude', model: 'opus' }, [p])?.model?.slug).toBe('opus');
  });
  test('writer and review-guide models expose and save their own traits at either scope', () => {
    const p = { ...provider('codex'), models };
    const selection = { instanceId: 'codex', model: 'a', options: [{ id: 'serviceTier', value: 'priority' }] };
    const settings = { sourceControlWriterModelSelection: selection, reviewGuideModelSelection: selection };
    const rows = sourceControlRows(settings, '', 'Local', [p], true).text;
    for (const key of ['sourceControlWriterModelSelection', 'reviewGuideModelSelection']) {
      expect(rows.find(row => row.key === key)).toMatchObject({ checked: true, traitsLabel: 'Medium Fast' });
      expect(scopedPatch(settings, '', key, 'codex:a#reasoningEffort=high')[key]).toMatchObject({ options: [{ id: 'serviceTier', value: 'priority' }, { id: 'reasoningEffort', value: 'high' }] });
      expect(scopedPatch(settings, 'p', key, 'codex:a#reasoningEffort=low').projectSettingsOverrides).toMatchObject({ p: { [key]: { options: [{ id: 'serviceTier', value: 'priority' }, { id: 'reasoningEffort', value: 'low' }] } } });
    }
  });
  test('the timeline hover card receives only matching subagent model traits', () => {
    const selection = { instanceId: 'codex', model: 'a', options: [{ id: 'reasoningEffort', value: 'high' }, { id: 'serviceTier', value: 'flex' }] };
    const item = { id: 'agent', title: 'Explore', model: 'a', providerInstanceId: 'codex', status: 'completed', modelSelection: selection };
    expect(subagent(item, { agents: [], providers: [{ ...provider('codex'), models }] })).toMatchObject({ effort: 'high', speed: 'ultrafast', speedLabel: 'Ultrafast mode on' });
    expect(subagent({ ...item, model: 'b' }, { agents: [], providers: [{ ...provider('codex'), models }] })).toMatchObject({ effort: '', speed: '' });
    expect(subagent({ ...item, modelSelection: undefined, origin: 'app_owned', childThreadId: 'child' }, { agents: [], providers: [{ ...provider('codex'), models }], threads: [{ id: 'child', modelSelection: selection }] })).toMatchObject({ effort: 'high', speed: 'ultrafast' });
  });
});

test('model name equality beats a favorite and equal substring matches ignore name position', () => {
  const item = { id: 'a', name: 'GPT 6', shortName: '', subProvider: '', driver: 'codex', providerName: 'Codex', providerId: 'codex', order: 0, favorite: false, legacy: false };
  expect(scoreModel(item, ' gpt   6 ')).toBeLessThan(scoreModel({ ...item, name: 'GPT 6 Mini', favorite: true }, 'gpt 6')!);
  expect(scoreModel({ ...item, name: 'Claude Sonnet 5' }, 'sonnet')).toBe(scoreModel({ ...item, name: 'Long Claude Sonnet 5' }, 'sonnet'));
});

test('Meta Enter creates while a row owns Enter, with busy and repeat guards', () => {
  const owner = contractComponent('palette.contract', 'CommandPaletteBody');
  const calls: unknown[][] = [];
  const scope = { view: { accessoryKey: '⌘ Enter', accessoryEnabled: true, accessoryOp: 'flow', accessoryArg: 'new-project', accessoryArg2: 'App', rows: [], jumpKeys: [] }, busy: false,
    preventDefault: () => calls.push(['prevent']), stopPropagation: () => calls.push(['stop']), run: (...args: unknown[]) => calls.push(args) };
  const event = { metaKey: true, ctrlKey: false, altKey: false, shiftKey: false, repeat: false };
  owner.call('inputKey', owner.state(), scope, ['Enter', event]);
  expect(calls).toEqual([['prevent'], ['stop'], ['flow', 'new-project', 'App']]);
  calls.length = 0;
  owner.call('inputKey', owner.state(), { ...scope, busy: true }, ['Enter', event]);
  expect(calls).toEqual([['prevent'], ['stop']]);
});
