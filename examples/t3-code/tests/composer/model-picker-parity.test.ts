// The provider model picker as T3 Code 1e2ecbd975's ModelPickerContent, ModelPickerSidebar,
// ModelListRow and ProviderModelPicker draw it (MIT, see LICENSE-T3): ⇧⌘↓/⇧⌘↑ skip a provider that cannot be chosen,
// rows show ⌘1–⌘9, a rail button names its state and reason, a search highlights its first row,
// Shift adds a model to a draft's several, and Scheduled Tasks' Model and Source Control's writer model
// open the same picker.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { applyShell, initialShell, obj, type Obj } from '../../src/client/domain';
import { adjacentPickerProvider, railLabel } from '../../src/composer/model-catalog';
import { modelCatalog, snapshot } from '../../src/client/presentation';
import { keyboardDispatch } from '../../src/shell/keyboard-dispatch';
import { taskModelMarks } from '../../src/settings/scheduled-view';
import { sourceControlRows } from '../../src/settings/source-control-view';
// Writing & review's rows, its Advanced ones after them (source-control-view.ts sourceControlRows).
const writingRows = (rows: ReturnType<typeof sourceControlRows>) => [...rows.writing, ...rows.writingAdvanced];
import { connected } from '../support/composer-controls-fixture';
import { fleet } from '../../src/connections/settings-b-fleet';
import { appPath, appText } from '../support/app-files';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';

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
const source = (file: string) => Bun.file(appPath(file)).text();
/** The lines from the first line of `file` that starts with `head` (after its indent) to the next line indented as deep or less. */
async function block(file: string, head: string): Promise<string> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line.trimStart().startsWith(head));
  if (start < 0) throw new Error(`${file}: no ${head}`);
  const indent = lines[start]!.length - lines[start]!.trimStart().length;
  const end = lines.findIndex((line, index) => index > start && line.trim() !== '' && line.length - line.trimStart().length <= indent);
  return lines.slice(start, end < 0 ? undefined : end).map(entry => entry.slice(indent)).join('\n');
}
/** A new thread's draft that can start several models (codex:model-a), and a Shift press for the native recorder. */
async function draft() {
  const context = await connected();
  obj(obj(context.native.config.environment).capabilities).requiredWorktreeBootstrap = true;
  await context.client.refresh(context.native, context.disk);
  return context;
}
beforeEach(() => { fleet.entries.clear(); fleet.saved = []; });
afterEach(() => { fleet.entries.clear(); fleet.saved = []; });

describe('the next and previous provider skip what cannot be chosen', () => {
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

describe('the first nine rows a person can choose show their jump shortcut', () => {
  test('⌘1… on the provider list, the search results and Favorites; the legacy header takes none, its rows follow', () => {
    const c = client();
    const rows = (catalog: ReturnType<typeof modelCatalog>) => catalog.models.map(row => [row.kind, row.id, row.jump, row.jumpKey]);
    expect(rows(modelCatalog(c, 'claudeAgent', ''))).toEqual([['model', 'claude-opus-5-5', '⌘1', 'Meta+1'], ['model', 'claude-sonnet-5-5', '⌘2', 'Meta+2'],
      ['model', 'claude-fable-5-1', '⌘3', 'Meta+3'], ['legacy', '', '', ''], ['legacy-model', 'claude-old', '⌘4', 'Meta+4']]);
    // The dispatch's own jump buttons cover the current rows; an open Legacy section's rows answer to their own aria-keyshortcuts.
    expect(keyboardDispatch(c, [], 'claudeAgent', '', pickerContext).filter(item => item.command.startsWith('modelPicker.jump')).map(item => item.command))
      .toEqual(['modelPicker.jump.1', 'modelPicker.jump.2', 'modelPicker.jump.3']);
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

describe('a rail button names its state and the provider\'s message', () => {
  test('describeUnavailableInstance and the locked-thread tooltip', () => {
    expect(railLabel(codexError, false)).toBe('Codex: Unavailable. Codex CLI is not authenticated. Run `codex login` and try again.');
    expect(railLabel(claude, false)).toBe('Claude');
    expect(railLabel({ ...claude, status: 'warning', message: ' Rate limited. ' }, false)).toBe('Claude: Limited. Rate limited.');
    expect(railLabel({ ...claude, status: 'checking', message: '' }, false)).toBe('Claude: Not ready.');
    expect(railLabel({ ...claude, enabled: false }, false)).toBe('Claude: Disabled in settings.');
    expect(railLabel(claude, true)).toBe('Claude is unavailable in this thread. Start a new thread to switch providers.');
    expect(modelCatalog(client(), '', '').providers.map(rail => rail.label)).toEqual(['Codex: Unavailable. Codex CLI is not authenticated. Run `codex login` and try again.', 'Claude']);
  });
});

// The rail's tooltip is drawn by the window's hover layer, so it outlives the rail: Escape or a pick unmounts the button
// under the pointer, which then gets no leave. The oracle's TooltipPopup goes with the picker; the root's hover surface
// counts the picker, and its change ends the tip.
describe('the rail tooltip goes with the picker', () => {
  test('closing the picker under a hovered rail button ends its tip', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const fns = contractFns('src/shell/hover-layer.contract', ['HoverTip']) as unknown as { hoverTipAtFrame: (...args: unknown[]) => Env };
    const label = 'Codex: Unavailable. Could not start Codex CLI.';
    const tip = fns.hoverTipAtFrame('picker-rail-tip-1', 'tip', label, [], 'left', 'center', { x: 60, y: 400, width: 36, height: 36 }, 300, 'picker-rail-tip-1');
    const source = appText('app.contract');
    const shownWhen = /^ {2}derive hoverShown = (.+)$/m.exec(source)![1]!;
    const state: Env = { hoverTip: { key: '' }, hoverOn: '', hoverOpen: 0, hoverUntil: 0, hoverWait: 0, hoverEnd: 0 };
    const scope: Env = { hoverOpenDelay: () => 0, hoverDelay: () => 0, hoverShown: false };
    const shown = () => value(shownWhen, state);
    root.call('hoverTipAt', state, scope, [tip, 'trigger', true]);
    expect(shown()).toBe(true);
    const task = /^ {2}task hoverSurface key=(`[^`]*`)$/m.exec(source)![1]!;
    const surface = (modelsOpen: boolean) => value(task, { settingsOpen: false, settingsRoute: '', utilityPage: '', modal: '', prSelected: '', data: { threadId: 't1' }, modelsOpen });
    expect(surface(false)).not.toBe(surface(true));
    // The key changed: the task runs hoverTicked a millisecond later.
    root.call('hoverTicked', state, scope, []);
    expect(shown()).toBe(false);
  });
});

describe('a search highlights its first row a person can choose', () => {
  test('firstEnabled is the first model row', () => {
    const c = client();
    expect(modelCatalog(c, 'claudeAgent', 'opus').firstEnabled).toBe(0);
    expect(modelCatalog(c, 'claudeAgent', 'zzzz').firstEnabled).toBe(-1);
  });
  // Read from the Contract source, as the stacking test below: the highlight and Return are the picker's view state.

  test('Return\'s target is any highlighted model row a person can choose, an open Legacy section\'s rows too', async () => {
    const picker = await block('model-picker.contract', 'component ModelPicker');
    const choosable = '(r.kind == "model" or r.kind == "legacy-model") and r.reason == ""';
    expect(picker).toContain(`derive targetId = catalog.models.filter((r) => r.index == highlight and ${choosable}).map((r) => r.id).join("")`);
    expect(picker).toContain(`derive targetProvider = catalog.models.filter((r) => r.index == highlight and ${choosable}).map((r) => r.providerId).join("")`);
    // The highlight reaches a legacy row only while its section is open: `count` holds the legacy rows then, and they follow the header.
    expect(picker).toContain('derive count = (legacyOpen ? catalog.count : catalog.count - catalog.legacyCount)');
    const catalog = modelCatalog(client(), 'claudeAgent', '');
    expect([catalog.count, catalog.legacyCount]).toEqual([5, 1]);
    expect(catalog.models.filter(row => row.kind !== 'model').map(row => [row.kind, row.index, row.id])).toEqual([['legacy', 3, ''], ['legacy-model', 4, 'claude-old']]);
  });
});

describe('Shift adds a model to a draft\'s several and keeps the picker open', () => {
  test('the picker\'s own Shift (n 1) adds and removes; `multiple` says the picker stays open', async () => {
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
  test('a ⌘N jump right after a Shift+click switches to its model', async () => {
    const { client: c, command } = await draft();
    await command('model', 'model-b', 'codex', 1);
    expect(snapshot(c).composer.fanout).toBe(true);
    // modelPicker.jump.N is the dispatch's plain pick (n 0).
    await command('model', 'model-b', 'codex-work', 0);
    expect(snapshot(c).composer.fanout).toBe(false);
    expect([c.providerId, c.modelId]).toEqual(['codex-work', 'model-b']);
  });
  test('Shift+Return on a focused row adds its model, as Shift+click does; a plain Return is the row\'s press', () => {
    const row = contractComponent('model-picker.contract', 'ModelRow');
    const selects: unknown[][] = [];
    const key = (modifiers: Record<string, boolean>) => row.call('rowKey', {}, { row: { id: 'model-b', providerId: 'codex', reason: '' }, busy: false,
      preventDefault: () => selects.push(['preventDefault']), select: (...args: unknown[]) => selects.push(args) },
      ['Enter', { key: 'Enter', code: 'Enter', repeat: false, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...modifiers }]);
    key({ shiftKey: true });
    key({});
    key({ shiftKey: true, metaKey: true });
    expect(selects).toEqual([['preventDefault'], ['model-b', 'codex', true]]);
  });
  // Read from the Contract source: the row's press, and the root's pick that keeps the picker open.
});

describe('the trigger is named by its model names; the Plan toggle reports pressed', () => {
  test('the names: the model the trigger shows, or every chosen model of a draft\'s several', async () => {
    const { client: c, command } = await draft();
    expect(snapshot(c).modelLabel).toBe('Model A');
    await command('model', 'model-b', 'codex', 1);
    expect(snapshot(c).composer.fanoutAria).toBe('Model A, Model B');
  });
});

describe('Scheduled Tasks\' Model and the writer model open the provider picker', () => {
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
    expect(of('claudeAgent:claude-opus-5-5')).toMatchObject({ name: 'Opus 5.5', driver: 'claudeAgent' }); // getTriggerDisplayModelName: the driver's family goes
    expect(of('claudeAgent:claude-gone')).toMatchObject({ name: 'claude-gone', driver: 'claudeAgent' });
    expect(of('')).toMatchObject({ name: '6 Luna', driver: 'codex' }); // modelShortName: GPT-6.1-Sol reads 6.1 Sol
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
    const rows = writingRows(sourceControlRows(obj(c.config.settings), 'pa', 'Studio', c.config.providers as Obj[], true));
    expect(rows.find(row => row.kind === 'model')).toMatchObject({ checked: true, valueLabel: 'Sonnet 5.5', driver: 'claudeAgent', status: '' });
  });
});
