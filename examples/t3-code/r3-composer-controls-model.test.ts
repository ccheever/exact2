// Lane composer-controls, round 3: upstream 4f7760e6a0 (subagent model names),
// 63f74e209d (provider-reported option labels), 151c241cd2 + a5b34b2537
// (background-work wording) and 5108c978b1 (latest executed run).
import { describe, expect, test } from 'bun:test';
import { obj } from './domain';
import { formatModelSlugName, reportedModelLabel, reportedSelection, resolveSelectableModel, triggerModelName } from './r3-composer-controls-model';
import { presentBackgroundWork, traitsDisplay } from './composer-controls-view';
import { traitsMenu } from './composer-presentation';
import { latestExecutedRun, resumeState } from './composer-controls';
import { selectionEffort, subagentBar } from './composer-controls-subagent';
import { snapshot } from './presentation';
import { opened } from './composer-controls-fixture';

const claudeModels = [
  { slug: 'claude-haiku-4-5', name: 'Claude Haiku 4.5', shortName: 'Haiku 4.5', aliases: ['claude-haiku-4-5-20251001', 'haiku'] },
  { slug: 'claude-opus-4-6', name: 'Claude Opus 4.6' },
  { slug: 'anthropic/claude-sonnet', name: 'Anthropic: Claude Sonnet', subProvider: 'Anthropic' },
];
const variant = { id: 'variant', label: 'Reasoning', type: 'select', options: [{ id: 'thinking', label: 'Thinking' }, { id: 'fast', label: 'Fast' }] };
const openCode = (options: object[] = []) => ({ instanceId: 'opencode', model: 'openai/gpt-5', options });

describe('subagent model names (4f7760e6a0)', () => {
  test('a reported dated id or alias resolves to the catalog entry and its trigger name', () => {
    expect(resolveSelectableModel('claudeAgent', 'claude-haiku-4-5-20251001', claudeModels)).toBe('claude-haiku-4-5');
    expect(resolveSelectableModel('claudeAgent', 'CLAUDE OPUS 4.6', claudeModels)).toBe('claude-opus-4-6');
    expect(resolveSelectableModel('cursor', 'opus-4.6', claudeModels)).toBe('claude-opus-4-6');
    expect(resolveSelectableModel('claudeAgent', 'unknown', claudeModels)).toBe('');
    expect(reportedModelLabel('claudeAgent', 'claude-haiku-4-5-20251001', claudeModels)).toBe('Haiku 4.5');
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
    const menu = traitsMenu([variant], [], openCode(), reported);
    expect(menu.items.filter(item => item.selected).map(item => item.value)).toEqual(['thinking']);
    expect(traitsMenu([variant], [], openCode(), null).items.some(item => item.selected)).toBe(false);
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
    expect(snapshot(client).composer.traitsLabel).toBe('Thinking');
    projection.providerThreads = [];
    expect(snapshot(client).composer.traitsLabel).toBe('Unknown');
  });
});

describe('background work (151c241cd2, a5b34b2537)', () => {
  test('subagent descriptions read as display names', () => {
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Subagent: Luna Window Properties' }])!.title).toBe('Waiting on subagent Luna Window Properties');
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Subagent:' }])!.title).toBe('Waiting on a subagent');
    const several = presentBackgroundWork([{ kind: 'subagent', description: 'Luna Window Properties' }, { kind: 'command', description: 'Review src/math.ts' },
      { kind: 'subagent', description: '/root/run_tests' }])!;
    expect(several).toEqual({ title: 'Waiting on 2 subagents and 1 command', description: 'Luna Window Properties, Run Tests, Review src/math.ts', waiting: true, segments: [] });
    expect(presentBackgroundWork([{ kind: 'command', description: '/root/run_tests' }])!.title).toBe('Running: /root/run_tests');
  });
  test('a subagent with its own thread is an inline link in the roster', () => {
    const roster = presentBackgroundWork([{ kind: 'command', description: 'npm test' }, { kind: 'subagent', description: 'Subagent: Review', childThreadId: 'child-1' }])!;
    expect(roster.segments).toEqual([{ key: '0:Review', label: 'Review', threadId: 'child-1', last: false }, { key: '1:npm test', label: 'npm test', threadId: '', last: true }]);
    expect(presentBackgroundWork([{ kind: 'subagent', description: 'Review', childThreadId: 'c' }])).toMatchObject({ title: 'Waiting on subagent Review', description: 'Review',
      segments: [{ label: 'Review', threadId: 'c', last: true }] });
  });
  test('only commands left (a dev server) run rather than wait, and do not pulse', () => {
    expect(presentBackgroundWork([{ kind: 'command', description: 'Start the dev server' }])).toEqual({ title: 'Running: Start the dev server', description: '', waiting: false, segments: [] });
    expect(presentBackgroundWork([{ kind: 'command' }])!.title).toBe('Running a command');
    expect(presentBackgroundWork([{ kind: 'command' }, { kind: 'command', description: 'b' }])).toMatchObject({ title: 'Running 2 commands', waiting: false });
    expect(presentBackgroundWork([{ kind: 'monitor' }])).toMatchObject({ title: 'Waiting on a monitor', waiting: true });
  });
  test('the banner dot is static for commands and pulses otherwise', async () => {
    const { client } = await opened();
    Object.assign(client.shell.threads[0]!, { pendingBackgroundTasks: [{ taskId: 'a', kind: 'command', description: 'npm run dev' }] });
    expect(snapshot(client).composer.notices[0]).toMatchObject({ title: 'Running: npm run dev', icon: 'dot-static' });
    Object.assign(client.shell.threads[0]!, { pendingBackgroundTasks: [{ taskId: 'a', kind: 'subagent', description: 'Subagent: Review' }] });
    expect(snapshot(client).composer.notices[0]).toMatchObject({ title: 'Waiting on subagent Review', icon: 'dot' });
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
    const { chordGlyphs } = await import('./r3-composer-controls-model');
    expect(chordGlyphs('Meta+Shift+M')).toBe('⇧⌘M');
    expect(chordGlyphs('Control+Alt+Enter')).toBe('⌃⌥Enter');
    expect(chordGlyphs('')).toBe('');
    const { client } = await opened();
    expect(snapshot(client).composer.modelTip).toBe('Model A · ⇧⌘M');
    client.config.keybindings = [{ command: 'modelPicker.toggle', shortcut: { key: 'k', modKey: true, altKey: true } }];
    expect(snapshot(client).composer.modelTip).toBe('Model A · ⌥⌘K');
  });
});
