// Lane composer-controls, round 3: several models for one new thread (the
// HEAD server advertises requiredWorktreeBootstrap): Shift-click adds a model,
// the trigger names them, and Send starts one background thread per model.
import { describe, expect, test } from 'bun:test';
import { obj } from './domain';
import { snapshot } from './presentation';
import { composerBranches } from './composer-controls-branch';
import { modelCatalog } from './presentation';
import { connected } from './composer-controls-fixture';
import { toasts } from './toast';

async function draft(bootstrap = true) {
  const context = await connected();
  obj(obj(context.native.config.environment).capabilities).requiredWorktreeBootstrap = bootstrap;
  await context.client.refresh(context.native, context.disk);
  const shift = () => { context.native.gesture = { modifiers: 'shift', source: 'pointer', ageMs: 12 }; };
  return { ...context, shift };
}

describe('multi-model drafts', () => {
  test('Shift-click adds a model; the trigger and rows show the set; one left becomes the model again', async () => {
    const { client, command, shift } = await draft();
    expect(client.threadId).toBe('');
    shift(); await command('model', 'model-b', 'codex');
    let view = snapshot(client).composer;
    expect(view).toMatchObject({ fanout: true, fanoutLabel: 'Model A, Model B', fanoutAria: 'Model A, Model B', fanoutMore: 0,
      fanoutKeys: ['codex:model-a', 'codex:model-b'] });
    expect(view.fanoutMarks.map(mark => mark.driver)).toEqual(['codex', 'codex']);
    expect(view.modelTip).toBe('Model A, Model B · ⇧⌘M');
    expect(modelCatalog(client, 'codex', '').models.map(row => [row.id, row.checked, row.selected])).toEqual([['model-a', true, true], ['model-b', true, true]]);
    shift(); await command('model', 'model-a', 'claude');
    shift(); await command('model', 'model-b', 'claude');
    view = snapshot(client).composer;
    expect(view).toMatchObject({ fanoutLabel: 'Model A, Model B, 2 more', fanoutMore: 1 });
    expect(view.fanoutMarks).toHaveLength(3);
    // Taking models out until one is left returns to a single selection.
    for (const [model, provider] of [['model-a', 'claude'], ['model-b', 'claude'], ['model-a', 'codex']]) { shift(); await command('model', model!, provider!); }
    expect(snapshot(client).composer.fanout).toBe(false);
    expect([client.providerId, client.modelId]).toEqual(['codex', 'model-b']);
  });
  test('a plain pick ends the fan-out; servers without worktree bootstrap never start one', async () => {
    const { client, command, shift } = await draft();
    shift(); await command('model', 'model-b', 'codex');
    await command('model', 'model-a', 'claude');
    expect(snapshot(client).composer.fanout).toBe(false);
    expect([client.providerId, client.modelId]).toEqual(['claude', 'model-a']);
    const old = await draft(false);
    old.shift(); await old.command('model', 'model-b', 'codex');
    expect(snapshot(old.client).composer.fanout).toBe(false);
    expect(old.client.modelId).toBe('model-b');
  });
  test('Send starts one background thread per model, each in a new worktree from the base branch', async () => {
    const { client, native, command, shift } = await draft();
    shift(); await command('model', 'model-a', 'claude');
    expect((await composerBranches(client, native, false, ''))).toMatchObject({ forceWorktree: true, envMode: 'worktree', envLabel: 'New worktree', branchLabel: 'From main' });
    await command('send', '', 'Build the parser');
    const launches = native.committed.filter(entry => entry.method === 'orchestration.launchThread');
    expect(launches.map(entry => [obj(entry.modelSelection).instanceId, obj(entry.modelSelection).model, entry.workspaceStrategy, obj(entry.initialMessage).text])).toEqual([
      ['codex', 'model-a', { type: 'worktree', baseRef: 'main' }, 'Build the parser'], ['claude', 'model-a', { type: 'worktree', baseRef: 'main' }, 'Build the parser']]);
    expect(toasts(client).map(toast => toast.title)).toContain('Started 2 threads in background');
    expect(client.threadId).toBe('');
    expect(client.draft).toBe('');
    expect(snapshot(client).composer.fanout).toBe(false);
  });
  test('without a Git base the send stops with the reference warning', async () => {
    const { client, native, command, shift } = await draft();
    shift(); await command('model', 'model-a', 'claude');
    await command('send', '', 'Build the parser');
    expect(native.committed.some(entry => entry.method === 'orchestration.launchThread')).toBe(false);
    expect(toasts(client).map(toast => [toast.title, toast.description])).toContainEqual(['Choose models and a base branch',
      'Multiple models need a new thread in a Git project. Each gets its own worktree.']);
  });
});
