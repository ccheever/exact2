// Each Settings page opens at its top, read from the Contract sources as settings-escape.test.ts reads its own; the
// offsets themselves are driven on macOS (a source read cannot scroll). Reference 1e2ecbd975: every page renders its
// own scroller (SettingsPageContainer, settingsLayout.tsx `data-settings-page-scroll`) inside an Outlet keyed on the
// scope search and the restore count (routes/settings.tsx SettingsContentLayout), so a new page mounts a scroller at
// offset 0; the search jump then scrolls its target into view (scrollToSettingsTarget).
import { describe, expect, test } from 'bun:test';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
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

  test('the bound offset is the search jump\'s only on the page it revealed on, and 0 on any other', async () => {
    const root = await source('app.contract');
    expect(root).toContain('  derive settingsPageScrollTop = settingsScrollPage == settingsPage ? settingsScrollTop : 0\n');
    expect(lines(root, 'settingsScrollTop=settingsPageScrollTop, settingsPage=settingsPage')).toHaveLength(1);
    // The jump records its page with its offset; leaving that page (or Settings) forgets it, so the page opened again
    // later mounts at 0 rather than at the old jump's offset.
    expect(root).toContain('              settingsScrollPage = settingsPage\n              settingsScrollTop = max(0, frame(settingsFocusTarget).y');
    expect(root).toContain('  task settingsPageLeft key=`${settingsOpen}|${settingsPage}`\n    after(1, settingsPageForget)\n  action settingsPageForget\n    settingsScrollPage = settingsOpen and settingsScrollPage == settingsPage ? settingsScrollPage : ""\n');
    // Nothing else writes the jump's page.
    expect(root.split('\n').filter(candidate => /^ {4,}settingsScrollPage = /.test(candidate))).toHaveLength(2);
  });
});
