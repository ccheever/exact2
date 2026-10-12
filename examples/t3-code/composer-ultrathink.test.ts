// composer-fidelity G9: the ultrathink flow and the Fast default through the
// client (composer-ultrathink.ts), against the composer-controls fake backend.
// Behavior per T3 Code 1e2ecbd975 TraitsPicker.tsx handleSelectChange and
// ChatView.tsx formatOutgoingPrompt (MIT; see LICENSE-T3).
import { describe, expect, test } from 'bun:test';
import { snapshot } from './presentation';
import type { Obj } from './domain';
import { connected, opened } from './composer-controls-fixture';

const claudeEffort = { id: 'effort', label: 'Reasoning', type: 'select', promptInjectedValues: ['ultrathink'],
  options: [{ id: 'medium', label: 'Medium' }, { id: 'high', label: 'High', isDefault: true }, { id: 'ultrathink', label: 'Ultrathink' }] };
const claudeModel = { slug: 'claude-opus', name: 'Claude Opus', isDefault: true, capabilities: { optionDescriptors: [claudeEffort] } };

async function onClaude() {
  const context = await opened();
  const providers = context.native.config.providers as Obj[];
  providers[2] = { ...providers[2], models: [claudeModel] };
  await context.client.refresh(context.native, context.disk);
  await context.command('model', 'claude-opus', 'claude');
  return context;
}

describe('ultrathink (G9)', () => {
  test('choosing the injected effort rewrites the draft and stores no option', async () => {
    const { client, command } = await onClaude();
    await command('draft', '', 'Fix the flaky test');
    const before = snapshot(client).requestKey;
    await command('model-option', 'effort', 'ultrathink');
    expect(client.draft).toBe('Ultrathink:\nFix the flaky test');
    expect(client.modelOptions).toEqual([]);
    const view = snapshot(client);
    expect(view.requestKey).not.toBe(before);
    expect(view.composer).toMatchObject({ ultrathink: true, traitsLabel: 'Ultrathink' });
    expect(view.composer.traits.find(item => item.value === 'ultrathink')?.selected).toBe(true);
  });
  test('"ultrathink" in the body locks the effort rows with the reference wording', async () => {
    const { client, command } = await onClaude();
    await command('draft', '', 'please ultrathink about the cache');
    const traits = snapshot(client).composer.traits;
    expect(traits.find(item => item.kind === 'note')?.label).toBe('Your prompt contains "ultrathink" in the text. Remove it to change this option.');
    expect(traits.filter(item => item.kind === 'option').every(item => item.disabled)).toBe(true);
    await command('model-option', 'effort', 'medium');
    expect(client.modelOptions).toEqual([]);
    expect(client.draft).toBe('please ultrathink about the cache');
  });
  test('another effort strips the prefix and is stored', async () => {
    const { client, command } = await onClaude();
    await command('draft', '', 'Ultrathink:\nFix it');
    await command('model-option', 'effort', 'medium');
    expect(client.draft).toBe('Fix it');
    expect(client.modelOptions).toEqual([{ id: 'effort', value: 'medium' }]);
    expect(snapshot(client).composer.ultrathink).toBe(false);
  });
  test('the sent text carries the prefix once', async () => {
    const { client, native, command } = await onClaude();
    await command('draft', '', 'Fix it');
    await command('model-option', 'effort', 'ultrathink');
    const before = native.committed.length;
    await command('send', '', client.draft);
    const sent = native.committed.slice(before).find(entry => entry.type === 'message.dispatch');
    expect(sent?.text).toBe('Ultrathink:\nFix it');
    expect(sent?.modelSelection).toEqual({ instanceId: 'claude', model: 'claude-opus' });
  });
});

describe('Fast default (G9)', () => {
  test('a model with fastMode sends fastMode:false until the user chooses Fast', async () => {
    const { client, native, command } = await connected();
    await command('select-thread', 't1');
    await client.refresh(native, (await connected()).disk);
    let before = native.committed.length;
    await command('send', '', 'one');
    expect(native.committed.slice(before).find(entry => entry.type === 'message.dispatch')?.modelSelection).toEqual({ instanceId: 'codex', model: 'model-a', options: [{ id: 'fastMode', value: false }] });
    await command('model-option', 'fastMode', 'true');
    before = native.committed.length;
    await command('send', '', 'two');
    expect(native.committed.slice(before).find(entry => entry.type === 'message.dispatch')?.modelSelection).toEqual({ instanceId: 'codex', model: 'model-a', options: [{ id: 'fastMode', value: true }] });
  });
});
