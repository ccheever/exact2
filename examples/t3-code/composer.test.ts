import { describe, expect, test } from 'bun:test';
import { composerPlaceholder, threadPhase, traitsMenu, commandChords, anchorsFrom, composerView, runtimeModes } from './composer-presentation';
import { pickerCatalog, scoreModel, favoriteKeys, pickerReady } from './model-catalog';

const badge = () => ({ providerBadge: '', providerBadgeColor: '' });
const provider = (instanceId: string, models: object[], extra: object = {}) => ({ instanceId, driver: 'codex', displayName: instanceId === 'codex' ? 'Codex' : 'Exact verification fixture',
  enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' }, models, ...extra });
const effort = { id: 'reasoningEffort', label: 'Reasoning', type: 'select', options: [
  { id: 'low', label: 'Low' }, { id: 'medium', label: 'Medium', isDefault: true }, { id: 'high', label: 'High' }] };
const fixtureModels = [{ slug: 'gpt-5.6-luna', name: 'GPT-5.6-Luna', capabilities: { optionDescriptors: [effort] } },
  { slug: 'gpt-6-astra', name: 'GPT-6-Astra', capabilities: { optionDescriptors: [effort] } }];
const client = (overrides: object = {}) => ({ config: { providers: [provider('exact_fixture', fixtureModels)] }, projection: {}, presentation: {},
  providerId: 'exact_fixture', modelId: 'gpt-6-astra', modelOptions: [], runtimeMode: 'full-access', interactionMode: 'default',
  threadId: 't1', projectId: 'p1', connection: 'connected', local: { favoriteModels: [] as string[], deviceSettings: { planModeEnabled: false } }, ...overrides });

describe('composer placeholder and phase', () => {
  test('follows the served build chain', () => {
    const base = { connected: true, approval: false, question: false, choiceOnly: false, projectRequired: false, providerUnavailable: false, phase: 'ready' };
    expect(composerPlaceholder(base)).toBe('Ask anything, @tag files/folders, $use skills, or / for commands');
    expect(composerPlaceholder({ ...base, phase: 'disconnected' })).toBe('Ask for changes, send follow-ups, or attach images');
    expect(composerPlaceholder({ ...base, approval: true })).toBe('Resolve this approval request to continue');
    expect(composerPlaceholder({ ...base, question: true, choiceOnly: true })).toBe('Choose an option above');
    expect(composerPlaceholder({ ...base, question: true })).toBe('Type your own answer, or leave this blank to use the selected option');
    expect(composerPlaceholder({ ...base, projectRequired: true })).toBe('Choose a project above to start a thread');
    expect(composerPlaceholder({ ...base, providerUnavailable: true })).toBe('Enable a provider in Settings to send a message');
    expect(composerPlaceholder({ ...base, connected: false })).toBe('Connect to your T3 server to send a message');
    // A pending request keeps its own prompt while the server is unreachable.
    expect(composerPlaceholder({ ...base, connected: false, approval: true })).toBe('Resolve this approval request to continue');
    expect(composerPlaceholder({ ...base, connected: false, question: true, choiceOnly: true })).toBe('Choose an option above');
    expect(composerPlaceholder({ ...base, planReady: true })).toBe('Add feedback to refine the plan, or leave this blank to implement it');
  });
  test('a thread without runs or a provider thread is disconnected; held queues do not count', () => {
    expect(threadPhase({ thread: {}, runs: [] })).toBe('disconnected');
    expect(threadPhase({ thread: {}, runs: [{ ordinal: 1, status: 'queued', queueHeld: true }] })).toBe('disconnected');
    expect(threadPhase({ thread: { activeProviderThreadId: 'p' }, runs: [] })).toBe('ready');
    expect(threadPhase({ thread: {}, runs: [{ ordinal: 1, status: 'completed' }] })).toBe('ready');
    expect(threadPhase({ thread: {}, runs: [{ ordinal: 1, status: 'completed' }, { ordinal: 2, status: 'running' }] })).toBe('running');
    expect(threadPhase({ thread: {}, runs: [{ ordinal: 2, status: 'starting' }] })).toBe('connecting');
    // A follow-up queued behind the running turn keeps the thread running (deriveThreadActivityRun).
    expect(threadPhase({ thread: {}, runs: [{ ordinal: 1, status: 'running' }, { ordinal: 2, status: 'queued' }] })).toBe('running');
    expect(threadPhase({ thread: {}, runs: [{ ordinal: 1, status: 'completed' }, { ordinal: 2, status: 'queued' }] })).toBe('connecting');
  });
});

describe('composer controls', () => {
  test('traits use the descriptor label, every select option and the Default badge', () => {
    const menu = traitsMenu([effort], []);
    expect(menu.label).toBe('Medium');
    expect(menu.items.map(item => [item.kind, item.label, item.selected, item.isDefault, item.index])).toEqual([
      ['header', 'Reasoning', false, false, -1], ['option', 'Low', false, false, 0], ['option', 'Medium', true, true, 1], ['option', 'High', false, false, 2]]);
    const two = traitsMenu([effort, { id: 'serviceTier', label: 'Speed', type: 'select', options: [{ id: 'default', label: 'Normal' }, { id: 'fast', label: 'Fast' }] },
      { id: 'thinking', label: 'Thinking', type: 'boolean', currentValue: true }], [{ id: 'reasoningEffort', value: 'high' }, { id: 'serviceTier', value: 'fast' }]);
    expect(two.label).toBe('High · Fast');
    // Boolean traits are On/Off radio groups after the selects (TraitsMenuContent).
    expect(two.items.map(item => item.kind)).toEqual(['header', 'option', 'option', 'option', 'divider', 'header', 'option', 'option', 'divider', 'header', 'option', 'option']);
    expect(two.items.slice(-3).map(item => [item.label, item.value, item.selected])).toEqual([['Thinking', '', false], ['On', 'true', true], ['Off', 'false', false]]);
    expect(two.count).toBe(7);
    expect(two.selected).toBe(2);
  });
  test('runtime options follow the provider support list and the served labels', () => {
    expect(runtimeModes.map(mode => mode.label)).toEqual(['Supervised', 'Auto-accept edits', 'Auto', 'Full access']);
    const view = composerView(client({ config: { providers: [provider('exact_fixture', fixtureModels, { supportedRuntimeModes: ['approval-required', 'full-access'] })] } }),
      { approval: false, question: false, choiceOnly: false });
    expect(view.runtimes.map(option => option.mode)).toEqual(['approval-required', 'full-access']);
    expect(view).toMatchObject({ runtimeLabel: 'Full access', runtimeIcon: 'lock-open', runtimeSelected: 1, traitsLabel: 'Medium', planVisible: false });
    const legacy = composerView(client({ runtimeMode: 'auto' , config: { providers: [provider('exact_fixture', fixtureModels, { supportedRuntimeModes: ['approval-required'] })] } }),
      { approval: false, question: false, choiceOnly: false });
    expect(legacy.runtimeLabel).toBe('Supervised');
  });
  test('plan toggle needs the device setting and a provider that allows it', () => {
    const on = { local: { favoriteModels: [], deviceSettings: { planModeEnabled: true } } };
    expect(composerView(client({ ...on, interactionMode: 'plan' }), { approval: false, question: false, choiceOnly: false })).toMatchObject({ planVisible: true, planActive: true });
    expect(composerView(client({ ...on, config: { providers: [provider('exact_fixture', fixtureModels, { showInteractionModeToggle: false })] } }),
      { approval: false, question: false, choiceOnly: false }).planVisible).toBe(false);
  });
  test('shortcuts resolve the configured chords, later rules winning', () => {
    const config = { keybindings: [
      { command: 'composer.effort', shortcut: { key: 'e', modKey: true, shiftKey: true }, whenAst: { type: 'not', node: { type: 'identifier', name: 'terminalFocus' } } },
      { command: 'composer.mode', shortcut: { key: 'a', modKey: true, shiftKey: true } },
      { command: 'chat.new', shortcut: { key: 'a', modKey: true, shiftKey: true } },
    ] };
    expect(commandChords(config, 'composer.effort', 'x')).toBe('Meta+Shift+e');
    expect(commandChords(config, 'composer.mode', 'x')).toBe('');
    expect(commandChords({}, 'composer.mode', 'Meta+Shift+A')).toBe('Meta+Shift+A');
  });
  test('trigger anchors come from the module status', () => {
    expect(anchorsFrom({ anchors: { traits: [147, 90.5], runtime: [250.5, 133], controls: [0, 393] } })).toEqual({ traits: { x: 147, width: 90.5 }, runtime: { x: 250.5, width: 133 }, controls: { x: 0, width: 393 }, implement: { x: 0, width: 0 }, meter: { x: 0, width: 0 }, actions: { x: 0, width: 0 }, more: { x: 0, width: 0 } });
    expect(anchorsFrom({})).toEqual({ traits: { x: 0, width: 0 }, runtime: { x: 0, width: 0 }, controls: { x: 0, width: 0 }, implement: { x: 0, width: 0 }, meter: { x: 0, width: 0 }, actions: { x: 0, width: 0 }, more: { x: 0, width: 0 } });
  });
});

describe('model picker catalog', () => {
  const two = () => client({ config: { providers: [provider('exact_fixture', fixtureModels),
    provider('other', [{ slug: 'm-old', name: 'Old', isLegacy: true }, { slug: 'm-new', name: 'Mini', shortName: 'Mini S', badge: 'new' }, { slug: 'm-b', name: 'Beta' }], { displayName: 'Other account' }),
    provider('off', [{ slug: 'x', name: 'X' }], { enabled: false }), provider('limited', [{ slug: 'y', name: 'Y' }], { status: 'warning' })] } });
  test('opens on the active instance, or favorites when any exist, and never touches the selection', () => {
    const c = two();
    const catalog = pickerCatalog(c, '', '', badge);
    expect(catalog.provider).toBe('exact_fixture');
    expect(catalog.models.map(row => [row.name, row.selected])).toEqual([['GPT-5.6-Luna', false], ['GPT-6-Astra', true]]);
    expect(catalog.providers.map(rail => [rail.id, rail.ready, rail.index])).toEqual([['exact_fixture', true, 1], ['other', true, 2], ['limited', false, 3]]);
    expect(catalog.railIndex).toBe(1);
    c.local.favoriteModels = [JSON.stringify(['other', 'm-b'])];
    const favorites = pickerCatalog(c, '', '', badge);
    expect(favorites).toMatchObject({ provider: 'favorites', favoritesSelected: true, railIndex: 0 });
    expect(favorites.models.map(row => row.id)).toEqual(['m-b']);
    expect(c.providerId).toBe('exact_fixture');
  });
  test('favorites lead an instance list; legacy models follow a collapsible header', () => {
    const c = two();
    c.local.favoriteModels = [JSON.stringify(['other', 'm-b'])];
    const catalog = pickerCatalog(c, 'other', '', badge);
    expect(catalog.models.map(row => [row.kind, row.name, row.index])).toEqual([['model', 'Beta', 0], ['model', 'Mini S', 1], ['legacy', 'Legacy models', 2], ['legacy-model', 'Old', 3]]);
    expect(catalog.models[2]!.label).toBe('1 models');
    expect(catalog).toMatchObject({ count: 4, legacyCount: 1 });
    expect(catalog.models[1]!.isNew).toBe(true);
  });
  test('search crosses instances, ranks by field and boosts favorites; rail hides', () => {
    const c = two();
    const catalog = pickerCatalog(c, 'other', 'astra', badge);
    expect(catalog.searching).toBe(true);
    expect(catalog.models.map(row => row.id)).toEqual(['gpt-6-astra']);
    expect(pickerCatalog(c, '', 'other account beta', badge).models.map(row => row.id)).toEqual(['m-b']);
    expect(pickerCatalog(c, '', 'zzz', badge).count).toBe(0);
    const item = { id: 'a', name: 'GPT-6-Astra', shortName: '', subProvider: '', providerId: 'p', providerName: 'Exact', driver: 'codex', favorite: false, legacy: false, isNew: false, order: 0 };
    expect(scoreModel(item, 'gpt')! < scoreModel(item, 'astra')!).toBe(true);
    expect(scoreModel({ ...item, favorite: true }, 'astra')).toBe(scoreModel(item, 'astra')! - 24);
    expect(scoreModel(item, 'gat')).not.toBeNull();
  });
  test('rail badges count only enabled sibling instances', () => {
    const counted: number[] = [];
    const counting = (_provider: unknown, providers: unknown[]) => { counted.push(providers.length); return { providerBadge: '', providerBadgeColor: '' }; };
    pickerCatalog(client({ config: { providers: [provider('exact_fixture', fixtureModels), provider('codex', fixtureModels, { enabled: false })] } }), '', '', counting);
    expect(counted).toEqual([1]);
  });
  test('readiness and favorite keys follow the reference', () => {
    expect(pickerReady({ enabled: true, status: 'ready' })).toBe(true);
    expect(pickerReady({ enabled: true, status: 'ready', availability: 'unavailable' })).toBe(false);
    expect(pickerReady({ enabled: false, status: 'ready' })).toBe(false);
    expect([...favoriteKeys([JSON.stringify(['a', 'b']), 'not json'])]).toEqual(['a:b']);
  });
});
