// 20261005-this-machine-network-access items 3-5: the This machine rows, the Authorized clients fold
// and their commands, against ConnectionsSettings.tsx (MIT, see LICENSE-T3; reference 1e2ecbd975):
// every state the rows draw (loading, local-only, reachable, Tailscale, empty, error), the pairing and
// client rows, the dialogs' serials (closing on success, or on any outcome for network access), the
// restart through applyLocalSetting with the new envelope, and the copy / reveal path.
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { formatElapsedDurationLabel, formatExpiresInLabel, networkPage, networkProjection, networkUi, resetNetwork, runNetworkOp, NETWORK_OPS, type NetworkInput } from './connections-network';
import { CONNECTION_OPS, connectionsProjection } from './connections';
import { access, createdCredentials, EMPTY_AUTH_ACCESS_SNAPSHOT, type AuthClientSession, type AuthPairingLink } from './auth-access';
import { createAdvertisedEndpoint, type AdvertisedEndpoint } from './advertised-endpoint';
import { primary } from './local-primary';
import { parseLocalBackendStatus } from './local-backend';
import { resetPrimary } from './local-primary-fixture';
import { toasts } from './toast';
import { T3Client } from './client';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';

const core = { id: 'desktop-core', label: 'Desktop', kind: 'core', isAddon: false };
const tailscaleProvider = { id: 'tailscale', label: 'Tailscale', kind: 'private-network', isAddon: true };
const loopback = createAdvertisedEndpoint({ id: 'desktop-loopback:16101', label: 'This machine', provider: core, source: 'desktop-core', httpBaseUrl: 'http://127.0.0.1:16101', reachability: 'loopback' });
const lan = createAdvertisedEndpoint({ id: 'desktop-lan:http://192.168.1.20:16101', label: 'Local network', provider: core, source: 'desktop-core', httpBaseUrl: 'http://192.168.1.20:16101', reachability: 'lan', isDefault: true });
const tailnetIp = createAdvertisedEndpoint({ id: 'tailscale-ip:http://100.90.1.2:16101', label: 'Tailscale IP', provider: tailscaleProvider, source: 'desktop-addon', httpBaseUrl: 'http://100.90.1.2:16101', reachability: 'private-network' });
const magic = (available: boolean): AdvertisedEndpoint => createAdvertisedEndpoint({ id: 'tailscale-magicdns:https://lane.tail.ts.net:8443/', label: 'Tailscale HTTPS', provider: tailscaleProvider,
  source: 'desktop-addon', httpBaseUrl: 'https://lane.tail.ts.net:8443/', reachability: 'private-network', hostedHttpsCompatibility: available ? 'compatible' : 'requires-configuration',
  status: available ? 'available' : 'unavailable' });
const NOW = Date.parse('2036-04-07T00:00:00.000Z');
const link = (id: string, overrides: Partial<AuthPairingLink> = {}): AuthPairingLink => ({ id, scopes: ['orchestration:read', 'orchestration:operate'], subject: 'pairing', label: 'iPad',
  createdAt: '2036-04-06T23:59:00.000Z', expiresAt: '2036-04-07T00:04:12.000Z', ...overrides });
const session = (id: string, overrides: Partial<AuthClientSession> = {}): AuthClientSession => ({ sessionId: id, subject: 'client', scopes: ['orchestration:read'], method: 'browser-session-cookie',
  client: { deviceType: 'mobile', os: 'iOS', browser: 'Safari', ipAddress: '192.168.1.9' }, issuedAt: '2036-04-06T00:00:00.000Z', expiresAt: '2036-05-06T00:00:00.000Z',
  lastConnectedAt: '2036-04-06T23:55:00.000Z', connected: true, current: false, ...overrides });
const networkOn = { mode: 'network-accessible' as const, endpointUrl: 'http://192.168.1.20:16101', advertisedHost: '192.168.1.20', tailscaleServeEnabled: false, tailscaleServePort: 443 };
const input = (overrides: Partial<NetworkInput> = {}): NetworkInput => ({ canManage: true, state: networkOn, endpoints: [loopback, lan, tailnetIp, magic(false)], defaultKey: null,
  access: { loaded: true, error: '', pairingLinks: [], clientSessions: [] }, credentials: new Map(), now: NOW, ...overrides });

beforeEach(() => resetNetwork());
afterEach(() => { resetNetwork(); resetPrimary(); createdCredentials.clear(); Object.assign(access, { environmentId: '', snapshot: EMPTY_AUTH_ACCESS_SNAPSHOT, loaded: false, error: '', wanted: false }); });

describe('This machine › Network access and Tailscale HTTPS rows', () => {
  it('says Loading… before the first read, and nothing without access:write', () => {
    expect(networkProjection(input({ canManage: false })).visible).toBe(false);
    const loading = networkProjection(input({ state: null, endpoints: [] }));
    expect([loading.visible, loading.fallback, loading.switchReady, loading.clientsVisible]).toEqual([true, 'Loading…', false, false]);
  });

  it('is limited to this machine while local-only, and never lists endpoints or clients then', () => {
    const view = networkProjection(input({ state: { ...networkOn, mode: 'local-only', endpointUrl: null, advertisedHost: null }, endpoints: [loopback] }));
    expect([view.networkOn, view.fallback, view.summaryUrl, view.endpoints, view.clientsVisible]).toEqual([false, 'Limited to this machine.', '', [], false]);
    expect(view.tailscaleDescription).toBe('Start Tailscale to set up HTTPS access through MagicDNS.');
    expect(view.tailscale).toBe(false);
  });

  it('is reachable at the default endpoint with +N for the others, and the rail marks the default', () => {
    const view = networkProjection(input());
    expect([view.summaryUrl, view.hidden, view.clientsVisible]).toEqual(['http://192.168.1.20:16101/', 2, true]);
    expect(view.endpoints.map(row => [row.label, row.isDefault, row.canSetDefault, row.available, row.needsSetup])).toEqual([
      ['This machine', false, true, true, false], ['Local network', true, false, true, false], ['Tailscale IP', false, true, true, false]]);
    // Tailscale HTTPS stays out of the rail; its row asks for setup.
    expect([view.tailscale, view.tailscaleOn, view.tailscaleDescription, view.tailscaleHost, view.tailscaleUrl])
      .toEqual([true, false, 'Use Tailscale Serve to expose this backend through a MagicDNS HTTPS URL.', 'lane.tail.ts.net', 'https://lane.tail.ts.net:8443/']);
    // A chosen default wins (t3-code.json defaultAdvertisedEndpointKey).
    const chosen = networkProjection(input({ defaultKey: 'tailscale:ip:http' }));
    expect([chosen.summaryUrl, chosen.endpoints.find(row => row.isDefault)?.label]).toEqual(['http://100.90.1.2:16101/', 'Tailscale IP']);
  });

  it('falls back to the exposure text without a network endpoint', () => {
    expect(networkProjection(input({ endpoints: [loopback] })).fallback).toBe('Reachable at http://192.168.1.20:16101');
    expect(networkProjection(input({ endpoints: [loopback], state: { ...networkOn, endpointUrl: null } })).fallback).toBe('Exposed on all interfaces. Pairing links use 192.168.1.20.');
    expect(networkProjection(input({ endpoints: [loopback], state: { ...networkOn, endpointUrl: null, advertisedHost: null } })).fallback).toBe('Exposed on all interfaces.');
  });

  it('shows Tailscale HTTPS as on with its URL once Serve answers, which alone makes the clients reachable', () => {
    const view = networkProjection(input({ state: { ...networkOn, mode: 'local-only', endpointUrl: null, advertisedHost: null, tailscaleServeEnabled: true, tailscaleServePort: 8443 },
      endpoints: [loopback, magic(true)] }));
    expect([view.tailscaleOn, view.tailscaleDescription, view.servePort, view.clientsVisible, view.networkOn]).toEqual([true, 'https://lane.tail.ts.net:8443/', 8443, true, false]);
  });

  it('shows the last failure as the row’s red text', () => {
    networkUi.exposureError = 'No reachable network address is available for desktop network access on port 16101.';
    expect(networkProjection(input()).error).toBe(networkUi.exposureError);
    expect(networkProjection(input({ loadError: 'Failed to read desktop network interfaces on darwin.' })).error).toBe(networkUi.exposureError);
  });
});

describe('Authorized clients', () => {
  it('lists pairing links with their expiry, scopes and share options, and drops an expired one', () => {
    const credentials = new Map([['link-1', 'PAIRCODE']]);
    const view = networkProjection(input({ credentials, access: { loaded: true, error: '', clientSessions: [],
      pairingLinks: [link('link-1'), link('link-2', { label: undefined, createdAt: '2036-04-06T23:58:00.000Z' }), link('gone', { expiresAt: '2036-04-06T23:59:59.000Z' })] } }));
    expect(view.pairings.map(row => row.id)).toEqual(['link-1', 'link-2']);
    const [mine, other] = view.pairings;
    expect([mine!.label, mine!.expires, mine!.scopeCount, mine!.share, mine!.copyCode, mine!.note]).toEqual(['iPad', 'Expires in 4m 12s', '2 scopes', true, false, '']);
    expect(mine!.options.map(option => [option.label, option.detail, option.qr])).toEqual([['This machine', 'Clients on this machine', false], ['Local network', 'Devices on the same network', true],
      ['Tailscale IP', 'Devices on your private network', true]]);
    expect(mine!.options[1]!.url).toBe('http://192.168.1.20:16101/pair#token=PAIRCODE');
    expect(mine!.options[0]!.cells).toEqual([]);
    expect(mine!.options[1]!.cells.length).toBeGreaterThan(20);
    expect([mine!.multiple, mine!.defaultOption]).toEqual([true, 'desktop-lan:http://192.168.1.20:16101']);
    // A link made elsewhere (or before this app run) has no credential here.
    expect([other!.label, other!.share, other!.note]).toEqual(['Pairing link', false, 'Create a new link to share from this client.']);
    expect(view.summary).toBe('0 clients · 2 pairing links');
    // Ticking: one second later the label follows; at expiry the row goes.
    expect(networkProjection(input({ credentials, now: NOW + 1000, access: { loaded: true, error: '', clientSessions: [], pairingLinks: [link('link-1')] } })).pairings[0]!.expires).toBe('Expires in 4m 11s');
    expect(networkProjection(input({ now: Date.parse('2036-04-07T00:04:12.000Z'), access: { loaded: true, error: '', clientSessions: [], pairingLinks: [link('link-1')] } })).pairings).toEqual([]);
  });

  it('pairs HTTPS endpoints through the hosted app and offers it first when it is the default', () => {
    // With network access off, Tailscale HTTPS is the only shareable endpoint, so it is the default.
    const view = networkProjection(input({ credentials: new Map([['link-1', 'PAIRCODE']]), defaultKey: 'tailscale:magicdns:https', endpoints: [loopback, magic(true)],
      state: { ...networkOn, mode: 'local-only', endpointUrl: null, advertisedHost: null, tailscaleServeEnabled: true, tailscaleServePort: 8443 },
      access: { loaded: true, error: '', clientSessions: [], pairingLinks: [link('link-1')] } }));
    const https = view.pairings[0]!.options.find(option => option.label === 'Tailscale HTTPS')!;
    expect([https.url, https.detail]).toEqual(['https://app.t3.codes/pair?host=https%3A%2F%2Flane.tail.ts.net%3A8443%2F#token=PAIRCODE', 'Opens the hosted app, no install needed']);
    expect(view.pairings[0]!.defaultOption).toBe(https.id);
  });

  it('lists this device first with its tag and the others with their device, live dot and Revoke', () => {
    const view = networkProjection(input({ access: { loaded: true, error: '', pairingLinks: [], clientSessions: [
      session('phone'), session('desktop', { current: true, connected: false, client: { label: 'T3 Code Desktop', deviceType: 'desktop' }, lastConnectedAt: null }),
      session('old', { connected: false, lastConnectedAt: '2036-04-01T10:00:00.000Z', client: { deviceType: 'unknown' } })] } }));
    expect(view.clients.map(row => [row.id, row.label, row.current, row.live, row.device])).toEqual([
      ['desktop', 'T3 Code Desktop', true, true, 'Desktop'], ['phone', 'iOS · Safari', false, true, 'Mobile · iOS · Safari · 192.168.1.9'], ['old', 'client', false, false, '']]);
    expect(view.clients[0]!.statusTip).toBe('Connected');
    expect(view.clients[1]!.statusTip).toBe('Connected for 5m');
    expect(view.clients[2]!.statusTip).toMatch(/^Last connected at /);
    expect([view.othersDisabled, view.summary, view.empty]).toEqual([false, '3 clients', false]);
    expect(networkProjection(input({ access: { loaded: true, error: '', pairingLinks: [], clientSessions: [session('d', { current: true })] } })).othersDisabled).toBe(true);
  });

  it('is empty only once the stream answered, and carries its failure', () => {
    expect(networkProjection(input()).empty).toBe(true);
    expect(networkProjection(input({ access: { loaded: false, error: '', pairingLinks: [], clientSessions: [] } })).empty).toBe(false);
    expect(networkProjection(input({ access: { loaded: false, error: 'Missing scope access:read.', pairingLinks: [], clientSessions: [] } })).accessError).toBe('Missing scope access:read.');
  });

  it('formats the countdown and the connected time as timestampFormat.ts', () => {
    const at = (seconds: number) => new Date(NOW + seconds * 1000).toISOString();
    expect([0, 3, 42, 60, 299, 3600, 3725, 86_400, 90_000].map(seconds => formatExpiresInLabel(at(seconds), NOW))).toEqual(['Expired', 'Expires in a moment', 'Expires in 42s',
      'Expires in 1m', 'Expires in 4m 59s', 'Expires in 1h', 'Expires in 1h 2m 5s', 'Expires in 1d', 'Expires in 1d 1h']);
    expect([2, 30, 600, 7200].map(seconds => formatElapsedDurationLabel(new Date(NOW - seconds * 1000).toISOString(), NOW))).toEqual(['just now', '30s', '10m', '2h']);
  });
});

// ── The commands, over a fake native module ───────────────────────────────
type Answer = (request: Obj) => Obj | undefined;
/** The fake native module; `settings` is desktop-settings.json as T3DesktopSettings.swift keeps it (decision U7). */
function fakeNative(answer: Answer): { native: Native; requests: Obj[]; settings: Obj } {
  const requests: Obj[] = [];
  const settings: Obj = { localEnvironmentEnabled: true, serverExposureMode: 'local-only', tailscaleServeEnabled: false, tailscaleServePort: 443 };
  return { requests, settings, native: { available: true, watch: () => {}, later: async request => {
    const value = request as Obj;
    requests.push(value);
    const custom = answer(value);
    // A status the native side answers carries the settings it holds.
    if (custom?.ok && (value.op === 'localBackendRestart' || value.op === 'localBackendSetEnabled')) return { ...custom, value: { ...obj(custom.value), desktopSettings: { ...settings } } };
    if (custom) return custom;
    if (value.op === 'desktopSettingsSet') {
      const { op: _op, ...patch } = value, before = JSON.stringify(settings);
      Object.assign(settings, patch);
      return { ok: true, generation: 1, value: { changed: JSON.stringify(settings) !== before, settings: { ...settings } } };
    }
    if (value.op === 'connect') return { ok: true, generation: 2, value: { state: 'connected', origin: 'http://127.0.0.1:16101', environmentId: 'env-local', message: '' } };
    return { ok: true, generation: 1, value: {} };
  } } };
}
const ready = (extra: Obj = {}) => parseLocalBackendStatus({ state: 'ready', enabled: true, port: 16101, httpBaseUrl: 'http://127.0.0.1:16101', wsBaseUrl: 'ws://127.0.0.1:16101',
  bearerReady: true, environmentId: 'env-local', label: 'Lane Mac', ...extra });
const facts = (interfaces: Obj, extra: Obj = {}) => ({ ok: true, generation: 1, value: { interfaces, lanHostOverride: '', httpsEndpointUrls: [],
  tailscale: { read: true, magicDnsName: 'lane.tail.ts.net', tailnetIpv4Addresses: [] }, probe: { url: '', read: true, reachable: false }, ...extra } });
const LAN = { en0: [{ address: '192.168.1.20', family: 'IPv4', internal: false }], lo0: [{ address: '127.0.0.1', family: 'IPv4', internal: true }] };

describe('Network access and Tailscale HTTPS commands (applyLocalSetting restarts the server in place)', () => {
  it('refuses network access without a reachable address: the row’s error, the toast, local-only kept, the dialog closed', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    const { native, requests, settings } = fakeNative(request => request.op === 'localNetworkFacts' ? facts({ lo0: [{ address: '127.0.0.1', family: 'IPv4', internal: true }] }) : undefined);
    await runNetworkOp(client, native, 'network-access', '', 'on');
    expect(networkUi.exposureError).toBe('No reachable network address is available for desktop network access on port 16101.');
    expect(toasts(client).map(toast => [toast.kind, toast.title, toast.description])).toEqual([['error', 'Could not update network access', networkUi.exposureError]]);
    expect(settings.serverExposureMode).toBe('local-only'); // refused before the write, as DesktopServerExposure.setMode
    expect(requests.some(request => request.op === 'desktopSettingsSet')).toBe(false);
    expect(networkUi.networkSerial).toBe(1);
    expect(requests.some(request => request.op === 'localBackendRestart')).toBe(false);
  });

  it('turns network access on: persists the mode and restarts the server bound to 0.0.0.0, then reconnects this machine', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    client.environmentId = 'env-local'; client.origin = 'http://127.0.0.1:16101';
    const { native, requests, settings } = fakeNative(request => request.op === 'localNetworkFacts' ? facts(LAN)
      : request.op === 'localBackendRestart' ? { ok: true, generation: 1, value: { ...ready(), host: request.host } } : undefined);
    const result = await runNetworkOp(client, native, 'network-access', '', 'on');
    // desktop:set-server-exposure-mode: persisted to desktop-settings.json, then the (stopgap) restart.
    expect(requests.filter(request => ['desktopSettingsSet', 'localBackendRestart'].includes(String(request.op))).map(request => request.op)).toEqual(['desktopSettingsSet', 'localBackendRestart']);
    expect(requests.find(request => request.op === 'desktopSettingsSet')).toEqual({ op: 'desktopSettingsSet', serverExposureMode: 'network-accessible' });
    expect(requests.find(request => request.op === 'localBackendRestart')).toEqual({ op: 'localBackendRestart', host: '0.0.0.0', tailscaleServeEnabled: false, tailscaleServePort: 443 });
    expect(requests.find(request => request.op === 'connect')).toMatchObject({ op: 'connect', origin: 'http://127.0.0.1:16101/', primary: true });
    expect(obj(result.status).state).toBe('connected');
    expect([settings.serverExposureMode, client.localBackend.settings.serverExposureMode, networkUi.exposureError, networkUi.networkSerial]).toEqual(['network-accessible', 'network-accessible', '', 1]);
    expect(client.local).not.toHaveProperty('serverExposureMode'); // t3-code.json no longer holds it
  });

  it('puts the mode back when the server cannot come back with it', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    let restarts = 0;
    const { native, requests, settings } = fakeNative(request => request.op === 'localNetworkFacts' ? facts(LAN)
      : request.op === 'localBackendRestart' ? (++restarts === 1 ? { ok: false, generation: 1, error: { kind: 'LocalEnvironment', message: 'The local server did not start in time.' } } : { ok: true, generation: 1, value: ready() })
        : undefined);
    await runNetworkOp(client, native, 'network-access', '', 'on');
    expect(requests.filter(request => request.op === 'localBackendRestart').map(request => request.host)).toEqual(['0.0.0.0', '127.0.0.1']);
    expect(requests.filter(request => request.op === 'desktopSettingsSet').map(request => request.serverExposureMode)).toEqual(['network-accessible', 'local-only']);
    expect([settings.serverExposureMode, networkUi.exposureError]).toEqual(['local-only', 'The local server did not start in time.']);
    expect(toasts(client).at(-1)?.title).toBe('Could not update network access');
  });

  it('sets up Tailscale HTTPS on the chosen port and closes its dialog; a bad port keeps it open with the toast', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    const { native, requests, settings } = fakeNative(request => request.op === 'localNetworkFacts' ? facts(LAN) : request.op === 'localBackendRestart' ? { ok: true, generation: 1, value: ready() } : undefined);
    await runNetworkOp(client, native, 'tailscale-serve', '', 'on:70000');
    expect([networkUi.tailscaleSerial, toasts(client).at(-1)?.title, toasts(client).at(-1)?.description]).toEqual([0, 'Could not set up Tailscale HTTPS', 'Enter a port from 1 to 65535.']);
    await runNetworkOp(client, native, 'tailscale-serve', '', 'on:8443');
    expect(requests.find(request => request.op === 'desktopSettingsSet')).toEqual({ op: 'desktopSettingsSet', tailscaleServeEnabled: true, tailscaleServePort: 8443 });
    expect(requests.find(request => request.op === 'localBackendRestart')).toEqual({ op: 'localBackendRestart', host: '127.0.0.1', tailscaleServeEnabled: true, tailscaleServePort: 8443 });
    expect([settings.tailscaleServeEnabled, settings.tailscaleServePort, networkUi.tailscaleSerial]).toEqual([true, 8443, 1]);
    await runNetworkOp(client, native, 'tailscale-serve', '', 'off');
    expect(requests.filter(request => request.op === 'localBackendRestart').at(-1)).toEqual({ op: 'localBackendRestart', host: '127.0.0.1', tailscaleServeEnabled: false, tailscaleServePort: 8443 });
    expect(networkUi.tailscaleSerial).toBe(2);
  });

  it('keeps the chosen default endpoint in the preference file', async () => {
    const client = new T3Client();
    await runNetworkOp(client, fakeNative(() => undefined).native, 'endpoint-default', '', 'tailscale:ip:http');
    expect((client.local as Obj).defaultAdvertisedEndpointKey).toBe('tailscale:ip:http');
  });
});

describe('pairing links and client commands', () => {
  it('creates a link with the chosen scopes, keeps its credential in memory and closes the dialog', async () => {
    const client = new T3Client();
    const { native, requests } = fakeNative(request => request.op === 'localAccess' ? { ok: true, generation: 1, value: { id: 'link-9', credential: 'SECRET', expiresAt: '2036-04-07T00:05:00.000Z' } } : undefined);
    await runNetworkOp(client, native, 'pairing-create', 'Living room iPad', 'orchestration:read terminal:operate bogus ');
    expect(requests).toEqual([{ op: 'localAccess', method: 'POST', path: '/api/auth/pairing-token', body: { label: 'Living room iPad', scopes: ['orchestration:read', 'terminal:operate'] } }]);
    expect([createdCredentials.get('link-9'), networkUi.createSerial]).toEqual(['SECRET', 1]);
    expect(JSON.stringify(client.local)).not.toContain('SECRET');
  });

  it('keeps the create dialog open with the reference toast when the server refuses', async () => {
    const client = new T3Client();
    const { native } = fakeNative(() => ({ ok: false, generation: 1, error: { kind: 'Http', message: 'HTTP 403', detail: '403' } }));
    await runNetworkOp(client, native, 'pairing-create', '', 'orchestration:read');
    expect([networkUi.createSerial, toasts(client).at(-1)?.title, toasts(client).at(-1)?.description])
      .toEqual([0, 'Could not create pairing URL', 'Primary environment request failed during create-pairing-credential (HTTP 403).']);
  });

  it('revokes others with the count toast and names a failed revoke', async () => {
    const client = new T3Client();
    const { native } = fakeNative(request => request.path === '/api/auth/clients/revoke-others' ? { ok: true, generation: 1, value: { revokedCount: 1 } }
      : request.path === '/api/auth/clients/revoke' ? { ok: false, generation: 1, error: { kind: 'Http', message: 'HTTP 404', detail: '404' } } : undefined);
    await runNetworkOp(client, native, 'clients-revoke-others', '', '');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Revoked 1 other client', description: 'Other paired clients will need a new pairing link before reconnecting.' });
    await runNetworkOp(client, native, 'client-revoke', 'session-1', '');
    expect([toasts(client).at(-1)?.title, networkUi.accessError, networkUi.settled])
      .toEqual(['Could not revoke client access', 'Primary environment request failed during revoke-client-session (HTTP 404).', 2]);
  });

  it('copies the chosen URL or the code, and reveals exactly the failed value when the pasteboard refuses', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    createdCredentials.set('link-1', 'PAIRCODE');
    access.environmentId = 'env-local'; access.loaded = true; access.snapshot = { pairingLinks: [link('link-1', { expiresAt: '2099-01-01T00:00:00.000Z' })], clientSessions: [] };
    let pasteboard = true;
    const { native, requests } = fakeNative(request => request.op === 'localNetworkFacts' ? facts(LAN)
      : request.op === 'copyText' ? (pasteboard ? { ok: true, generation: 1, value: { copied: true } } : { ok: false, generation: 1, error: { kind: 'Clipboard', message: 'Could not copy the message.' } })
        : request.op === 'localBackendRestart' ? { ok: true, generation: 1, value: ready() } : undefined);
    await runNetworkOp(client, native, 'network-access', '', 'on');
    await networkPage(client, native, true, NOW, true);
    await runNetworkOp(client, native, 'pairing-copy', 'link-1', 'desktop-lan:http://192.168.1.20:16101');
    expect(requests.filter(request => request.op === 'copyText').at(-1)).toEqual({ op: 'copyText', text: 'http://192.168.1.20:16101/pair#token=PAIRCODE' });
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Pairing URL copied', description: 'Open it in the client you want to pair to this environment.' });
    await runNetworkOp(client, native, 'pairing-copy', 'link-1', 'code');
    expect(toasts(client).at(-1)).toMatchObject({ title: 'Pairing code copied', description: 'Paste it into another client to finish pairing.' });
    pasteboard = false;
    await runNetworkOp(client, native, 'pairing-copy', 'link-1', 'desktop-lan:http://192.168.1.20:16101');
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'error', title: 'Could not copy pairing URL', description: 'Could not copy the message.' });
    const reveal = networkProjection(input({ endpoints: [loopback, lan] })).reveal;
    expect([reveal.open, reveal.title, reveal.value, reveal.rows, reveal.qr, reveal.linkId]).toEqual([true, 'Pairing link', 'http://192.168.1.20:16101/pair#token=PAIRCODE', 4, true, 'link-1']);
    expect(reveal.description).toBe('Clipboard copy is unavailable here. Open or manually copy this full pairing URL on the device you want to connect.');
    await runNetworkOp(client, native, 'pairing-reveal-close', 'link-1', '');
    expect(networkProjection(input()).reveal.open).toBe(false);
    // A failed code copy reveals the code, with no QR.
    await runNetworkOp(client, native, 'pairing-copy', 'link-1', 'code');
    expect([networkUi.reveal?.value, networkProjection(input()).reveal.title, networkProjection(input()).reveal.qr]).toEqual(['PAIRCODE', 'Pairing code', false]);
  });
});

describe('the Connections page carries the section', () => {
  it('routes every network op through Connections, and a closed page stops the access stream', async () => {
    for (const op of NETWORK_OPS) expect(CONNECTION_OPS).toContain(op);
    expect(connectionsProjection({ connection: 'disconnected', origin: '', environmentId: '', statusMessage: '', scopes: [], config: {} }, []).network.visible).toBe(false);
    primary.update(ready(), true);
    const client = new T3Client();
    const { native } = fakeNative(request => request.op === 'localNetworkFacts' ? facts(LAN) : undefined);
    const open = await networkPage(client, native, true, NOW, true);
    expect([open.visible, open.loading, open.fallback]).toEqual([true, false, 'Limited to this machine.']);
    expect(access.wanted).toBe(true);
    await networkPage(client, native, false, NOW, true);
    expect(access.wanted).toBe(false);
  });

  it('keeps its snapshot while the page stays open and revalidates on open once 30 s old (desktopNetworkAccessStateAtom, U9)', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    Object.assign(client.localBackend.settings, { tailscaleServeEnabled: true, tailscaleServePort: 443 });
    let reachable = false, reads = 0;
    const { native } = fakeNative(request => request.op !== 'localNetworkFacts' ? undefined
      : (reads++, facts(LAN, { probe: { url: String(request.probe ?? ''), read: true, reachable } })));
    expect((await networkPage(client, native, true, NOW, true)).tailscaleOn).toBe(false);
    const afterOpen = reads;
    reachable = true; // Tailscale Serve starts answering while the page is open
    for (const tick of [1_000, 2_000, 40_000]) expect((await networkPage(client, native, true, NOW + tick, true)).tailscaleOn).toBe(false);
    expect(reads).toBe(afterOpen); // never polled while open
    await networkPage(client, native, false, NOW + 41_000, true);
    expect((await networkPage(client, native, true, NOW + 41_000, true)).tailscaleOn).toBe(true); // stale: revalidated on open
    reachable = false;
    await networkPage(client, native, false, NOW + 50_000, true);
    expect((await networkPage(client, native, true, NOW + 50_000, true)).tailscaleOn).toBe(true); // 9 s old: kept
  });

  it('says Loading… while Tailscale’s first read is pending, then shows its endpoint', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    client.localBackend.settings.serverExposureMode = 'network-accessible';
    let read = false;
    const { native } = fakeNative(request => request.op === 'localNetworkFacts' ? facts(LAN, { tailscale: { read, magicDnsName: read ? 'lane.tail.ts.net' : null, tailnetIpv4Addresses: [] } }) : undefined);
    expect((await networkPage(client, native, true, NOW, true)).fallback).toBe('Loading…');
    read = true;
    const view = await networkPage(client, native, true, NOW, true);
    expect([view.loading, view.networkOn, view.summaryUrl, view.tailscale]).toEqual([false, true, 'http://192.168.1.20:16101/', true]);
  });
});
