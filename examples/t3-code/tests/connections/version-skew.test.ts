// Port of T3 Code's apps/web/src/versionSkew.test.ts (1e2ecbd975, MIT, see LICENSE-T3),
// with the case names kept. APP_VERSION is pinned per case by passing the client version.
import { beforeEach, describe, expect, it } from 'bun:test';
import {
  buildVersionMismatchDismissalKey, dismissServerUpdateFailure, dismissVersionMismatch, isServerUpdateFailureDismissed,
  isVersionMismatchDismissed, resolveServerConfigVersionMismatch, resolveServerSelfUpdateCapability, resolveVersionMismatch,
  supportsDesktopAppUpdate, CLIENT_VERSION, type DismissalStore,
} from '../../src/connections/version-skew';
import type { ServerUpdateState } from '../../src/connections/server-update';

const MISMATCH_HINT = 'Version mismatch. Try syncing the client and server to the same T3 Code version.';
let APP_VERSION = '0.0.34';
const resolve = (serverVersion: string | null | undefined) => resolveVersionMismatch(serverVersion, APP_VERSION);

describe('versionSkew', () => {
  beforeEach(() => { APP_VERSION = '0.0.34'; });

  it('dismisses only the current failed attempt without clearing its retry state', () => {
    // A failed state carries the attempt the native job numbered (server-update.ts serverUpdateStateForJob).
    const failure = { status: 'failed', stage: 'downloading', fromVersion: '0.0.33', targetVersion: '0.0.34', message: 'Download failed.', attempt: 'skew-1' } as const satisfies ServerUpdateState;
    const retryFailure = { ...failure, attempt: 'skew-2' };
    const otherEnvironmentFailure = { ...failure, attempt: 'skew-3' };

    dismissServerUpdateFailure(failure);

    expect(isServerUpdateFailureDismissed(failure)).toBe(true);
    expect(failure.status).toBe('failed');
    expect(failure.message).toBe('Download failed.');
    expect(isServerUpdateFailureDismissed(retryFailure)).toBe(false);
    expect(isServerUpdateFailureDismissed(otherEnvironmentFailure)).toBe(false);
  });

  it('does not dismiss an update that is still running', () => {
    const running = { status: 'running', stage: 'resuming', fromVersion: '0.0.33', targetVersion: '0.0.34', attempt: 'skew-4' } as const satisfies ServerUpdateState;
    dismissServerUpdateFailure(running);
    expect(isServerUpdateFailureDismissed(running)).toBe(false);
  });

  it('does not warn when versions match', () => {
    expect(resolve(APP_VERSION)).toBeNull();
  });

  it('this client is the 0.1.4 Nightly, so a 0.1.2 server reads as out of date and its update targets 0.1.4', () => {
    expect(CLIENT_VERSION).toBe('0.1.4-nightly.20261027.3770');
    expect(resolveVersionMismatch('0.1.2-nightly.20261021.3457')).toEqual({ clientVersion: CLIENT_VERSION, serverVersion: '0.1.2-nightly.20261021.3457', hint: MISMATCH_HINT });
    expect(resolveVersionMismatch(CLIENT_VERSION)).toBeNull();
  });

  it('returns a mismatch when the server is behind the client', () => {
    expect(resolve('0.0.33')).toEqual({ clientVersion: '0.0.34', serverVersion: '0.0.33', hint: MISMATCH_HINT });
  });

  it('does not warn when the server is ahead of the client', () => {
    expect(resolve('0.0.35')).toBeNull();
    expect(resolve('9.9.9')).toBeNull();
  });

  it('does not warn when a nightly and a stable build share a core version', () => {
    expect(resolve('0.0.34-nightly.20260818.1124')).toBeNull();
    APP_VERSION = '0.0.34-nightly.20260818.1124';
    expect(resolve('0.0.34')).toBeNull();
  });

  it.each(['0.0.34-nightly.20260823.1124', '0.0.34-nightly.20260824.1124'])('warns when nightly server %s is behind a nightly client on the same release', serverVersion => {
    APP_VERSION = '0.0.34-nightly.20260824.1125';
    expect(resolve(serverVersion)).toEqual({ clientVersion: '0.0.34-nightly.20260824.1125', serverVersion, hint: MISMATCH_HINT });
  });

  it('does not warn when a nightly server is ahead on the same release', () => {
    APP_VERSION = '0.0.34-nightly.20260824.1125';
    expect(resolve('0.0.34-nightly.20260824.1126')).toBeNull();
  });

  it('treats a nightly server built past the client as ahead, not skew', () => {
    expect(resolve('0.0.35-nightly.20260818.1124')).toBeNull();
  });

  it('still warns when a nightly client outruns the server by a release', () => {
    APP_VERSION = '0.0.35-nightly.20260818.1124';
    expect(resolve('0.0.34')).toEqual({ clientVersion: '0.0.35-nightly.20260818.1124', serverVersion: '0.0.34', hint: MISMATCH_HINT });
  });

  it('falls back to string inequality when a version is not semver', () => {
    expect(resolve('dev')).toEqual({ clientVersion: '0.0.34', serverVersion: 'dev', hint: MISMATCH_HINT });
    APP_VERSION = 'dev';
    expect(resolve('dev')).toBeNull();
    expect(resolve('0.0.34')).toMatchObject({ serverVersion: '0.0.34' });
  });

  it('reads the server version from config descriptors', () => {
    expect(resolveServerConfigVersionMismatch({ environment: { environmentId: 'environment-1', label: 'Remote', platform: { os: 'darwin', arch: 'arm64' },
      serverVersion: '0.0.33', capabilities: { repositoryIdentity: true } } }, APP_VERSION)).toMatchObject({ serverVersion: '0.0.33' });
  });

  it('keys dismissals by environment, client version, and server version', () => {
    const store: DismissalStore = { versionMismatchDismissals: [] };
    const environmentId = 'environment-dismissal';
    const key = buildVersionMismatchDismissalKey(environmentId, { clientVersion: APP_VERSION, serverVersion: '9.9.9' });

    expect(key).toBe(`${environmentId}:${APP_VERSION}:9.9.9`);
    expect(isVersionMismatchDismissed(key, store)).toBe(false);

    dismissVersionMismatch(key, store);

    expect(isVersionMismatchDismissed(key, store)).toBe(true);
    expect(isVersionMismatchDismissed(buildVersionMismatchDismissalKey(environmentId, { clientVersion: APP_VERSION, serverVersion: '9.9.10' }), store)).toBe(false);
  });

  it('reads desktop-managed update capabilities from config descriptors', () => {
    expect(resolveServerSelfUpdateCapability({ environment: { environmentId: 'environment-desktop', label: 'Desktop', platform: { os: 'darwin', arch: 'arm64' },
      serverVersion: '9.9.9', capabilities: { repositoryIdentity: true, serverSelfUpdate: 'desktop-managed' } } })).toBe('desktop-managed');
    expect(resolveServerSelfUpdateCapability(null)).toBeNull();
  });

  it('detects remote desktop-app update support from config descriptors', () => {
    const descriptor = (desktopAppUpdate?: boolean) => ({ environment: { environmentId: 'environment-desktop', label: 'Desktop', platform: { os: 'darwin', arch: 'arm64' },
      serverVersion: '9.9.9', capabilities: { repositoryIdentity: true, serverSelfUpdate: 'desktop-managed', ...(desktopAppUpdate === undefined ? {} : { desktopAppUpdate }) } } });
    expect(supportsDesktopAppUpdate(descriptor(true))).toBe(true);
    expect(supportsDesktopAppUpdate(descriptor(false))).toBe(false);
    expect(supportsDesktopAppUpdate(descriptor())).toBe(false);
    expect(supportsDesktopAppUpdate(null)).toBe(false);
  });
});
