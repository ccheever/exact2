// Each Settings page owns its scroller and search offset. The jump's actions run here with Contract's action semantics
// (contract-action-fixture.ts); the scrolling itself is driven on macOS. Reference 1e2ecbd975: every page renders its
// own scroller (SettingsPageContainer, settingsLayout.tsx `data-settings-page-scroll`) inside an Outlet keyed on the
// scope search and the restore count (routes/settings.tsx SettingsContentLayout), so a new page mounts a scroller at
// offset 0; the search jump then scrolls its target into view (scrollToSettingsTarget).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractComponent, value, type Env } from '../support/contract-action-fixture';
import { settingsNavigation } from '../../src/settings/settings-search';
import { appPath } from '../support/app-files';

const source = (file: string) => Bun.file(appPath(file)).text();
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
    const text = readFileSync(appPath('app.contract'), 'utf8');
    const derive = (name: string) => text.match(new RegExp(`^  derive ${name} = (.+)$`, 'm'))![1]!;
    const focused: string[] = [];
    const frames: Record<string, { x: number; y: number; width: number; height: number }> = {
      'setting-time-format': { x: 0, y: 900, width: 600, height: 40 }, 'settings-content-column': { x: 0, y: 100, width: 600, height: 2000 },
      'settings-content-scroll': { x: 0, y: 60, width: 600, height: 600 },
    };
    const state: Env = { settingsOpen: true, settingsRoute: 'appearance', settingsMachine: '', settingsProjectKey: '', settingsCheckout: '', settingsRestores: 0,
      settingsFocusTarget: '', settingsScrollPage: '', settingsScrollTop: 0, settingsQuery: '', settingsMenu: '', modalError: '', providerLinkTarget: '' };
    const available = { available: true };
    let core = { ready: true, route: 'appearance', sections: [{}] };
    // The root's scope: its derives over the current state, its resources and the host's frame and focus.
    const scope = (): Env => {
      const env: Env = { ...state, settingsCore: core, storageSettings: available, archives: available, diagnostics: available, bindings: available, scheduled: available,
        sourceControl: available, integrations: available, max: Math.max, floor: Math.floor, frame: (id: string) => frames[id] ?? { x: 0, y: 0, width: 0, height: 0 },
        focus: (id: string) => focused.push(id) };
      for (const name of ['settingsPage', 'settingsPageScrollTop', 'settingsJumpReady']) env[name] = value(derive(name), env);
      return env;
    };
    const run = (action: string, args: unknown[] = []) => root.call(action, state, scope(), args);
    // From Appearance, the search picks General's "Time format": the route changes and General's settings are not in yet.
    run('corePick', ['general', 'setting-time-format', '', '']);
    expect(scope().settingsJumpReady).toBe(false);
    run('focusSettingsTarget');
    expect(state.settingsFocusTarget).toBe('setting-time-format');
    run('settingsPageForget'); // the page-left task (after 1 ms)
    expect(scope().settingsPageScrollTop).toBe(0); // the new page's scroller mounts at its top
    // General's settings arrive: the jump is ready, and its task (after 1 ms, once the page is laid out) scrolls to the row.
    core = { ready: true, route: 'general', sections: [{}] };
    expect(scope().settingsJumpReady).toBe(true);
    run('focusSettingsTarget');
    expect(scope().settingsPageScrollTop).toBe(900 - 100 + 20 - 300 + 0.25); // the row's centre at the scroller's
    expect(focused).toEqual(['setting-time-format']);
    expect(state.settingsFocusTarget).toBe('');
    // The page-left task firing after the jump keeps it: the offset is this page's.
    run('settingsPageForget');
    expect(scope().settingsPageScrollTop).toBe(520.25);
    // Leaving for another page forgets it, so General opened later mounts at its top.
    run('coreNavigate', ['appearance']);
    expect(scope().settingsPageScrollTop).toBe(0);
    run('settingsPageForget');
    expect(state.settingsScrollPage).toBe('');
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
    expect(lines(root, 'settingsScrollTop=settingsPageScrollTop, settingsPage=settingsPage')).toHaveLength(1);
    // Nothing but the jump and the page-left task writes the jump's page.
    expect(root.split('\n').filter(candidate => /^ {4,}settingsScrollPage = /.test(candidate))).toHaveLength(2);
  });
});
