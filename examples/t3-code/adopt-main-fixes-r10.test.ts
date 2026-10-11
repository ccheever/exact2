// adopt-main-fixes-r10 RT-6: since main #457 (X55) the Mac host exposes `role="status"` and `role="alert"` as AppKit's
// live-region groups (AXApplicationStatus, polite; AXApplicationAlert, assertive) and `dialog` as AXApplicationDialog,
// so an explicit `aria-live` is written only where T3 Code (1e2ecbd975, MIT, see LICENSE-T3) writes one: the browser
// profile removal error (IntegrationsSettings.tsx `<p aria-live="polite">`) and the toast viewport (Base UI's
// Toast.Viewport: role "region", aria-live "polite"). The provider setup's status lines keep the reference's
// `role="status"` (ProviderSetupSection.tsx, ProviderAuthenticationSection.tsx); the Codex account description is plain
// text there (CodexSetupSection.tsx accountDescription in a SettingsRow or CodexWelcomeCard description).
import { expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';

const dir = import.meta.dir;
const views = readdirSync(dir).filter(name => name.endsWith('.contract')).sort()
  .map(name => ({ name, lines: readFileSync(`${dir}/${name}`, 'utf8').split('\n') }));
const lines = (file: string, needle: string) => views.find(v => v.name === file)!.lines.filter(l => l.includes(needle));

test('an explicit aria-live only where the reference writes one', () => {
  const live = views.flatMap(({ name, lines }) => lines.filter(l => /\baria-live=/.test(l) && !/^\s*\/\//.test(l)).map(l => `${name}: ${/testId=("[^"]*"|`[^`]*`)/.exec(l)?.[1]}`));
  expect(live).toEqual(['browser-profiles.contract: "browser-profile-remove-error"', 'shell-toast.contract: "toast-region"']);
  expect(lines('shell-toast.contract', 'testId="toast-region"')[0]).toContain('role="region" aria-live="polite" aria-label="Notifications"');
  expect(lines('shell-toast.contract', 'role="dialog" aria-label=toast.title')).toHaveLength(1);
});

test('the provider setup status lines are status nodes; the Codex account description is plain text', () => {
  for (const needle of ['text runtime.pendingLabel ', 'text runtime.statusMessage ', 'text account.description max-width="36rem"']) {
    const found = lines('providers-setup.contract', needle);
    expect(found).toHaveLength(1);
    expect(found[0]).toContain('role="status"');
  }
  const codex = lines('codex-setup.contract', 'text setup.description ');
  expect(codex).toHaveLength(2);
  for (const line of codex) expect(line).not.toContain('role=');
});

// The Settings surface is a route in the reference (SettingsSidebarNav's navigation "Settings" and the page), not a
// dialog; since #457 a `role="dialog"` box is AXApplicationDialog, so the clone's Settings row writes none.
// The pull request link preview is Base UI's PreviewCard popup, which has no role either.
test('the Settings surface and the pull request link preview are no dialogs', () => {
  for (const [file, needle] of [['app-settings.contract', 'testId="settings-dialog"'], ['pages-pr-links.contract', 'testId="pull-request-link-preview"']]) {
    const found = lines(file!, needle!);
    expect(found).toHaveLength(1);
    expect(found[0]).not.toContain('role=');
  }
});
