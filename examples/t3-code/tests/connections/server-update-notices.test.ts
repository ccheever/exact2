import { AUTH_STANDARD_CLIENT_SCOPES } from '../../src/connections/remote-scopes';
// The composer's offline and server-version notices, the version card and their ops.
// "environment reconnect warning grace" ports the cases of
// T3 Code's apps/web/src/components/ChatView.logic.test.ts (1e2ecbd975, MIT, see
// LICENSE-T3) with their names; the timer itself is the root task app.contract runs.
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { primaryAt, resetPrimary } from '../support/local-primary-fixture';
afterEach(resetPrimary);
import { obj, type Obj } from '../../src/client/domain';
import type { Files, Native } from '../../src/client/protocol';
import { T3Client } from '../../src/client/client';
import { toasts } from '../../src/shell/toast';
import { fleet } from '../../src/connections/settings-b-fleet';
import { setJobs, type OutdatedJob } from '../../src/connections/settings-b-outdated';
import { composerNotices } from '../../src/composer/composer-controls-view';
import { CLIENT_VERSION } from '../../src/connections/version-skew';
import {
  ENVIRONMENT_DISCONNECT_DELAY_MS, ENVIRONMENT_RECONNECT_WARNING_GRACE_MS, hasEnvironmentReconnectWarningGraceElapsed, noteServerUpdateClock, serverClock,
  serverUpdateView, systemComposerNotices, versionCard,
} from '../../src/connections/server-update-notices';

const environmentId = 'env-remote';
function harness(capabilities: Obj = { serverSelfUpdate: 'boot-service', serverSelfUpdateProgress: true }, origin = 'http://10.0.0.2:3773', serverVersion = '0.0.45') {
  const calls: Obj[] = [];
  let saved = '';
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'fleetOutdatedUpdate') return { ok: true, generation: 1, value: { started: true, attempt: 'n1' } };
    if (request.op === 'setEnvironmentEnabled') return { ok: true, generation: 1, value: { saved: [{ origin, environmentId, enabled: false }] } };
    if (request.op === 'disconnect') return { ok: true, generation: 2, value: { state: 'disconnected', origin, environmentId: '', message: 'Disconnected.' } };
    return { ok: true, generation: 1, value: {} };
  } };
  const storage: Files = { fs: { async mkdir() {}, async readFile() { if (!saved) throw new Error('missing'); return new TextEncoder().encode(saved).buffer; },
    async atomicWriteFile(_path, bytes) { saved = new TextDecoder().decode(bytes); } } };
  const client = new T3Client();
  Object.assign(client, { available: true, generation: 1, connection: 'connected', origin, environmentId, projectId: 'p1', threadId: 't1',
    configLive: true, shellLive: true, threadLive: true, scopes: [...AUTH_STANDARD_CLIENT_SCOPES],
    config: { environment: { environmentId, label: 'Studio', serverVersion, capabilities: { serverResolvedCommandContext: true, ...capabilities } }, settings: {} } });
  client.thread = { projection: { thread: { id: 't1' } }, sequence: 0, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: null };
  const command = (op: string, id = '', value = '') => client.command(op, id, value, 0, native, storage);
  return { client, native, calls, command, saved: () => saved };
}
const job = (patch: Partial<OutdatedJob> = {}): OutdatedJob => ({ status: 'running', stage: 'downloading', fromVersion: '0.0.45', targetVersion: CLIENT_VERSION, message: '',
  resultVersion: '', label: 'server', mode: 'connected', attempt: 'n1', ...patch });
const key = `http://10.0.0.2:3773\n${environmentId}`;

beforeEach(() => { setJobs({}); fleet.saved = []; });

describe('environment reconnect warning grace', () => {
  it('shows a persistent reconnect after the grace period', () => {
    const { client } = harness();
    client.connection = 'reconnecting';
    const episode = serverClock(client).reconnecting;
    expect(ENVIRONMENT_RECONNECT_WARNING_GRACE_MS).toBe(2_000);
    expect(systemComposerNotices(client).map(item => item.title)).not.toContain('Studio is reconnecting');
    noteServerUpdateClock(client, episode, '');
    expect(systemComposerNotices(client).find(item => item.id.startsWith('environment-unavailable'))).toMatchObject({ title: 'Studio is reconnecting', variant: 'warning' });
  });

  it('cancels the warning when the connection recovers during the grace period', () => {
    const { client } = harness();
    client.connection = 'reconnecting';
    const first = serverClock(client).reconnecting;
    client.connection = 'connected';
    expect(serverClock(client).reconnecting).toBe('');
    client.connection = 'reconnecting';
    // The elapsed grace of the earlier outage does not carry to the next one.
    noteServerUpdateClock(client, first, '');
    expect(serverClock(client).reconnecting).not.toBe(first);
    expect(systemComposerNotices(client).some(item => item.id.startsWith('environment-unavailable'))).toBe(false);
  });

  it('does not reuse elapsed grace from another environment', () => {
    expect(hasEnvironmentReconnectWarningGraceElapsed('env#1', 'env#1')).toBe(true);
    expect(hasEnvironmentReconnectWarningGraceElapsed('environment-remote#2', 'env#1')).toBe(false);
  });
});

describe('offline banner', () => {
  it('is offline at once with Reconnect, and offers Disconnect server only after 20 s for a non-primary environment', () => {
    const { client } = harness();
    client.connection = 'error';
    expect(systemComposerNotices(client)[0]).toMatchObject({ title: 'Studio is offline', variant: 'error', action: 'su:reconnect', actionLabel: 'Reconnect' });
    expect(systemComposerNotices(client)[0]!.action2).toBeUndefined();
    expect(ENVIRONMENT_DISCONNECT_DELAY_MS).toBe(20_000);
    noteServerUpdateClock(client, '', serverClock(client).unavailable);
    expect(systemComposerNotices(client)[0]).toMatchObject({ action2: 'su:disconnect', action2Label: 'Disconnect server',
      action2Tip: "Hide this server's threads. Switch it on again in Environments." });
    primaryAt('http://127.0.0.1:3773', environmentId); // this Mac's embedded server cannot be disconnected from here
    const local = harness({}, 'http://127.0.0.1:3773').client;
    local.connection = 'disconnected';
    noteServerUpdateClock(local, '', serverClock(local).unavailable);
    expect(systemComposerNotices(local)[0]).toMatchObject({ title: 'Studio is offline', variant: 'warning' });
    expect(systemComposerNotices(local)[0]!.action2).toBeUndefined();
  });

  it('stays hidden while an update restarts the server, and the details toggle shows the dot', () => {
    const { client } = harness();
    setJobs({ [key]: job({ stage: 'resuming' }) });
    client.connection = 'reconnecting';
    noteServerUpdateClock(client, serverClock(client).reconnecting, '');
    const items = systemComposerNotices(client);
    expect(items.map(item => item.id)).toEqual([`server-version:${environmentId}`]);
    // A static icon: the title says the update runs (ComposerServerUpdateIcon).
    expect(items[0]).toMatchObject({ title: 'Updating server', description: 'Restarting…', icon: 'download', priority: 1 });
    // A plain title and description: the details come from a clipped description, not a tooltip on the title.
    expect(items[0]!.tip).toBeUndefined();
    expect(serverUpdateView(client).attention).toBe(true);
  });

  it('Disconnect server switches the environment off and goes Home', async () => {
    const { client, calls, command } = harness();
    fleet.saved = [{ origin: 'http://10.0.0.2:3773', environmentId, label: 'Studio', enabled: true }];
    client.connection = 'error';
    await command('su:disconnect');
    expect(calls.find(call => call.op === 'setEnvironmentEnabled')).toMatchObject({ environmentId, enabled: false });
    expect(client.threadId).toBe('');
  });
});

describe('server update notice', () => {
  it('shows no notice for an available update: the sidebar\'s update pill offers it', () => {
    const { client } = harness();
    expect(systemComposerNotices(client).some(item => item.id.startsWith('server-version'))).toBe(false);
    expect(versionCard(client)).toMatchObject({ versionServer: '0.0.45', versionClient: CLIENT_VERSION });
  });

  it('names the manual and desktop paths of Retry as ServerUpdateAction does', () => {
    const failed = (capabilities: Obj) => { const { client } = harness(capabilities); setJobs({ [key]: job({ status: 'failed', message: 'Nope.', attempt: 'f1' }) }); return systemComposerNotices(client).at(-1)!; };
    expect(failed({ serverInstallation: { kind: 'npm-global', prefix: '/opt/node' } })).toMatchObject({ actionLabel: 'Copy update command', dismiss: 'su:dismiss' });
    expect(failed({})).toMatchObject({ actionLabel: 'Copy relaunch command' });
    const desktop = failed({ serverSelfUpdate: 'desktop-managed' });
    expect(desktop.action).toBeUndefined();
    expect(desktop).toMatchObject({ dismiss: 'su:dismiss', description: 'Nope.' });
    expect(failed({ serverSelfUpdate: 'desktop-managed', desktopAppUpdate: true })).toMatchObject({ action: 'su:update', actionLabel: 'Retry' });
  });

  it('shows the failure with Retry, dismisses that attempt only, and persists the version dismissal', async () => {
    const { client, command, saved } = harness();
    setJobs({ [key]: job({ status: 'failed', stage: 'installing', message: 'Server update failed: The package could not be verified.', attempt: 'f1' }) });
    expect(systemComposerNotices(client).at(-1)).toMatchObject({ title: 'Could not update server', description: 'Server update failed: The package could not be verified.',
      variant: 'error', icon: 'circle-alert', iconTone: 'error', actionLabel: 'Retry', dismissId: 'f1' });
    await command('su:dismiss', 'f1');
    expect(systemComposerNotices(client).some(item => item.id.startsWith('server-version'))).toBe(false);
    expect(JSON.parse(saved()).shell.versionMismatchDismissals).toEqual([`${environmentId}:${CLIENT_VERSION}:0.0.45`]);
    // A new attempt shows again; the version notice stays dismissed.
    setJobs({ [key]: job({ status: 'failed', message: 'Again.', attempt: 'f2' }) });
    expect(systemComposerNotices(client).at(-1)).toMatchObject({ description: 'Again.' });
    setJobs({});
    expect(systemComposerNotices(client).some(item => item.id.startsWith('server-version'))).toBe(false);
    expect(versionCard(client)).toEqual({ versionClient: '', versionServer: '', versionLabel: '' });
  });

  it('runs before passive notices while updating and starts one job per click', async () => {
    const { client, command, calls } = harness();
    await command('su:update');
    await command('su:update');
    const starts = calls.filter(call => call.op === 'fleetOutdatedUpdate');
    expect(starts).toHaveLength(2); // the native job is single-flight; the second answer is "started: false" there
    expect(starts[0]).toMatchObject({ fleet: key, mode: 'connected', targetVersion: CLIENT_VERSION, fromVersion: '0.0.45', label: 'server' });
    setJobs({ [key]: job({ attempt: 'n1' }) });
    await command('su:update');
    expect(calls.filter(call => call.op === 'fleetOutdatedUpdate')).toHaveLength(2);
    const notices = composerNotices(client, 0);
    expect(notices[0]).toMatchObject({ id: `server-version:${environmentId}`, title: 'Updating server', description: 'Downloading…', action: '', dismiss: '' });
  });

  it('asks before updating a desktop app, then updates on Confirm only', async () => {
    const { client, command, calls } = harness({ serverSelfUpdate: 'desktop-managed', desktopAppUpdate: true });
    await command('su:update');
    expect(serverUpdateView(client)).toMatchObject({ confirmTitle: 'Update the T3 Code desktop app that runs the server?', confirmBody: 'It will close and relaunch on that machine.' });
    expect(calls.some(call => call.op === 'fleetOutdatedUpdate')).toBe(false);
    await command('su:cancel');
    expect(serverUpdateView(client).confirmTitle).toBe('');
    await command('su:update');
    await command('su:confirm');
    expect(calls.filter(call => call.op === 'fleetOutdatedUpdate')).toHaveLength(1);
    expect(serverUpdateView(client).confirmTitle).toBe('');
  });

  it('puts the version-differ card in the details and shares the dismissal', async () => {
    const { client, command } = harness();
    expect(versionCard(client)).toEqual({ versionClient: CLIENT_VERSION, versionServer: '0.0.45', versionLabel: 'server' });
    expect(serverUpdateView(client).attention).toBe(true);
    await command('su:dismiss');
    expect(versionCard(client).versionServer).toBe('');
    expect(serverUpdateView(client).attention).toBe(false);
    expect(toasts(client)).toEqual([]);
  });

  it('shows nothing for a current server', () => {
    const { client } = harness({ serverSelfUpdate: 'respawn' }, 'http://10.0.0.2:3773', CLIENT_VERSION);
    expect(systemComposerNotices(client)).toEqual([]);
    expect(serverUpdateView(client).attention).toBe(false);
  });
});
