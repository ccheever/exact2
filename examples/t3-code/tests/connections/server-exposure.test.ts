// Network access for the embedded server: DesktopServerExposure, ported with the reference's
// test names (MIT, see LICENSE-T3; reference 2359643cff: apps/desktop/src/backend/
// DesktopServerExposure.test.ts). The Effect layers become injected functions: the settings store,
// the interface read, the status reader (the native `tailscale status`, whose per-address cache is
// macos/tests/local-backend/network.swift's) and the probe.
import { describe, expect, it } from 'bun:test';
import { DEFAULT_EXPOSURE_SETTINGS, DesktopServerExposure, DesktopServerExposureModePersistenceError, DesktopTailscaleServePersistenceError, adoptNetworkPrefs,
  decodeExposureSettings, memoryExposureSettings, requiresBackendRelaunch, type ExposureSettings, type ExposureSettingsStore } from '../../src/connections/server-exposure';
import type { NetworkInterfaces, TailscaleStatus } from '../../src/connections/tailscale';
import { parseDesktopSettings } from '../../src/connections/local-backend';
import { adoptLocalPrefs } from '../../src/connections/local-environment';

const emptyNetworkInterfaces: NetworkInterfaces = {};
const lanNetworkInterfaces: NetworkInterfaces = { en0: [{ address: '192.168.1.20', family: 'IPv4', internal: false }] };
const tailnetNetworkInterfaces: NetworkInterfaces = { tailscale0: [{ address: '100.90.1.2', family: 'IPv4', internal: false }] };

function harness(networkInterfaces: NetworkInterfaces, options: { env?: Record<string, string>; status?: () => Promise<TailscaleStatus | null>; store?: ExposureSettingsStore } = {}) {
  const holder = { settings: { ...DEFAULT_EXPOSURE_SETTINGS } as ExposureSettings };
  const settings = options.store ?? memoryExposureSettings(holder);
  const env = options.env ?? {};
  const exposure = new DesktopServerExposure({
    settings, readNetworkInterfaces: async () => networkInterfaces,
    // mockSpawnerLayer("{}"): a status without a MagicDNS name or addresses.
    readStatus: options.status ?? (async () => ({ magicDnsName: null, tailnetIpv4Addresses: [] })),
    probe: async () => false,
    config: { ...(env.T3CODE_DESKTOP_LAN_HOST ? { desktopLanHostOverride: env.T3CODE_DESKTOP_LAN_HOST } : {}),
      desktopHttpsEndpointUrls: (env.T3CODE_DESKTOP_HTTPS_ENDPOINTS ?? '').split(',').map(url => url.trim()).filter(Boolean) },
  });
  return { exposure, holder, settings };
}

describe('DesktopServerExposure', () => {
  it('falls back to local-only without losing the requested network preference', async () => {
    const { exposure, settings } = harness(emptyNetworkInterfaces);
    await settings.setServerExposureMode('network-accessible');
    const state = await exposure.configureFromSettings({ port: 4173 });
    expect(state.mode).toBe('local-only');
    expect(state.endpointUrl).toBeNull();
    expect(settings.get().serverExposureMode).toBe('network-accessible');
    const backendConfig = exposure.backendConfig();
    expect(backendConfig.bindHost).toBe('127.0.0.1');
    expect(backendConfig.httpBaseUrl.href).toBe('http://127.0.0.1:4173/');
  });

  it('returns a typed error when network access is explicitly unavailable', async () => {
    const { exposure } = harness(emptyNetworkInterfaces);
    await exposure.configureFromSettings({ port: 4173 });
    const error = await exposure.setMode('network-accessible').then(() => null, caught => caught);
    expect(error?._tag).toBe('DesktopServerExposureNoNetworkAddressError');
    expect(error?.port).toBe(4173);
    expect(error?.message).toBe('No reachable network address is available for desktop network access on port 4173.');
  });

  it('persists network-accessible mode and updates backend binding state', async () => {
    const { exposure, settings } = harness(lanNetworkInterfaces);
    await exposure.configureFromSettings({ port: 4173 });
    const change = await exposure.setMode('network-accessible');
    expect(change.requiresRelaunch).toBe(true);
    expect(change.state).toEqual({ mode: 'network-accessible', endpointUrl: 'http://192.168.1.20:4173', advertisedHost: '192.168.1.20', tailscaleServeEnabled: false, tailscaleServePort: 443 });
    const backendConfig = exposure.backendConfig();
    expect(backendConfig.bindHost).toBe('0.0.0.0');
    expect(backendConfig.httpBaseUrl.href).toBe('http://127.0.0.1:4173/');
    expect(settings.get().serverExposureMode).toBe('network-accessible');
    expect((await exposure.setMode('network-accessible')).requiresRelaunch).toBe(false);
    expect((await exposure.setMode('local-only')).requiresRelaunch).toBe(true);
    expect((await exposure.setMode('local-only')).requiresRelaunch).toBe(false);
  });

  it('advertises the current LAN address after the network changes', async () => {
    const networkInterfaces: NetworkInterfaces = { ...lanNetworkInterfaces };
    const { exposure } = harness(networkInterfaces);
    await exposure.configureFromSettings({ port: 4173 });
    await exposure.setMode('network-accessible');
    networkInterfaces.en0 = [{ address: '10.0.0.5', family: 'IPv4', internal: false }];
    const state = await exposure.getState();
    expect(state.advertisedHost).toBe('10.0.0.5');
    expect(state.endpointUrl).toBe('http://10.0.0.5:4173');
    expect((await exposure.getAdvertisedEndpoints()).map(endpoint => endpoint.httpBaseUrl)).toEqual(['http://127.0.0.1:4173/', 'http://10.0.0.5:4173/']);
    expect(exposure.backendConfig().bindHost).toBe('0.0.0.0');
  });

  it('persists tailscale serve preferences atomically and reports no-op updates', async () => {
    const { exposure, settings } = harness(emptyNetworkInterfaces);
    await exposure.configureFromSettings({ port: 4173 });
    const changed = await exposure.setTailscaleServeEnabled({ enabled: true, port: 8443 });
    expect(changed.requiresRelaunch).toBe(true);
    expect(changed.state.tailscaleServeEnabled).toBe(true);
    expect(changed.state.tailscaleServePort).toBe(8443);
    const unchanged = await exposure.setTailscaleServeEnabled({ enabled: true, port: 8443 });
    expect(unchanged.requiresRelaunch).toBe(false);
    expect(settings.get()).toMatchObject({ tailscaleServeEnabled: true, tailscaleServePort: 8443 });
    expect((await exposure.setTailscaleServeEnabled({ enabled: true, port: 443 })).requiresRelaunch).toBe(true);
    expect((await exposure.setTailscaleServeEnabled({ enabled: false })).requiresRelaunch).toBe(true);
    expect((await exposure.setTailscaleServeEnabled({ enabled: false, port: 443 })).requiresRelaunch).toBe(false);
    expect(settings.get()).toMatchObject({ tailscaleServeEnabled: false, tailscaleServePort: 443 });
    // DesktopAppSettings treats the port as a setting even while Serve is disabled.
    expect((await exposure.setTailscaleServeEnabled({ enabled: false, port: 8443 })).requiresRelaunch).toBe(true);
    expect((await exposure.setTailscaleServeEnabled({ enabled: false, port: 8443 })).requiresRelaunch).toBe(false);
  });

  it('preserves persistence request context and the settings failure chain', async () => {
    const diskFailure = new Error('disk exploded');
    const settingsFailure = Object.assign(new Error('Failed to write desktop settings.'), { cause: diskFailure });
    const store: ExposureSettingsStore = { get: () => DEFAULT_EXPOSURE_SETTINGS, setServerExposureMode: async () => { throw settingsFailure; }, setTailscaleServe: async () => { throw settingsFailure; } };
    const { exposure } = harness(lanNetworkInterfaces, { store });
    await exposure.configureFromSettings({ port: 4173 });
    const before = await exposure.getState();
    const modeError = await exposure.setMode('network-accessible').then(() => null, caught => caught);
    expect(modeError).toBeInstanceOf(DesktopServerExposureModePersistenceError);
    expect(modeError.mode).toBe('network-accessible');
    expect(modeError.cause).toBe(settingsFailure);
    expect(modeError.cause.cause).toBe(diskFailure);
    expect(modeError.message).toBe('Failed to persist desktop server exposure mode network-accessible.');
    expect(modeError.message).not.toContain(diskFailure.message);
    const tailscaleError = await exposure.setTailscaleServeEnabled({ enabled: true, port: 8443 }).then(() => null, caught => caught);
    expect(tailscaleError).toBeInstanceOf(DesktopTailscaleServePersistenceError);
    expect([tailscaleError.enabled, tailscaleError.port, tailscaleError.cause, tailscaleError.cause.cause]).toEqual([true, 8443, settingsFailure, diskFailure]);
    expect(tailscaleError.message).toBe('Failed to persist desktop Tailscale Serve settings (enabled: true, port: 8443).');
    expect(tailscaleError.message).not.toContain(diskFailure.message);
    expect(await exposure.getState()).toEqual(before);
  });

  it("uses cached Tailscale identity to exclude another VPN's pairing address", async () => {
    let statusReads = 0;
    const { exposure } = harness({ ...lanNetworkInterfaces, pvpnksintrf1: [{ address: '100.85.0.1', family: 'IPv4', internal: false }],
      vpn0: [{ address: '100.74.126.34', family: 'IPv4', internal: false }] },
    { status: async () => { statusReads += 1; return { magicDnsName: null, tailnetIpv4Addresses: ['100.74.126.34'] }; } });
    await exposure.configureFromSettings({ port: 4173 });
    await exposure.setMode('network-accessible');
    expect((await exposure.getAdvertisedEndpoints()).map(endpoint => endpoint.httpBaseUrl)).toEqual(['http://127.0.0.1:4173/', 'http://192.168.1.20:4173/', 'http://100.74.126.34:4173/']);
    expect(statusReads).toBe(1);
  });

  it('advertises the Wi-Fi or Ethernet address ahead of bridges and tunnels', async () => {
    const { exposure } = harness({ bridge0: [{ address: '192.168.2.1', family: 'IPv4', internal: false }], utun4: [{ address: '10.8.0.2', family: 'IPv4', internal: false }],
      en0: [{ address: '192.168.1.20', family: 'IPv4', internal: false }] });
    await exposure.configureFromSettings({ port: 4173 });
    expect((await exposure.setMode('network-accessible')).state.advertisedHost).toBe('192.168.1.20');
  });

  it("keeps a host bridge's LAN address ahead of later adapters", async () => {
    const { exposure } = harness({ vmbr0: [{ address: '192.168.1.30', family: 'IPv4', internal: false }], enx00e04c680001: [{ address: '192.168.42.10', family: 'IPv4', internal: false }] });
    await exposure.configureFromSettings({ port: 4173 });
    expect((await exposure.setMode('network-accessible')).state.advertisedHost).toBe('192.168.1.30');
  });

  it('keeps LAN and Tailscale endpoints distinct when Tailscale is enumerated first', async () => {
    const { exposure } = harness({ ...tailnetNetworkInterfaces, ...lanNetworkInterfaces });
    await exposure.configureFromSettings({ port: 4173 });
    await exposure.setMode('network-accessible');
    expect((await exposure.getAdvertisedEndpoints()).map(endpoint => endpoint.httpBaseUrl)).toEqual(['http://127.0.0.1:4173/', 'http://192.168.1.20:4173/', 'http://100.90.1.2:4173/']);
  });

  it('keeps Tailscale-only hosts network-accessible', async () => {
    const { exposure, settings } = harness(tailnetNetworkInterfaces);
    await settings.setServerExposureMode('network-accessible');
    const state = await exposure.configureFromSettings({ port: 4173 });
    expect(state.mode).toBe('network-accessible');
    expect(state.advertisedHost).toBeNull();
    expect(state.endpointUrl).toBeNull();
    expect(exposure.backendConfig().bindHost).toBe('0.0.0.0');
    expect((await exposure.getAdvertisedEndpoints()).map(endpoint => [endpoint.reachability, endpoint.httpBaseUrl])).toEqual([
      ['loopback', 'http://127.0.0.1:4173/'], ['private-network', 'http://100.90.1.2:4173/']]);
  });

  it('does not spawn the tailscale CLI while server exposure is local-only', async () => {
    const { exposure } = harness(lanNetworkInterfaces, { status: async () => { throw new Error('unexpected tailscale spawn'); } });
    await exposure.configureFromSettings({ port: 4173 });
    // mode stays at default "local-only", tailscaleServeEnabled stays false.
    expect((await exposure.getAdvertisedEndpoints()).map(endpoint => endpoint.httpBaseUrl)).toEqual(['http://127.0.0.1:4173/']);
  });

  it('preserves explicit Tailscale exposure overrides', async () => {
    const { exposure } = harness(lanNetworkInterfaces, { env: { T3CODE_DESKTOP_LAN_HOST: '100.90.1.2', T3CODE_DESKTOP_HTTPS_ENDPOINTS: 'https://public.example.test' } });
    await exposure.configureFromSettings({ port: 4173 });
    const change = await exposure.setMode('network-accessible');
    expect(change.state.advertisedHost).toBe('100.90.1.2');
    expect(change.state.endpointUrl).toBe('http://100.90.1.2:4173');
    expect((await exposure.getAdvertisedEndpoints()).map(endpoint => endpoint.httpBaseUrl)).toEqual(['http://127.0.0.1:4173/', 'http://100.90.1.2:4173/', 'https://public.example.test/']);
  });

  it('advertises loopback, LAN, and configured manual endpoints from runtime state', async () => {
    const { exposure } = harness(lanNetworkInterfaces, { env: { T3CODE_DESKTOP_HTTPS_ENDPOINTS: 'https://desktop.example.ts.net,http://desktop.example.test:3773,not-a-url' } });
    await exposure.configureFromSettings({ port: 3773 });
    await exposure.setMode('network-accessible');
    const core = { id: 'desktop-core', label: 'Desktop', kind: 'core', isAddon: false }, manual = { id: 'manual', label: 'Manual', kind: 'manual', isAddon: false };
    expect(await exposure.getAdvertisedEndpoints()).toEqual([
      { id: 'desktop-loopback:3773', label: 'This machine', provider: core, httpBaseUrl: 'http://127.0.0.1:3773/', wsBaseUrl: 'ws://127.0.0.1:3773/', reachability: 'loopback',
        compatibility: { hostedHttpsApp: 'mixed-content-blocked', desktopApp: 'compatible' }, source: 'desktop-core', status: 'available', description: 'Loopback endpoint for this desktop app.' },
      { id: 'desktop-lan:http://192.168.1.20:3773', label: 'Local network', provider: core, httpBaseUrl: 'http://192.168.1.20:3773/', wsBaseUrl: 'ws://192.168.1.20:3773/', reachability: 'lan',
        compatibility: { hostedHttpsApp: 'mixed-content-blocked', desktopApp: 'compatible' }, source: 'desktop-core', status: 'available', isDefault: true,
        description: 'Reachable from devices on the same network.' },
      { id: 'manual:https://desktop.example.ts.net', label: 'Custom HTTPS', provider: manual, httpBaseUrl: 'https://desktop.example.ts.net/', wsBaseUrl: 'wss://desktop.example.ts.net/',
        reachability: 'public', compatibility: { hostedHttpsApp: 'compatible', desktopApp: 'compatible' }, source: 'user', status: 'unknown',
        description: 'User-configured HTTPS endpoint for this desktop backend.' },
      { id: 'manual:http://desktop.example.test:3773', label: 'Custom endpoint', provider: manual, httpBaseUrl: 'http://desktop.example.test:3773/', wsBaseUrl: 'ws://desktop.example.test:3773/',
        reachability: 'public', compatibility: { hostedHttpsApp: 'mixed-content-blocked', desktopApp: 'compatible' }, source: 'user', status: 'unknown',
        description: 'User-configured endpoint for this desktop backend.' },
    ]);
  });
});

describe('exposure keys (DesktopAppSettings decoding; desktop-settings.json)', () => {
  it('decodes the three keys with the reference defaults; t3-code.json carries only the default endpoint', () => {
    expect(decodeExposureSettings({})).toEqual({ serverExposureMode: 'local-only', tailscaleServeEnabled: false, tailscaleServePort: 443 });
    expect(decodeExposureSettings({ serverExposureMode: 'network-accessible', tailscaleServeEnabled: true, tailscaleServePort: 8443 }))
      .toEqual({ serverExposureMode: 'network-accessible', tailscaleServeEnabled: true, tailscaleServePort: 8443 });
    // normalizeTailscaleServePort: out of range, fractional or not a number falls back to 443.
    for (const port of [0, 70_000, 44.5, '8443']) expect(decodeExposureSettings({ tailscaleServePort: port }).tailscaleServePort).toBe(443);
    // The native status's desktopSettings parse the same way (local-backend.ts).
    expect(parseDesktopSettings({ serverExposureMode: 'network-accessible', tailscaleServeEnabled: true, tailscaleServePort: 8443, localEnvironmentEnabled: false }))
      .toEqual({ localEnvironmentEnabled: false, serverExposureMode: 'network-accessible', tailscaleServeEnabled: true, tailscaleServePort: 8443, updateChannel: 'latest' });
    expect(parseDesktopSettings(undefined)).toEqual({ localEnvironmentEnabled: true, serverExposureMode: 'local-only', tailscaleServeEnabled: false, tailscaleServePort: 443, updateChannel: 'latest' });
    // A saved t3-code.json from before the move to desktop-settings.json: only the default endpoint stays (uiStateStore's key); the rest moved.
    const next: Record<string, unknown> = {};
    adoptNetworkPrefs(next, { serverExposureMode: 'network-accessible', tailscaleServePort: 8443, defaultAdvertisedEndpointKey: 'tailscale:ip:http' });
    expect(next).toEqual({ defaultAdvertisedEndpointKey: 'tailscale:ip:http' });
    const saved = { version: 1, localEnvironmentEnabled: false, serverExposureMode: 'network-accessible', defaultAdvertisedEndpointKey: 'desktop-core:lan:http' };
    const local: Record<string, unknown> = {};
    expect(adoptLocalPrefs(local, saved)).toBe(true); // the next refresh saves t3-code.json without them
    expect(local).toEqual({ defaultAdvertisedEndpointKey: 'desktop-core:lan:http' });
    expect(adoptLocalPrefs({}, { version: 1, defaultAdvertisedEndpointKey: 'x' })).toBe(false);
  });

  it('asks for a relaunch only when the port, bind host or local URL changes', () => {
    const base = { port: 4173, bindHost: '127.0.0.1', localHttpUrl: 'http://127.0.0.1:4173' };
    expect(requiresBackendRelaunch(base, { ...base })).toBe(false);
    expect(requiresBackendRelaunch(base, { ...base, bindHost: '0.0.0.0' })).toBe(true);
    expect(requiresBackendRelaunch(base, { ...base, port: 4174 })).toBe(true);
  });
});
