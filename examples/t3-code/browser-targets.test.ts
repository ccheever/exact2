// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3), under their own names: apps/web/src/browser/
// browserTargetResolver.test.ts (19), apps/web/src/components/preview/useDiscoveredLocalServers.test.ts (`mergeServers`,
// 11 with its three it.each rows), previewEmptyStateLogic.test.ts (`getConfiguredPreviewUrls`, 1) and
// apps/web/src/portDiscoveryState.test.ts (`boundConfiguredLocalServerUrls`, 2). Substitution: `readPreparedConnection`
// is the resolver's last argument (the client's connected origin). Clone rows (marked) cover the stream.
import { describe, expect, it } from 'bun:test';
import {
  CONFIGURED_LOCAL_SERVER_URLS_MAX_ITEMS, DISCOVERED_SERVERS_KEY, PREVIEW_URL_MAX_LENGTH, boundConfiguredLocalServerUrls, discoveredServers, discoveredServersEvent,
  getConfiguredPreviewUrls, isPrivateNetworkHost, isPublicFaviconHost, mergeServers, readDiscoveredServers, resolveBrowserNavigationTarget, resolveDiscoveredServerUrl,
  watchDiscoveredServers, type DiscoveredLocalServer,
} from './browser-targets';
import type { T3Client } from './client';
import type { Native } from './protocol';

const ENV = 'environment-1';
const at = (httpBaseUrl: string) => ({ httpBaseUrl });

describe('browser target resolver', () => {
  it('maps environment ports onto a private network host', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'environment-port', port: 5173, path: '/dashboard' }, at('http://192.168.1.25:3773'))).toEqual({
      requestedUrl: 'http://localhost:5173/dashboard', resolvedUrl: 'http://192.168.1.25:5173/dashboard', resolutionKind: 'direct-private-network', environmentId: ENV,
    });
  });
  it('preserves explicit loopback URL navigation for a remote Tailscale environment', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'url', url: 'http://localhost:5173/dashboard?mode=test#results' }, at('http://100.65.180.100:3773'))).toEqual({
      requestedUrl: 'http://localhost:5173/dashboard?mode=test#results', resolvedUrl: 'http://localhost:5173/dashboard?mode=test#results', resolutionKind: 'direct', environmentId: ENV,
    });
  });
  it('preserves explicit IPv4 loopback URL navigation for a private network environment', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'url', url: 'http://127.0.0.1:5999/' }, at('http://192.168.1.50:3773'))).toEqual({
      requestedUrl: 'http://127.0.0.1:5999/', resolvedUrl: 'http://127.0.0.1:5999/', resolutionKind: 'direct', environmentId: ENV,
    });
  });
  it('preserves URL credentials on explicit loopback navigation', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'url', url: 'http://user:p%40ss@localhost:5173/dashboard' }, at('http://100.65.180.100:3773')).resolvedUrl).toBe('http://user:p%40ss@localhost:5173/dashboard');
  });
  it('preserves credentialed loopback URLs for private IPv6 environments', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'url', url: 'http://user:p%40ss@localhost:5173/dashboard?mode=test#results' }, at('http://[fd7a:115c:a1e0::53]:3773')).resolvedUrl)
      .toBe('http://user:p%40ss@localhost:5173/dashboard?mode=test#results');
  });
  it('preserves schemeless localhost navigation for a remote environment', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'url', url: 'localhost:3000/app' }, at('http://192.168.1.25:3773')).resolvedUrl).toBe('localhost:3000/app');
  });
  it('keeps localhost navigation local for a local environment', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'url', url: 'localhost:3000/app' }, at('http://127.0.0.1:3773'))).toEqual({
      requestedUrl: 'localhost:3000/app', resolvedUrl: 'localhost:3000/app', resolutionKind: 'direct', environmentId: ENV,
    });
  });
  it('keeps localhost navigation local for the full IPv4 loopback range', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'url', url: 'http://localhost:3000/app' }, at('http://127.0.0.2:3773'))).toEqual({
      requestedUrl: 'http://localhost:3000/app', resolvedUrl: 'http://localhost:3000/app', resolutionKind: 'direct', environmentId: ENV,
    });
  });
  it('refuses public relay hosts until the authenticated gateway exists', () => {
    expect(() => resolveBrowserNavigationTarget(ENV, { kind: 'environment-port', port: 5173 }, at('https://relay.example.com'))).toThrow(/authenticated preview gateway/);
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'url', url: 'http://localhost:5173' }, at('https://relay.example.com'))).toMatchObject({ resolvedUrl: 'http://localhost:5173', resolutionKind: 'direct' });
  });
  it('normalizes schemeless localhost server-picker values', () => {
    expect(resolveDiscoveredServerUrl(ENV, 'localhost:5173', at('http://localhost:3773'))).toBe('http://localhost:5173/');
    expect(resolveDiscoveredServerUrl(ENV, '0.0.0.0:3000/app', at('http://localhost:3773'))).toBe('http://localhost:3000/app');
  });
  it('maps discovered loopback servers onto a remote environment host', () => {
    expect(resolveDiscoveredServerUrl(ENV, 'localhost:3000/app', at('http://192.168.1.25:3773'))).toBe('http://192.168.1.25:3000/app');
  });
  it('preserves localhost server-picker values when the prepared base is 127.0.0.1', () => {
    expect(resolveDiscoveredServerUrl(ENV, 'localhost:5173/app?x=1#top', at('http://127.0.0.1:3773'))).toBe('http://localhost:5173/app?x=1#top');
  });
  it('normalizes public URLs without treating them as environment ports', () => {
    expect(resolveDiscoveredServerUrl(ENV, 'example.com/app', null)).toBe('https://example.com/app');
  });
  it('supports private IPv6 environment hosts', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'environment-port', port: 5173, path: '/app?mode=test' }, at('http://[fd7a:115c:a1e0::53]:3773')).resolvedUrl).toBe('http://[fd7a:115c:a1e0::53]:5173/app?mode=test');
  });
  it('supports a local IPv6 environment host', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'environment-port', port: 5173 }, at('http://[::1]:3773')).resolvedUrl).toBe('http://localhost:5173/');
  });
  it('maps local IPv4 environment ports onto localhost for dual-stack guests', () => {
    expect(resolveBrowserNavigationTarget(ENV, { kind: 'environment-port', port: 5173, path: '/app' }, at('http://127.0.0.1:3773')).resolvedUrl).toBe('http://localhost:5173/app');
  });
  it('leaves malformed input for the normal navigation error path', () => {
    expect(resolveDiscoveredServerUrl(ENV, '   ', null)).toBe('   ');
  });
  it('classifies exact private IPv4 and IPv6 boundaries', () => {
    const privateHosts = ['0.0.0.0', '10.0.0.0', '10.255.255.255', '100.64.0.0', '100.127.255.255', '127.0.0.0', '127.255.255.255', '169.254.0.0', '169.254.255.255', '172.16.0.0', '172.31.255.255',
      '192.168.0.0', '192.168.255.255', '198.18.0.0', '198.19.255.255', 'fc00::', 'fdff:ffff:ffff:ffff:ffff:ffff:ffff:ffff', 'fe80::', 'febf:ffff:ffff:ffff:ffff:ffff:ffff:ffff', '::ffff:192.168.1.1',
      'localhost.', 'localhost..', 'devbox.', 'devbox..', 'printer.local.', 'printer.local..', 'printer.home.arpa.', 'printer.home.arpa..', 'devbox.example.ts.net.', 'devbox.example.ts.net..'];
    const publicHosts = ['1.0.0.0', '100.63.255.255', '100.128.0.0', '169.253.255.255', '169.255.0.0', '172.15.255.255', '172.32.0.0', '192.167.255.255', '192.169.0.0', '198.17.255.255', '198.20.0.0',
      'fbff:ffff::', 'fec0::', '2001:4860:4860::8888', '::ffff:8.8.8.8', 'example.com.'];
    expect(privateHosts.filter(host => !isPrivateNetworkHost(host))).toEqual([]);
    expect(publicHosts.filter(isPrivateNetworkHost)).toEqual([]);
  });
  it('allows only globally routable hosts to reach a public favicon provider', () => {
    const nonPublic = ['192.0.0.0', '192.0.0.255', '192.0.2.0', '192.0.2.255', '192.88.99.0', '192.88.99.255', '198.51.100.0', '198.51.100.255', '203.0.113.0', '203.0.113.255', '224.0.0.0', '255.255.255.255',
      '::2', '100::', '100::ffff:ffff:ffff:ffff', '100:0:0:1::', '100:0:0:1:ffff:ffff:ffff:ffff', '64:ff9b:1::1', '64:ff9b::a00:1', '64:ff9b::7f00:1', '64:ff9b::c0a8:101', '64:ff9b::c000:201', '2001:5::1',
      '2001:2::', '2001:2:0:ffff:ffff:ffff:ffff:ffff', '2001:db8::', '2001:db8:ffff:ffff:ffff:ffff:ffff:ffff', '3fff::', '3fff:fff:ffff:ffff:ffff:ffff:ffff:ffff', '5f00::1', 'fec0::',
      'ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff', '::ffff:192.0.2.1', 'app.test', 'app.test..', 'printer.local..', 'printer.home.arpa..', 'devbox.example.ts.net..', '127.0.0.1..', '127.1..', '10.1..',
      '172.16.1..', '192.168.1..', 'service.internal', 'hidden.onion'];
    const publicHosts = ['191.255.255.255', '192.0.1.255', '192.0.3.0', '198.51.99.255', '198.51.101.0', '203.0.112.255', '203.0.114.0', '223.255.255.255', '1.1.1.1', '2001:4860:4860::8888',
      '2606:4700:4700::1111', '64:ff9b::808:808', '2001:1::1', '2001:3::1', '2001:4:112::1', '2001:20::1', '2001:30::1', '::ffff:8.8.8.8', 'example.com', 'example.com.'];
    expect(nonPublic.filter(isPublicFaviconHost)).toEqual([]);
    expect(publicHosts.filter(host => !isPublicFaviconHost(host))).toEqual([]);
  });
});

const scannerServer = (overrides: Partial<DiscoveredLocalServer & { requestedUrl: string }>): DiscoveredLocalServer & { requestedUrl: string } => ({
  host: 'localhost', port: 5173, url: 'http://localhost:5173', requestedUrl: overrides.url ?? 'http://localhost:5173', processName: 'vite', pid: 1234, terminal: null, ...overrides,
});

describe('mergeServers', () => {
  it('returns scanner-only entries unchanged', () => {
    const result = mergeServers({ scanner: [scannerServer({})], configuredUrls: [] });
    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({ host: 'localhost', port: 5173, requestedUrl: 'http://localhost:5173', source: 'scanner', processName: 'vite' });
  });
  it('enriches a configured entry with live process metadata when scanner sees it', () => {
    const result = mergeServers({ scanner: [scannerServer({ port: 5173, processName: 'node', pid: 9999 })], configuredUrls: ['http://localhost:5173'] });
    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({ port: 5173, source: 'configured', processName: 'node', pid: 9999 });
  });
  it("excludes configured entries that the live scanner doesn't see", () => {
    expect(mergeServers({ scanner: [], configuredUrls: ['http://localhost:5173'] })).toHaveLength(0);
  });
  it('ignores non-loopback configured URLs', () => {
    const result = mergeServers({ scanner: [scannerServer({})], configuredUrls: ['https://example.com', 'ws://localhost:5173'] });
    expect(result).toHaveLength(1);
    expect(result[0]?.source).toBe('scanner');
  });
  it('sorts configured live servers before scanner-only servers', () => {
    const result = mergeServers({ scanner: [scannerServer({ port: 8080 }), scannerServer({ port: 3000 })], configuredUrls: ['http://localhost:8080'] });
    expect(result.map(s => `${s.source}:${s.port}`)).toEqual(['configured:8080', 'scanner:3000']);
  });
  it('dedupes by lowercased host', () => {
    const result = mergeServers({ scanner: [scannerServer({ host: 'Localhost', port: 5173 })], configuredUrls: ['http://localhost:5173'] });
    expect(result).toHaveLength(1);
    expect(result[0]?.source).toBe('configured');
  });
  it.each(['127.0.0.1', '0.0.0.0', '[::1]'])('matches configured loopback alias %s to a live localhost server', host => {
    const result = mergeServers({ scanner: [scannerServer({ requestedUrl: 'http://localhost:5173/dashboard?mode=test#results' })], configuredUrls: [`http://${host}:5173/dashboard?mode=test#results`] });
    expect(result).toHaveLength(1);
    expect(result[0]?.source).toBe('configured');
    expect(result[0]?.requestedUrl).toBe('http://localhost:5173/dashboard?mode=test#results');
  });
  it('keeps the scanner-verified path and protocol', () => {
    const result = mergeServers({ scanner: [scannerServer({ url: 'https://env-42.example.dev:5173/', requestedUrl: 'http://localhost:5173/dashboard?mode=test#results' })], configuredUrls: ['https://localhost:5173/dashboard?mode=test#results'] });
    expect(result[0]?.url).toBe('https://env-42.example.dev:5173/');
    expect(result[0]?.requestedUrl).toBe('http://localhost:5173/dashboard?mode=test#results');
  });
  it('overlays a configured path when an older server does not advertise path probing', () => {
    const result = mergeServers({ scanner: [scannerServer({ requestedUrl: 'http://localhost:5173/' })], configuredUrls: ['https://localhost:5173/docs?mode=test#results'], configuredUrlProbing: false });
    expect(result[0]?.requestedUrl).toBe('https://localhost:5173/docs?mode=test#results');
  });
  it('does not overlay an unverified configured path when the server probes paths', () => {
    const result = mergeServers({ scanner: [scannerServer({ requestedUrl: 'http://localhost:5173/' })], configuredUrls: ['http://localhost:5173/docs'], configuredUrlProbing: true });
    expect(result[0]?.requestedUrl).toBe('http://localhost:5173/');
  });
  it("keeps a scanner entry's pre-resolution requestedUrl distinct from a resolved url", () => {
    const result = mergeServers({ scanner: [scannerServer({ port: 5173, url: 'https://env-42.example.dev:5173/', requestedUrl: 'http://localhost:5173/' })], configuredUrls: [] });
    expect(result[0]?.url).toBe('https://env-42.example.dev:5173/');
    expect(result[0]?.requestedUrl).toBe('http://localhost:5173/');
  });
});

describe('getConfiguredPreviewUrls', () => {
  it('collects configured preview URLs from project scripts', () => {
    expect(getConfiguredPreviewUrls([{ previewUrl: 'http://localhost:5173' }, {}, { previewUrl: 'http://localhost:3000' }])).toEqual(['http://localhost:5173', 'http://localhost:3000']);
  });
});

describe('boundConfiguredLocalServerUrls', () => {
  it('keeps subscription payloads within the discovery RPC bounds', () => {
    const urls = Array.from({ length: CONFIGURED_LOCAL_SERVER_URLS_MAX_ITEMS + 1 }, (_, index) => `http://localhost:${3_000 + index}`);
    urls.unshift('https://example.com', 'not a URL', `http://localhost/${'a'.repeat(PREVIEW_URL_MAX_LENGTH)}`);
    const bounded = boundConfiguredLocalServerUrls(urls);
    expect(bounded).toHaveLength(CONFIGURED_LOCAL_SERVER_URLS_MAX_ITEMS);
    expect(bounded.every(url => url.startsWith('http://localhost:'))).toBe(true);
  });
  it('does not let fragment-only variants crowd out another server', () => {
    const fragments = Array.from({ length: CONFIGURED_LOCAL_SERVER_URLS_MAX_ITEMS }, (_, index) => `http://localhost:3000/docs#section-${index}`);
    expect(boundConfiguredLocalServerUrls([...fragments, 'http://localhost:4000/app'])).toEqual(['http://localhost:3000/docs#section-0', 'http://localhost:4000/app']);
  });
});

describe('the discovery stream (clone)', () => {
  const fakeClient = () => {
    const calls: Record<string, unknown>[] = [];
    let serial = 0;
    const client = { generation: 1, connection: 'connected', restAccess: () => ({ call: async (request: Record<string, unknown>) => { calls.push(request); return request.op === 'subscribe' ? { id: `sub-${++serial}` } : {}; } }) } as unknown as T3Client;
    return { client, calls };
  };
  const native = {} as Native;
  it('subscribes while the empty state shows, resubscribes when the configured URLs change, and lets go after', async () => {
    const { client, calls } = fakeClient();
    await watchDiscoveredServers(client, native, true, ['http://localhost:5173', 'https://example.com']);
    await watchDiscoveredServers(client, native, true, ['http://localhost:5173']);
    expect(calls).toEqual([{ op: 'subscribe', key: DISCOVERED_SERVERS_KEY, method: 'subscribeDiscoveredLocalServers', payload: { configuredUrls: ['http://localhost:5173/'] } }]);
    discoveredServersEvent(client, { subscriptionId: 'sub-1', value: { servers: [{ host: 'localhost', port: 5173, url: 'http://localhost:5173/', processName: 'vite', pid: 7, terminal: null }, { host: '', port: 1 }], scannedAt: 'now' } });
    expect(discoveredServers(client).servers.map(server => server.port)).toEqual([5173]);
    await watchDiscoveredServers(client, native, true, []);
    expect(calls.slice(1)).toEqual([{ op: 'unsubscribe', key: DISCOVERED_SERVERS_KEY }, { op: 'subscribe', key: DISCOVERED_SERVERS_KEY, method: 'subscribeDiscoveredLocalServers', payload: {} }]);
    discoveredServersEvent(client, { subscriptionId: 'sub-1', value: { servers: [] } }); // the old subscription's late value is ignored
    expect(discoveredServers(client).servers).toHaveLength(1);
    await watchDiscoveredServers(client, native, false, []);
    expect(calls.at(-1)).toEqual({ op: 'unsubscribe', key: DISCOVERED_SERVERS_KEY });
    expect(discoveredServers(client)).toMatchObject({ servers: [], live: false });
  });
  it('waits for the transport\'s retry: a transport error keeps the list, a scheduled retry subscribes again', async () => {
    const { client, calls } = fakeClient();
    await watchDiscoveredServers(client, native, true, []);
    discoveredServersEvent(client, { subscriptionId: 'sub-1', value: { servers: [{ host: 'localhost', port: 5173, url: 'http://localhost:5173/', processName: null, pid: null, terminal: null }] } });
    discoveredServersEvent(client, { subscriptionId: 'sub-1', value: { _transportError: { message: 'not authorized' } } });
    await watchDiscoveredServers(client, native, true, []);
    expect(calls).toHaveLength(1); // no loop past the transport's backoff
    expect(discoveredServers(client).servers).toHaveLength(1);
    discoveredServersEvent(client, { subscriptionId: 'sub-1', value: { _retryDue: true } });
    await watchDiscoveredServers(client, native, true, []);
    expect(calls.map(call => call.op)).toEqual(['subscribe', 'subscribe']);
  });
  it('reads a list defensively', () => {
    expect(readDiscoveredServers({ servers: [{ host: ' localhost ', port: 70000, url: 'x' }, { host: 'localhost', port: 3000, url: 'http://localhost:3000/', processName: ' ', pid: -1, terminal: { threadId: 't', terminalId: 'term' } }], configuredUrlProbing: true }))
      .toEqual({ servers: [{ host: 'localhost', port: 3000, url: 'http://localhost:3000/', processName: null, pid: null, terminal: { threadId: 't', terminalId: 'term' } }], configuredUrlProbing: true });
  });
});
