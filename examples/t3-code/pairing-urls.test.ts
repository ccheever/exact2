// 20261005-this-machine-network-access item 5: pairing URLs and the share panel's endpoint choice,
// ported with the reference's test names (MIT, see LICENSE-T3; reference 1e2ecbd975:
// components/settings/pairingUrls.test.ts and ConnectionsSettings.logic.test.ts). The hosted app's
// URL is a parameter here (the reference stubs VITE_HOSTED_APP_URL).
import { describe, expect, it } from 'bun:test';
import type { AdvertisedEndpoint } from './advertised-endpoint';
import { buildHostedPairingUrl, endpointDefaultPreferenceKey, endpointShareHint, isQrShareableEndpoint, resolveAdvertisedEndpointPairingUrl, resolveDesktopPairingUrl,
  resolveHostedPairingUrl, selectPairingEndpoint, selectQrEndpointOption } from './pairing-urls';
import { parsePairing } from './protocol';
import { resolveRemotePairingTarget } from './remote';

describe('settings pairing URL helpers', () => {
  it('uses direct backend pairing URLs for HTTP endpoints', () => {
    expect(resolveHostedPairingUrl('http://192.168.1.44:3773', 'PAIRCODE')).toBeNull();
    expect(resolveDesktopPairingUrl('http://192.168.1.44:3773', 'PAIRCODE')).toBe('http://192.168.1.44:3773/pair#token=PAIRCODE');
  });

  it('uses hosted pairing URLs for HTTPS endpoints', () => {
    expect(resolveHostedPairingUrl('https://host.tailnet.example.ts.net:3773', 'PAIRCODE', 'https://preview.t3.codes'))
      .toBe('https://preview.t3.codes/pair?host=https%3A%2F%2Fhost.tailnet.example.ts.net%3A3773#token=PAIRCODE');
  });
});

function makeEndpoint(overrides: Partial<AdvertisedEndpoint>): AdvertisedEndpoint {
  return { id: 'desktop-lan:http://192.168.1.42:4780', label: 'Local network', provider: { id: 'desktop-core', label: 'Desktop', kind: 'core', isAddon: false },
    httpBaseUrl: 'http://192.168.1.42:4780', wsBaseUrl: 'ws://192.168.1.42:4780', reachability: 'lan', compatibility: { hostedHttpsApp: 'unknown', desktopApp: 'compatible' },
    source: 'desktop-core', status: 'available', ...overrides };
}

describe('isQrShareableEndpoint', () => {
  it('excludes loopback endpoints so a scanned phone never dials itself', () => {
    expect(isQrShareableEndpoint(makeEndpoint({ id: 'desktop-loopback:4780', reachability: 'loopback', httpBaseUrl: 'http://127.0.0.1:4780' }))).toBe(false);
  });

  it('excludes unavailable endpoints and keeps reachable ones', () => {
    expect(isQrShareableEndpoint(makeEndpoint({ status: 'unavailable' }))).toBe(false);
    expect(isQrShareableEndpoint(makeEndpoint({}))).toBe(true);
    expect(isQrShareableEndpoint(makeEndpoint({ reachability: 'private-network', status: 'unknown' }))).toBe(true);
  });
});

describe('selectQrEndpointOption', () => {
  const options = [
    { id: 'desktop-loopback:4780', preferenceKey: 'desktop-core:loopback:http', qrShareable: false },
    { id: 'tailscale-ip:http://100.84.12.7:4780', preferenceKey: 'tailscale:ip:http', qrShareable: true },
    { id: 'tailscale-ip:http://100.84.12.8:4780', preferenceKey: 'tailscale:ip:http', qrShareable: true },
    { id: 'desktop-lan:http://192.168.1.42:4780', preferenceKey: 'desktop-core:lan:http', qrShareable: true },
  ];

  it('resolves an explicit selection by unique endpoint id, not the shared preference key', () => {
    expect(selectQrEndpointOption(options, 'tailscale-ip:http://100.84.12.8:4780', null)?.id).toBe('tailscale-ip:http://100.84.12.8:4780');
  });

  it('falls back to the saved default preference key when nothing is selected', () => {
    expect(selectQrEndpointOption(options, null, 'desktop-core:lan:http')?.id).toBe('desktop-lan:http://192.168.1.42:4780');
  });

  it('skips non-QR-shareable options in the fallback so the panel never opens on loopback', () => {
    expect(selectQrEndpointOption(options, 'tailscale-ip:gone', 'nope')?.id).toBe('tailscale-ip:http://100.84.12.7:4780');
  });

  it('returns the first option when nothing is QR-shareable, and null when empty', () => {
    expect(selectQrEndpointOption(options.slice(0, 1), null, null)?.id).toBe('desktop-loopback:4780');
    expect(selectQrEndpointOption([], 'anything', 'anything')).toBeNull();
  });
});

describe('the share panel’s endpoints (ConnectionsSettings.tsx helpers)', () => {
  const loopback = makeEndpoint({ id: 'desktop-loopback:4780', label: 'This machine', reachability: 'loopback', httpBaseUrl: 'http://127.0.0.1:4780/',
    compatibility: { hostedHttpsApp: 'mixed-content-blocked', desktopApp: 'compatible' } });
  const lan = makeEndpoint({ isDefault: true, compatibility: { hostedHttpsApp: 'mixed-content-blocked', desktopApp: 'compatible' } });
  const https = makeEndpoint({ id: 'tailscale-magicdns:https://desktop.tail.ts.net/', label: 'Tailscale HTTPS', reachability: 'private-network', httpBaseUrl: 'https://desktop.tail.ts.net/',
    compatibility: { hostedHttpsApp: 'compatible', desktopApp: 'compatible' } });

  it('keys defaults by endpoint type and picks the saved default, the marked default, then a non-loopback one', () => {
    expect([loopback, lan, https].map(endpointDefaultPreferenceKey)).toEqual(['desktop-core:loopback:http', 'desktop-core:lan:http', 'tailscale:magicdns:https']);
    expect(endpointDefaultPreferenceKey(makeEndpoint({ id: 'manual:https://x.example', label: 'Custom HTTPS', provider: { id: 'manual', label: 'Manual', kind: 'manual', isAddon: false },
      reachability: 'public', httpBaseUrl: 'https://x.example/' }))).toBe('manual:public:https:Custom HTTPS');
    expect(selectPairingEndpoint([loopback, lan, https], 'tailscale:magicdns:https')?.id).toBe(https.id);
    expect(selectPairingEndpoint([loopback, lan, https], null)?.id).toBe(lan.id);
    expect(selectPairingEndpoint([loopback, https], null)?.id).toBe(https.id);
    expect(selectPairingEndpoint([loopback], null)).toBeNull();
    expect(selectPairingEndpoint([{ ...https, status: 'unavailable' }], 'tailscale:magicdns:https')).toBeNull();
  });

  it('pairs HTTPS endpoints through the hosted app and names each route', () => {
    const hosted = resolveAdvertisedEndpointPairingUrl(https, 'PAIRCODE');
    expect(hosted).toBe('https://app.t3.codes/pair?host=https%3A%2F%2Fdesktop.tail.ts.net%2F#token=PAIRCODE');
    expect(resolveAdvertisedEndpointPairingUrl(lan, 'PAIRCODE')).toBe('http://192.168.1.42:4780/pair#token=PAIRCODE');
    expect(endpointShareHint(https, hosted)).toBe('Opens the hosted app, no install needed');
    expect(endpointShareHint(lan, 'http://192.168.1.42:4780/pair#token=PAIRCODE')).toBe('Devices on the same network');
    expect(endpointShareHint(loopback, 'http://127.0.0.1:4780/pair#token=PAIRCODE')).toBe('Clients on this machine');
    expect(buildHostedPairingUrl({ host: 'https://desktop.tail.ts.net/', token: 'PAIRCODE', label: ' iPad ' })).toBe('https://app.t3.codes/pair?host=https%3A%2F%2Fdesktop.tail.ts.net%2F&label=iPad#token=PAIRCODE');
  });

  it("is read back by this app's own pairing (protocol.ts parsePairing, remote.ts)", () => {
    expect(parsePairing('http://192.168.1.42:4780/pair#token=PAIRCODE', '')).toEqual({ origin: 'http://192.168.1.42:4780', credential: 'PAIRCODE' });
    expect(resolveRemotePairingTarget({ pairingUrl: 'https://app.t3.codes/pair?host=https%3A%2F%2Fdesktop.tail.ts.net%2F#token=PAIRCODE' })).toMatchObject({ origin: 'https://desktop.tail.ts.net', credential: 'PAIRCODE' });
  });
});
