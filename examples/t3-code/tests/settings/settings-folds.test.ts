// Settings pages grouped as the reference groups them at 7c3b3b3605ef: the Advanced fold (AdvancedSettings.tsx), the
// search results that open it, the defaults a fresh store starts with, and the rows drawn from the contracts.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { changedDeviceLabels, decodeClientPrefs } from '../../src/settings/settings-core';
import { searchTarget, settingsNavigation, FOLDED_TARGETS } from '../../src/settings/settings-search';
import { CATALOG } from '../../src/settings/settings-catalog';
import { followUpBehavior } from '../../src/composer/composer-controls';
import { contractComponent } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';
import { commandLabel } from '../../src/settings/keybinding-view';
import { sourceControlRows } from '../../src/settings/source-control-view';

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

  test("a result under a fold names the fold's reveal stop before its own row", () => {
    expect(searchTarget(item('skills-in-slash-menu'))).toBe('settings-advanced-composer-reveal setting-skills-in-slash-menu');
    expect(searchTarget(item('response-streaming'))).toBe('settings-advanced-text-generation-reveal setting-response-streaming');
    expect(searchTarget(item('worktree-submodules'))).toBe('settings-advanced-worktree-creation-reveal setting-worktree-submodules');
    expect(searchTarget(item('storage-worktree-keep-when'))).toBe('settings-advanced-storage-worktrees-reveal storage-worktree-keep-when');
    expect(searchTarget(item('browser-default-zoom'))).toBe('settings-advanced-browser-reveal browser-default-zoom');
    expect(searchTarget(item('source-control-writer-model'))).toBe('settings-advanced-source-control-text-generation-reveal setting-source-control-writer-model');
    // A row outside any fold keeps its own target.
    expect(searchTarget(item('send-shortcut'))).toBe('setting-send-shortcut');
    expect(settingsNavigation('show skills', context)).toMatchObject({ firstRoute: 'chat', firstTarget: 'settings-advanced-composer-reveal setting-skills-in-slash-menu' });
    // Every folded result is a catalog row (a renamed id would silently lose its fold).
    for (const id of Object.keys(FOLDED_TARGETS).filter(id => id !== 'thread-title-instructions')) expect([id, CATALOG.some(entry => entry[0] === id)]).toEqual([id, true]);
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
    expect(appText('settings-scheduled.contract')).toContain('SettingsModelTrigger(rowId="task-model"');
  });

  test('the terminal preview grows to its 12 rows, and the Mac mini is the reference glyph', () => {
    const rows = appText('settings-rows.contract');
    expect(rows).toContain('column width="100%" height=(12 * size * 1.38 + 4)');
    expect(rows).toContain(' watching for changes, press ');
    const kit = appText('settings-b-kit.contract');
    expect(kit).toContain('path d="M5.5 6h13A2.5 2.5 0 0 1 21 8.5v7a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 15.5v-7A2.5 2.5 0 0 1 5.5 6z"');
    expect(kit).toContain('circle cx=7.5 cy=12 r=1 fill="currentColor" stroke="none"');
  });

  test("page controls carry the focus-stop word the Settings guard reads", () => {
    for (const [file, marker] of [['settings-kit.contract', 'id=switchId testId=switchId'], ['settings-scoped-switch.contract', 'role="switch"'], ['settings-advanced.contract', 'press=toggle']]) {
      const line = appText(file).split('\n').find(text => text.includes(marker))!;
      expect([file, line.includes('data-focus-stop=')]).toEqual([file, true]);
    }
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
