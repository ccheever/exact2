// 20261005-portable-app-download item 3: the first-launch view's content from the embedded server's
// install status, and its Retry and Quit.
import { describe, expect, test } from 'bun:test';
import { CHECKING, SETUP_FAILED_TITLE, SETUP_TITLE, firstLaunchLocal, firstLaunchView, unpacking } from './first-launch';
import { parseLocalBackendStatus } from './local-backend';

const status = (raw: object) => parseLocalBackendStatus(raw);

describe('first-launch view', () => {
  test('is hidden unless the runtime is being unpacked or its unpack failed', () => {
    for (const state of ['refused', 'runtime-missing', 'starting', 'ready', 'restarting', 'stopped']) {
      expect(firstLaunchView(status({ state, install: { phase: 'extract', fraction: 1 } })).show).toBe(false);
    }
    expect(firstLaunchView(undefined).show).toBe(false);
    // A port failure is `failed` without an install reason: fatal, never this view.
    expect(firstLaunchView(status({ state: 'failed', failure: 'No desktop backend port is available' })).show).toBe(false);
  });

  test('checking, then unpacking with the percentage', () => {
    expect(firstLaunchView(status({ state: 'installing' }))).toEqual({ show: true, failed: false, title: SETUP_TITLE, status: CHECKING, percent: 0, error: '' });
    expect(firstLaunchView(status({ state: 'installing', install: { phase: 'verify', fraction: 0.6 } }))).toMatchObject({ status: 'Checking the server files…', percent: 0 });
    expect(firstLaunchView(status({ state: 'installing', install: { phase: 'extract', fraction: 0.42 } }))).toMatchObject({ title: 'Setting up T3 Code…', status: 'Unpacking the server files… 25%', percent: 42 });
  });

  test('the stage line (the live region) changes only at a stage change and every 25 %', () => {
    const lines = new Set<string>();
    for (let step = 0; step <= 50; step++) lines.add(firstLaunchView(status({ state: 'installing', install: { phase: 'extract', fraction: step / 50 } })).status);
    expect([...lines]).toEqual([0, 25, 50, 75, 100].map(n => `Unpacking the server files… ${n}%`));
    expect(unpacking(0.999)).toBe('Unpacking the server files… 75%');
    expect(unpacking(1)).toBe('Unpacking the server files… 100%');
  });

  test('the bar follows every report', () => {
    const percents = [0, 0.02, 0.04, 0.5, 0.98, 1].map(fraction => firstLaunchView(status({ state: 'installing', install: { phase: 'extract', fraction } })).percent);
    expect(percents).toEqual([0, 2, 4, 50, 98, 100]);
  });

  test('a failed unpack shows its reason', () => {
    const reason = 'There is not enough disk space to set up T3 Code: it needs 260 MB in /Users/someone/.t3/runtime/versions, and 41 MB is free.';
    expect(firstLaunchView(status({ state: 'failed', install: { reason } }))).toEqual({ show: true, failed: true, title: SETUP_FAILED_TITLE, status: '', percent: 0, error: reason });
    expect(SETUP_FAILED_TITLE).toBe('T3 Code could not be set up');
  });
});

describe('first-launch buttons', () => {
  const native = (answer: object) => {
    const asked: unknown[] = [];
    return { asked, native: { available: true, watch() {}, later: async (request: unknown) => { asked.push(request); return answer; } } };
  };

  test('Retry asks the backend to install again and takes the status it answers', async () => {
    const { asked, native: bridge } = native({ ok: true, generation: 0, value: { state: 'installing', install: { phase: 'verify', fraction: 0 } } });
    const client = { localBackend: status({ state: 'failed', install: { reason: 'The bundled t3.tar.gz does not match its pinned SHA-256.' } }) };
    expect(await firstLaunchLocal(client, bridge, 'setup-retry')).toBe('');
    expect(asked).toEqual([{ op: 'localBackendRetry' }]);
    expect(firstLaunchView(client.localBackend)).toMatchObject({ show: true, failed: false, status: CHECKING });
  });

  test('a Retry that fails again leaves the status to the backend', async () => {
    const { native: bridge } = native({ ok: false, generation: 0, error: { kind: 'LocalEnvironment', message: 'still damaged' } });
    const client = { localBackend: status({ state: 'failed', install: { reason: 'damaged' } }) };
    expect(await firstLaunchLocal(client, bridge, 'setup-retry')).toBe('');
    expect(client.localBackend.install?.reason).toBe('damaged');
  });

  test('Quit asks the app to quit', async () => {
    const { asked, native: bridge } = native({ ok: true, generation: 0 });
    expect(await firstLaunchLocal({ localBackend: status({ state: 'failed', install: { reason: 'x' } }) }, bridge, 'setup-quit')).toBe('');
    expect(asked).toEqual([{ op: 'localBackendQuit' }]);
  });
});
