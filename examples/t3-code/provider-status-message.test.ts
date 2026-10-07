// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/web/src/components/chat/
// ProviderStatusBanner.test.tsx (original names). The markup checks read the banner's
// view model, which chat.contract's AlertStack draws.
import { describe, expect, test } from 'bun:test';
import type { Obj } from './domain';
import { getProviderStatusBannerKey, getProviderStatusMessage, hasProviderSetup, providerBanner, shouldShowProviderStatusBanner, formatProviderDriverKindLabel } from './provider-status-message';

function warningProvider(): Obj {
  return { instanceId: 'codex', driver: 'codex', displayName: 'Codex', enabled: true, installed: true, version: '1.0.0', status: 'warning',
    auth: { status: 'authenticated' }, checkedAt: '2026-07-23T12:00:00.000Z', message: 'Provider is temporarily degraded.', models: [], slashCommands: [], skills: [] };
}

describe('ProviderStatusBanner', () => {
  test('waits for an Antigravity auth result before showing a sign-in warning', () => {
    const status = { ...warningProvider(), instanceId: 'google_work', driver: 'antigravity', auth: { status: 'unknown' },
      message: 'Antigravity is installed. Google account access is not checked yet.' };
    expect(shouldShowProviderStatusBanner(status, null)).toBe(false);
    expect(shouldShowProviderStatusBanner({ ...status, auth: { status: 'unauthenticated' }, message: 'Sign in with Google to use Antigravity.' }, null)).toBe(true);
  });

  test('shows Antigravity installation and startup failures before auth is checked', () => {
    const status = { ...warningProvider(), driver: 'antigravity', auth: { status: 'unknown' } };
    expect(shouldShowProviderStatusBanner({ ...status, installed: false }, null)).toBe(true);
    expect(shouldShowProviderStatusBanner({ ...status, status: 'error' }, null)).toBe(true);
    expect(shouldShowProviderStatusBanner({ ...status, driver: 'codex' }, null)).toBe(true);
  });

  test('stays hidden after its current warning is dismissed', () => {
    const status = warningProvider();
    expect(shouldShowProviderStatusBanner(status, null)).toBe(true);
    expect(shouldShowProviderStatusBanner(status, getProviderStatusBannerKey(status))).toBe(false);
  });

  test('renders an accessible dismiss control for provider warnings', () => {
    const banner = providerBanner(warningProvider());
    expect(banner.providerBannerAlert).toBe(true);
    expect(banner.providerBannerDismiss).toBe('Dismiss Codex provider warning');
  });

  test('labels error dismiss controls with the correct severity', () => {
    expect(providerBanner({ ...warningProvider(), status: 'error' }).providerBannerDismiss).toBe('Dismiss Codex provider error');
  });
});

describe('getProviderStatusMessage', () => {
  test("preserves the environment's authentication error", () => {
    const message = 'SUBSCRIPTION_REQUIRED: This Google account cannot use Antigravity.';
    expect(getProviderStatusMessage({ ...warningProvider(), driver: 'antigravity', status: 'error', auth: { status: 'unauthenticated' }, message })).toBe(message);
  });

  test('points a signed-out Antigravity account to Google sign-in without a CLI command', () => {
    expect(getProviderStatusMessage({ ...warningProvider(), driver: 'antigravity', status: 'error', auth: { status: 'unauthenticated' }, message: '' })).toBe('Open provider setup to sign in with Google.');
  });

  test('requires installation on the environment before sign-in', () => {
    expect(getProviderStatusMessage({ ...warningProvider(), driver: 'antigravity', displayName: 'Google work account', installed: false, status: 'error',
      auth: { status: 'unauthenticated' }, message: '' })).toBe('Open provider setup to install Antigravity on this environment.');
  });

  test('keeps CLI sign-in advice for a provider without integrated setup', () => {
    expect(getProviderStatusMessage({ ...warningProvider(), status: 'error', auth: { status: 'unauthenticated' }, message: '' })).toBe('Sign in via the CLI to authenticate again.');
  });
});

describe('the clone\'s banner projection', () => {
  test('offers "Open provider setup" for the instance only when it has in-app setup', () => {
    expect(hasProviderSetup({ driver: 'antigravity' })).toBe(true);
    expect(hasProviderSetup({ driver: 'cursor', setup: { canAuthenticate: true, canInstall: false } })).toBe(true);
    expect(hasProviderSetup({ driver: 'codex', setup: { canAuthenticate: false, canInstall: false } })).toBe(false);
    const antigravity = { ...warningProvider(), instanceId: 'google_work', driver: 'antigravity', displayName: '', status: 'error', auth: { status: 'unauthenticated' }, message: '' };
    expect(providerBanner(antigravity)).toMatchObject({ providerBannerTitle: 'Antigravity is unauthenticated', providerBannerMessage: 'Open provider setup to sign in with Google.',
      providerBannerSetup: 'google_work', providerBannerWarning: false, providerBannerAlert: true });
    expect(providerBanner({ ...warningProvider(), status: 'error', auth: { status: 'unauthenticated' }, message: '' }).providerBannerSetup).toBe('');
  });
  test('an unsupported version is a status, a broken one an alert; ready and disabled show nothing', () => {
    const ready = { ...warningProvider(), status: 'ready', compatibilityAdvisory: { status: 'unsupported', message: 'Update.' } };
    expect(providerBanner(ready)).toMatchObject({ providerBannerTitle: 'Codex 1.0.0 is unsupported', providerBannerMessage: 'Update.', providerBannerAlert: false, providerBannerWarning: true });
    expect(providerBanner({ ...ready, compatibilityAdvisory: { status: 'broken', message: null } })).toMatchObject({ providerBannerAlert: true, providerBannerWarning: false,
      providerBannerMessage: 'Provider is temporarily degraded.' });
    expect(providerBanner({ ...ready, compatibilityAdvisory: null }).providerBannerKey).toBe('');
    expect(providerBanner({ ...warningProvider(), status: 'disabled' }).providerBannerKey).toBe('');
  });
  test('formatProviderDriverKindLabel', () => {
    expect(formatProviderDriverKindLabel('claudeAgent')).toBe('Claude Agent');
    expect(formatProviderDriverKindLabel('acp_registry')).toBe('Acp Registry');
  });
});
