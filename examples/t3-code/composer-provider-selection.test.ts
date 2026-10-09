// composer-provider-state-and-details: the composer's provider and model rule
// (composer-provider-selection.ts). The first three groups port T3 Code's own tests
// by name (ChatView.logic.test.ts resolveComposerProviderSelection,
// providerInstances.test.ts resolveSelectableProviderInstance, modelSelection.test.ts);
// the last drives the composer the audit compared (CO-6, CO-9, CO-10).
import { describe, expect, test } from 'bun:test';
import { obj, type Obj } from './domain';
import { modelCatalog, snapshot } from './presentation';
import { providerEntries, resolveComposerProviderSelection, resolveSelectableProviderInstanceEntry, resolveAppModelSelectionForInstance,
  composerSelection, applyProviderInstanceSettings, DEFAULT_MODEL } from './composer-provider-selection';
import { provider as fixtureProvider, Fake, storage, opened } from './composer-controls-fixture';
import { T3Client } from './client';
import { composerBranches } from './composer-controls-branch';

const wire = (driver: string, instanceId = driver, overrides: Obj = {}): Obj => ({ driver, instanceId, enabled: true, installed: true, status: 'ready',
  auth: { status: 'authenticated' }, version: null, models: [], slashCommands: [], skills: [], ...overrides });
const entry = (driver: string, instanceId = driver, overrides: Obj = {}) => providerEntries([wire(driver, instanceId, overrides)])[0]!;
const selectable = (providers: Obj[], instanceId: string | undefined) => resolveSelectableProviderInstanceEntry(providerEntries(providers), instanceId)?.instanceId;

describe('resolveComposerProviderSelection', () => {
  test.each([['claudeAgent', 'claude_work'], ['codex', 'codex_work'], ['ollama', 'local_models']])(
    'keeps imported %s history selectable through its custom instance', (driver, instanceId) => {
      const imported = entry(driver, instanceId);
      const entries = [entry(driver === 'codex' ? 'claudeAgent' : 'codex'), imported];
      expect(resolveComposerProviderSelection({ entries, candidateInstanceIds: [instanceId], lockedProvider: driver, lockedInstanceId: instanceId })
        .selectedProviderEntry?.instanceId).toBe(instanceId);
    });
  test.each(['missing', 'disabled'] as const)('does not move imported history to another driver when its instance is %s', state => {
    const imported = entry('claudeAgent', 'claude_work', { enabled: false }), other = entry('codex');
    const entries = state === 'missing' ? [other] : [other, imported];
    expect(resolveComposerProviderSelection({ entries, candidateInstanceIds: [other.instanceId, imported.instanceId], lockedProvider: 'claudeAgent',
      lockedInstanceId: imported.instanceId }).selectedProviderEntry).toBeUndefined();
  });
  test("uses the custom instance's capability instead of the default instance", () => {
    const fallback = entry('antigravity', 'antigravity', { showInteractionModeToggle: true }), custom = entry('antigravity', 'google_work', { showInteractionModeToggle: false });
    const selection = resolveComposerProviderSelection({ entries: [fallback, custom], candidateInstanceIds: [custom.instanceId], lockedProvider: null, lockedInstanceId: null });
    expect(selection.selectedProviderEntry?.instanceId).toBe('google_work');
    expect(selection.selectedProviderEntry?.snapshot.showInteractionModeToggle).toBe(false);
  });
  test("uses the fallback provider's plan capability after the draft's instance is disabled", () => {
    const disabled = entry('antigravity', 'antigravity', { enabled: false, showInteractionModeToggle: false }), fallback = entry('codex');
    expect(resolveComposerProviderSelection({ entries: [disabled, fallback], candidateInstanceIds: [disabled.instanceId], lockedProvider: null, lockedInstanceId: null })
      .selectedProviderEntry?.instanceId).toBe('codex');
  });
  test('keeps a signed-out selection instead of silently switching providers', () => {
    const signedOut = entry('antigravity', 'google_work', { status: 'error', auth: { status: 'unauthenticated' }, models: [{ slug: 'gemini-pro', name: 'Gemini Pro', isCustom: false }] });
    expect(resolveComposerProviderSelection({ entries: [entry('codex'), signedOut], candidateInstanceIds: [signedOut.instanceId], lockedProvider: null, lockedInstanceId: null })
      .selectedProviderEntry?.instanceId).toBe('google_work');
  });
  test('names the requested instance for setup when nothing can run the turn', () => {
    const entries = [entry('claudeAgent', 'claudeAgent', { status: 'error', installed: false }), entry('codex', 'codex', { status: 'error', auth: { status: 'unauthenticated' } })];
    // A new draft asks for nothing: an errored instance is never invented as its default.
    expect(resolveComposerProviderSelection({ entries, candidateInstanceIds: ['', null, undefined], lockedProvider: null, lockedInstanceId: null }))
      .toMatchObject({ selectedProviderEntry: undefined, unavailableProviderInstanceId: undefined });
  });
});

describe('applyProviderInstanceSettings', () => {
  const applied = (driver: string, instanceId: string, settings: Obj) => applyProviderInstanceSettings([entry(driver, instanceId)], settings)[0]!;
  test('uses settings when a streamed snapshot still reports a disabled default as enabled', () => {
    expect(applied('codex', 'codex', { providerInstances: { codex: { driver: 'codex', enabled: false } }, providers: {} }).enabled).toBe(false);
  });
  test('treats a removed custom instance snapshot as disabled', () => {
    expect(applied('claudeAgent', 'claude_work', { providerInstances: {}, providers: {} }).enabled).toBe(false);
  });
  test.each(['constructor', 'toString'])('treats a removed custom instance named %s as disabled', instanceId => {
    expect(applied('claudeAgent', instanceId, { providerInstances: {}, providers: {} }).enabled).toBe(false);
  });
  test('uses settings for a configured custom instance named constructor', () => {
    expect(applied('claudeAgent', 'constructor', { providerInstances: { constructor: { driver: 'claudeAgent', enabled: false } }, providers: {} }).enabled).toBe(false);
  });
  test('treats a removed default instance for a fork driver as disabled', () => {
    const forked = applied('constructor', 'constructor', { providerInstances: {}, providers: {} });
    expect(forked.isDefault).toBe(true);
    expect(forked.enabled).toBe(false);
  });
  test('uses legacy settings for a built-in default instance', () => {
    expect(applied('codex', 'codex', { providerInstances: {}, providers: { codex: { enabled: false } } }).enabled).toBe(false);
  });
  test('the composer drops an instance the settings just disabled, though its snapshot still says enabled', () => {
    const providers = [wire('codex'), wire('claudeAgent')];
    const settings = { providerInstances: { codex: { driver: 'codex', enabled: false }, claudeAgent: { driver: 'claudeAgent' } }, providers: {} };
    expect(composerSelection({ config: { providers, settings }, providerId: 'codex', modelId: '', local: {} })).toMatchObject({ instanceId: 'claudeAgent' });
    // An instance missing from providerInstances (just deleted) is disabled too; with nothing left the composer asks for one.
    const deleted = { config: { providers: [wire('codex', 'codex_work')], settings: { providerInstances: {}, providers: {} } }, providerId: 'codex_work', modelId: '', local: {}, configLive: true };
    expect(composerSelection(deleted)).toMatchObject({ instanceId: '', noProviderAvailable: true, showProviderUnavailable: true });
  });
});

describe('resolveSelectableProviderInstance', () => {
  test('returns the requested instance when it is enabled and available', () => {
    expect(selectable([wire('codex'), wire('claudeAgent', 'claude_work')], 'claude_work')).toBe('claude_work');
  });
  test('falls back to the first enabled and available instance', () => {
    expect(selectable([wire('codex', 'codex', { enabled: false }), wire('claudeAgent')], 'codex')).toBe('claudeAgent');
  });
  test('prefers a ready instance over an enabled one whose driver cannot start', () => {
    expect(selectable([wire('codex', 'codex', { status: 'error' }), wire('claudeAgent')], undefined)).toBe('claudeAgent');
  });
  test('prefers an unprobed (warning) instance over one whose probe errored', () => {
    expect(selectable([wire('codex', 'codex', { status: 'error' }), wire('claudeAgent', 'claudeAgent', { status: 'warning' })], undefined)).toBe('claudeAgent');
  });
  test('keeps a requested instance even when its probe errored', () => {
    expect(selectable([wire('codex', 'codex', { status: 'error' }), wire('claudeAgent')], 'codex')).toBe('codex');
  });
  test('does not invent an errored instance as a new-user default', () => {
    expect(selectable([wire('codex', 'codex', { status: 'error' })], undefined)).toBeUndefined();
  });
  test('does not return disabled, unavailable, or unknown instances when none are sendable', () => {
    const providers = [wire('codex', 'codex', { enabled: false }), wire('claudeAgent', 'claudeAgent', { availability: 'unavailable' })];
    expect(selectable(providers, 'codex')).toBeUndefined();
    expect(selectable(providers, 'claudeAgent')).toBeUndefined();
    expect(selectable(providers, 'removed_instance')).toBeUndefined();
  });
});

describe('instance-scoped model selection', () => {
  const models = (slugs: string[]) => slugs.map(slug => ({ slug, name: slug, isCustom: false, capabilities: {} }));
  test('falls back from an explicit non-OpenCode draft with a missing model', () => {
    const providers = [wire('codex', 'codex', { models: models(['gpt-5.6-sol']) })];
    expect(resolveAppModelSelectionForInstance({}, {}, providers, 'codex', 'gpt-missing', true)).toBe('gpt-5.6-sol');
    // The composer's own choice for that instance resolves the same way, and its options are dropped at dispatch.
    const client = { config: { providers, settings: {} }, providerId: 'codex', modelId: 'gpt-missing', local: {} };
    expect(composerSelection(client)).toMatchObject({ instanceId: 'codex', model: 'gpt-5.6-sol' });
  });
  test('preserves an explicit draft OpenCode selection while the catalog is empty', () => {
    const providers = [wire('opencode', 'opencode_work')];
    expect(resolveAppModelSelectionForInstance({}, {}, providers, 'opencode_work', 'openrouter/kimi-k3', true)).toBe('openrouter/kimi-k3');
  });
  test('a thread whose instance reports no models shows and sends the driver default (DEFAULT_MODEL_BY_PROVIDER)', () => {
    const providers = [wire('codex', 'codex', { status: 'error', auth: { status: 'unauthenticated' } })];
    const client = { config: { providers, settings: {} }, providerId: 'codex', modelId: 'gpt-5.4', local: {} };
    expect(composerSelection(client)).toMatchObject({ instanceId: 'codex', model: DEFAULT_MODEL, showProviderUnavailable: false });
    expect(DEFAULT_MODEL).toBe('gpt-6-astra');
  });
});

describe('the composer the audit compared', () => {
  const noProviders = () => [
    fixtureProvider('claudeAgent', 'claudeAgent', { installed: false, status: 'error', models: [], setup: { canInstall: true } }),
    fixtureProvider('codex', 'codex', { status: 'error', auth: { status: 'unauthenticated' }, models: [], setup: { canAuthenticate: true } }),
  ];
  test('CO-9: a draft with no selectable provider asks to enable one, beside the setup control', async () => {
    const client = new T3Client(), native = new Fake(), disk = storage();
    native.config.providers = noProviders(); native.config.settings = { defaultRuntimeMode: 'full-access' };
    await client.refresh(native, disk);
    expect(client.threadId).toBe('');
    expect(snapshot(client).composer).toMatchObject({ placeholder: 'Enable a provider in Settings to send a message', noProvider: 'Open provider settings', providerSetupId: 'claudeAgent' });
  });
  test('CO-10: a thread on an enabled but failing provider keeps its model, mode and access controls', async () => {
    const context = await opened();
    context.client.config.providers = noProviders();
    const view = snapshot(context.client);
    expect(view.composer).toMatchObject({ noProvider: '', providerSetupId: '', runtimeLabel: 'Full access' });
    expect(view.composer.placeholder).not.toBe('Enable a provider in Settings to send a message');
    expect(view).toMatchObject({ providerDriver: 'codex', modelLabel: DEFAULT_MODEL });
  });
  test('CO-6: a stored model the catalog does not list shows and sends the instance default', async () => {
    const native = new Fake();
    (native.details.t1!.projection as Obj).thread = { ...(native.details.t1!.projection as Obj).thread as Obj, modelSelection: { instanceId: 'codex', model: 'gpt-5.4' } };
    const shellThread = (native.shell.threads as Obj[])[0]!; shellThread.modelSelection = { instanceId: 'codex', model: 'gpt-5.4' };
    const client = new T3Client(), disk = storage();
    await client.refresh(native, disk);
    await client.command('select-thread', 't1', '', 0, native, disk);
    await client.refresh(native, disk);
    expect(client.modelId).toBe('gpt-5.4');
    const view = snapshot(client);
    expect(view.modelLabel).toBe('Model A');
    expect(view.composer.modelTip).toBe('Model A · ⇧⌘M');
    // ChatComposer passes the resolved model to the picker: its row is the checked one.
    expect(modelCatalog(client, 'codex', '').models.filter(row => row.selected).map(row => row.key)).toEqual(['codex:model-a']);
    await client.command('send', '', 'Check the default', 0, native, disk);
    expect(native.committed.at(-1)).toMatchObject({ type: 'message.dispatch', modelSelection: { instanceId: 'codex', model: 'model-a' } });
  });
  test('a draft whose default instance is gone: the picker checks, and Shift+click fans out from, the model it shows', async () => {
    const client = new T3Client(), native = new Fake(), disk = storage();
    obj(obj(native.config.environment).capabilities).requiredWorktreeBootstrap = true;
    native.config.settings = { defaultModelSelection: { instanceId: 'missing', model: 'model-b' }, defaultRuntimeMode: 'full-access' };
    await client.refresh(native, disk);
    expect([client.threadId, client.providerId, client.modelId]).toEqual(['', '', '']);
    expect(snapshot(client)).toMatchObject({ modelLabel: 'Model B', providerDriver: 'codex', canSend: true });
    expect(modelCatalog(client, 'codex', '').models.filter(row => row.selected).map(row => row.key)).toEqual(['codex:model-b']);
    // onToggleModel seeds the set with selectedModelSelection, the shown model.
    native.gesture = { modifiers: 'shift', source: 'pointer', ageMs: 12 };
    await client.command('model', 'model-a', 'claude', 0, native, disk);
    expect(snapshot(client).composer).toMatchObject({ fanout: true, fanoutKeys: ['codex:model-b', 'claude:model-a'], fanoutLabel: 'Model B, Model A' });
    await composerBranches(client, native, false, ''); // the strip reads the checkout's branch, the worktrees' base
    await client.command('send', '', 'Both models', 0, native, disk);
    expect(native.committed.filter(call => call.method === 'orchestration.launchThread').map(call => obj(call.modelSelection)))
      .toMatchObject([{ instanceId: 'codex', model: 'model-b' }, { instanceId: 'claude', model: 'model-a' }]);
  });
});
