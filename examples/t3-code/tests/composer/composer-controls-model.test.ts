// Upstream 4f7760e6a0 (subagent model names),
// 63f74e209d (provider-reported option labels), 151c241cd2 + a5b34b2537
// (background-work wording) and 5108c978b1 (latest executed run).
import { describe, expect, test } from 'bun:test';
import { announcements } from '../../src/shell/announce';
import { obj, type Obj } from '../../src/client/domain';
import { displayModelName, formatModelSlugName, reportedModelLabel, reportedSelection, resolveSelectableModel, triggerModelName } from '../../src/composer/composer-controls-model';
import { modelShortName } from '../../src/client/model-short-name';
import { presentBackgroundWork, traitsDisplay } from '../../src/composer/composer-controls-view';
import { effortColumn } from '../../src/composer/composer-model-effort';
import { latestExecutedRun, resumeState } from '../../src/composer/composer-controls';
import { selectionEffort, subagentBar } from '../../src/composer/composer-controls-subagent';
import { modelCatalog, snapshot } from '../../src/client/presentation';
import { opened } from '../support/composer-controls-fixture';
import { threadNowRows } from '../../src/composer/thread-end';

const claudeModels = [
  { slug: 'claude-haiku-4-5', name: 'Claude Haiku 4.5', shortName: 'Haiku 4.5', aliases: ['claude-haiku-4-5-20251001', 'haiku'] },
  { slug: 'claude-opus-4-6', name: 'Claude Opus 4.6' },
  { slug: 'anthropic/claude-sonnet', name: 'Anthropic: Claude Sonnet', subProvider: 'Anthropic' },
];
const variant = { id: 'variant', label: 'Reasoning', type: 'select', options: [{ id: 'thinking', label: 'Thinking' }, { id: 'fast', label: 'Fast' }] };
const openCode = (options: object[] = []) => ({ instanceId: 'opencode', model: 'openai/gpt-5', options });

describe('the name you would say out loud (modelShortName.test.ts)', () => {
  test.each([
    ['Claude Opus 5.5', ['Claude '], 'Opus 5.5'], ['GPT-6.1-Sol', ['GPT-'], '6.1 Sol'], ['GPT 6.0 Astra', ['GPT-', 'GPT '], '6.0 Astra'],
    ['Grok 4 Pro', [], undefined], ['Claude Opus 5.5', [], 'Opus 5.5'], ['NVIDIA: Nemotron 3 Super', [], 'Nemotron 3 Super'],
    ['GPT-5.5', ['GPT-'], undefined], ['Gemini 3.1 Pro', ['Gemini '], '3.1 Pro'],
  ] as const)('gives %s on families %j the short name %s', (name, families, expected) => {
    expect(modelShortName(name, families)).toBe(expected);
  });
  test('a trigger says the spoken name on its driver\'s families; rows, tooltips and accessible names the full one', () => {
    const gemini = { slug: 'gemini-3.1-pro', name: 'Gemini 3.1 Pro' };
    expect(triggerModelName(gemini, 'antigravity')).toBe('3.1 Pro');
    expect(triggerModelName(gemini, 'cursor')).toBe('Gemini 3.1 Pro');
    expect(triggerModelName({ slug: 'x', name: 'Claude Opus 4.5', shortName: 'Opus' }, 'claudeAgent')).toBe('Opus');
    expect(displayModelName({ slug: 'x', name: 'Claude Opus 4.5', shortName: 'Opus' })).toBe('Claude Opus 4.5');
    // A caller that names no provider keeps the full name (or the server's shortName): its label does not change.
    expect(triggerModelName({ slug: 'x', name: 'Claude Sonnet 5.5' })).toBe('Claude Sonnet 5.5');
    expect(triggerModelName({ slug: 'x', name: 'Claude Sonnet 5.5', shortName: 'Sonnet' })).toBe('Sonnet');
  });
  test('the trigger on an Antigravity model says "3.1 Pro", its accessible name the full name', async () => {
    const { client } = await opened();
    const providers = client.config.providers as Obj[], claude = providers.find(provider => provider.instanceId === 'claude')!;
    providers.push({ ...claude, instanceId: 'antigravity', driver: 'antigravity', displayName: 'Antigravity',
      models: [{ slug: 'gemini-3.1-pro', name: 'Gemini 3.1 Pro', isDefault: true, capabilities: { optionDescriptors: [] } }] });
    client.providerId = 'antigravity'; client.modelId = 'gemini-3.1-pro';
    expect(snapshot(client)).toMatchObject({ modelShortLabel: '3.1 Pro', modelLabel: 'Gemini 3.1 Pro' });
  });
});

describe('the composer trigger names the model it runs', () => {
  test('the trigger says "Opus 4.5"; its tooltip and accessible name the full name; picker rows the full name', async () => {
    const { client } = await opened();
    const claude = (client.config.providers as Obj[]).find(provider => provider.instanceId === 'claude')!;
    claude.models = [{ slug: 'claude-opus-4-5', name: 'Claude Opus 4.5', isDefault: true, capabilities: { optionDescriptors: [] } }];
    client.providerId = 'claude'; client.modelId = 'claude-opus-4-5';
    const view = snapshot(client);
    expect(view).toMatchObject({ modelShortLabel: 'Opus 4.5', modelLabel: 'Claude Opus 4.5', modelUnavailable: false });
    expect(view.composer.modelTip).toBe('Claude Opus 4.5 · ⇧⌘M');
    expect(modelCatalog(client, 'claude', '').models.find(row => row.id === 'claude-opus-4-5')?.name).toBe('Claude Opus 4.5');
    // An unavailable model: the tooltip says so, and its badge's text is measured for the footer's layout.
    Object.assign(obj(claude.models[0]), { isUnavailable: true });
    const unavailable = snapshot(client);
    expect(unavailable.composer.modelTip).toBe('Claude Opus 4.5 (Unavailable) · ⇧⌘M');
    expect(unavailable.composer.menuProbes).toContainEqual({ key: 't:10:500:Unavailable', text: 'Unavailable', size: 10, weight: 500 });
  });
});

describe('subagent model names (4f7760e6a0)', () => {
  test('a reported dated id or alias resolves to the catalog entry and its trigger name', () => {
    expect(resolveSelectableModel('claudeAgent', 'claude-haiku-4-5-20251001', claudeModels)).toBe('claude-haiku-4-5');
    expect(resolveSelectableModel('claudeAgent', 'CLAUDE OPUS 4.6', claudeModels)).toBe('claude-opus-4-6');
    expect(resolveSelectableModel('cursor', 'opus-4.6', claudeModels)).toBe('claude-opus-4-6');
    expect(resolveSelectableModel('claudeAgent', 'unknown', claudeModels)).toBe('');
    expect(reportedModelLabel('claudeAgent', 'claude-haiku-4-5-20251001', claudeModels)).toBe('Haiku 4.5');
    // modelShortName on an unknown driver: the vendor prefix and the family go; "Sonnet" still names the model. With no
    // driver named, only the sub-provider qualifier goes.
    expect(triggerModelName(claudeModels[2]!, '')).toBe('Sonnet');
    expect(triggerModelName(claudeModels[2]!)).toBe('Claude Sonnet');
    expect(reportedModelLabel('claudeAgent', 'claude-sonnet-4-6', claudeModels)).toBe('Claude Sonnet 4.6');
    expect(formatModelSlugName('gpt-6-astra')).toBe('GPT-6-Astra');
    expect(formatModelSlugName('openai/gpt-5.4-mini')).toBe('openai/GPT-5.4-Mini');
    expect(formatModelSlugName('some-custom-model')).toBe('some-custom-model');
  });
  test('the bar shows the resolved short name', async () => {
    const { client } = await opened();
    const provider = (client.config.providers as Array<Record<string, unknown>>).find(entry => entry.instanceId === 'claude')!;
    provider.models = claudeModels;
    Object.assign(obj(client.thread!.projection.thread), { lineage: { relationshipToParent: 'subagent', parentThreadId: 't2' }, creationSource: 'provider',
      modelSelection: { instanceId: 'claude', model: 'claude-haiku-4-5-20251001' } });
    expect(subagentBar(client, 0)).toMatchObject({ subagent: true, subagentModel: 'Haiku 4.5', subagentEffort: '' });
  });
});

describe('provider-reported option labels (63f74e209d)', () => {
  test('the report belongs to the active provider thread of the selected instance', () => {
    const projection = { thread: { activeProviderThreadId: 'pt2', modelSelection: { instanceId: 'opencode', model: 'openai/gpt-5' } },
      providerThreads: [{ id: 'pt1', providerInstanceId: 'opencode', nativeMetadata: { modelSelection: openCode([{ id: 'variant', value: 'fast' }]) } },
        { id: 'pt2', providerInstanceId: 'opencode', nativeMetadata: { modelSelection: openCode([{ id: 'variant', value: 'thinking' }]) } }] };
    expect(reportedSelection(projection)).toEqual(openCode([{ id: 'variant', value: 'thinking' }]));
    expect(reportedSelection({ ...projection, thread: { ...projection.thread, activeProviderThreadId: 'pt9' } })).toBeNull();
    expect(reportedSelection({ ...projection, thread: { ...projection.thread, modelSelection: { instanceId: 'other', model: 'x' } } })).toBeNull();
  });
  test('an unset OpenCode variant reads Unknown until the provider reports it', () => {
    const selection = openCode();
    expect(traitsDisplay('opencode', [variant], [], selection, null).label).toBe('Unknown');
    expect(traitsDisplay('opencode', [{ ...variant, currentValue: 'fast' }], [], selection, null).label).toBe('Unknown');
    expect(traitsDisplay('opencode', [variant], [], selection, openCode([{ id: 'variant', value: 'default' }])).label).toBe('Default');
    expect(traitsDisplay('opencode', [variant], [], selection, openCode([{ id: 'variant', value: 'thinking' }])).label).toBe('Thinking');
    // An explicit choice, or another model, drops the report.
    const chosen = [{ id: 'variant', value: 'fast' }];
    expect(traitsDisplay('opencode', [variant], chosen, openCode(chosen), openCode([{ id: 'variant', value: 'thinking' }])).label).toBe('Fast');
    expect(traitsDisplay('opencode', [variant], [], { ...selection, model: 'openai/gpt-6' }, openCode([{ id: 'variant', value: 'thinking' }])).label).toBe('Unknown');
    // Other option ids with no match have no label; without a selection the descriptor's value stands.
    const effort = { id: 'effort', label: 'Effort', type: 'select', options: [{ id: 'low', label: 'Low' }] };
    expect(traitsDisplay('opencode', [effort], [], selection, openCode([{ id: 'effort', value: 'xhigh' }])).label).toBe('');
    expect(traitsDisplay('opencode', [{ ...variant, currentValue: 'fast' }], []).label).toBe('Fast');
  });
  test('the menu checks the reported radio and the subagent effort reads it too', () => {
    const reported = openCode([{ id: 'variant', value: 'thinking' }]);
    const column = (report: typeof reported | null) => effortColumn({ driver: 'opencode', descriptors: [variant], selections: [], selection: openCode(), reported: report,
      ultra: { primaryId: '', controlled: false, inBody: false }, unavailable: false, labels: {} }).rows;
    expect(column(reported).filter(row => row.selected).map(row => row.value)).toEqual(['thinking']);
    expect(column(null).some(row => row.selected)).toBe(false);
    const models = [{ slug: 'openai/gpt-5', name: 'GPT-5', capabilities: { optionDescriptors: [variant] } }];
    expect(selectionEffort(openCode(), models, reported)).toBe('Thinking');
    expect(selectionEffort(openCode(), models, null)).toBe('Unknown');
  });
  test('the composer trait label follows the open thread\'s report', async () => {
    const { client } = await opened();
    const model = obj((client.config.providers as Array<Record<string, unknown>>).find(entry => entry.instanceId === 'codex')!);
    (model.models as Array<Record<string, unknown>>)[0]!.capabilities = { optionDescriptors: [variant] };
    const projection = client.thread!.projection;
    Object.assign(obj(projection.thread), { activeProviderThreadId: 'pt' });
    projection.providerThreads = [{ id: 'pt', providerInstanceId: 'codex', nativeMetadata: { modelSelection: { instanceId: 'codex', model: 'model-a', options: [{ id: 'variant', value: 'thinking' }] } } }];
    expect(snapshot(client).composer.modelDetail).toBe('Thinking');
    projection.providerThreads = [];
    expect(snapshot(client).composer.modelDetail).toBe('Unknown');
  });
});

describe('background work (151c241cd2, a5b34b2537)', () => {
  test('subagent descriptions read as display names', () => {
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Subagent: Luna Window Properties' }])!.title).toBe('Waiting on subagent Luna Window Properties');
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Subagent:' }])!.title).toBe('Waiting on a subagent');
    const several = presentBackgroundWork([{ kind: 'subagent', description: 'Luna Window Properties' }, { kind: 'command', description: 'Review src/math.ts' },
      { kind: 'subagent', description: '/root/run_tests' }])!;
    expect(several).toMatchObject({ title: 'Waiting on 2 subagents and 1 command', description: 'Luna Window Properties, Run Tests, Review src/math.ts', waiting: true, segments: [] });
    expect(presentBackgroundWork([{ kind: 'command', description: '/root/run_tests' }])!.title).toBe('Running: /root/run_tests');
  });
  test('a subagent with its own thread is an inline link in the roster', () => {
    const roster = presentBackgroundWork([{ kind: 'command', description: 'npm test' }, { kind: 'subagent', description: 'Subagent: Review', childThreadId: 'child-1' }])!;
    expect(roster.segments).toEqual([{ key: '0:Review', label: 'Review', threadId: 'child-1', aria: 'Open subagent Review', last: false },
      { key: '1:npm test', label: 'npm test', threadId: '', aria: 'Open subagent npm test', last: true }]);
    // The title already names a lone subagent, so its link reads "Show" (#16022).
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Review', childThreadId: 'c' }])).toMatchObject({ title: 'Waiting on subagent Review', description: 'Show',
      segments: [{ label: 'Show', aria: 'Show subagent Review', threadId: 'c', last: true }] });
  });
  test('commands and monitors left (a dev server, its watchdog) run rather than wait unless the agent awaits them', () => {
    expect(presentBackgroundWork([{ kind: 'command', description: 'Start the dev server' }])).toEqual({ title: 'Running: Start the dev server', description: '', waiting: false, segments: [],
      items: [{ taskId: '', kind: 'command', label: 'Start the dev server', childThreadId: '' }] });
    expect(presentBackgroundWork([{ kind: 'command' }])!.title).toBe('Running a command');
    expect(presentBackgroundWork([{ kind: 'command' }, { kind: 'command', description: 'b' }])).toMatchObject({ title: 'Running 2 commands', waiting: false });
    expect(presentBackgroundWork([{ kind: 'monitor' }])).toMatchObject({ title: 'Running a monitor', waiting: false });
    expect(presentBackgroundWork([{ kind: 'monitor', awaited: true }])).toMatchObject({ title: 'Waiting on a monitor', waiting: true });
    expect(presentBackgroundWork([{ kind: 'command', description: 'npm test', awaited: true }])).toMatchObject({ title: 'Waiting on command npm test', waiting: true });
    expect(presentBackgroundWork([{ kind: 'background_task' }])).toMatchObject({ title: 'Waiting on a background task', waiting: true });
  });
  test('the line at the end of the timeline names the newest activity, without dots (#25299)', async () => {
    const { client } = await opened();
    Object.assign(client.shell.threads[0]!, { pendingBackgroundTasks: [{ taskId: 'a', kind: 'command', description: 'npm run dev' }] });
    client.projection.turnItems = [{ id: 'a', type: 'command', startedAt: '2026-10-28T00:00:00.000Z' }];
    expect(snapshot(client).composer.notices).toEqual([]);
    // One command: "Running" then its name in the code font, its elapsed time, then Stop.
    expect(threadNowRows(client)[0]).toMatchObject({ kind: 'thread-now', title: 'Running', body: 'npm run dev', meta: true, icon: 'terminal', detail: '',
      startedMs: Date.parse('2026-10-28T00:00:00.000Z') });
    expect(threadNowRows(client)[0]!.activities).toEqual([expect.objectContaining({ status: 'action', label: 'Stop', detail: 'cc:stop-background', ok: true })]);
    // One subagent: its newest progress on one line, then Open.
    Object.assign(client.shell.threads[0]!, { pendingBackgroundTasks: [{ taskId: 'n1', kind: 'subagent', description: 'Subagent: Review', childThreadId: 'c1' }] });
    client.projection.turnItems = [{ id: 'i1', nativeItemRef: { nativeId: 'n1' }, type: 'subagent', progress: 'Reading\n  src/app.ts', updatedAt: '2026-10-28T00:00:05.000Z' }];
    expect(threadNowRows(client)[0]).toMatchObject({ title: 'Waiting on', body: 'Review', meta: false, icon: 'circle-dashed', detail: 'Reading src/app.ts', startedMs: 0 });
    expect(threadNowRows(client)[0]!.activities!.map(entry => [entry.label, entry.detail, entry.targetId, entry.tone])).toEqual([['Open', 'select-thread', 'c1', 'muted'], ['Stop', 'cc:stop-background', '', '']]);
    // Several: the count, then the newest subagent progress, else the names (in the code font when all are commands).
    Object.assign(client.shell.threads[0]!, { pendingBackgroundTasks: [{ taskId: 'n1', kind: 'subagent', description: 'Review' }, { taskId: 'n2', kind: 'subagent', description: 'Lint' }] });
    client.projection.turnItems = [{ id: 'n1', type: 'subagent', progress: 'old', updatedAt: '2026-10-28T00:00:01.000Z' }, { id: 'n2', type: 'subagent', progress: 'new', updatedAt: '2026-10-28T00:00:09.000Z' }];
    expect(threadNowRows(client)[0]).toMatchObject({ title: 'Waiting on 2 subagents', body: '', detail: 'Lint: new' });
    Object.assign(client.shell.threads[0]!, { pendingBackgroundTasks: [{ taskId: 'x', kind: 'command', description: 'a' }, { taskId: 'y', kind: 'command', description: 'b' }] });
    expect(threadNowRows(client)[0]).toMatchObject({ title: 'Running 2 commands', detail: 'a, b', meta: true, icon: 'terminal' });
  });
});

describe('latest executed run (5108c978b1)', () => {
  test('the run that ended last wins over a higher ordinal; an unfinished run is the latest', () => {
    const runs = [{ id: 'resumed', ordinal: 2, status: 'interrupted', startedAt: 'x', completedAt: '2026-10-03T02:00:00.000Z' },
      { id: 'ended', ordinal: 3, status: 'completed', startedAt: 'x', completedAt: '2026-10-03T01:00:00.000Z' },
      { id: 'queued', ordinal: 4, status: 'queued' }];
    expect(latestExecutedRun({ runs })?.id).toBe('resumed');
    expect(resumeState({ thread: {}, runs }).runId).toBe('resumed');
    expect(latestExecutedRun({ runs: [...runs, { id: 'live', ordinal: 1, status: 'running', startedAt: 'x', completedAt: null }] })?.id).toBe('live');
    const tie = [{ id: 'a', ordinal: 1, status: 'completed', completedAt: '2026-10-03T01:00:00.000Z' }, { id: 'b', ordinal: 2, status: 'completed', completedAt: '2026-10-03T01:00:00.000Z' }];
    expect(latestExecutedRun({ runs: tie })?.id).toBe('b');
  });
});

describe('composer tooltips', () => {
  test('the model tooltip carries the picker shortcut as glyphs', async () => {
    const { chordGlyphs } = await import('../../src/composer/composer-controls-model');
    expect(chordGlyphs('Meta+Shift+M')).toBe('⇧⌘M');
    expect(chordGlyphs('Control+Alt+Enter')).toBe('⌃⌥Enter');
    expect(chordGlyphs('')).toBe('');
    const { client } = await opened();
    expect(snapshot(client).composer.modelTip).toBe('Model A · ⇧⌘M');
    client.config.keybindings = [{ command: 'modelPicker.toggle', shortcut: { key: 'k', modKey: true, altKey: true } }];
    expect(snapshot(client).composer.modelTip).toBe('Model A · ⌥⌘K');
  });
});

describe('the model picker announces a shortcut\'s pick (ModelPickerContent onWindowKeyDown)', () => {
  test('⇧⌘↑/⇧⌘↓ say the tab, ⌘1–9 "Model: <name>", one message for the picker at a time', async () => {
    const { client, command } = await opened();
    // The focus settles for a tick, then the next one speaks, alternating between the pair of regions.
    const spoken = (at: number) => { announcements(client, at); const view = announcements(client, at + 500); return view.first || view.second; };
    await command('cclocal:picker-provider-key', 'favorites');
    expect(spoken(1_000)).toBe('Favorites');
    await command('cclocal:picker-provider-key', 'claude');
    await command('cclocal:picker-model-key', 'model-b', 'claude');
    expect(spoken(3_000)).toBe('Model: Model B');
    await command('cclocal:picker-model-key', 'gone-model', 'claude');
    expect(spoken(5_000)).toBe('Model: gone-model');
  });
});
