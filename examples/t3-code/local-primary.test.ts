// 20261005-local-primary-environment: "This machine" is the primary environment. The first two
// describes port T3 Code's tests with their original names (MIT, see LICENSE-T3; reference
// 1e2ecbd975: apps/web/src/environments/primary/bootstrap.test.ts, components/chat/folderDrop.test.ts);
// the rest cover the clone's wiring: the launch, the fleet, the switch, settings search, first run,
// the landing and the migration toast.
import { afterEach, describe, expect, it, test } from 'bun:test';
import { DesktopEnvironmentBootstrapIncompleteError, folderDropTarget, primary, primaryPhase, readPrimaryTarget, resolvePrimaryEnvironmentHttpUrl,
  dropPrimaryDuplicates, isPrimaryEnvironment, sessionScopes, LocalPrimary } from './local-primary';
import { parseLocalBackendStatus } from './local-backend';
import { primaryAt, primaryOff, noPrimary, resetPrimary } from './local-primary-fixture';
import { reconnectOnLaunch, launchFocus, launchChoice } from './r8-pointer-reconnect';
import { EnvironmentFleet } from './settings-b-fleet';
import { applyLocalSetting, thisMachine, LOCAL_OFF_DESCRIPTION, LOCAL_ON_DESCRIPTION, TURN_OFF, TURN_ON } from './this-machine';
import { connectionsProjection } from './connections';
import { searchSettings, searchContext } from './settings-search';
import { pagesHome, noEnvironmentDescription } from './pages-home';
import { welcomeView } from './pages-welcome';
import { applyLifecycle, migrationDescription, migrationToast, primaryLifecycle, primaryWelcome } from './local-lifecycle';
import { toasts } from './toast';
import { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';

afterEach(() => { resetPrimary(); primaryLifecycle.environmentId = ''; primaryLifecycle.welcome = null; primaryLifecycle.migration = null; });

const ready = (extra: Obj = {}) => parseLocalBackendStatus({ state: 'ready', enabled: true, httpBaseUrl: 'http://127.0.0.1:16437', wsBaseUrl: 'ws://127.0.0.1:16437', bearerReady: true,
  environmentId: 'env-local', label: 'Lane Mac', serverVersion: '0.0.46-nightly.20261005.2667', ...extra });

describe('environmentBootstrap', () => {
  it('describes which desktop bootstrap endpoint is missing', () => {
    let error: unknown;
    try { readPrimaryTarget(parseLocalBackendStatus({ state: 'ready', httpBaseUrl: 'http://127.0.0.1:3773' }), true); } catch (caught) { error = caught; }
    expect(error).toBeInstanceOf(DesktopEnvironmentBootstrapIncompleteError);
    expect(error).toMatchObject({ hasHttpBaseUrl: true, hasWsBaseUrl: false, message: 'Desktop bootstrap is missing wsBaseUrl for the local environment.' });
    // The singleton records it as a problem: no primary, and none will come without a change.
    const local = new LocalPrimary();
    local.update(parseLocalBackendStatus({ state: 'ready', httpBaseUrl: 'http://127.0.0.1:3773' }), true);
    expect([local.target, local.problem, local.unavailable]).toEqual([null, 'Desktop bootstrap is missing wsBaseUrl for the local environment.', true]);
  });

  it('has no primary target when the desktop local environment is disabled', () => {
    expect(readPrimaryTarget(ready(), false)).toBeNull();
    expect(readPrimaryTarget(ready({ enabled: false }), true)).toBeNull();
    primaryOff();
    expect(() => resolvePrimaryEnvironmentHttpUrl('/api/auth/session')).toThrow('The local environment is disabled.');
  });
});

describe('folderDropTarget', () => {
  const environmentId = 'environment-1';
  it('targets local when the thread is on the primary environment', () => {
    expect(folderDropTarget({ localEnvironmentDisabled: false, environmentId, primaryEnvironmentId: environmentId })).toBe('local');
  });
  it('targets remote when Electron has no local environment', () => {
    expect(folderDropTarget({ localEnvironmentDisabled: true, environmentId, primaryEnvironmentId: environmentId })).toBe('remote');
  });
  it('targets remote when the thread lives on another environment', () => {
    expect(folderDropTarget({ localEnvironmentDisabled: false, environmentId: 'environment-2', primaryEnvironmentId: environmentId })).toBe('remote');
  });
  it('targets remote when no primary environment is known', () => {
    expect(folderDropTarget({ localEnvironmentDisabled: false, environmentId, primaryEnvironmentId: null })).toBe('remote');
  });
});

describe('the primary from the embedded server', () => {
  test('no bases is no primary; a refused development build has none; ready builds it from the descriptor', () => {
    expect(readPrimaryTarget(parseLocalBackendStatus({ state: 'installing' }), true)).toBeNull();
    expect(readPrimaryTarget(parseLocalBackendStatus({ state: 'refused', refused: 'Development build: …' }), true)).toBeNull();
    expect(readPrimaryTarget(ready(), true)).toEqual({ id: 'primary', source: 'desktop-managed', httpBaseUrl: 'http://127.0.0.1:16437/', wsBaseUrl: 'ws://127.0.0.1:16437/',
      environmentId: 'env-local', label: 'Lane Mac' });
    primaryAt('http://127.0.0.1:16437', 'env-local');
    expect(resolvePrimaryEnvironmentHttpUrl('/api/auth/session')).toBe('http://127.0.0.1:16437/api/auth/session');
    expect(isPrimaryEnvironment('env-local')).toBe(true);
    expect(sessionScopes('env-local', ['orchestration:read'])).toEqual(['orchestration:read', 'orchestration:operate', 'terminal:operate', 'review:write', 'relay:read', 'access:read', 'access:write', 'relay:write']);
    expect(sessionScopes('env-remote', ['orchestration:read'])).toEqual(['orchestration:read']);
  });

  test('its phases: installing and starting connect, a restart reconnects with the exit, a failure says why, ready follows the socket', () => {
    const connected = { phase: 'connected' as const, message: '' }, idle = { phase: 'available' as const, message: '' };
    expect(primaryPhase(parseLocalBackendStatus({ state: 'installing' }), connected)).toEqual({ phase: 'connecting', message: '' });
    expect(primaryPhase(parseLocalBackendStatus({ state: 'starting' }), connected)).toEqual({ phase: 'connecting', message: '' });
    expect(primaryPhase(parseLocalBackendStatus({ state: 'restarting', lastExit: 'signal=9' }), connected)).toEqual({ phase: 'reconnecting', message: 'The local server exited (signal=9).' });
    expect(primaryPhase(parseLocalBackendStatus({ state: 'failed', failure: 'No desktop backend port is available.' }), connected)).toEqual({ phase: 'error', message: 'No desktop backend port is available.' });
    expect(primaryPhase(ready(), connected)).toEqual(connected);
    expect(primaryPhase(ready(), idle)).toEqual({ phase: 'connecting', message: '' });
  });
});

// ── The launch ─────────────────────────────────────────────────────────────
class Fake implements Native {
  available = true; calls: Obj[] = [];
  /** desktop-settings.json as the native side keeps it (T3DesktopSettings.swift); `settingsFailure` fails the next write. */
  settings: Obj = { localEnvironmentEnabled: true, serverExposureMode: 'local-only', tailscaleServeEnabled: false, tailscaleServePort: 443 };
  settingsFailure = '';
  constructor(public saved: Obj[] = [], public answers: Record<string, (request: Obj) => unknown> = {}) {}
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const answer = this.answers[String(request.op)];
    if (answer) { const value = answer(request); return value instanceof Error ? { ok: false, generation: 1, error: { kind: 'LocalEnvironment', message: value.message, uncertain: false } } : { ok: true, generation: 1, value }; }
    if (request.op === 'environments') return { ok: true, generation: 0, value: { saved: this.saved } };
    if (request.op === 'desktopSettingsSet') {
      if (this.settingsFailure) { const message = this.settingsFailure; this.settingsFailure = ''; return { ok: false, generation: 0, error: { kind: 'DesktopSettings', message } }; }
      const { op: _op, ...patch } = request, next = { ...this.settings, ...patch }, changed = JSON.stringify(next) !== JSON.stringify(this.settings);
      this.settings = next;
      return { ok: true, generation: 0, value: { changed, settings: next } };
    }
    return { ok: true, generation: 0, value: {} };
  }
}
const remote = { origin: 'https://box.example.com', environmentId: 'env-box', label: 'Box', enabled: true };
const disconnected = () => ({ origin: '', environmentId: '', connection: 'disconnected' });

describe('a launch opens the primary', () => {
  test('the focus token, else the last origin, else the primary while it is on, else the first saved one', () => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    expect(launchChoice([remote], { focus: 'primary', origin: '' })).toBe('primary');
    expect(launchChoice([remote], { focus: '', origin: 'https://box.example.com/' })).toBe(remote);
    expect(launchChoice([remote], { focus: '', origin: '' })).toBe('primary');
    primaryOff();
    expect(launchChoice([remote], { focus: 'primary', origin: '' })).toBe(remote);
  });

  test('it waits for the server and its bearer, then connects with `primary: true`; the fleet leaves it alone meanwhile', async () => {
    primary.update(parseLocalBackendStatus({ state: 'installing', install: { phase: 'extract', fraction: 0.4 } }), true);
    const client = disconnected(), native = new Fake([remote]);
    expect(await reconnectOnLaunch(client, native, { state: 'disconnected', origin: '', environmentId: '', focus: 'primary' })).toBe(false);
    expect(native.calls.filter(call => call.op === 'connect')).toEqual([]);
    primary.update(parseLocalBackendStatus({ state: 'starting', httpBaseUrl: 'http://127.0.0.1:16437', wsBaseUrl: 'ws://127.0.0.1:16437' }), true);
    expect(launchFocus(client)).toEqual({ origin: 'http://127.0.0.1:16437', environmentId: '', connection: 'connecting' });
    expect(await reconnectOnLaunch(client, native, { state: 'disconnected' })).toBe(false);
    primary.update(ready(), true);
    expect(await reconnectOnLaunch(client, native, { state: 'disconnected' })).toBe(true);
    expect(native.calls.filter(call => call.op === 'connect')).toEqual([{ op: 'connect', origin: 'http://127.0.0.1:16437', primary: true }]);
    expect(launchFocus(client)).toEqual({ origin: 'http://127.0.0.1:16437', environmentId: 'env-local', connection: 'connecting' });
  });

  test('a refused development build falls back to the saved environment', async () => {
    resetPrimary();
    const client = disconnected(), native = new Fake([remote]);
    await reconnectOnLaunch(client, native, { state: 'disconnected', origin: '', environmentId: '' });
    noPrimary();
    expect(await reconnectOnLaunch(client, native, { state: 'disconnected' })).toBe(true);
    expect(native.calls.filter(call => call.op === 'connect')).toEqual([{ op: 'connect', origin: 'https://box.example.com', credential: '' }]);
  });
});

describe('the fleet keeps the primary in the background', () => {
  test('a saved focus leaves the primary to the fleet, connected with the memory bearer; a saved duplicate is removed (U6)', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const duplicate = { origin: 'http://127.0.0.1:16999', environmentId: 'env-local', label: 'Paired before', enabled: true };
    const native = new Fake([remote, duplicate], { status: () => ({ state: 'disconnected', origin: '', environmentId: '', message: '' }) });
    const source = new EnvironmentFleet();
    await source.sync(native, { origin: remote.origin, environmentId: remote.environmentId, connection: 'connected' });
    expect(source.saved.map(entry => entry.environmentId)).toEqual(['env-box']);
    expect(native.calls.filter(call => call.op === 'forgetEnvironment')).toEqual([{ op: 'forgetEnvironment', origin: 'http://127.0.0.1:16999', environmentId: 'env-local' }]);
    expect(native.calls.filter(call => call.op === 'connect')).toEqual([{ op: 'connect', fleet: 'http://127.0.0.1:16437\nenv-local', origin: 'http://127.0.0.1:16437', primary: true }]);
    expect([...source.entries.values()].map(entry => [entry.environmentId, entry.primary])).toEqual([['env-local', true]]);
    // No primary id, no duplicates to drop.
    expect(await dropPrimaryDuplicates(native, [duplicate], new LocalPrimary())).toEqual([]);
  });
});

// ── The switch and the section ─────────────────────────────────────────────
function switchClient(focusPrimary: boolean): T3Client {
  const client = new T3Client();
  Object.assign(client, focusPrimary ? { origin: 'http://127.0.0.1:16437', environmentId: 'env-local', connection: 'connected' } : { origin: '', environmentId: '', connection: 'disconnected' });
  return client;
}

describe('the Local environment switch (applyLocalSetting, the U4 stopgap)', () => {
  test('turning off hands the focus to a saved environment, then stops the server; the setting persists', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const fleet = (await import('./settings-b-fleet')).fleet;
    fleet.saved = [remote];
    const client = switchClient(true);
    const native = new Fake([remote], {
      disconnect: () => ({ state: 'disconnected', origin: '', environmentId: '', message: 'Disconnected.' }),
      connect: request => ({ state: 'connecting', origin: request.origin, environmentId: '', message: '' }),
      localBackendSetEnabled: request => ({ state: 'stopped', enabled: request.enabled }),
    });
    const result = await applyLocalSetting(client, native, { localEnvironmentEnabled: false });
    // DesktopAppSettings.setLocalEnvironmentEnabled persists to desktop-settings.json first (decision U7).
    expect(native.calls.map(call => call.op)).toEqual(['desktopSettingsSet', 'disconnect', 'fleetStop', 'connect', 'fleetStop', 'localBackendSetEnabled']);
    expect(native.calls[0]).toEqual({ op: 'desktopSettingsSet', localEnvironmentEnabled: false });
    expect(native.calls.find(call => call.op === 'connect')).toMatchObject({ origin: 'https://box.example.com', credential: '' });
    expect(native.calls.find(call => call.op === 'localBackendSetEnabled')).toEqual({ op: 'localBackendSetEnabled', enabled: false });
    expect(result.status).toMatchObject({ origin: 'https://box.example.com', state: 'connecting' });
    expect(native.settings.localEnvironmentEnabled).toBe(false);
    expect(client.local).not.toHaveProperty('localEnvironmentEnabled'); // t3-code.json no longer holds it
    expect([primary.disabled, primary.target]).toEqual([true, null]);
    fleet.saved = [];
  });

  test('turning on starts the server and, with nothing focused, connects to it', async () => {
    primaryOff();
    const client = switchClient(false);
    client.localBackend = parseLocalBackendStatus({ state: 'stopped', enabled: false, desktopSettings: { localEnvironmentEnabled: false } });
    const native = new Fake([], {
      localBackendSetEnabled: () => ({ state: 'ready', enabled: true, httpBaseUrl: 'http://127.0.0.1:16437', wsBaseUrl: 'ws://127.0.0.1:16437', bearerReady: true, environmentId: 'env-local', label: 'Lane Mac',
        desktopSettings: { localEnvironmentEnabled: true } }),
      connect: request => ({ state: 'connected', origin: request.origin, environmentId: 'env-local', message: '' }),
    });
    native.settings.localEnvironmentEnabled = false;
    const result = await applyLocalSetting(client, native, { localEnvironmentEnabled: true });
    expect(native.calls.filter(call => call.op !== 'fleetStop')).toEqual([{ op: 'desktopSettingsSet', localEnvironmentEnabled: true }, { op: 'localBackendSetEnabled', enabled: true },
      { op: 'connect', origin: 'http://127.0.0.1:16437/', primary: true }]);
    expect([native.settings.localEnvironmentEnabled, client.localBackend.settings.localEnvironmentEnabled]).toEqual([true, true]);
    expect(result.status).toMatchObject({ environmentId: 'env-local', state: 'connected' });
    expect(primary.target?.environmentId).toBe('env-local');
  });

  test('a failure puts the setting back and answers its reason (shown under the dialog\'s description)', async () => {
    primaryOff();
    const client = switchClient(false);
    client.localBackend = parseLocalBackendStatus({ state: 'stopped', enabled: false, desktopSettings: { localEnvironmentEnabled: false } });
    const native = new Fake([], { localBackendSetEnabled: () => new Error('The local server stopped before it was ready (code=1).') });
    native.settings.localEnvironmentEnabled = false;
    expect(await applyLocalSetting(client, native, { localEnvironmentEnabled: true })).toEqual({ status: null, generation: -1 });
    // Persisted on, then written back off when the server could not start.
    expect(native.calls.filter(call => call.op === 'desktopSettingsSet').map(call => call.localEnvironmentEnabled)).toEqual([true, false]);
    expect([native.settings.localEnvironmentEnabled, client.localBackend.settings.localEnvironmentEnabled]).toEqual([false, false]);
    expect(primary.disabled).toBe(true);
    // The dialog's inline error (the view's), not the command's; the next change clears it.
    expect(thisMachine(undefined, null)).toMatchObject({ enabled: false, error: 'The local server stopped before it was ready (code=1).' });
  });

  test('a desktop-settings.json write failure changes nothing and shows the reference\'s DesktopSettingsWriteError text', async () => {
    primaryOff();
    const client = switchClient(false);
    client.localBackend = parseLocalBackendStatus({ state: 'stopped', enabled: false, desktopSettings: { localEnvironmentEnabled: false } });
    const native = new Fake([]);
    native.settings.localEnvironmentEnabled = false;
    native.settingsFailure = 'Desktop settings write failed during replace-settings-file at /lane/t3-home/userdata/desktop-settings.json.';
    expect(await applyLocalSetting(client, native, { localEnvironmentEnabled: true })).toEqual({ status: null, generation: -1 });
    expect(native.calls.map(call => call.op)).toEqual(['desktopSettingsSet']); // the server is never touched
    expect([native.settings.localEnvironmentEnabled, client.localBackend.settings.localEnvironmentEnabled, primary.disabled]).toEqual([false, false, true]);
    expect(thisMachine(undefined, null)).toMatchObject({ enabled: false, error: 'Desktop settings write failed during replace-settings-file at /lane/t3-home/userdata/desktop-settings.json.' });
  });

  test('the section holds still while a change runs ("Restarting…"), then shows the new value', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local', 'Lane Mac');
    const host = { connection: 'connected', origin: 'http://127.0.0.1:16437', environmentId: 'env-local', statusMessage: '', scopes: [], config: { environment: { label: 'Lane Mac' } } };
    expect(connectionsProjection(host, []).thisMachine).toMatchObject({ title: 'Lane Mac', enabled: true });
    let release: () => void = () => {};
    const stopped = new Promise<void>(resolve => { release = resolve; });
    const native = new Fake([], { disconnect: () => ({ state: 'disconnected', origin: '', environmentId: '', message: '' }) });
    const later = native.later.bind(native);
    native.later = async (input: unknown) => {
      if (obj(input).op === 'localBackendSetEnabled') { await stopped; return { ok: true, generation: 1, value: { state: 'stopped', enabled: false } }; }
      return later(input);
    };
    const client = switchClient(true);
    const change = applyLocalSetting(client, native, { localEnvironmentEnabled: false });
    await new Promise(resolve => setTimeout(resolve, 5));
    primary.update(parseLocalBackendStatus({ state: 'stopped', enabled: false }), false); // a refresh while the server stops
    expect(connectionsProjection({ ...host, connection: 'disconnected', environmentId: '' }, []).thisMachine).toMatchObject({ title: 'Lane Mac', enabled: true });
    release(); await change;
    expect(connectionsProjection({ ...host, connection: 'disconnected', environmentId: '' }, []).thisMachine).toMatchObject({ title: 'This machine', enabled: false });
  });

  test('the section: title, the switch row, the dialog copy and the Version row', () => {
    const off = thisMachine(undefined, null);
    expect(off).toMatchObject({ title: 'This machine', kind: 'desktop', menu: false, enabled: true, description: LOCAL_ON_DESCRIPTION, canManage: true, version: 'Loading…',
      dialogTitle: TURN_OFF.title, dialogBody: TURN_OFF.body, dialogConfirm: 'Restart and turn off' });
    expect(TURN_OFF.body).toBe('T3 Code will restart without running a server on this computer. Any agents and terminals running here will stop, and other devices will no longer be able to connect to this computer. Your projects, history, and remote environments are unaffected.');
    primaryOff();
    expect(thisMachine(undefined, null)).toMatchObject({ enabled: false, description: LOCAL_OFF_DESCRIPTION, canManage: false, dialogTitle: TURN_ON.title, dialogConfirm: 'Restart and turn on',
      dialogBody: 'T3 Code will restart and start running a server on this computer again.' });
    noPrimary();
    expect(thisMachine(undefined, null)).toMatchObject({ status: 'Development build: set T3_LOCAL_HOME and T3_LOCAL_PORT to start the local server.', canManage: false });
    primaryAt('http://127.0.0.1:16437', 'env-local', 'Lane Mac');
    const host = { connection: 'connected', origin: 'http://127.0.0.1:16437', environmentId: 'env-local', statusMessage: '', scopes: ['orchestration:read'],
      config: { environment: { label: 'Lane Mac', serverVersion: '0.0.46-nightly.20261005.2667', platform: { machine: 'laptop' }, capabilities: { environmentIcon: true } } } };
    const page = connectionsProjection(host, []);
    expect(page.thisMachine).toMatchObject({ title: 'Lane Mac', kind: 'laptop', menu: true, version: '0.0.46-nightly.20261005.2667 · http://127.0.0.1:16437/', upToDate: true, update: '', updateNote: '' });
    expect(page.environments).toEqual([]);
    // An older embedded server that only its desktop app can update: the sentence, no button.
    const older = connectionsProjection({ ...host, config: { environment: { ...host.config.environment, serverVersion: '0.0.45', capabilities: { serverSelfUpdate: 'desktop-managed' } } } }, []);
    expect(older.thisMachine).toMatchObject({ update: '', updateNote: 'Update the desktop app on that machine to update this server.', upToDate: false });
  });
});

describe('settings search follows this machine', () => {
  test('Local environment is a desktop row; local-backend rows follow the switch; WSL and Windows rows never show on the Mac', () => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const ids = (query: string) => searchSettings(query, searchContext({}, true, 'all')).map(item => item.id);
    expect(ids('local environment')).toContain('local-environment');
    expect(ids('environment icon')).toContain('environment-icon');
    expect(ids('WSL')).toEqual([]);
    expect(ids('Windows Subsystem')).toEqual([]);
    expect(ids('T3 Connect')).not.toContain('t3-connect');
    primaryOff();
    expect(ids('environment icon')).not.toContain('environment-icon');
    expect(ids('local environment')).toContain('local-environment');
  });
});

describe('first run and the landing with a primary', () => {
  test('a fresh primary opens the wizard with This machine first and preselected', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local', 'Lane Mac');
    applyLifecycle('env-local', { type: 'welcome', payload: { cwd: '/Users/me', projectName: 'me', bootstrapStatus: 'complete' } });
    const client = new T3Client();
    Object.defineProperty(client, 'preferencesLoaded', { value: true });
    Object.assign(client, { origin: 'http://127.0.0.1:16437', environmentId: 'env-local', connection: 'connected', configLive: true, shellLive: true, threadLive: true,
      config: { cwd: '/Users/me', environment: { label: 'Lane Mac', environmentId: 'env-local' } } });
    const view = await welcomeView(client, new Fake([]), { step: 'connect', now: Date.parse('2026-10-07T12:00:00.000Z') });
    expect(view.show).toBe(true);
    expect(view.computers.map(computer => [computer.label, computer.status, computer.selected])).toEqual([['Lane Mac', 'Connected', true]]);
    expect(primaryWelcome()).toMatchObject({ bootstrapStatus: 'complete' });
  });

  test('a primary that already holds projects opens the app and records setup as done', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local', 'Lane Mac');
    applyLifecycle('env-local', { type: 'welcome', payload: { cwd: '/Users/me', projectName: 'me' } });
    const client = new T3Client();
    Object.defineProperty(client, 'preferencesLoaded', { value: true });
    Object.assign(client, { origin: 'http://127.0.0.1:16437', environmentId: 'env-local', connection: 'connected', configLive: true, shellLive: true, threadLive: true,
      config: { cwd: '/Users/me', environment: { label: 'Lane Mac' } } });
    client.shell.projects = [{ id: 'p1', workspaceRoot: '/Users/me/a' }, { id: 'p2', workspaceRoot: '/Users/me/b' }];
    const view = await welcomeView(client, new Fake([]), { step: 'connect', now: Date.parse('2026-10-07T12:00:00.000Z') });
    expect(view.show).toBe(false);
    expect((client.local as { pages?: { onboardingCompletedAt: string } }).pages?.onboardingCompletedAt).toBe('2026-10-07T12:00:00.000Z');
  });

  test('the landing: connecting while the primary starts; the switched-off home names it', async () => {
    primary.update(parseLocalBackendStatus({ state: 'starting', httpBaseUrl: 'http://127.0.0.1:16437', wsBaseUrl: 'ws://127.0.0.1:16437' }), true);
    const client = new T3Client();
    expect((await pagesHome(client, new Fake([]), false)).landing).toBe('offline');
    primaryOff();
    const home = await pagesHome(client, new Fake([]), false);
    expect(home.landing).toBe('no-environment');
    expect(home.noEnvironmentDescription).toBe('The local environment is turned off. Connect a remote environment, or turn the local environment back on in Connections.');
    expect(noEnvironmentDescription(false)).toBe('Open Connections and add that machine using its pairing link. This app must be able to reach it.');
  });
});

describe('LegacyThreadMigrationToast (CN5)', () => {
  test('a loading toast with the count while the primary migrates; it closes when the migration completes', () => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const client = new T3Client();
    applyLifecycle('env-local', { version: 1, sequence: 3, type: 'legacyThreadMigration', payload: { status: 'running', totalThreadCount: 1234 } });
    migrationToast(client); migrationToast(client);
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description, toast.timeoutMs])).toEqual([
      ['loading', 'Restoring your threads…', 'Migrating 1,234 threads from the previous version. You can keep working while this finishes.', 0]]);
    expect(migrationDescription(1)).toBe('Migrating 1 thread from the previous version. You can keep working while this finishes.');
    applyLifecycle('env-local', { version: 1, sequence: 4, type: 'legacyThreadMigration', payload: { status: 'complete', totalThreadCount: 1234 } });
    migrationToast(client);
    expect(toasts(client)).toEqual([]);
    // Another environment's events are not the primary's.
    applyLifecycle('env-other', { type: 'legacyThreadMigration', payload: { status: 'running', totalThreadCount: 2 } });
    migrationToast(client);
    expect(toasts(client)).toEqual([]);
  });
});
