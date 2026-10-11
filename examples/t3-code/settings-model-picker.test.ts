// settings-model-picker: Settings → General's New threads › Model and Text generation model open
// the composer's ProviderModelPicker (model-catalog.ts) over the settings scope, at T3 Code
// 1e2ecbd975: ProjectDefaultsSettings.tsx, SettingsPanels.tsx, ModelPickerContent.tsx and
// useScopedModelAvailability.ts.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { applyShell, initialShell, obj, str, type Obj } from './domain';
import type { Native } from './protocol';
import { applyCoreSetting, generalSections, resolveScope, serverContext } from './settings-core';
import { settingsPickerCatalog } from './settings-model-picker';
import { modelCatalog } from './presentation';
import { fleet, type FleetEntry } from './settings-b-fleet';
import { toasts } from './toast';
import { settingsFailure } from './shell-commands';
import { keyboardDispatch } from './keyboard-dispatch';

const badge = () => ({ providerBadge: '', providerBadgeColor: '' });
const capabilities = { projectSettingsOverrides: true, threadAutoSettlement: true, threadRestartContinuation: true };
const ready = { enabled: true, installed: true, auth: { status: 'authenticated' }, status: 'ready' };
const codex = { instanceId: 'codex', driver: 'codex', displayName: 'Codex', ...ready,
  models: [{ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna', isDefault: true }, { slug: 'gpt-6-astra', name: 'GPT-6-Astra' }, { slug: 'gpt-4-old', name: 'GPT-4 Old', isLegacy: true }] };
const claude = { instanceId: 'claudeAgent', driver: 'claudeAgent', displayName: 'Claude', ...ready,
  models: [{ slug: 'claude-opus-4-7', name: 'Claude Opus 4.7', isDefault: true }, { slug: 'claude-sonnet-4-6', name: 'Claude Sonnet 4.6' }] };
const cursor = { instanceId: 'cursor', driver: 'cursor', displayName: 'Cursor', ...ready, supportsTextGeneration: false, models: [{ slug: 'auto', name: 'Auto' }] };
const binding = (key: string, command: string, when: Obj | null = null): Obj => ({ command, shortcut: { key, modKey: true, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false }, ...(when ? { whenAst: when } : {}) });
const pickerOpen = { type: 'identifier', name: 'modelPickerOpen' };

/** The focused environment ("Studio") with Codex, Claude and Cursor; a Fake transport answers settings reads and writes. */
function client(settings: Obj = {}) {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env-a', origin: 'http://127.0.0.1:16800', scopes: ['orchestration:read', 'orchestration:operate'] });
  c.config = { environment: { environmentId: 'env-a', label: 'Studio', capabilities }, providers: [codex, claude, cursor],
    keybindings: [binding('1', 'modelPicker.jump.1', pickerOpen), binding('2', 'modelPicker.jump.2', pickerOpen)], settings: { projectSettingsOverrides: {}, ...settings } };
  c.shell = applyShell(initialShell(), { snapshotSequence: 1, projects: [{ id: 'pa', title: 'App', workspaceRoot: '/a/app', repositoryIdentity: { canonicalKey: 'github.com/acme/app' } }], threads: [] });
  c.projectId = 'pa'; c.providerId = 'claudeAgent'; c.modelId = 'claude-sonnet-4-6'; // the composer's own choice, which no settings pick may change
  return c;
}
class Fake implements Native {
  available = true; calls: Obj[] = [];
  constructor(private c: T3Client) {}
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const target = request.fleet ? fleet.entries.get(str(request.fleet)) : undefined;
    const config = target ? target.config : this.c.config;
    if (request.method === 'server.updateSettings') return { ok: true, generation: target?.generation ?? 1, value: { ...obj(config.settings), ...obj(obj(request.payload).patch) } };
    if (request.method === 'server.getSettings') return { ok: true, generation: 1, value: obj(config.settings) };
    return { ok: false, generation: target?.generation ?? 1, error: { kind: 'x', message: 'missing', uncertain: false } };
  }
}
function entry(key: string, environmentId: string, label: string, providers: Obj[]): FleetEntry {
  const shell = applyShell(initialShell(), { snapshotSequence: 1, projects: [{ id: `p-${environmentId}`, title: 'App', workspaceRoot: `/b/${environmentId}`, repositoryIdentity: { canonicalKey: 'github.com/acme/app' } }], threads: [] });
  return { key, origin: key.split('\n')[0]!, environmentId, phase: 'connected', message: '', traceId: '', generation: 3, synchronized: 3, lastEvent: 0, subscriptions: {},
    config: { environment: { environmentId, label, capabilities }, providers, settings: { projectSettingsOverrides: {} } }, shell, scopes: [], error: '', requested: true };
}
const B = 'http://127.0.0.1:16801\nenv-b';
beforeEach(() => { fleet.entries.clear(); fleet.saved = []; });
afterEach(() => { fleet.entries.clear(); fleet.saved = []; });
const updates = (native: Fake) => native.calls.filter(call => call.method === 'server.updateSettings').map(call => [call.fleet ?? '', obj(call.payload).patch]);

describe('the picker for a General model row', () => {
  test('rail, selection and list follow the setting, not the composer', () => {
    const c = client({ defaultModelSelection: { instanceId: 'codex', model: 'gpt-6-astra' } });
    const catalog = settingsPickerCatalog(c, 'default-model:|||', '', '', badge);
    // No favorites yet: the rail opens on the setting's instance (ModelPickerContent selectedInstanceId).
    expect([catalog.provider, catalog.railIndex, catalog.favoritesSelected]).toEqual(['codex', 1, false]);
    expect(catalog.providers.map(rail => [rail.id, rail.selected])).toEqual([['codex', true], ['claudeAgent', false], ['cursor', false]]);
    expect(catalog.models.map(row => [row.kind, row.name, row.selected, row.reason])).toEqual([['model', 'GPT-5.6-Luna', false, ''], ['model', 'GPT-6-Astra', true, ''],
      ['legacy', 'Legacy models', false, ''], ['legacy-model', 'GPT-4 Old', false, '']]);
    expect(catalog.legacyDefault).toBe(false);
    // Switching the rail browses another provider; nothing marks the composer's Claude Sonnet.
    const claudeRail = settingsPickerCatalog(c, 'default-model:|||', 'claudeAgent', '', badge);
    expect(claudeRail.models.map(row => [row.id, row.selected])).toEqual([['claude-opus-4-7', false], ['claude-sonnet-4-6', false]]);
    // presentation.ts routes a target to this catalog; the composer's own catalog is unchanged.
    expect(modelCatalog(c, '', '', 'default-model:|||').provider).toBe('codex');
    expect(modelCatalog(c, '', '').models.find(row => row.selected)?.id).toBe('claude-sonnet-4-6');
  });
  test('search crosses providers, an unmatched query is empty and clearing restores the browse state', () => {
    const c = client();
    const byName = settingsPickerCatalog(c, 'default-model:|||', 'codex', 'opus', badge);
    expect([byName.searching, byName.models.map(row => row.id)]).toEqual([true, ['claude-opus-4-7']]);
    expect(settingsPickerCatalog(c, 'default-model:|||', 'codex', 'claude', badge).models.map(row => row.id)).toEqual(['claude-opus-4-7', 'claude-sonnet-4-6']);
    expect(settingsPickerCatalog(c, 'default-model:|||', 'codex', 'zzzz', badge).count).toBe(0);
    const cleared = settingsPickerCatalog(c, 'default-model:|||', 'codex', '', badge);
    expect([cleared.searching, cleared.provider, cleared.count]).toEqual([false, 'codex', 4]);
  });
  test('favorites are the composer\'s shared store and open first; favoriting writes no setting', () => {
    const c = client();
    c.local.favoriteModels = [JSON.stringify(['claudeAgent', 'claude-opus-4-7'])];
    const catalog = settingsPickerCatalog(c, 'default-model:|||', '', '', badge);
    expect([catalog.provider, catalog.railIndex, catalog.models.map(row => [row.id, row.favorite])]).toEqual(['favorites', 0, [['claude-opus-4-7', true]]]);
    // In a provider's list the favorite leads (sortProviderModelItems groupFavorites).
    expect(settingsPickerCatalog(c, 'default-model:|||', 'claudeAgent', '', badge).models.map(row => row.id)).toEqual(['claude-opus-4-7', 'claude-sonnet-4-6']);
    expect(modelCatalog(c, '', '').provider).toBe('favorites');
    expect(obj(c.config.settings).defaultModelSelection).toBeUndefined();
  });
  test('a legacy setting opens its legacy section expanded', () => {
    const c = client({ defaultModelSelection: { instanceId: 'codex', model: 'gpt-4-old' } });
    const catalog = settingsPickerCatalog(c, 'default-model:|||', '', '', badge);
    expect([catalog.legacyDefault, catalog.models.find(row => row.id === 'gpt-4-old')?.selected]).toEqual([true, true]);
    expect(settingsPickerCatalog(c, 'default-model:|||', 'claudeAgent', '', badge).legacyDefault).toBe(false);
  });
  test('Text generation lists only the providers that support it', () => {
    const c = client({ textGenerationModelSelection: { instanceId: 'claudeAgent', model: 'claude-opus-4-7' } });
    const catalog = settingsPickerCatalog(c, 'text-generation-model:|||', '', '', badge);
    expect(catalog.providers.map(rail => rail.id)).toEqual(['codex', 'claudeAgent']);
    expect([catalog.provider, catalog.models.find(row => row.selected)?.id]).toEqual(['claudeAgent', 'claude-opus-4-7']);
    expect(settingsPickerCatalog(c, 'text-generation-model:|||', '', 'auto', badge).count).toBe(0);
    expect(settingsPickerCatalog(c, 'default-model:|||', '', 'auto', badge).models.map(row => row.id)).toEqual(['auto']);
    // A target that names no model row lists nothing.
    expect(settingsPickerCatalog(c, 'default-permissions:|||', '', '', badge).count).toBe(0);
  });
});

describe('writes and scoped availability', () => {
  test('a pick writes the setting at environment and project scope and never the thread', async () => {
    const c = client(), native = new Fake(c);
    await applyCoreSetting(c, native, 'default-model:|||', 'codex|gpt-6-astra');
    await applyCoreSetting(c, native, 'default-model:||github.com/acme/app|', 'claudeAgent|claude-opus-4-7');
    await applyCoreSetting(c, native, 'text-generation-model:|||', 'codex|gpt-5.6-luna');
    expect(updates(native)).toEqual([['', { defaultModelSelection: { instanceId: 'codex', model: 'gpt-6-astra' } }],
      ['', { projectSettingsOverrides: { pa: { defaultModelSelection: { instanceId: 'claudeAgent', model: 'claude-opus-4-7' } } } }],
      ['', { textGenerationModelSelection: { instanceId: 'codex', model: 'gpt-5.6-luna' } }]]);
    expect([c.providerId, c.modelId]).toEqual(['claudeAgent', 'claude-sonnet-4-6']);
    // The project pick reads back as an override; the environment keeps its own value.
    const project = settingsPickerCatalog(c, 'default-model:||github.com/acme/app|', '', '', badge);
    expect([project.provider, project.models.find(row => row.selected)?.id]).toEqual(['claudeAgent', 'claude-opus-4-7']);
    expect(settingsPickerCatalog(c, 'default-model:|||', '', '', badge).models.find(row => row.selected)?.id).toBe('gpt-6-astra');
    // Text generation refuses a provider without text generation.
    await expect(applyCoreSetting(c, native, 'text-generation-model:|||', 'cursor|auto')).rejects.toThrow('unavailable');
    expect(updates(native).length).toBe(3);
  });
  test('All environments disables a model one target lacks; that environment alone allows it', async () => {
    const c = client({ defaultModelSelection: { instanceId: 'codex', model: 'gpt-5.6-luna' } }), native = new Fake(c);
    fleet.entries.set(B, entry(B, 'env-b', 'Build box', [{ ...codex, models: [codex.models[0]!] }, claude]));
    const all = settingsPickerCatalog(c, 'default-model:|||', 'codex', '', badge);
    const reason = 'This model is unavailable on Build box. Select that environment to choose its model separately.';
    expect(all.models.filter(row => row.kind !== 'legacy').map(row => [row.id, row.reason])).toEqual([['gpt-5.6-luna', ''], ['gpt-6-astra', reason], ['gpt-4-old', reason]]);
    expect(settingsPickerCatalog(c, 'default-model:|env-a||', 'codex', '', badge).models.every(row => row.reason === '')).toBe(true);
    // A stale or scripted pick is still refused with the reason, as setModel's toast says it.
    await expect(applyCoreSetting(c, native, 'default-model:|||', 'codex|gpt-6-astra')).rejects.toThrow(reason);
    expect(updates(native)).toEqual([]);
    await applyCoreSetting(c, native, 'default-model:|||', 'claudeAgent|claude-sonnet-4-6');
    expect(updates(native)).toEqual([['', { defaultModelSelection: { instanceId: 'claudeAgent', model: 'claude-sonnet-4-6' } }], [B, { defaultModelSelection: { instanceId: 'claudeAgent', model: 'claude-sonnet-4-6' } }]]);
    // Disagreeing targets: a neutral "Mixed" trigger without a provider mark or Traits picker.
    obj(fleet.entries.get(B)!.config.settings).defaultModelSelection = { instanceId: 'codex', model: 'gpt-5.6-luna' };
    const row = generalSections(c, serverContext(c, resolveScope(c, '', '', ''), new Map())).flatMap(section => section.rows).find(item => item.id === 'default-model')!;
    expect([row.label, row.mixed, row.label2, row.inheritance]).toEqual(['Mixed', true, '', 'mixed']);
  });
  test('the reason is the toast a refused pick shows', async () => {
    const c = client();
    expect(settingsFailure(c, 'settings-core', 'default-model:|||', 'codex|x', 'This model is unavailable on Build box.')).toBe('Default model not saved');
    expect(settingsFailure(c, 'settings-core', 'text-generation-model:|||', 'codex|x', 'denied')).toBe('Text generation model not saved');
    expect(settingsFailure(c, 'settings-core', 'default-model:effort|||', 'high', 'denied')).toBe('');
    expect(toasts(c).map(toast => [toast.kind, toast.title])).toEqual([['error', 'Default model not saved'], ['error', 'Text generation model not saved']]);
  });
});

describe('keyboard', () => {
  test('in Settings the open picker\'s jumps choose from its own list and skip disabled models', () => {
    const c = client();
    fleet.entries.set(B, entry(B, 'env-b', 'Build box', [{ ...codex, models: [codex.models[1]!] }, claude]));
    const context = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: true, draftThreadRoute: false, modalOpen: true, settingsOpen: true, diffOpen: false, settingsRoute: 'general' };
    const jumps = (target: string) => keyboardDispatch(c, [], 'codex', '', { ...context, modelTarget: target }).filter(item => item.command.startsWith('modelPicker.jump')).map(item => [item.command, item.target, item.extra]);
    expect(jumps('default-model:|||')).toEqual([['modelPicker.jump.1', 'gpt-6-astra', 'codex']]);
    expect(jumps('default-model:|env-a||')).toEqual([['modelPicker.jump.1', 'gpt-5.6-luna', 'codex'], ['modelPicker.jump.2', 'gpt-6-astra', 'codex']]);
    expect(keyboardDispatch(c, [], 'codex', '', { ...context, modelPickerOpen: false, modelTarget: 'default-model:|||' }).some(item => item.command.startsWith('modelPicker'))).toBe(false);
  });
});

// fix-misc-batch (#298 bug 8), read from the Contract source as dialog-focus.test.ts reads its dialogs: what
// the real-input batch found missing on screen. The macOS drive in tasks/closed/20261008-fix-misc-batch.md shows it;
// this guards the wiring.
describe('the picker\'s empty state (model-picker.contract)', () => {
  const lines = async () => (await Bun.file(new URL('model-picker.contract', import.meta.url)).text()).split('\n');
  const indent = (line: string) => line.length - line.trimStart().length;
  test('"No models found" is the list\'s sibling, not inside the scroll that has no height when nothing matches', async () => {
    const source = await lines();
    const list = source.findIndex(line => line.includes('testId="model-list"'));
    const empty = source.findIndex(line => line.includes('"No models found"') && line.includes('testId="model-empty"'));
    expect(source[list]).toContain('scroll flex=(count > 0 ? 1 : 0)');
    // The `when` that holds it sits at the scroll's own depth (ComboboxEmpty after ComboboxListVirtualized).
    expect(source[empty - 1]!.trim()).toBe('when count == 0 and setupCount == 0');
    expect(indent(source[empty - 1]!)).toBe(indent(source[list]!));
  });
});
