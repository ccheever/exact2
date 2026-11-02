// Tailscale HTTPS for "This machine": the pure parts of packages/tailscale (tailscale.test.ts)
// and apps/desktop/src/backend/tailscaleEndpointProvider.test.ts, ported with the reference's test
// names (MIT, see LICENSE-T3; reference 74541bab17). The process tests (spawn, exit diagnostics, the
// 1.5 s timeout) are the native reader's (macos/tests/local-backend/network.swift); `tailscale serve`
// runs inside the embedded server, so its tests stay with the server.
import { describe, expect, it } from 'bun:test';
import { TailscaleStatusParseError, buildTailscaleHttpsBaseUrl, isTailscaleIpv4Address, parseTailscaleMagicDnsName, parseTailscaleStatus,
  resolveTailscaleAdvertisedEndpoints } from '../../src/connections/tailscale';

const tailscaleStatusJson = `{"Self":{"DNSName":"desktop.tail.ts.net.","TailscaleIPs":["100.100.100.100","fd7a:115c:a1e0::1","192.168.1.20"]}}`;

describe('tailscale', () => {
  it('detects Tailnet IPv4 addresses', () => {
    expect(isTailscaleIpv4Address('100.64.0.1')).toBe(true);
    expect(isTailscaleIpv4Address('100.127.255.254')).toBe(true);
    expect(isTailscaleIpv4Address('100.128.0.1')).toBe(false);
    expect(isTailscaleIpv4Address('192.168.1.44')).toBe(false);
  });

  it('parses MagicDNS names from tailscale status', () => {
    expect(parseTailscaleMagicDnsName(tailscaleStatusJson)).toBe('desktop.tail.ts.net');
    expect(parseTailscaleMagicDnsName('{}')).toBeNull();
  });

  it('parses status facts', () => {
    expect(parseTailscaleStatus(tailscaleStatusJson)).toEqual({ magicDnsName: 'desktop.tail.ts.net', tailnetIpv4Addresses: ['100.100.100.100'] });
  });

  it('preserves status decoding failures without exposing cause text', () => {
    let error: unknown;
    try { parseTailscaleStatus('{not-json'); } catch (caught) { error = caught; }
    expect(error).toBeInstanceOf(TailscaleStatusParseError);
    const parse = error as TailscaleStatusParseError;
    expect(parse.message).toBe('Failed to decode tailscale status JSON.');
    expect(parse.cause).toBeDefined();
    expect(parse.message).not.toContain(String(parse.cause));
  });

  it('builds clean HTTPS base URLs', () => {
    expect(buildTailscaleHttpsBaseUrl({ magicDnsName: 'desktop.tail.ts.net' })).toBe('https://desktop.tail.ts.net/');
    expect(buildTailscaleHttpsBaseUrl({ magicDnsName: 'desktop.tail.ts.net', servePort: 8443 })).toBe('https://desktop.tail.ts.net:8443/');
  });
});

const provider = { id: 'tailscale', label: 'Tailscale', kind: 'private-network', isAddon: true };
const unexpected = { readStatus: async (): Promise<null> => { throw new Error('unexpected tailscale status process'); },
  probe: async (): Promise<boolean> => { throw new Error('unexpected Tailscale HTTPS probe'); } };

const protonAndTailscaleInterfaces = {
  pvpnksintrf1: [{ address: '100.85.0.1', family: 'IPv4', internal: false }],
  tailscale0: [{ address: '100.74.126.34', family: 'IPv4', internal: false }],
};
const urls = (endpoints: readonly { httpBaseUrl: string }[]) => endpoints.map(endpoint => endpoint.httpBaseUrl);

describe('tailscale endpoint provider', () => {
  it("does not advertise Proton's CGNAT address as a Tailscale pairing endpoint", async () => {
    expect(urls(await resolveTailscaleAdvertisedEndpoints({ port: 3773, ...unexpected, networkInterfaces: protonAndTailscaleInterfaces,
      statusJson: `{"Self":{"TailscaleIPs":["100.74.126.34"]}}` }))).toEqual(['http://100.74.126.34:3773/']);
  });

  it('does not advertise CGNAT interfaces when Tailscale reports no assigned addresses', async () => {
    expect(await resolveTailscaleAdvertisedEndpoints({ port: 3773, ...unexpected, networkInterfaces: { pvpnksintrf1: protonAndTailscaleInterfaces.pvpnksintrf1 },
      statusJson: `{"BackendState":"NoState","Self":{"TailscaleIPs":null}}` })).toEqual([]);
  });

  it('keeps native Tailscale interfaces with missing, malformed, or stale status', async () => {
    for (const interfaceName of ['tailscale0', 'Tailscale']) {
      for (const statusJson of [null, 'not-json', `{"Self":{"TailscaleIPs":[]}}`]) {
        expect(urls(await resolveTailscaleAdvertisedEndpoints({ port: 3773, ...unexpected, statusJson,
          networkInterfaces: { pvpnksintrf1: protonAndTailscaleInterfaces.pvpnksintrf1, [interfaceName]: protonAndTailscaleInterfaces.tailscale0 } }))).toEqual(['http://100.74.126.34:3773/']);
      }
    }
  });

  it('recognizes a macOS Tailscale tunnel by its IPv6 address when status is unavailable', async () => {
    expect(urls(await resolveTailscaleAdvertisedEndpoints({ port: 3773, ...unexpected, statusJson: null, networkInterfaces: {
      utun3: [{ address: '100.85.0.1', family: 'IPv4', internal: false }],
      utun4: [{ address: '100.74.126.34', family: 'IPv4', internal: false }, { address: 'fd7a:115c:a1e0::1234', family: 'IPv6', internal: false }],
    } }))).toEqual(['http://100.74.126.34:3773/']);
  });

  it('uses assigned addresses for renamed adapters without IPv6', async () => {
    expect(urls(await resolveTailscaleAdvertisedEndpoints({ port: 3773, ...unexpected, networkInterfaces: { vpn0: [{ address: '100.74.126.34', family: 'IPv4', internal: false }] },
      statusJson: `{"Self":{"TailscaleIPs":["100.74.126.34"]}}` }))).toEqual(['http://100.74.126.34:3773/']);
  });

  it('does not advertise cached Tailscale addresses absent from local interfaces', async () => {
    expect(await resolveTailscaleAdvertisedEndpoints({ port: 3773, ...unexpected, networkInterfaces: { pvpnksintrf1: protonAndTailscaleInterfaces.pvpnksintrf1 },
      statusJson: `{"Self":{"TailscaleIPs":["100.74.126.35"]}}` })).toEqual([]);
  });

  it('parses MagicDNS names from tailscale status', () => {
    expect(parseTailscaleMagicDnsName(`{"Self":{"DNSName":"desktop.tail.ts.net."}}`)).toBe('desktop.tail.ts.net');
    expect(parseTailscaleMagicDnsName('{}')).toBeNull();
    expect(() => parseTailscaleMagicDnsName('not-json')).toThrow(TailscaleStatusParseError);
  });

  it('resolves Tailscale endpoints as add-on advertised endpoints', async () => {
    const endpoints = await resolveTailscaleAdvertisedEndpoints({ port: 3773, ...unexpected,
      networkInterfaces: { tailscale0: [{ address: '100.100.100.100', family: 'IPv4', internal: false, netmask: '255.192.0.0', cidr: '100.100.100.100/10', mac: '00:00:00:00:00:00' }] },
      statusJson: `{"Self":{"DNSName":"desktop.tail.ts.net.","TailscaleIPs":["100.100.100.100"]}}` });
    expect(endpoints).toEqual([
      { id: 'tailscale-ip:http://100.100.100.100:3773', label: 'Tailscale IP', provider, httpBaseUrl: 'http://100.100.100.100:3773/', wsBaseUrl: 'ws://100.100.100.100:3773/',
        reachability: 'private-network', compatibility: { hostedHttpsApp: 'mixed-content-blocked', desktopApp: 'compatible' }, source: 'desktop-addon', status: 'available',
        description: 'Reachable from devices on the same Tailnet.' },
      { id: 'tailscale-magicdns:https://desktop.tail.ts.net/', label: 'Tailscale HTTPS', provider, httpBaseUrl: 'https://desktop.tail.ts.net/', wsBaseUrl: 'wss://desktop.tail.ts.net/',
        reachability: 'private-network', compatibility: { hostedHttpsApp: 'requires-configuration', desktopApp: 'compatible' }, source: 'desktop-addon', status: 'unavailable',
        description: 'MagicDNS hostname. Configure Tailscale Serve for HTTPS access.' },
    ]);
  });

  it('uses one injected status reader for both IP and MagicDNS endpoints', async () => {
    let readerCalls = 0;
    const endpoints = await resolveTailscaleAdvertisedEndpoints({ port: 3773, networkInterfaces: protonAndTailscaleInterfaces, probe: unexpected.probe,
      readStatus: async () => { readerCalls += 1; return { magicDnsName: 'desktop.tail.ts.net', tailnetIpv4Addresses: ['100.74.126.34'] }; } });
    expect(readerCalls).toBe(1);
    expect(urls(endpoints)).toEqual(['http://100.74.126.34:3773/', 'https://desktop.tail.ts.net/']);
  });

  it('marks the Tailscale HTTPS endpoint available after Serve is enabled and reachable', async () => {
    const endpoints = await resolveTailscaleAdvertisedEndpoints({ port: 3773, networkInterfaces: {}, statusJson: `{"Self":{"DNSName":"desktop.tail.ts.net."}}`, serveEnabled: true,
      readStatus: unexpected.readStatus, probe: async () => true });
    expect(endpoints).toEqual([{ id: 'tailscale-magicdns:https://desktop.tail.ts.net/', label: 'Tailscale HTTPS', provider, httpBaseUrl: 'https://desktop.tail.ts.net/',
      wsBaseUrl: 'wss://desktop.tail.ts.net/', reachability: 'private-network', compatibility: { hostedHttpsApp: 'compatible', desktopApp: 'compatible' }, source: 'desktop-addon',
      status: 'available', description: 'HTTPS endpoint served by Tailscale Serve.' }]);
  });
});
