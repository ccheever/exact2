// Lane r3-settings: outdated servers can be updated even when the client can't connect
// (upstream 22e9d35613). The fixture servers speak protocol 2, so these drive the
// TypeScript half against T3Fleet's reply shapes; T3Fleet itself is covered by
// apple/tests/fleet (a real local WebSocket).
import { describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';
import { CLIENT_VERSION, connectionsPage, runConnectionOp, type ConnectionHost } from './connections';
import { announceJobs, compatibility, outdatedRow, readJobs, setJobs, stageLabel } from './settings-b-outdated';
import { toasts } from './toast';

const OLD = 'https://old.example.com', KEY = `${OLD}\nenv-old`;
const descriptor = (extra: Obj = {}): Obj => ({ environmentId: 'env-old', label: 'Old box', serverVersion: '0.0.30', orchestrationProtocolVersion: 1, capabilities: { serverSelfUpdate: 'respawn' }, ...extra });
const host = (): ConnectionHost => ({ connection: 'connected', origin: 'http://127.0.0.1:14796', environmentId: 'env-a', statusMessage: 'Connected.', scopes: ['orchestration:read', 'orchestration:operate'],
  config: { environment: { label: 'Fixture A', serverVersion: CLIENT_VERSION, capabilities: {} } } });

class FakeNative implements Native {
  available = true; calls: Obj[] = []; jobs: Record<string, Obj> = {}; probe: Obj = descriptor(); pairError = '';
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const ok = (value: unknown) => ({ ok: true, generation: 4, value });
    switch (request.op) {
      case 'environments': return ok({ saved: [{ origin: 'http://127.0.0.1:14796', environmentId: 'env-a', label: 'Fixture A', enabled: true },
        { origin: OLD, environmentId: 'env-old', label: 'Old box', enabled: false }] });
      case 'connectionPreferences': return ok({ text: '{}' });
      case 'status': return ok({ state: 'connected', origin: 'http://127.0.0.1:14796', environmentId: 'env-a' });
      case 'sshTargets': return ok({ targets: {} });
      case 'fleetOutdatedProbe': return ok(this.probe);
      case 'fleetOutdatedJobs': return ok({ jobs: this.jobs });
      case 'fleetOutdatedUpdate':
        this.jobs[`${OLD}\nenv-old`] = { status: 'running', stage: 'downloading', fromVersion: request.fromVersion, targetVersion: request.targetVersion, message: '', resultVersion: '', label: request.label };
        return ok({ started: true });
      case 'fleetOutdatedAck': delete this.jobs[String(request.fleet)]; return ok({ jobs: this.jobs });
      case 'pairEnvironment': return { ok: false, generation: 0, error: { kind: 'Protocol', message: 'This app requires T3 orchestration protocol 2.', uncertain: false } };
      case 'fleetOutdatedPair': return this.pairError ? { ok: false, generation: 0, error: { kind: 'Protocol', message: this.pairError, uncertain: false } }
        : ok({ origin: OLD, environmentId: 'env-old', label: 'Old box', serverUpdateRequired: true });
      default: return ok({});
    }
  }
}

describe('protocol compatibility by direction', () => {
  test('older servers name the host to update; newer ones ask for a client update', () => {
    expect(compatibility(descriptor({ orchestrationProtocolVersion: 2 }), 'Old box')).toBeNull();
    expect(compatibility(descriptor(), 'Old box')).toEqual({ message: 'This client requires a newer server. Update T3 Code on Old box to connect.', serverUpdateRequired: true });
    expect(compatibility(descriptor({ orchestrationProtocolVersion: undefined, capabilities: {} }), 'Old box')!.serverUpdateRequired).toBe(false);
    expect(compatibility(descriptor({ capabilities: { serverSelfUpdate: 'desktop-managed' } }), 'Old box')!.serverUpdateRequired).toBe(false);
    expect(compatibility(descriptor({ capabilities: { serverSelfUpdate: 'desktop-managed', desktopAppUpdate: true } }), 'Old box')!.serverUpdateRequired).toBe(true);
    expect(compatibility(descriptor({ orchestrationProtocolVersion: 3 }), 'Old box')).toEqual({ message: 'This client is not supported by this server. Update your app or use a compatible release to connect to Old box.', serverUpdateRequired: false });
  });
  test('the row reads Update, then progress, then Retry update after a failure', () => {
    expect(outdatedRow(KEY, 'Old box', descriptor(), undefined)).toMatchObject({ action: 'Update', progress: '', failure: '', fromVersion: '0.0.30' });
    const running = { status: 'running', stage: 'installing', fromVersion: '0.0.30', targetVersion: CLIENT_VERSION, message: '', resultVersion: '', label: 'Old box' };
    expect(outdatedRow(KEY, 'Old box', descriptor(), running)).toMatchObject({ action: '', progress: 'Downloading…', resuming: false });
    expect(outdatedRow(KEY, 'Old box', descriptor(), { ...running, stage: 'resuming' })).toMatchObject({ progress: 'Restarting…', resuming: true });
    expect(outdatedRow(KEY, 'Old box', descriptor(), { ...running, status: 'failed', message: 'Old box did not come back on a compatible T3 Code version.' }))
      .toMatchObject({ action: 'Retry update', failure: 'Old box did not come back on a compatible T3 Code version.' });
    expect(outdatedRow(KEY, 'Old box', descriptor({ capabilities: {} }), undefined).action).toBe('');
    expect(stageLabel('downloading')).toBe('Downloading…');
  });
});

describe('Connections: an outdated saved host', () => {
  test('a switched-off host is probed; it reads Client not supported with Update before its switch', async () => {
    const native = new FakeNative();
    setJobs({});
    const page = await connectionsPage(host(), native, true);
    const row = page.environments.find(entry => entry.environmentId === 'env-old')!;
    expect(row).toMatchObject({ unsupported: true, enabled: false, outdatedAction: 'Update', subtitle: 'https://old.example.com/ · Client not supported', switchTip: 'Client not supported',
      tooltip: 'This client requires a newer server. Update T3 Code on Old box to connect.' });
    expect(native.calls.some(call => call.op === 'fleetOutdatedProbe' && call.fleet === KEY)).toBe(true);
  });
  test('Update starts one background job toward this client release and the row follows it', async () => {
    const native = new FakeNative();
    setJobs({});
    await connectionsPage(host(), native, true);
    await runConnectionOp(native, 'environment-update-outdated', KEY, 'Old box', true);
    const start = native.calls.find(call => call.op === 'fleetOutdatedUpdate')!;
    expect(start).toMatchObject({ fleet: KEY, label: 'Old box', targetVersion: CLIENT_VERSION, fromVersion: '0.0.30' });
    native.jobs[KEY] = { ...native.jobs[KEY], stage: 'resuming' };
    const row = (await connectionsPage(host(), native, true)).environments.find(entry => entry.environmentId === 'env-old')!;
    expect(row).toMatchObject({ outdatedAction: '', progress: 'Restarting…', subtitle: 'https://old.example.com/ · Restarting' });
  });
  test('finished jobs toast once: success acknowledges, failure leaves Retry update', async () => {
    const native = new FakeNative();
    const client = { local: {} } as unknown as T3Client;
    setJobs({});
    native.jobs[KEY] = { status: 'done', stage: 'resuming', fromVersion: '0.0.30', targetVersion: CLIENT_VERSION, message: '', resultVersion: CLIENT_VERSION, label: 'Old box' };
    await announceJobs(native, client);
    expect(native.calls.length).toBe(0); // no job started in this process: the fleet pass never asks
    await readJobs(native);
    await announceJobs(native, client);
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['success', 'Old box server updated', `Reconnected on t3@${CLIENT_VERSION}.`]]);
    expect(native.jobs[KEY]).toBeUndefined();
    native.jobs[KEY] = { status: 'failed', stage: 'downloading', fromVersion: '0.0.30', targetVersion: CLIENT_VERSION, message: 'Server update failed: npm exited 1', resultVersion: '', label: 'Old box' };
    await announceJobs(native, client);
    await announceJobs(native, client);
    expect(toasts(client).filter(toast => toast.kind === 'error').map(toast => [toast.title, toast.description])).toEqual([['Server update failed', 'Server update failed: npm exited 1']]);
  });
  test('pairing an outdated self-updating host saves it; any other mismatch keeps the direction message', async () => {
    const native = new FakeNative();
    const client = { local: {} } as unknown as T3Client;
    await runConnectionOp(native, 'environment-add', '', `${OLD}/pair#token=one-time`, true, client);
    expect(native.calls.map(call => call.op)).toContain('fleetOutdatedPair');
    expect(native.calls.find(call => call.op === 'fleetOutdatedPair')).toMatchObject({ fleet: `${OLD}\n` });
    expect(toasts(client).at(-1)?.title).toBe('Backend added');
    native.pairError = 'This client is not supported by this server. Update your app or use a compatible release to connect to Old box.';
    await expect(runConnectionOp(native, 'environment-add', '', `${OLD}/pair#token=one-time`, true, client)).rejects.toThrow('Update your app or use a compatible release');
  });
});
