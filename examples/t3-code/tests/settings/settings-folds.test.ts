// Settings pages grouped as the reference groups them at 7c3b3b3605ef: the Advanced fold (AdvancedSettings.tsx), the
// search results that open it, the defaults a fresh store starts with, and the rows drawn from the contracts.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { changedDeviceLabels, decodeClientPrefs } from '../../src/settings/settings-core';
import { foldStopOf, searchTarget, settingsCatalog, settingsNavigation, FOLDED_TARGETS } from '../../src/settings/settings-search';
import { CATALOG } from '../../src/settings/settings-catalog';
import { followUpBehavior } from '../../src/composer/composer-controls';
import { contractComponent } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';
import { commandLabel } from '../../src/settings/keybinding-view';
import { sourceControlRows } from '../../src/settings/source-control-view';
import { taskModelMarks } from '../../src/settings/scheduled-view';

const context = { connected: true, autoSettle: true, scopeKind: 'all', keybindings: [], localBackend: true, localEnvironment: true };
const item = (id: string) => { const row = CATALOG.find(entry => entry[0] === id)!; return { id, route: row[2], target: row[3] || id }; };

describe('Advanced fold', () => {
  test('opens closed, toggles, and opens for the jump that focuses its reveal stop', () => {
    const fold = contractComponent('settings-advanced.contract', 'AdvancedFold');
    const state = fold.state();
    expect(state.open).toBe(false);
    expect(fold.drawn('children', state, {})).toBe(false);
    fold.call('toggle', state, {}, []);
    expect([state.open, fold.drawn('children', state, {})]).toEqual([true, true]);
    fold.call('toggle', state, {}, []);
    fold.call('reveal', state, {}, []);
    fold.call('reveal', state, {}, []);
    expect(state.open).toBe(true);
    // The stop is the jump's alone: a press never focuses it, and Tab passes it by.
    expect(appText('settings-advanced.contract')).toMatch(/box id=`settings-advanced-\$\{foldId\}-reveal` tabindex=-1 focusin=reveal/);
  });

  test("a result names its row, and a row under a fold names the fold's reveal stop", () => {
    // Upstream's hash is `targetId ?? id`; AdvancedSettings opens for the `targetIds` inside it.
    const folded = (id: string) => [searchTarget(item(id)), foldStopOf(searchTarget(item(id)))];
    expect(folded('skills-in-slash-menu')).toEqual(['skills-in-slash-menu', 'settings-advanced-composer-reveal']);
    expect(folded('response-streaming')).toEqual(['response-streaming', 'settings-advanced-text-generation-reveal']);
    expect(folded('worktree-submodules')).toEqual(['worktree-submodules', 'settings-advanced-worktree-creation-reveal']);
    // Keep worktrees with local changes and Worktree cleanup land on the section, which opens its fold.
    expect(folded('storage-worktree-keep-when')).toEqual(['storage-worktrees', 'settings-advanced-storage-worktrees-reveal']);
    expect(folded('storage-worktrees')).toEqual(['storage-worktrees', 'settings-advanced-storage-worktrees-reveal']);
    expect(folded('browser-default-zoom')).toEqual(['browser-default-zoom', 'settings-advanced-browser-reveal']);
    expect(folded('source-control-writer-model')).toEqual(['source-control-writer-model', 'settings-advanced-source-control-text-generation-reveal']);
    expect(folded('panel-animations')).toEqual(['panel-animations', 'settings-advanced-appearance-interface-reveal']);
    // A row outside any fold has no stop; a host's settings open its row's details (DiscoveryItemRow).
    expect(folded('send-shortcut')).toEqual(['send-shortcut', '']);
    expect(folded('source-control-host-github')).toEqual(['source-control-host-github', 'source-control-host-github-reveal']);
    expect(settingsNavigation('show skills', context)).toMatchObject({ firstRoute: 'chat', firstTarget: 'skills-in-slash-menu' });
    // Every folded target is a catalog row's id or target (a renamed id would silently lose its fold).
    for (const id of Object.keys(FOLDED_TARGETS)) expect([id, CATALOG.some(entry => entry[0] === id || entry[3] === id)]).toEqual([id, true]);
  });

  test("the catalog is the search's and the palette's: every result with its target, keybinding commands by their row", () => {
    const items = settingsCatalog({ ...context, keybindings: [{ command: 'sidebar.toggle', key: 'mod+b' }, { command: 'sidebar.toggle', key: 'mod+shift+b' }, { command: 'thread.stop', key: 'mod+.' }] });
    expect(items.find(entry => entry.id === 'keep-threads-with-pull-requests')).toMatchObject({ route: 'threads', target: 'auto-settle-inactive-threads' });
    // A command with a default key lands on its row; one without, on the Keybindings section (KEYBINDING_SEARCH_ITEMS).
    expect(items.filter(entry => entry.secondary)).toEqual([
      expect.objectContaining({ id: 'keybinding-sidebar.toggle', route: 'keybindings', target: 'keybinding-sidebar.toggle', terms: ['sidebar.toggle', 'mod+b', 'mod+shift+b'] }),
      expect.objectContaining({ id: 'keybinding-thread.stop', route: 'keybindings', target: 'keybindings' })]);
    // Off the desktop's platform rows: no Windows or T3 Connect result.
    expect(items.some(entry => entry.id === 'wsl-backend' || entry.id === 't3-connect')).toBe(false);
  });

  test('a source control host opens its details for the jump that focuses its reveal stop', () => {
    const host = contractComponent('settings-source-control.contract', 'DiscoveryItem');
    const state = host.state();
    const scope = { item: { kind: 'github', label: 'GitHub', details: true }, credentials: [] };
    expect([state.open, host.drawn('id=`source-control-host-${item.kind}`', state, scope)]).toEqual([false, false]);
    expect(host.drawn('id=`source-control-host-${item.kind}-reveal`', state, scope)).toBe(true);
    host.call('reveal', state, scope, []);
    host.call('reveal', state, scope, []);
    expect([state.open, host.drawn('id=`source-control-host-${item.kind}`', state, scope)]).toEqual([true, true]);
    expect(host.drawn('id=`source-control-host-${item.kind}-reveal`', state, { ...scope, item: { ...scope.item, details: false } })).toBe(false);
  });
});

describe('defaults', () => {
  test('a fresh store steers follow-ups and shows the Working section, with nothing to reset', () => {
    const client = new T3Client();
    expect([client.local.clientSettings.followUpBehavior, client.local.clientSettings.sidebarWorkingShelfEnabled]).toEqual(['steer', true]);
    expect(followUpBehavior(client)).toBe('steer');
    expect(changedDeviceLabels(client.local as never)).not.toContain('followUpBehavior');
    expect(changedDeviceLabels(client.local as never)).not.toContain('sidebarWorkingShelfEnabled');
    expect(decodeClientPrefs({ followUpBehavior: 'queue', sidebarWorkingShelfEnabled: false })).toMatchObject({ followUpBehavior: 'queue', sidebarWorkingShelfEnabled: false });
  });
});

describe('page contracts', () => {
  test('Browser draws Agent access, Browsing with its tab defaults under Advanced, and Recordings', () => {
    const panel = contractComponent('settings-source-control.contract', 'IntegrationsPanel');
    const scope = { view: 'browser', error: '', data: { error: '', available: true } };
    expect(['SectionTitle(title="Agent access")', 'SectionTitle(title="Browsing")', 'SectionTitle(title="Recordings")', 'BrowserTabDefaults(view=', 'BrowserRecordings(view=']
      .map(marker => panel.drawn(marker, {}, scope))).toEqual([true, true, true, true, true]);
    expect(panel.drawn('SectionTitle(title="Agent access")', {}, { ...scope, data: { error: '', available: false } })).toBe(false);
    const source = appText('settings-source-control.contract');
    expect(source).not.toContain('PullRequestLinksRow'); // Open pull requests in T3 Code is Diffs' (settings-core.ts)
    expect(source.indexOf('AdvancedFold(foldId="browser"')).toBeLessThan(source.indexOf('BrowserTabDefaults(view='));
  });

  test("Writing & review closes Source control, its templates and writer model under Advanced, and Git has no fetch interval", () => {
    const source = appText('settings-source-control.contract');
    expect(source).toContain('SectionTitle(title="Writing & review")');
    expect(source).toContain('AdvancedFold(foldId="source-control-text-generation"');
    expect(source).not.toContain('Git fetch interval');
  });

  test('Settings pickers are bordered as the selects beside them, with regular text and a 70% chevron', () => {
    const rows = appText('settings-rows.contract');
    const picker = appText('settings-picker.contract');
    expect(picker).toMatch(/style SettingsPickerFace[\s\S]*border-width=1[\s\S]*box-shadow="0 1px 2px #0000000d"[\s\S]*:hover/);
    expect(picker).toMatch(/component SettingsModelTrigger[\s\S]*class=SettingsPickerFace[\s\S]*box opacity=0\.7/);
    expect(rows.match(/class=SettingsPickerFace/g)?.length).toBe(2); // the model and traits triggers
    expect(appText('settings-scheduled.contract')).toContain('SettingsModelTrigger(stop="enabled", rowId="task-model"');
  });

  test('the terminal preview grows to its 12 rows, and the Mac mini is the reference glyph', () => {
    const rows = appText('settings-rows.contract');
    expect(rows).toContain('column width="100%" height=(12 * size * 1.38 + 4)');
    expect(rows).toContain(' watching for changes, press ');
    const kit = appText('settings-b-kit.contract');
    expect(kit).toContain('path d="M5.5 6h13A2.5 2.5 0 0 1 21 8.5v7a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 15.5v-7A2.5 2.5 0 0 1 5.5 6z"');
    expect(kit).toContain('circle cx=7.5 cy=12 r=1 fill="currentColor" stroke="none"');
  });

});

describe('model triggers', () => {
  test("the writer's and a scheduled task's triggers name the model as the composer's does, without its driver's family", () => {
    // ProviderModelPicker's trigger title, getTriggerDisplayModelName(model, driverKind) (providerIconUtils.ts).
    const providers = [{ instanceId: 'claudeAgent', driver: 'claudeAgent', enabled: true, installed: true, status: 'ready', auth: { status: 'authenticated' },
      models: [{ slug: 'claude-opus-4-5', name: 'Claude Opus 4.5' }] }];
    const writer = sourceControlRows({ sourceControlWriterModelSelection: { instanceId: 'claudeAgent', model: 'claude-opus-4-5' } }, '', 'Mac', providers, true)
      .writingAdvanced.find(row => row.key === 'sourceControlWriterModelSelection')!;
    expect(writer.valueLabel).toBe('Opus 4.5');
    expect(taskModelMarks(providers, '').map(mark => mark.name)).toEqual(['Opus 4.5', 'Opus 4.5']);
  });
});

describe('copy', () => {
  test('no em dashes in this lane\'s sentences, and the queue command named as the reference names it', () => {
    expect(commandLabel('thread.steerQueuedMessage')).toBe('Queue: Send First Queued Message');
    expect(sourceControlRows({}, '', 'Local', [], true).repositories.find(row => row.key === 'removeAgentCreditsOnMerge')!.description)
      .toBe('Remove recognized agent credit lines from merge and squash messages on hosts that support it, keeping human co-authors. Includes auto-merge. Excludes merge queues, stack merges, and existing commits.');
    expect(appText('settings-scheduled.contract')).toContain('"Run a prompt automatically on an interval, at a fixed time, or when a webhook is called."');
    expect(appText('settings-a-telemetry.contract')).toMatch(/else\n\s+text "-" font-size="0\.625rem"/);
    expect(appText('providers.contract')).toContain('Custom intervals appear as Advanced in Maintenance settings.');
  });
});
