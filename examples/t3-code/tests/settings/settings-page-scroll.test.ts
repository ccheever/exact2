// Each Settings page owns its scroller and search offset. The jump's actions run here with Contract's action semantics
// (contract-action-fixture.ts); the scrolling itself is driven on macOS. Reference 1e2ecbd975: every page renders its
// own scroller (SettingsPageContainer, settingsLayout.tsx `data-settings-page-scroll`) inside an Outlet keyed on the
// scope search and the restore count (routes/settings.tsx SettingsContentLayout), so a new page mounts a scroller at
// offset 0; the search jump then scrolls its target into view (scrollToSettingsTarget).
import { describe, expect, test } from 'bun:test';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { settingsNavigation } from '../../src/settings/settings-search';
import { appText } from '../support/app-files';


const source = async (file: string) => appText(file);
const lines = (text: string, needle: string) => text.split('\n').filter(candidate => candidate.includes(needle));

describe('a Settings page owns its scroller', () => {
  test('the scroller is keyed on the page, so another route, scope or restore mounts a new one', async () => {
    const settings = (await source('app-settings.contract')).split('\n');
    const at = settings.findIndex(candidate => candidate.includes('id="settings-content-scroll"'));
    expect(at).toBeGreaterThan(0);
    expect(settings[at - 1]!.trim()).toBe('for page in [settingsPage] key=page');
    expect(settings[at]).toContain('scrollTop=settingsScrollTop');
    const root = await source('app.contract');
    expect(root).toContain('  derive settingsPage = `${settingsRoute}|${settingsMachine}|${settingsProjectKey}|${settingsCheckout}|${settingsRestores}`\n');
    // A confirmed "Restore defaults" counts (onRestored → setRestoreSignal).
    expect(root).toContain('settingsRestores = settingsRestores + (op == "settings-core" and id == `restore-device-defaults:|${settingsCore.scopeKey}` ? 1 : 0)');
  });

  test('a jump from another page lands on its row: the new page waits for its data, then the jump\'s offset wins over the reset', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const text = appText('app.contract');
    const derive = (name: string) => text.match(new RegExp(`^  derive ${name} = (.+)$`, 'm'))![1]!;
    const focused: string[] = [];
    const frames: Record<string, { x: number; y: number; width: number; height: number }> = {
      'time-format': { x: 0, y: 900, width: 600, height: 40 }, 'settings-content-column': { x: 0, y: 100, width: 600, height: 2000 },
      'settings-content-scroll': { x: 0, y: 60, width: 600, height: 600 },
    };
    const state: Env = { settingsOpen: true, settingsRoute: 'appearance', settingsMachine: '', settingsProjectKey: '', settingsCheckout: '', settingsRestores: 0,
      settingsFocusTarget: '', settingsJumpOpened: '', settingsJumpWaits: 0, settingsScrollPage: '', settingsScrollTop: 0, settingsQuery: '', settingsMenu: '', modalError: '', providerLinkTarget: '' };
    const available = { available: true };
    // settingsCore answers for one jump target: its fold's reveal stop and the scope notice, if any.
    let core = { ready: true, route: 'appearance', sections: [{}], jumpTarget: '', jumpStop: '', notice: '' };
    let reveals: string[] = [];
    // The root's scope: its derives over the current state, its resources and the host's frame and focus.
    const scope = (): Env => {
      const env: Env = { ...state, settingsCore: core, storageSettings: available, archives: available, diagnostics: available, bindings: available, scheduled: available,
        sourceControl: available, integrations: available, providerPage: available, projectsSettings: { ready: true }, max: Math.max, floor: Math.floor, frame: (id: string) => frames[id] ?? { x: 0, y: 0, width: 0, height: 0 },
        focus: (id: string) => focused.push(id), querySelectorAll: () => reveals };
      for (const name of ['settingsPage', 'settingsPageScrollTop', 'settingsJumpReady']) env[name] = value(derive(name), env);
      return env;
    };
    const run = (action: string, args: unknown[] = []) => root.call(action, state, scope(), args);
    // From Appearance, the search picks General's "Time format": the route changes and General's settings are not in yet.
    run('corePick', ['general', 'time-format', '', '']);
    expect(scope().settingsJumpReady).toBe(false);
    run('focusSettingsTarget');
    expect(state.settingsFocusTarget).toBe('time-format');
    run('settingsPageForget'); // the page-left task (after 1 ms)
    expect(scope().settingsPageScrollTop).toBe(0); // the new page's scroller mounts at its top
    // General's settings arrive: the jump is ready, and its task (after 1 ms, once the page is laid out) scrolls to the row.
    core = { ...core, route: 'general' };
    expect(scope().settingsJumpReady).toBe(false); // the answer is still the last target's
    core = { ...core, jumpTarget: 'time-format' };
    expect(scope().settingsJumpReady).toBe(true);
    run('focusSettingsTarget');
    expect(scope().settingsPageScrollTop).toBe(900 - 100 + 20 - 300 + 0.25); // the row's centre at the scroller's
    expect(focused).toEqual(['time-format']);
    expect(state.settingsFocusTarget).toBe('');
    // The page-left task firing after the jump keeps it: the offset is this page's.
    run('settingsPageForget');
    expect(scope().settingsPageScrollTop).toBe(520.25);
    // Leaving for another page forgets it, so General opened later mounts at its top.
    run('coreNavigate', ['appearance']);
    expect(scope().settingsPageScrollTop).toBe(0);
    run('settingsPageForget');
    expect(state.settingsScrollPage).toBe('');
    // Every page split from General (9db433f4ba) waits for its own sections the same way, Chat's here.
    run('corePick', ['chat', 'send-shortcut', '', '']);
    expect(scope().settingsJumpReady).toBe(false);
    core = { ...core, route: 'chat', jumpTarget: 'send-shortcut' };
    expect(scope().settingsJumpReady).toBe(true);
    // A scope notice stands in for the page (SettingsScopeBoundary): the jump waits for a scope that has the row.
    core = { ...core, notice: 'Send shortcut is not available for the selected target.' };
    expect(scope().settingsJumpReady).toBe(false);
    core = { ...core, notice: '' };
    // A row under a closed Advanced fold is not drawn yet. The first pass focuses the fold's reveal stop, which opens it,
    // and keeps the target; the passes after it (every 50 ms, settingsJumpFold) wait for the open fold to draw the row,
    // then scroll to the row and focus it, as AdvancedSettings opens the fold before the row mounts
    // (AdvancedSettings.tsx:23). Here the fold ends a short page, so the stop sits below the window's edge and the row
    // lower still.
    run('corePick', ['chat', 'skills-in-slash-menu', '', '']);
    core = { ...core, jumpTarget: 'skills-in-slash-menu', jumpStop: 'settings-advanced-composer-reveal' };
    reveals = ['settings-advanced-composer-reveal'];
    frames['settings-advanced-composer-reveal'] = { x: 0, y: 1200, width: 1, height: 1 };
    focused.length = 0;
    const scrolled = state.settingsScrollTop;
    run('focusSettingsTarget');
    expect(focused).toEqual(['settings-advanced-composer-reveal']);
    expect(state.settingsFocusTarget).toBe('skills-in-slash-menu');
    expect(state.settingsScrollTop).toBe(scrolled); // nothing scrolls to the stop
    expect(state.settingsJumpOpened).toBe(`skills-in-slash-menu|${scope().settingsPage}`);
    run('focusSettingsTarget'); // the fold has not laid its rows out yet: the pass waits
    expect([focused.length, state.settingsFocusTarget, state.settingsJumpWaits]).toEqual([1, 'skills-in-slash-menu', 1]);
    frames['skills-in-slash-menu'] = { x: 0, y: 1260, width: 600, height: 40 }; // the open fold's row
    run('focusSettingsTarget');
    expect(focused).toEqual(['settings-advanced-composer-reveal', 'skills-in-slash-menu']);
    expect(Math.floor(scope().settingsPageScrollTop as number)).toBe(1260 - 100 + 20 - 300); // the row's centre, not the stop's
    expect([state.settingsFocusTarget, state.settingsJumpOpened]).toEqual(['', '']);
    // The fold already open: the stop's focus (opening again is idempotent), then the row on the next pass.
    run('corePick', ['chat', 'skills-in-slash-menu', '', '']);
    focused.length = 0;
    run('focusSettingsTarget');
    run('focusSettingsTarget');
    expect(focused).toEqual(['settings-advanced-composer-reveal', 'skills-in-slash-menu']);
    // A fold that has not drawn the row after twenty passes (a second) keeps the focus on its stop, and the jump ends.
    run('corePick', ['chat', 'skills-in-slash-menu', '', '']);
    delete frames['skills-in-slash-menu'];
    focused.length = 0;
    for (let pass = 0; pass < 21; pass++) run('focusSettingsTarget');
    expect([focused, state.settingsFocusTarget]).toEqual([['settings-advanced-composer-reveal'], 'skills-in-slash-menu']);
    run('focusSettingsTarget');
    expect([focused, state.settingsFocusTarget]).toEqual([['settings-advanced-composer-reveal', 'settings-advanced-composer-reveal'], '']);
    // A fold this page does not draw leaves the target as it is.
    run('corePick', ['chat', 'skills-in-slash-menu', '', '']);
    frames['skills-in-slash-menu'] = { x: 0, y: 1260, width: 600, height: 40 };
    reveals = [];
    focused.length = 0;
    run('focusSettingsTarget');
    expect(focused).toEqual(['skills-in-slash-menu']);
  });

  test('a jump left before it lands opens its fold again next time; Providers waits for the open page, and every jump for its row', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const text = appText('app.contract');
    const derive = (name: string) => text.match(new RegExp(`^  derive ${name} = (.+)$`, 'm'))![1]!;
    const focused: string[] = [];
    const box = (y: number, height: number) => ({ x: 0, y, width: 600, height });
    const frames: Record<string, ReturnType<typeof box>> = { 'settings-content-column': box(100, 2000), 'settings-content-scroll': { x: 0, y: 60, width: 600, height: 600 },
      'settings-advanced-composer-reveal': box(1200, 1) };
    const state: Env = { settingsOpen: true, settingsRoute: 'general', settingsMachine: '', settingsProjectKey: '', settingsCheckout: '', settingsRestores: 0,
      settingsFocusTarget: '', settingsJumpOpened: '', settingsJumpWaits: 0, settingsScrollPage: '', settingsScrollTop: 0, settingsQuery: '', settingsMenu: '', modalError: '', providerLinkTarget: '' };
    const available = { available: true };
    let core = { ready: true, route: 'chat', sections: [{}], jumpTarget: 'skills-in-slash-menu', jumpStop: 'settings-advanced-composer-reveal', notice: '' };
    let providerPage = { available: true, environment: '' };
    const scope = (): Env => {
      const env: Env = { ...state, settingsCore: core, storageSettings: available, archives: available, diagnostics: available, bindings: available, scheduled: available,
        sourceControl: available, integrations: available, providerPage, projectsSettings: { ready: true }, max: Math.max, floor: Math.floor,
        frame: (id: string) => frames[id] ?? box(0, 0), focus: (id: string) => focused.push(id), querySelectorAll: () => ['settings-advanced-composer-reveal'] };
      for (const name of ['settingsPage', 'settingsPageScrollTop', 'settingsJumpReady']) env[name] = value(derive(name), env);
      return env;
    };
    const run = (action: string, args: unknown[] = []) => root.call(action, state, scope(), args);
    // The first pass opens the fold; leaving the page before the row is drawn starts the next jump to it over, so it
    // opens the fold of the page drawn then (closed again) instead of waiting for a row that never comes.
    run('corePick', ['chat', 'skills-in-slash-menu', '', '']);
    run('focusSettingsTarget');
    expect([focused, state.settingsJumpOpened]).toEqual([['settings-advanced-composer-reveal'], `skills-in-slash-menu|${scope().settingsPage}`]);
    run('coreNavigate', ['general']);
    expect(state.settingsJumpOpened).toBe('');
    run('corePick', ['chat', 'skills-in-slash-menu', '', '']);
    run('focusSettingsTarget');
    expect(focused).toEqual(['settings-advanced-composer-reveal', 'settings-advanced-composer-reveal']);
    // Every write of the jump's target, its landing too, clears the pass it had reached: a new query, Settings opened
    // again, a page, a result picked by press or Return, a toast's or the palette's link to a row.
    for (const file of ['settings-members.contract', 'shell-members.contract', 'palette-members.contract']) {
      const body = appText(file).split('\n');
      const writes = body.flatMap((line, at) => /^\s+settingsFocusTarget = /.test(line) ? [[line.trim(), body[at + 1]!.trim().startsWith('settingsJumpOpened = ""')]] : []);
      expect(writes.length).toBeGreaterThan(0);
      expect(writes.filter(([, clears]) => !clears)).toEqual([]);
    }
    // Providers' answer while the page was closed is available too, with no environment and no Advanced rows: the
    // jump waits for the open page's.
    run('corePick', ['providers', 'compact-idle-threads', '', '']);
    core = { ...core, route: 'providers', jumpTarget: 'compact-idle-threads', jumpStop: '' };
    expect(scope().settingsJumpReady).toBe(false);
    providerPage = { available: true, environment: 'env1' };
    expect(scope().settingsJumpReady).toBe(true);
    // A row not drawn yet is waited for, every 50 ms (settingsJumpFold), then scrolled to and focused.
    focused.length = 0;
    run('focusSettingsTarget');
    run('focusSettingsTarget');
    expect([focused, state.settingsFocusTarget, state.settingsJumpWaits]).toEqual([[], 'compact-idle-threads', 1]);
    frames['compact-idle-threads'] = box(1500, 60);
    run('focusSettingsTarget');
    expect([focused, state.settingsFocusTarget, state.settingsJumpOpened]).toEqual([['compact-idle-threads'], '', '']);
    expect(Math.floor(state.settingsScrollTop as number)).toBe(1500 - 100 + 30 - 300);
    // One never drawn is landed on as it is after a second.
    delete frames['compact-idle-threads'];
    run('corePick', ['providers', 'compact-idle-threads', '', '']);
    focused.length = 0;
    for (let pass = 0; pass < 21; pass++) run('focusSettingsTarget');
    expect([focused, state.settingsFocusTarget]).toEqual([[], 'compact-idle-threads']);
    run('focusSettingsTarget');
    expect([focused, state.settingsFocusTarget]).toEqual([['compact-idle-threads'], '']);
  });

  // Search, Return, then Escape before the jump landed: Settings closes with the target kept. Manage scheduled tasks
  // opens it again without openSettings, and must not land that jump on the page it opens.
  test('Settings opened by Manage scheduled tasks drops a jump left open, as every way into Settings does', () => {
    const root = contractComponent('app.contract', 'T3Code');
    const text = appText('app.contract');
    const derive = (name: string) => text.match(new RegExp(`^  derive ${name} = (.+)$`, 'm'))![1]!;
    const focused: string[] = [];
    const box = (y: number, height: number) => ({ x: 0, y, width: 600, height });
    const frames: Record<string, ReturnType<typeof box>> = { 'settings-content-column': box(100, 2000), 'settings-content-scroll': { x: 0, y: 60, width: 600, height: 600 } };
    const state: Env = { settingsOpen: false, settingsRoute: 'providers', settingsMachine: '', settingsProjectKey: '', settingsCheckout: '', settingsRestores: 0,
      settingsFocusTarget: 'compact-idle-threads', settingsJumpOpened: '', settingsJumpWaits: 0, settingsScrollPage: '', settingsScrollTop: 40, settingsQuery: '', settingsMenu: '',
      modalError: '', providerLinkTarget: '', settingsEnvironmentId: '', settingsProjectId: '', restEditor: '', restSubject: '', taskModelPick: '', detailsOpen: false, detailsEpoch: 0, commandPending: true };
    const available = { available: true };
    const scope = (): Env => {
      const core = { ready: true, route: String(state.settingsRoute), sections: [{}], jumpTarget: String(state.settingsFocusTarget), jumpStop: '', notice: '' };
      const env: Env = { ...state, settingsCore: core, storageSettings: available, archives: available, diagnostics: available, bindings: available, scheduled: available,
        sourceControl: available, integrations: available, providerPage: { available: true, environment: 'env1' }, projectsSettings: { ready: true }, max: Math.max, floor: Math.floor,
        frame: (id: string) => frames[id] ?? box(0, 0), focus: (id: string) => focused.push(id), querySelectorAll: () => [], settings: { environmentId: 'env1' },
        detailsShown: false, detailsPopover: false, data: { projectId: '' }, canvas: { popoverEpoch: 0 } };
      for (const name of ['settingsPage', 'settingsPageScrollTop', 'settingsJumpReady']) env[name] = value(derive(name), env);
      return env;
    };
    root.call('detailsAct', state, scope(), ['ui:scheduled-tasks', '', '']);
    expect([state.settingsOpen, state.settingsRoute, state.settingsFocusTarget, state.settingsJumpOpened]).toEqual([true, 'scheduled-tasks', '', '']);
    for (let pass = 0; pass < 22; pass++) root.call('focusSettingsTarget', state, scope(), []);
    expect([focused, state.settingsScrollTop]).toEqual([[], 40]);
    // Every write that opens Settings sits with openSettings() or a jump target of its own.
    const body = text.split('\n');
    const opens = body.flatMap((line, at) => {
      if (!/^\s+settingsOpen = true/.test(line)) return [];
      const indent = line.length - line.trimStart().length, inBlock = (entry: string) => entry.trim() === '' || entry.length - entry.trimStart().length >= indent;
      let from = at, to = at;
      while (from > 0 && inBlock(body[from - 1]!)) from--;
      while (to + 1 < body.length && inBlock(body[to + 1]!)) to++;
      return [body.slice(from, to + 1).some(entry => entry.trim().startsWith('settingsFocusTarget = ') || entry.trim().startsWith('openSettings()'))];
    });
    expect(opens.length).toBeGreaterThan(2);
    expect(opens.filter(clears => !clears)).toEqual([]);
  });

  test("a section's result centres the section's header, as scrollAndFocusSettingsTarget does; a row's centres the row", () => {
    const root = contractComponent('app.contract', 'T3Code');
    const text = appText('app.contract');
    const derive = (name: string) => text.match(new RegExp(`^  derive ${name} = (.+)$`, 'm'))![1]!;
    const focused: string[] = [];
    const box = (y: number, height: number) => ({ x: 0, y, width: 600, height });
    const frames: Record<string, { x: number; y: number; width: number; height: number }> = {
      'settings-content-column': box(100, 4000), 'settings-content-scroll': { x: 0, y: 60, width: 600, height: 600 },
      // Keybindings' and Providers' first sections top their pages, so their results leave the scroller at its top.
      keybindings: box(100, 120), 'keybindings-scroll-target': box(100, 28), providers: box(100, 900), 'providers-scroll-target': box(100, 28),
      // Browsing sits lower on its page: its header, not its middle, comes to the scroller's centre.
      browser: box(900, 700), 'browser-scroll-target': box(900, 28),
      'send-shortcut': box(900, 60),
    };
    const state: Env = { settingsOpen: true, settingsRoute: 'general', settingsMachine: '', settingsProjectKey: '', settingsCheckout: '', settingsRestores: 0,
      settingsFocusTarget: '', settingsJumpOpened: '', settingsJumpWaits: 0, settingsScrollPage: '', settingsScrollTop: 0, settingsQuery: '', settingsMenu: '', modalError: '', providerLinkTarget: '', keybindingQuery: '' };
    const available = { available: true };
    let core = { ready: true, route: 'general', sections: [{}], jumpTarget: '', jumpStop: '', notice: '' };
    const scope = (): Env => {
      const env: Env = { ...state, settingsCore: core, storageSettings: available, archives: available, diagnostics: available, bindings: available, scheduled: available,
        sourceControl: available, integrations: available, providerPage: available, projectsSettings: { ready: true }, max: Math.max, floor: Math.floor,
        frame: (id: string) => frames[id] ?? box(0, 0), focus: (id: string) => focused.push(id), querySelectorAll: () => [] };
      for (const name of ['settingsPage', 'settingsPageScrollTop', 'settingsJumpReady']) env[name] = value(derive(name), env);
      return env;
    };
    const jump = (route: string, target: string) => {
      root.call('corePick', state, scope(), [route, target, '', '']);
      core = { ...core, route, jumpTarget: target };
      root.call('focusSettingsTarget', state, scope(), []);
      return [Math.floor(state.settingsScrollTop as number), focused.at(-1)];
    };
    expect(jump('keybindings', 'keybindings')).toEqual([0, 'keybindings']); // the filter stays in view
    expect(jump('providers', 'providers')).toEqual([0, 'providers']); // the top of the list stays in view
    expect(jump('browser', 'browser')).toEqual([900 - 100 + 28 / 2 - 300, 'browser']);
    expect(jump('chat', 'send-shortcut')).toEqual([900 - 100 + 60 / 2 - 300, 'send-shortcut']);
  });

  test("Keybindings' and Providers' results name their first section, and every section's header is its scroll target", () => {
    const keys = appText('settings-keybindings.contract'), providers = appText('providers.contract');
    // KeybindingsSettings.tsx and ProviderSettingsPanel.tsx spread searchableSetting(…) on the first SettingsSection.
    expect(keys).toContain('column width="100%" gap="2rem" testId="keybindings-settings"\n');
    expect(keys).toMatch(/column width="100%" gap="0\.625rem" id="keybindings" tabindex=-1\n\s+row [^\n]* id="keybindings-scroll-target"\n\s+text "Keybindings"/);
    expect(providers).toContain('column width="100%" gap="2rem" testId="providers-settings"\n');
    expect(providers).toMatch(/column width="100%" gap="0\.625rem" id="providers" tabindex=-1\n(\s+\/\/[^\n]*\n)+\s+row [^\n]* id="providers-scroll-target"\n/);
    expect(appText('settings-kit.contract')).toMatch(/id=sectionId tabindex=-1 testId=sectionId\n\s+row [^\n]* id=`\$\{sectionId\}-scroll-target`\n/);
    expect(appText('settings-rows.contract').match(/id=`\$\{section\.id\}-scroll-target`/g)).toHaveLength(1); // no result names Legacy features itself
    expect(appText('settings-members.contract')).toContain('let anchor = frame(`${target}-scroll-target`).height > 0 ? `${target}-scroll-target` : target');
  });

  test('a jump to a legacy row opens Legacy features first, by Return as by a press on the result', () => {
    const window = contractComponent('app-settings.contract', 'SettingsWindow');
    const results = (query: string) => settingsNavigation(query, { connected: true, autoSettle: true, scopeKind: 'environment' });
    for (const [via, args] of [['Enter', ['Enter']], ['press', []]] as const) {
      const state: Env = { settingsEscapeHeld: false, settingsLegacyOpen: false, modelAdding: '' };
      const nav = results('legacy');
      expect(nav.firstId.startsWith('search-result-legacy-')).toBe(true);
      const scope: Env = { settingsQuery: 'legacy', settingsNavigation: nav, coreSearchKeyRoot: () => {}, corePickRoot: () => {} };
      if (via === 'Enter') window.call('coreSearchKey', state, scope, [...args]);
      else window.call('corePick', state, scope, [nav.firstRoute, nav.firstTarget, '', nav.firstId]);
      expect(state.settingsLegacyOpen).toBe(true);
    }
    // Another result leaves the section as it was.
    const state: Env = { settingsEscapeHeld: false, settingsLegacyOpen: false, modelAdding: '' };
    window.call('coreSearchKey', state, { settingsQuery: 'time format', settingsNavigation: results('time format'), coreSearchKeyRoot: () => {} }, ['Enter']);
    expect(state.settingsLegacyOpen).toBe(false);
  });

  test('the jump runs once its page is ready, not only on the one-second poll', async () => {
    const root = await source('app.contract');
    expect(root).toContain('  task settingsJump when settingsJumpReady key=`${settingsFocusTarget}|${settingsPage}`\n    after(1, focusSettingsTarget)\n');
    // A fold the jump opened, or a row not drawn yet, is waited for every 50 ms until the row is drawn.
    expect(root).toContain('  task settingsJumpFold when settingsJumpReady and settingsJumpOpened != "" key=settingsJumpOpened\n    every(50, focusSettingsTarget)\n');
    expect(lines(root, 'settingsScrollTop=settingsPageScrollTop, settingsPage=settingsPage')).toHaveLength(1);
    // Nothing but the jump and the page-left task writes the jump's page.
    expect(root.split('\n').filter(candidate => /^ {4,}settingsScrollPage = /.test(candidate))).toHaveLength(2);
  });
});
