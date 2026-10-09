// model-picker-parity: the provider model picker as T3 Code 1e2ecbd975's ModelPickerContent, ModelPickerSidebar,
// ModelListRow and ProviderModelPicker draw it (MIT, see LICENSE-T3): ⇧⌘↓/⇧⌘↑ skip a provider that cannot be chosen
// (CO-4), rows show ⌘1–⌘9 (CO-1), a rail button names its state and reason (CO-2), a search highlights its first row
// (CO-3), Shift adds a model to a draft's several (CO-7), and Scheduled Tasks' Model and Source Control's writer model
// open the same picker (S2-4).
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { T3Client } from './client';
import { applyShell, initialShell, obj, type Obj } from './domain';
import { adjacentPickerProvider, railLabel } from './model-catalog';
import { modelCatalog, snapshot } from './presentation';
import { keyboardDispatch } from './keyboard-dispatch';
import { taskModelMarks } from './scheduled-view';
import { sourceControlRows } from './source-control-view';
import { connected } from './composer-controls-fixture';
import { fleet } from './settings-b-fleet';

const ready = { enabled: true, installed: true, availability: 'available', auth: { status: 'authenticated' }, status: 'ready' };
const codexError = { instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true, installed: true, auth: { status: 'unauthenticated' }, status: 'error',
  message: 'Codex CLI is not authenticated. Run `codex login` and try again.', models: [{ slug: 'gpt-6-luna', name: 'GPT-6-Luna', isDefault: true }] };
const claude = { instanceId: 'claudeAgent', driver: 'claudeAgent', displayName: 'Claude', ...ready,
  models: [{ slug: 'claude-opus-5-5', name: 'Claude Opus 5.5', badge: 'new' }, { slug: 'claude-sonnet-5-5', name: 'Claude Sonnet 5.5' }, { slug: 'claude-fable-5-1', name: 'Claude Fable 5.1', isDefault: true },
    { slug: 'claude-old', name: 'Claude Old', isLegacy: true }] };
const pickerOpen = { type: 'identifier', name: 'modelPickerOpen' };
const jumpBindings = Array.from({ length: 9 }, (_, index) => ({ command: `modelPicker.jump.${index + 1}`, whenAst: pickerOpen,
  shortcut: { key: String(index + 1), modKey: true, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false } }));
const arrows = [['arrowup', 'modelPicker.previousProvider'], ['arrowdown', 'modelPicker.nextProvider']].map(([key, command]) => ({ command, whenAst: pickerOpen,
  shortcut: { key, modKey: true, metaKey: false, ctrlKey: false, altKey: false, shiftKey: true } }));

function client(providers: Obj[] = [codexError, claude], settings: Obj = {}) {
  const c = new T3Client();
  Object.assign(c, { connection: 'connected', configLive: true, shellLive: true, threadLive: true, generation: 1, environmentId: 'env-a', origin: 'http://127.0.0.1:16800' });
  c.config = { environment: { environmentId: 'env-a', label: 'Studio', capabilities: {} }, providers, keybindings: [...jumpBindings, ...arrows], settings: { projectSettingsOverrides: {}, ...settings } };
  c.shell = applyShell(initialShell(), { snapshotSequence: 1, projects: [{ id: 'pa', title: 'App', workspaceRoot: '/a/app' }], threads: [] });
  c.projectId = 'pa'; c.providerId = 'claudeAgent'; c.modelId = 'claude-fable-5-1';
  return c;
}
const pickerContext = { composerFocus: false, editableFocus: false, turnRunning: false, modelPickerOpen: true, draftThreadRoute: true, modalOpen: false, settingsOpen: false, diffOpen: false };
beforeEach(() => { fleet.entries.clear(); fleet.saved = []; });
afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

describe('CO-4: the next and previous provider skip what cannot be chosen', () => {
  test('adjacentModelPickerProvider over Favorites and the selectable instances, wrapping', () => {
    const providers = [{ id: 'codex', selectable: false }, { id: 'claudeAgent', selectable: true }, { id: 'cursor', selectable: true }];
    expect(adjacentPickerProvider({ provider: 'favorites', providers }, 1)).toBe('claudeAgent');
    expect(adjacentPickerProvider({ provider: 'claudeAgent', providers }, 1)).toBe('cursor');
    expect(adjacentPickerProvider({ provider: 'cursor', providers }, 1)).toBe('favorites');
    expect(adjacentPickerProvider({ provider: 'favorites', providers }, -1)).toBe('cursor');
    expect(adjacentPickerProvider({ provider: 'claudeAgent', providers }, -1)).toBe('favorites');
    // A choice outside the list (the disabled Codex an active model left selected): down to Favorites, up to the last.
    expect(adjacentPickerProvider({ provider: 'codex', providers }, 1)).toBe('favorites');
    expect(adjacentPickerProvider({ provider: 'codex', providers }, -1)).toBe('cursor');
  });
  test('from Favorites, ⇧⌘↓ selects Claude past the disabled Codex button', () => {
    const c = client();
    c.local.favoriteModels = [JSON.stringify(['claudeAgent', 'claude-opus-5-5'])];
    const catalog = modelCatalog(c, '', '');
    expect(catalog.provider).toBe('favorites');
    expect(catalog.providers.map(rail => [rail.id, rail.selectable])).toEqual([['codex', false], ['claudeAgent', true]]);
    const keys = keyboardDispatch(c, [], '', '', pickerContext).filter(item => item.command.endsWith('Provider')).map(item => [item.command, item.target]);
    expect(keys).toEqual([['modelPicker.previousProvider', 'claudeAgent'], ['modelPicker.nextProvider', 'claudeAgent']]);
    const fromClaude = keyboardDispatch(c, [], 'claudeAgent', '', pickerContext).filter(item => item.command.endsWith('Provider')).map(item => item.target);
    expect(fromClaude).toEqual(['favorites', 'favorites']);
  });
});

describe('CO-1: the first nine rows a person can choose show their jump shortcut', () => {
  test('⌘1… on the provider list, the search results and Favorites; the legacy header takes none', () => {
    const c = client();
    const rows = (catalog: ReturnType<typeof modelCatalog>) => catalog.models.map(row => [row.kind, row.id, row.jump, row.jumpKey]);
    expect(rows(modelCatalog(c, 'claudeAgent', ''))).toEqual([['model', 'claude-opus-5-5', '⌘1', 'Meta+1'], ['model', 'claude-sonnet-5-5', '⌘2', 'Meta+2'],
      ['model', 'claude-fable-5-1', '⌘3', 'Meta+3'], ['legacy', '', '', ''], ['legacy-model', 'claude-old', '', '']]);
    expect(rows(modelCatalog(c, 'claudeAgent', 'sonnet'))).toEqual([['model', 'claude-sonnet-5-5', '⌘1', 'Meta+1']]);
    c.local.favoriteModels = [JSON.stringify(['claudeAgent', 'claude-fable-5-1'])];
    expect(rows(modelCatalog(c, '', ''))).toEqual([['model', 'claude-fable-5-1', '⌘1', 'Meta+1']]);
    // The dispatch's jump keys choose the same rows (a favorite leads its provider's list).
    expect(modelCatalog(c, 'claudeAgent', '').models.slice(0, 3).map(row => [row.id, row.jump])).toEqual([['claude-fable-5-1', '⌘1'], ['claude-opus-5-5', '⌘2'], ['claude-sonnet-5-5', '⌘3']]);
    expect(keyboardDispatch(c, [], 'claudeAgent', '', pickerContext).filter(item => item.command.startsWith('modelPicker.jump')).map(item => [item.command, item.target]))
      .toEqual([['modelPicker.jump.1', 'claude-fable-5-1'], ['modelPicker.jump.2', 'claude-opus-5-5'], ['modelPicker.jump.3', 'claude-sonnet-5-5']]);
  });
  test('a rebound jump shows its own chord; an unbound one shows nothing', () => {
    const c = client();
    c.config.keybindings = [{ ...jumpBindings[0], shortcut: { key: '1', modKey: true, altKey: true } }];
    expect(modelCatalog(c, 'claudeAgent', '').models.slice(0, 2).map(row => [row.jump, row.jumpKey])).toEqual([['⌥⌘1', 'Meta+Alt+1'], ['', '']]);
  });
});

describe('CO-2: a rail button names its state and the provider\'s message', () => {
  test('describeUnavailableInstance and the locked-thread tooltip', () => {
    expect(railLabel(codexError, false)).toBe('Codex — Unavailable. Codex CLI is not authenticated. Run `codex login` and try again.');
    expect(railLabel(claude, false)).toBe('Claude');
    expect(railLabel({ ...claude, status: 'warning', message: ' Rate limited. ' }, false)).toBe('Claude — Limited. Rate limited.');
    expect(railLabel({ ...claude, status: 'checking', message: '' }, false)).toBe('Claude — Not ready.');
    expect(railLabel({ ...claude, enabled: false }, false)).toBe('Claude — Disabled in settings.');
    expect(railLabel(claude, true)).toBe('Claude is unavailable in this thread. Start a new thread to switch providers.');
    expect(modelCatalog(client(), '', '').providers.map(rail => rail.label)).toEqual(['Codex — Unavailable. Codex CLI is not authenticated. Run `codex login` and try again.', 'Claude']);
  });
});

describe('CO-3: a search highlights its first row a person can choose', () => {
  test('firstEnabled is the first model row', () => {
    const c = client();
    expect(modelCatalog(c, 'claudeAgent', 'opus').firstEnabled).toBe(0);
    expect(modelCatalog(c, 'claudeAgent', 'zzzz').firstEnabled).toBe(-1);
  });
});

describe('CO-7: Shift adds a model to a draft\'s several and keeps the picker open', () => {
  test('the picker\'s own Shift (n 1) adds and removes without the native gesture; `multiple` says the picker stays open', async () => {
    const { client: c, native, disk, command } = await connected();
    obj(obj(native.config.environment).capabilities).requiredWorktreeBootstrap = true;
    await c.refresh(native, disk);
    expect(modelCatalog(c, 'codex', '').multiple).toBe(true);
    await command('model', 'model-b', 'codex', 1);
    expect(snapshot(c).composer).toMatchObject({ fanout: true, fanoutKeys: ['codex:model-a', 'codex:model-b'] });
    await command('model', 'model-b', 'codex', 1);
    expect(snapshot(c).composer.fanout).toBe(false);
    expect([c.providerId, c.modelId]).toEqual(['codex', 'model-a']);
    // A settings picker never adds (no onToggleModel).
    expect(modelCatalog(c, '', '', 'task-model:|env|').multiple).toBe(false);
  });
  test('a started thread has one model: Shift picks as a click does', async () => {
    const { client: c, native, disk, command } = await connected();
    expect(modelCatalog(c, 'codex', '').multiple).toBe(false);
    await command('model', 'model-b', 'codex', 1);
    expect(snapshot(c).composer.fanout).toBe(false);
    expect(c.modelId).toBe('model-b');
    void native; void disk;
  });
});

describe('S2-4: Scheduled Tasks\' Model and the writer model open the provider picker', () => {
  test('the scheduled task picker lists the editing environment\'s instances, checks the draft\'s model and offers no setup', () => {
    const setupCodex = { ...codexError, setup: { canAuthenticate: true } };
    const c = client([setupCodex, claude]);
    const target = `task-model:|env-a|${encodeURIComponent('claudeAgent:claude-sonnet-5-5')}`;
    const catalog = modelCatalog(c, '', '', target);
    expect(catalog.provider).toBe('claudeAgent');
    expect(catalog.models.filter(row => row.selected).map(row => row.id)).toEqual(['claude-sonnet-5-5']);
    expect(catalog.providers.map(rail => [rail.id, rail.selectable])).toEqual([['codex', false], ['claudeAgent', true]]);
    expect(catalog.setup).toEqual([]);
    expect(catalog.models.map(row => row.reason).every(reason => reason === '')).toBe(true);
    // No model yet: the first instance (Codex, which cannot be chosen) is the rail's choice, so "No models found".
    const empty = modelCatalog(c, '', '', 'task-model:|env-a|');
    expect([empty.provider, empty.count]).toEqual(['codex', 0]);
    // The General picker of the same environment still offers the setup.
    expect(modelCatalog(c, 'codex', '').setup.map(entry => entry.label)).toEqual(['Open provider setup']);
  });
  test('the Model trigger: the model name, the draft\'s unknown slug, or "Choose model"', () => {
    const marks = taskModelMarks([codexError, claude], 'claudeAgent:claude-gone');
    const of = (value: string) => marks.find(entry => entry.value === value);
    expect(of('claudeAgent:claude-opus-5-5')).toMatchObject({ name: 'Claude Opus 5.5', driver: 'claudeAgent' });
    expect(of('claudeAgent:claude-gone')).toMatchObject({ name: 'claude-gone', driver: 'claudeAgent' });
    expect(of('')).toMatchObject({ name: 'GPT-6-Luna', driver: 'codex' });
    expect(taskModelMarks([{ ...codexError, models: [] }, claude], '').find(entry => entry.value === '')).toMatchObject({ name: 'Choose model', driver: 'codex' });
  });
  test('the writer picker: text generation instances, the writer\'s model, provider setup and the page\'s scope', () => {
    const cursor = { instanceId: 'cursor', driver: 'cursor', displayName: 'Cursor', ...ready, supportsTextGeneration: false, models: [{ slug: 'auto', name: 'Auto' }] };
    const setupCodex = { ...codexError, setup: { canAuthenticate: true } };
    const c = client([setupCodex, claude, cursor], { sourceControlWriterModelSelection: { instanceId: 'claudeAgent', model: 'claude-opus-5-5' },
      projectSettingsOverrides: { pa: { sourceControlWriterModelSelection: { instanceId: 'claudeAgent', model: 'claude-sonnet-5-5' } } } });
    const environment = modelCatalog(c, '', '', 'source-control-writer-model:|env-a:');
    expect(environment.providers.map(rail => rail.id)).toEqual(['codex', 'claudeAgent']);
    expect(environment.models.filter(row => row.selected).map(row => row.id)).toEqual(['claude-opus-5-5']);
    expect(modelCatalog(c, 'codex', '', 'source-control-writer-model:|env-a:').setup.map(entry => entry.label)).toEqual(['Open provider setup']);
    const project = modelCatalog(c, '', '', 'source-control-writer-model:|env-a:pa');
    expect(project.models.filter(row => row.selected).map(row => row.id)).toEqual(['claude-sonnet-5-5']);
    // The row's trigger: the writer's mark and name.
    const rows = sourceControlRows(obj(c.config.settings), 'pa', 'Studio', c.config.providers as Obj[], true).text;
    expect(rows.find(row => row.kind === 'model')).toMatchObject({ checked: true, valueLabel: 'Claude Sonnet 5.5', driver: 'claudeAgent', status: '' });
  });
});
