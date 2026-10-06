import { test, expect } from 'bun:test';
import { connectionsProjection, connectionsPage, environmentStatus, runConnectionOp, savedStatus, statusText, versionMismatch, compareSemver,
  machineKind, decodePrefs, loadPreference, summarizeLoad, summarizeRouting, routingKey, type ConnectionHost } from './connections';
import { fleet, phaseOf, isLoopback, environmentKey, type FleetEntry } from './settings-b-fleet';
import { initialShell } from './domain';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';

const host = (connection = 'connected', extra: Partial<ConnectionHost> = {}): ConnectionHost => ({
  connection, origin: 'http://127.0.0.1:14796', environmentId: 'env-a', statusMessage: 'Connected.', scopes: ['orchestration:read', 'orchestration:operate'],
  config: { environment: { label: 'Fixture A', platform: { machine: 'laptop' }, serverVersion: '0.0.46', capabilities: { environmentIcon: true } } }, ...extra,
});
const entry = (key: string, extra: Partial<FleetEntry> = {}): FleetEntry => ({ key, origin: key.split('\n')[0]!, environmentId: key.split('\n')[1]!, phase: 'connected', message: '', traceId: '',
  generation: 1, synchronized: 1, lastEvent: 0, subscriptions: {}, config: {}, shell: initialShell(), scopes: [], error: '', requested: true, ...extra });

class FakeTransport implements Native {
  available = true; calls: Obj[] = []; saved: Obj[] = []; prefs = '{}';
  generation = 4; copied: string[] = [];
  watch() {}
  async later(input: unknown): Promise<unknown> {
    const request = obj(input); this.calls.push(request);
    const ok = (value: unknown) => ({ ok: true, generation: this.generation, value });
    if (request.fleet) return ok(request.op === 'status' ? { state: 'disconnected', origin: '', environmentId: '', message: '' } : {});
    if (request.op === 'environments') return ok({ saved: this.saved });
    if (request.op === 'status') return ok({ state: 'connected', origin: 'http://127.0.0.1:14796', environmentId: 'env-a', message: '', failureKind: '', traceId: '' });
    if (request.op === 'connectionPreferences') return ok({ text: this.prefs });
    if (request.op === 'setConnectionPreferences') { this.prefs = String(request.text); return ok({ text: this.prefs }); }
    if (request.op === 'copyText') { this.copied.push(String(request.text)); return ok({ copied: true }); }
    if (request.op === 'setEnvironmentEnabled') {
      const found = this.saved.find(saved => saved.environmentId === request.environmentId);
      if (!found) return { ok: false, generation: this.generation, error: { kind: 'Missing', message: 'That environment is no longer saved on this device.', uncertain: false } };
      found.enabled = request.enabled; return ok({ saved: this.saved });
    }
    if (request.op === 'pairEnvironment') {
      if (request.credential === 'invalid-fixture-code') return { ok: false, generation: this.generation, error: { kind: 'Authentication', message: 'The environment credential is invalid.', uncertain: false } };
      this.saved.push({ origin: request.origin, environmentId: 'env-b', label: 'Fixture B', machine: 'laptop', enabled: true });
      return ok({ origin: request.origin, environmentId: 'env-b', label: 'Fixture B' });
    }
    if (request.op === 'connect') { this.generation++; return ok({ state: 'connected', origin: request.origin, environmentId: String(request.origin).endsWith('14796') ? 'env-a' : 'env-b', message: 'Connected.' }); }
    if (request.op === 'disconnect') { this.generation++; return ok({ state: 'disconnected', origin: 'http://127.0.0.1:14796', environmentId: 'env-a', message: 'Disconnected.' }); }
    if (request.op === 'forgetEnvironment') { this.saved = this.saved.filter(entry => entry.environmentId !== request.environmentId); return ok({ state: 'connected', origin: 'http://127.0.0.1:14796', environmentId: 'env-a', message: '' }); }
    return { ok: false, generation: this.generation, error: { kind: 'Arguments', message: 'Unknown native operation.', uncertain: false } };
  }
}

test('adding a route requests the standard scopes while checking the existing environment', async () => {
  const native = new FakeTransport();
  await runConnectionOp(native, 'environment-route-add', 'env-a https://route.example.com', 'ROUTECODE', false);
  expect(native.calls.find(call => call.op === 'pairEnvironment')).toEqual({
    op: 'pairEnvironment', origin: 'https://route.example.com', credential: 'ROUTECODE', expectedEnvironmentId: 'env-a',
    scope: 'orchestration:read orchestration:operate terminal:operate review:write relay:read',
  });
});

test('status copy follows savedBackendStatus and connectionStatusText', () => {
  expect(savedStatus(true, 'connected', '')).toEqual({ text: 'Connected', tone: 'muted' });
  expect(savedStatus(false, 'connected', '')).toEqual({ text: 'Off', tone: 'muted' });
  expect(savedStatus(false, 'unsupported', '')).toEqual({ text: 'Client not supported', tone: 'muted' });
  expect(savedStatus(true, 'available', '')).toEqual({ text: 'Not connected', tone: 'muted' });
  expect(environmentStatus(true, 'reconnecting', 'The server connection closed (1006). Reconnecting in 4s…')).toEqual({ text: 'Reconnecting: The server connection closed (1006).', tone: 'error' });
  expect(environmentStatus(true, 'error', 'The saved session expired. Pair with the server again.')).toEqual({ text: 'Connection failed: The saved session expired. Pair with the server again.', tone: 'error' });
  expect(statusText('reconnecting', 'down')).toBe('Failed to connect. Reconnecting... Reason: down');
  expect(statusText('error', '')).toBe('Connection failed');
  expect(phaseOf({ state: 'error', failureKind: 'Protocol', message: 'This app requires T3 orchestration protocol 2.' }).phase).toBe('unsupported');
  expect(phaseOf({ state: 'reconnecting', message: 'Down. Reconnecting in 8s…', traceId: 't-1' })).toEqual({ phase: 'reconnecting', message: 'Down.', traceId: 't-1' });
});

test('version skew follows versionSkew.ts: only an older server needs an update', () => {
  expect(versionMismatch('0.0.46-nightly.20261003.2610', '0.0.45')).toBeNull();
  expect(versionMismatch('0.0.44', '0.0.45')).toEqual({ serverVersion: '0.0.44', clientVersion: '0.0.45' });
  expect(versionMismatch('0.0.45-nightly.1', '0.0.45')).toBeNull(); // same core: no warning
  expect(versionMismatch('0.0.45-nightly.20261001.1', '0.0.45-nightly.20261003.2')).not.toBeNull();
  expect(compareSemver('1.0.0-alpha', '1.0.0')).toBe(-1);
  expect(compareSemver('1.0.0-alpha.2', '1.0.0-alpha.10')).toBe(-1);
  expect(machineKind({ settings: { environmentIcon: 'cloud' }, environment: { platform: { machine: 'laptop' } } })).toBe('cloud');
  expect(machineKind({ environment: { platform: { machine: 'toaster' } } })).toBe('server');
  expect(machineKind({}, 'desktop')).toBe('desktop');
});

test('every paired environment, a loopback one included, is a saved row under Environments in catalog order', () => {
  expect(isLoopback('http://127.0.0.1:3773')).toBe(true);
  expect(isLoopback('http://localhost:3773/')).toBe(true);
  expect(isLoopback('https://devbox.example.com')).toBe(false);
  // r9-connect: this client bundles no server, so a paired loopback server is not "This machine".
  const one = connectionsProjection(host(), [{ origin: 'http://127.0.0.1:14796', environmentId: 'env-a', label: 'Fixture A', enabled: true }]);
  expect(one.environments.map(row => [row.label, row.subtitle, row.active, row.first])).toEqual([['Fixture A', 'http://127.0.0.1:14796/ · Connected', true, true]]);
  expect(one.machines).toEqual([]);
  expect('thisMachine' in one).toBe(false);
  const saved = [{ origin: 'http://127.0.0.1:14796', environmentId: 'env-a', label: 'Fixture A', enabled: true },
    { origin: 'https://remote.example.com', environmentId: 'env-b', label: 'Remote', machine: 'desktop', enabled: true },
    { origin: 'https://off.example.com', environmentId: 'env-c', label: 'Off box', machine: 'cloud', enabled: false }];
  const entries = new Map([[environmentKey('https://remote.example.com', 'env-b'), entry(environmentKey('https://remote.example.com', 'env-b'),
    { config: { environment: { label: 'Remote', serverVersion: '0.0.40', platform: { machine: 'desktop' }, capabilities: { environmentIcon: true } } } })]]);
  const page = connectionsProjection(host(), saved, entries);
  expect(page.environments.map(row => [row.label, row.subtitle, row.enabled, row.dimmed, row.first, row.kind])).toEqual([
    ['Fixture A', 'http://127.0.0.1:14796/ · Connected', true, false, true, 'laptop'],
    ['Remote', 'https://remote.example.com/ · Connected · 0.0.40', true, false, false, 'desktop'],
    ['Off box', 'https://off.example.com/ · Off', false, true, false, 'cloud']]);
  expect(page.environments[1]).toMatchObject({ tooltip: 'Connected\nUpdate available: 0.0.40 → 0.0.46-nightly.20261004.1', switchTip: 'Switch off', update: 'Copy relaunch command', updateNote: '', iconLock: '' });
  expect(page.environments[2]).toMatchObject({ tooltip: 'Switched off', switchTip: 'Switch on', update: '', iconLock: 'Connect to this environment to change its icon.' });
  expect(page.environments[1]!.icons.map(icon => [icon.kind, icon.selected, icon.note])).toEqual([['server', false, ''], ['cloud', false, ''], ['linux', false, ''],
    ['desktop', true, 'detected'], ['laptop', false, ''], ['mac-mini', false, ''], ['mac-studio', false, '']]);
  expect(page.machines.map(machine => [machine.label, machine.subtitle, machine.weightLabel, machine.routingLabel])).toEqual([
    ['Fixture A', 'This machine', 'Normal', 'Off'], ['Remote', 'https://remote.example.com/', 'Normal', 'Off']]);
  expect(page).toMatchObject({ loadBalancing: false, loadSummary: 'Off', githubSummary: 'Off', updateCount: 0 });
  expect(JSON.stringify(page)).not.toContain('token');
});

test('failure tone, trace ID and unsupported clients follow the row rules', () => {
  const saved = [{ origin: 'https://remote.example.com', environmentId: 'env-b', label: 'Remote', enabled: true }];
  const key = environmentKey('https://remote.example.com', 'env-b');
  const failing = connectionsProjection(host(), saved, new Map([[key, entry(key, { phase: 'error', message: 'Session expired.', traceId: 'trace-9' })]]));
  expect(failing.environments.map(row => row.environmentId)).toEqual(['env-a', 'env-b']); // the connected focus is listed too
  expect(failing.environments[1]).toMatchObject({ errorTone: true, traceId: 'trace-9', subtitle: 'https://remote.example.com/ · Connection failed: Session expired.', tooltip: 'Connection failed. Reason: Session expired.' });
  const unsupported = connectionsProjection(host(), saved, new Map([[key, entry(key, { phase: 'unsupported', message: 'This app requires T3 orchestration protocol 2.' })]]));
  expect(unsupported.environments[1]).toMatchObject({ unsupported: true, enabled: false, switchTip: 'Client not supported', errorTone: false,
    tooltip: 'This app requires T3 orchestration protocol 2.', subtitle: 'https://remote.example.com/ · Client not supported' });
  const focused = connectionsProjection(host('error', { origin: 'https://remote.example.com', environmentId: 'env-b', statusMessage: 'The server returned HTTP 401.' }), saved, new Map(), '{}', { traceId: 'trace-f' });
  expect(focused.environments[0]).toMatchObject({ active: true, errorTone: true, traceId: 'trace-f', subtitle: 'https://remote.example.com/ · Connection failed: The server returned HTTP 401.' });
});

test('load balancing and GitHub sharing preferences decode and summarize like the reference', () => {
  expect(loadPreference(undefined)).toBe(50);
  expect(loadPreference(10)).toBe(25);
  expect(loadPreference(70)).toBe(100);
  const prefs = decodePrefs(JSON.stringify({ loadBalancingEnabled: true, loadBalancingWeights: { a: 100, b: 'x' }, githubRouting: { k: 'read', j: 'admin' } }));
  expect(prefs).toEqual({ loadBalancingEnabled: true, loadBalancingWeights: { a: 100 }, githubRouting: { k: 'read' } });
  expect(decodePrefs('not json')).toEqual({ loadBalancingEnabled: false, loadBalancingWeights: {}, githubRouting: {} });
  expect(summarizeLoad([{ environmentId: 'a', label: 'Mac' }, { environmentId: 'b', label: 'Box' }], { a: 100, b: 0 })).toBe('Mac prefer · Box manual only');
  expect(summarizeRouting([{ label: 'Mac', permission: 'read' }, { label: 'Box', permission: 'read-write' }, { label: 'Pi', permission: 'read' }])).toBe('Box read and act · Mac, Pi read PRs');
  const saved = [{ origin: 'http://127.0.0.1:14796', environmentId: 'env-a', label: 'Fixture A' }, { origin: 'https://remote.example.com', environmentId: 'env-b', label: 'Remote' }];
  const page = connectionsProjection(host(), saved, new Map(), JSON.stringify({ loadBalancingEnabled: true, loadBalancingWeights: { 'env-b': 0 },
    githubRouting: { [routingKey('https://remote.example.com', 'env-b')]: 'read-write' } }));
  expect(page).toMatchObject({ loadBalancing: true, loadSummary: 'Remote manual only', githubSummary: 'Remote read and act' });
  expect(page.machines[1]!.weights.find(choice => choice.selected)).toEqual({ value: '0', label: 'Manual only', selected: true });
});

test('adding pairs beside the focused connection, connects when there is none, and toasts', async () => {
  const native = new FakeTransport();
  await expect(runConnectionOp(native, 'environment-add', '', '', true)).rejects.toThrow('Enter a backend host.');
  await expect(runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14806', '', true)).rejects.toThrow('Enter a pairing code.');
  await expect(runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14806', 'invalid-fixture-code', true)).rejects.toThrow('The environment credential is invalid.');
  expect(await runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14806', 'PAIRCODE', true)).toEqual({ status: null, generation: -1 });
  expect(native.calls.filter(call => call.op === 'connect' && !call.fleet)).toHaveLength(0);
  expect(native.calls.find(call => call.op === 'pairEnvironment' && call.credential === 'PAIRCODE')).toEqual({ op: 'pairEnvironment', origin: 'http://127.0.0.1:14806', credential: 'PAIRCODE', scope: 'orchestration:read orchestration:operate terminal:operate review:write relay:read' });
  const result = await runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14806', 'PAIRCODE', false);
  expect(result.status).toMatchObject({ state: 'connected', environmentId: 'env-b' });
  const page = await connectionsPage(host(), native, true);
  expect(page.environments.map(row => row.environmentId)).toEqual(['env-a', 'env-b']); // both paired: both saved rows
  const toasts = (await import('./toast')).toasts;
  const client = { origin: 'http://127.0.0.1:14796', environmentId: 'env-a', connection: 'connected' } as never;
  await runConnectionOp(native, 'environment-add', 'http://127.0.0.1:14806', 'PAIRCODE', true, client);
  expect(toasts(client).map(toast => [toast.kind, toast.title])).toEqual([['success', 'Backend added']]);
  await expect(runConnectionOp(native, 'environment-add', '', '', true, client)).rejects.toThrow();
  expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not add backend', description: 'Enter a backend host.', stacked: true });
  fleet.entries.clear();
});

test('the switch persists per environment and moves focus home when the focused one goes off', async () => {
  const native = new FakeTransport();
  native.saved = [{ origin: 'http://127.0.0.1:14796', environmentId: 'env-a', label: 'This Mac', enabled: true },
    { origin: 'https://remote.example.com', environmentId: 'env-b', label: 'Remote', enabled: true }];
  const remoteKey = environmentKey('https://remote.example.com', 'env-b');
  // Switching a background environment off keeps the focused connection.
  const off = await runConnectionOp(native, 'environment-enabled', remoteKey, 'off', true);
  expect(off.status).toBeNull();
  expect(native.saved[1]!.enabled).toBe(false);
  expect(native.calls.some(call => call.op === 'fleetStop' && call.fleet === remoteKey)).toBe(true);
  // Switching it on with nothing focused connects it as the focused one.
  native.saved[1]!.enabled = false;
  const on = await runConnectionOp(native, 'environment-enabled', remoteKey, 'on', false);
  expect(on.status).toMatchObject({ state: 'connected', environmentId: 'env-b' }); // nothing focused: it becomes the focused one
  // The focused remote switched off: disconnect, then focus returns to this machine.
  const client = { origin: 'https://remote.example.com', environmentId: 'env-b', connection: 'connected' } as never;
  native.calls = [];
  const home = await runConnectionOp(native, 'environment-enabled', remoteKey, 'off', true, client);
  expect(native.calls.filter(call => !call.fleet && ['disconnect', 'connect'].includes(String(call.op))).map(call => [call.op, call.origin ?? ''])).toEqual([['disconnect', ''], ['connect', 'http://127.0.0.1:14796']]);
  expect(home.status).toMatchObject({ environmentId: 'env-a' });
  await expect(runConnectionOp(native, 'environment-enabled', environmentKey('https://gone.example.com', 'env-z'), 'on', true)).rejects.toThrow('no longer saved');
  await runConnectionOp(native, 'environment-forget', 'https://remote.example.com', 'env-b', true);
  expect(native.calls.at(-1)).toEqual({ op: 'fleetStop', fleet: remoteKey });
  expect(native.saved.map(saved => saved.environmentId)).toEqual(['env-a']);
  await expect(runConnectionOp(native, 'environment-rename', '', '', true)).rejects.toThrow('Unknown action');
  fleet.entries.clear();
});

test('trace IDs copy, preferences persist on this device, and icon writes need a connection', async () => {
  const native = new FakeTransport();
  await runConnectionOp(native, 'environment-trace', 'k', 'trace-123', true);
  expect(native.copied).toEqual(['trace-123']);
  await expect(runConnectionOp(native, 'environment-trace', 'k', '', true)).rejects.toThrow('That trace ID is unavailable.');
  await runConnectionOp(native, 'load-balancing', '', 'true', true);
  await runConnectionOp(native, 'load-weight', 'env-b', '25', true);
  await runConnectionOp(native, 'github-routing', environmentKey('https://remote.example.com', 'env-b'), 'read', true);
  expect(decodePrefs(native.prefs)).toEqual({ loadBalancingEnabled: true, loadBalancingWeights: { 'env-b': 25 }, githubRouting: { [routingKey('https://remote.example.com', 'env-b')]: 'read' } });
  await runConnectionOp(native, 'github-routing', environmentKey('https://remote.example.com', 'env-b'), 'off', true);
  expect(decodePrefs(native.prefs).githubRouting).toEqual({});
  await expect(runConnectionOp(native, 'load-weight', 'env-b', '42', true)).rejects.toThrow('Choose a load preference.');
  await expect(runConnectionOp(native, 'environment-icon', environmentKey('https://remote.example.com', 'env-b'), 'cloud', true)).rejects.toThrow('Connect to this environment to change its icon.');
  await expect(runConnectionOp(native, 'environment-icon', environmentKey('https://remote.example.com', 'env-b'), 'toaster', true)).rejects.toThrow('Choose one of the listed icons.');
});
