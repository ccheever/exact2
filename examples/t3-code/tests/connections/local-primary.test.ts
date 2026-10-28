// "This machine" is the primary environment. The first two
// describes port T3 Code's tests with their original names (MIT, see LICENSE-T3; reference
// 1e2ecbd975: apps/web/src/environments/primary/bootstrap.test.ts, components/chat/folderDrop.test.ts);
// the rest cover the clone's wiring: the launch, the fleet, the switch, settings search, first run,
// the landing and the migration toast.
import { afterEach, describe, expect, it, test } from 'bun:test';
import { DesktopEnvironmentBootstrapIncompleteError, folderDropTarget, primary, primaryPhase, readPrimaryTarget, resolvePrimaryEnvironmentHttpUrl,
  dropPrimaryDuplicates, isPrimaryEnvironment, sessionScopes, LocalPrimary } from '../../src/connections/local-primary';
import { parseLocalBackendStatus } from '../../src/connections/local-backend';
import { primaryAt, primaryOff, noPrimary, resetPrimary } from '../support/local-primary-fixture';
import { reconnectOnLaunch, launchFocus, launchChoice, primaryTakesFocus } from '../../src/connections/relaunch-reconnect';
import { EnvironmentFleet } from '../../src/connections/settings-b-fleet';
import { applyLocalSetting, thisMachine, LOCAL_OFF_DESCRIPTION, LOCAL_ON_DESCRIPTION, TURN_OFF, TURN_ON } from '../../src/connections/this-machine';
import { connectionsProjection, runConnectionOp } from '../../src/connections/connections';
import { searchSettings, searchContext } from '../../src/settings/settings-search';
import { pagesHome, noEnvironmentDescription } from '../../src/pages/pages-home';
import { welcomeView } from '../../src/pages/pages-welcome';
import { applyLifecycle, migrationDescription, migrationToast, primaryLifecycle, primaryWelcome } from '../../src/connections/local-lifecycle';
import { toasts } from '../../src/shell/toast';
import { T3Client } from '../../src/client/client';
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';

afterEach(() => { resetPrimary(); primaryLifecycle.environmentId = ''; primaryLifecycle.welcome = null; primaryLifecycle.migration = null; });

const ready = (extra: Obj = {}) => parseLocalBackendStatus({ state: 'ready', enabled: true, httpBaseUrl: 'http://127.0.0.1:16437', wsBaseUrl: 'ws://127.0.0.1:16437', bearerReady: true,
  environmentId: 'env-local', label: 'Lane Mac', serverVersion: '0.1.4-nightly.20261027.3770', ...extra });

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
    expect(sessionScopes('env-local', ['orchestration:read'])).toEqual([...AUTH_STANDARD_CLIENT_SCOPES]);
    expect(sessionScopes('env-remote', ['orchestration:read'])).toEqual(['orchestration:read']);
  });

  test('its phases: installing and starting connect, a restart reconnects with the exit, a failure says why, ready follows the socket', () => {
    const connected = { phase: 'connected' as const, message: '' }, idle = { phase: 'available' as const, message: '' };
    expect(primaryPhase(parseLocalBackendStatus({ state: 'installing' }), connected)).toEqual({ phase: 'connecting', message: '' });
    expect(primaryPhase(parseLocalBackendStatus({ state: 'starting' }), connected)).toEqual({ phase: 'connecting', message: '' });
    expect(primaryPhase(parseLocalBackendStatus({ state: 'restarting', lastExit: 'killed by SIGKILL' }), connected)).toEqual({ phase: 'reconnecting', message: 'The local server exited (killed by SIGKILL).' });
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
  settingsWrite: () => Promise<void> = async () => {};
  constructor(public saved: Obj[] = [], public answers: Record<string, (request: Obj) => unknown> = {}) {}
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const answer = this.answers[String(request.op)];
    if (answer) { const value = await answer(request); return value instanceof Error ? { ok: false, generation: 1, error: { kind: 'LocalEnvironment', message: value.message, uncertain: false } } : { ok: true, generation: 1, value }; }
    if (request.op === 'environments') return { ok: true, generation: 0, value: { saved: this.saved } };
    if (request.op === 'desktopSettingsSet') {
      await this.settingsWrite();
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
  test('a saved focus leaves the primary to the fleet, connected with the memory bearer; a saved duplicate is removed', async () => {
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
    expect(await dropPrimaryDuplicates(native, [duplicate], new LocalPrimary())).toEqual({ origins: [], focusDropped: false });
  });

  test('a duplicate\'s pull request sharing trust goes first, as installPlatformRegistration; a failed write keeps the entry', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const duplicate = { origin: 'http://192.168.1.20:3773', environmentId: 'env-local', label: 'Paired before', enabled: true };
    const trust = { githubRouting: { [JSON.stringify(['env-local', 'http://192.168.1.20:3773/'])]: 'read', [JSON.stringify(['env-box', 'https://box.example.com/'])]: 'read-write',
      [JSON.stringify([JSON.stringify(['BearerConnectionTarget', 'env-local', 'http://a']), JSON.stringify(['SshConnectionTarget', 'env-local', 'h', 'h', 'u', 22])])]: 'read' }, loadBalancingEnabled: true };
    let stored = JSON.stringify(trust), writable = false;
    const native = new Fake([duplicate], {
      connectionPreferences: () => ({ text: stored }),
      setConnectionPreferences: request => writable ? (stored = String(request.text), {}) : new Error('Could not save.'),
      forgetEnvironment: () => ({ state: 'connected', forgotFocus: false }),
    });
    expect(await dropPrimaryDuplicates(native, [duplicate])).toEqual({ origins: [], focusDropped: false });
    expect(native.calls.some(call => call.op === 'forgetEnvironment')).toBe(false);
    writable = true;
    expect(await dropPrimaryDuplicates(native, [duplicate])).toEqual({ origins: ['http://192.168.1.20:3773'], focusDropped: false });
    expect(JSON.parse(stored)).toEqual({ githubRouting: { [JSON.stringify(['env-box', 'https://box.example.com/'])]: 'read-write' }, loadBalancingEnabled: true });
    expect(native.calls.filter(call => call.op === 'forgetEnvironment')).toEqual([{ op: 'forgetEnvironment', origin: 'http://192.168.1.20:3773', environmentId: 'env-local' }]);
  });

  test('a window focused on a duplicate moves to the primary when it is dropped (the reference replaces the entry in place)', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const duplicate = { origin: 'http://192.168.1.20:3773', environmentId: 'env-local', label: 'Paired before', enabled: true };
    const native = new Fake([duplicate], { forgetEnvironment: () => ({ state: 'disconnected', origin: '', environmentId: '', forgotFocus: true }) });
    const source = new EnvironmentFleet();
    // A launch that opened the duplicate by its remembered origin before the primary named itself.
    const client = { origin: duplicate.origin, environmentId: 'env-local', connection: 'connected' };
    await reconnectOnLaunch(client, native, { state: 'connected', origin: duplicate.origin, environmentId: 'env-local' });
    await source.sync(native, client);
    expect(source.takeFocusDropped()).toBe(true);
    expect(source.takeFocusDropped()).toBe(false);
    primaryTakesFocus(client);
    // The next refresh sees the focus disconnected (Swift's forget) and connects the primary.
    Object.assign(client, { origin: '', environmentId: '', connection: 'disconnected' });
    expect(await reconnectOnLaunch(client, native, { state: 'disconnected' })).toBe(true);
    expect(native.calls.filter(call => call.op === 'connect' && !call.fleet)).toEqual([{ op: 'connect', origin: 'http://127.0.0.1:16437', primary: true }]);
  });

  test('pairing this machine\'s own server saves nothing: the primary id goes with the pairing and no route is placed', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const client = switchClient(true);
    const native = new Fake([], { pairEnvironment: request => ({ origin: String(request.origin), environmentId: 'env-local', label: 'Lane Mac', primary: request.primaryEnvironmentId === 'env-local' }) });
    await runConnectionOp(native, 'environment-add', 'http://192.168.1.20:16437', 'PAIRCODE', true, client);
    expect(native.calls.find(call => call.op === 'pairEnvironment')).toMatchObject({ origin: 'http://192.168.1.20:16437', primaryEnvironmentId: 'env-local' });
    // No placeRoute (it would list the saved environments before the fleet's own listing).
    expect(native.calls.filter(call => call.op === 'environments')).toHaveLength(1);
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Backend added', description: 'The environment is saved and will reconnect on app startup.' });
  });
});

// ── The switch and the section ─────────────────────────────────────────────
function switchClient(focusPrimary: boolean): T3Client {
  const client = new T3Client();
  Object.assign(client, focusPrimary ? { origin: 'http://127.0.0.1:16437', environmentId: 'env-local', connection: 'connected' } : { origin: '', environmentId: '', connection: 'disconnected' });
  return client;
}

describe('the Local environment switch persists before requesting app relaunch', () => {
  test.each([{ enabled: false }, { enabled: true }])('Local enabled=$enabled requests relaunch only after a changed successful write', async ({ enabled }) => {
    const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
    for (const outcome of ['changed', 'unchanged', 'rejected']) {
      const before = outcome === 'unchanged' ? enabled : !enabled;
      primary.update(ready({ enabled: before, desktopSettings: { localEnvironmentEnabled: before } }), before);
      const client = switchClient(before), native = new Fake([remote]);
      client.localBackend = primary.status;
      native.settings.localEnvironmentEnabled = before;
      if (outcome === 'rejected') native.settingsFailure = 'Desktop settings write failed.';
      let release = () => {}, entered = () => {};
      const writing = new Promise<void>(resolve => { entered = resolve; });
      const persisted = new Promise<void>(resolve => { release = resolve; });
      native.settingsWrite = async () => { entered(); await persisted; };
      const completed: Awaited<ReturnType<T3Client['command']>>[] = [];
      const command = client.command('local-environment', '', enabled ? 'on' : 'off', 0, native, files).then(result => { completed.push(result); return result; });
      await writing;
      expect(completed).toEqual([]);
      expect(native.settings.localEnvironmentEnabled).toBe(before);
      expect(native.calls.filter(call => call.op === 'desktopSettingsSet')).toEqual([{ op: 'desktopSettingsSet', localEnvironmentEnabled: enabled }]);
      release();
      const result = await command;
      expect(result.relaunch).toBe(outcome === 'changed');
      expect(completed.filter(result => result.relaunch)).toHaveLength(outcome === 'changed' ? 1 : 0);
      expect(native.settings.localEnvironmentEnabled).toBe(outcome === 'rejected' ? before : enabled);
      expect(native.calls.filter(call => ['disconnect', 'connect', 'fleetStop', 'localBackendSetEnabled', 'localBackendRestart'].includes(String(call.op)))).toEqual([]);
      expect(primary.disabled).toBe(!before); // This process keeps its running server until orderly quit.
      if (outcome === 'rejected') expect(thisMachine(undefined, null).error).toBe('Desktop settings write failed.');
      expect(client.local).not.toHaveProperty('localEnvironmentEnabled');
    }
  });

  test.each([{ outcome: 'changed' }, { outcome: 'unchanged' }, { outcome: 'rejected' }])('a superseded Local write keeps only its accepted relaunch request: $outcome', async ({ outcome }) => {
    const before = outcome !== 'unchanged';
    primary.update(ready({ enabled: before, desktopSettings: { localEnvironmentEnabled: before } }), before);
    const client = switchClient(before), native = new Fake();
    client.localBackend = primary.status;
    native.settings.localEnvironmentEnabled = before;
    if (outcome === 'rejected') native.settingsFailure = 'Desktop settings write failed.';
    let release = () => {}, entered = () => {}, preferenceWrites = 0;
    const writing = new Promise<void>(resolve => { entered = resolve; });
    const persisted = new Promise<void>(resolve => { release = resolve; });
    native.settingsWrite = async () => { entered(); await persisted; };
    const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() { preferenceWrites++; } } };
    const completed: Awaited<ReturnType<T3Client['command']>>[] = [];
    const local = client.command('local-environment', '', 'off', 0, native, files).then(result => { completed.push(result); return result; });
    await writing;
    expect(completed).toEqual([]);
    const newer = await client.command('endpoint-default', '', 'tailscale:ip:http', 0, native, files);
    expect(newer.relaunch).toBe(false);
    client.error = 'The newer command owns this message.';
    release();
    const accepted = await local;
    expect(accepted.relaunch).toBe(outcome === 'changed');
    expect([accepted, newer].filter(result => result.relaunch)).toHaveLength(outcome === 'changed' ? 1 : 0);
    expect(native.settings.localEnvironmentEnabled).toBe(outcome === 'rejected' ? before : false);
    expect(client.localBackend.settings.localEnvironmentEnabled).toBe(before); // A stale handler cannot adopt its reply.
    expect(client.error).toBe('The newer command owns this message.');
    expect(preferenceWrites).toBe(1); // Only the current command runs its follow-up preference save.
    expect(native.calls.filter(call => call.op === 'desktopSettingsSet')).toHaveLength(1);
    expect(native.calls.filter(call => ['disconnect', 'connect', 'fleetStop', 'localBackendSetEnabled', 'localBackendRestart'].includes(String(call.op)))).toEqual([]);
  });

  test('a stale connection reply is still refused without a relaunch request or state adoption', async () => {
    const client = switchClient(false);
    let release = () => {}, entered = () => {};
    const connecting = new Promise<void>(resolve => { entered = resolve; });
    const connected = new Promise<void>(resolve => { release = resolve; });
    const native = new Fake([], { connect: async () => { entered(); await connected; return { state: 'connected', origin: remote.origin, environmentId: remote.environmentId, message: '' }; } });
    const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
    const old = client.command('connect', remote.origin, '', 0, native, files);
    await connecting;
    const newer = await client.command('endpoint-default', '', 'tailscale:ip:http', 0, native, files);
    client.error = 'The newer command owns this message.';
    release();
    expect((await old).relaunch).toBe(false);
    expect(newer.relaunch).toBe(false);
    expect([client.environmentId, client.connection, client.error]).toEqual(['', 'disconnected', 'The newer command owns this message.']);
  });

  test('route correlation belongs only to a keyboard move request, including a refused command', async () => {
    const client = switchClient(false);
    const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {} } };
    expect((await client.command('environment-route-move', 'env', 'route-a>route-b\nroute-a,route-b', 42, null, files)).routeReorder).toBe(42);
    expect((await client.command('environment-route-move', 'env', 'route-a>route-b', 42, null, files)).routeReorder).toBe(0);
    expect((await client.command('endpoint-default', '', 'line1\nline2', 42, null, files)).routeReorder).toBe(0);
  });

  test.each(['preferences', 'snapshot cleanup'])('an accepted desktop write keeps its relaunch request when later %s fails', async failedStep => {
    primaryAt('http://127.0.0.1:16437', 'env-local');
    const client = switchClient(true), native = new Fake();
    client.localBackend = primary.status;
    let preferenceWrites = 0;
    const files: Files = { fs: { async mkdir() {}, async readFile() { throw new Error('missing'); }, async atomicWriteFile() {
      preferenceWrites++;
      if (failedStep === 'preferences') throw new Error('preferences disk full');
    } } };
    if (failedStep === 'snapshot cleanup') client.flushSnapshotReleases = async () => { throw new Error('snapshot cleanup failed'); };
    const result = await client.command('local-environment', '', 'off', 0, native, files);
    expect(result.relaunch).toBe(true);
    expect(native.settings.localEnvironmentEnabled).toBe(false);
    expect(native.calls.filter(call => call.op === 'desktopSettingsSet')).toHaveLength(1);
    expect(preferenceWrites).toBe(1);
    if (failedStep === 'preferences') expect(client.error).toBe('Could not save local drafts and preferences. Keep a copy before closing.');
    else expect(result.message).toBe('snapshot cleanup failed');
    // A repeated confirmation is a no-op and must not manufacture another request.
    expect((await client.command('local-environment', '', 'off', 0, native, files)).relaunch).toBe(false);
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

  test('the section holds still while persistence is pending', async () => {
    primaryAt('http://127.0.0.1:16437', 'env-local', 'Lane Mac');
    const host = { connection: 'connected', origin: 'http://127.0.0.1:16437', environmentId: 'env-local', statusMessage: '', scopes: [], config: { environment: { label: 'Lane Mac' } } };
    expect(connectionsProjection(host, []).thisMachine).toMatchObject({ title: 'Lane Mac', enabled: true });
    let release = () => {}, entered = () => {};
    const writing = new Promise<void>(resolve => { entered = resolve; });
    const persisted = new Promise<void>(resolve => { release = resolve; });
    const native = new Fake();
    native.settingsWrite = async () => { entered(); await persisted; };
    const client = switchClient(true);
    const change = applyLocalSetting(client, native, { localEnvironmentEnabled: false });
    await writing;
    primary.update(parseLocalBackendStatus({ state: 'stopped', enabled: false }), false); // An independent status refresh while persistence is pending.
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
      config: { environment: { label: 'Lane Mac', serverVersion: '0.1.4-nightly.20261027.3770', platform: { machine: 'laptop' }, capabilities: { environmentIcon: true } } } };
    const page = connectionsProjection(host, []);
    expect(page.thisMachine).toMatchObject({ title: 'Lane Mac', kind: 'laptop', menu: true, version: '0.1.4-nightly.20261027.3770 · http://127.0.0.1:16437/', upToDate: true, update: '', updateNote: '' });
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
    expect(ids('environment icon')).toContain('environment-icon'); // upstream's result has no condition (settingsSearch.ts)
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
    // The local server runs (WelcomeWizard's localAvailable): "Add a computer" starts closed and Continue takes the focus.
    expect(view.pairStartsOpen).toBe(false);
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
