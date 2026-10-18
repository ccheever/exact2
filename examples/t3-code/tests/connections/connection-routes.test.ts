// T3 Code 9c161462c9 packages/client-runtime/src/connection/routes.test.ts (its cases, original
// names) and registry.test.ts "EnvironmentRegistry routes" cases that are pure route edits here,
// ported to bun:test over the clone's plain route records (connection-routes.ts).
import { describe, expect, it } from 'bun:test';
import {
  connectionRouteId, connectionRouteKind, connectionRouteLabel, credentialConnectionId, entryWithRoutes, gitHubRoutingConnectionKey,
  insertRoute, mergeLearnedRoutes, registerRoute, removedWithRelay, reorderRoutes, routesAfterRemoving, savedRoutes, sendsTokenInClear,
  singleRouteKey, storedRoute, upsertRoute, usableRoutes, connectionRouteAddress, type ConnectionRoute, type RouteEntry,
} from '../../src/connections/connection-routes';

const ENVIRONMENT_ID = 'environment-1';
const direct = (id: string, httpBaseUrl: string): ConnectionRoute => ({ id, origin: httpBaseUrl.replace(/\/+$/, ''), kind: '' });
const RELAY: ConnectionRoute = { id: 'relay', origin: '', kind: 'relay' };
const LAN = direct('lan', 'http://192.168.1.10:3773/');
const TAILNET = direct('tailnet', 'https://desk.tail1234.ts.net/');
const PUBLIC = direct('public', 'https://desk.example.com/');
const entryOf = (routes: ConnectionRoute[]): RouteEntry => ({ environmentId: ENVIRONMENT_ID, label: 'Desk', routes });
const learned = (id: string, httpBaseUrl: string): ConnectionRoute => ({ ...direct(id, httpBaseUrl), learned: true });

describe('connection routes', () => {
  it('classifies direct routes by address', () => {
    expect(connectionRouteKind(LAN)).toBe('lan');
    // Tailscale, Cloudflare WARP and Mesh, and other VPNs share 100.64.0.0/10.
    expect(connectionRouteKind(direct('ip', 'http://100.101.102.103:3773/'))).toBe('vpn');
    expect(connectionRouteLabel(direct('mesh', 'http://100.96.0.1:3773/'))).toBe('VPN');
    expect(connectionRouteKind(TAILNET)).toBe('tailnet');
    expect(connectionRouteKind(PUBLIC)).toBe('public');
    expect(connectionRouteKind(direct('lo', 'http://127.0.0.1:3773/'))).toBe('loopback');
    expect(connectionRouteKind(direct('ts6', 'http://[fd7a:115c:a1e0::1]:3773/'))).toBe('tailnet');
    expect(connectionRouteLabel(TAILNET)).toBe('Tailscale');
    expect(connectionRouteLabel(RELAY)).toBe('T3 Connect');
  });

  it('places a new route after faster kinds and ahead of T3 Connect', () => {
    expect(insertRoute([RELAY], LAN)).toEqual([LAN, RELAY]);
    expect(insertRoute([LAN, RELAY], TAILNET)).toEqual([LAN, TAILNET, RELAY]);
    expect(insertRoute([TAILNET], LAN)).toEqual([LAN, TAILNET]);
    expect(insertRoute([LAN, TAILNET], RELAY)).toEqual([LAN, TAILNET, RELAY]);
  });

  it("keeps a user's order when a saved route is replaced", () => {
    // The user preferred T3 Connect over the LAN; re-pairing the LAN keeps that.
    const repaired = direct('lan', 'http://192.168.1.11:3773/');
    expect(upsertRoute([RELAY, LAN], repaired)).toEqual([RELAY, repaired]);
  });
});

describe('learned routes', () => {
  const relayOnly = entryOf([RELAY]);
  const ids = (routes: ConnectionRoute[] | null) => routes?.map(route => connectionRouteId(route)) ?? null;

  it('learns a LAN address over T3 Connect, ahead of it, using the T3 Connect credential', () => {
    const routes = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://192.168.1.10:3773/' }], allowInsecure: true });
    expect(ids(routes)).toEqual([`learned:${ENVIRONMENT_ID}:http://192.168.1.10:3773`, 'relay']);
    expect(routes![0]!).toMatchObject({ learned: true, authorization: 't3-connect', origin: 'http://192.168.1.10:3773' });
  });

  it('replaces a learned LAN address when the server reports a new one', () => {
    const first = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://192.168.1.10:3773/' }], allowInsecure: true })!;
    const moved = mergeLearnedRoutes({ entry: entryOf(first), activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://10.0.0.42:3773/' }], allowInsecure: true });
    expect(ids(moved)).toEqual([`learned:${ENVIRONMENT_ID}:http://10.0.0.42:3773`, 'relay']);
  });

  it('keeps a learned route where the user moved it while the server reports it', () => {
    const lan = { httpBaseUrl: 'http://192.168.1.10:3773/' };
    const first = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [lan], allowInsecure: true })!;
    // The user prefers T3 Connect over the learned LAN address.
    const reordered = entryWithRoutes(relayOnly, [first[1]!, first[0]!]);
    expect(mergeLearnedRoutes({ entry: reordered, activeRoute: RELAY, reported: [lan], allowInsecure: true })).toBeNull();
    // A newly reported address is still placed by speed.
    const next = mergeLearnedRoutes({ entry: reordered, activeRoute: RELAY, reported: [lan, { httpBaseUrl: 'http://100.101.102.103:3773/' }], allowInsecure: true });
    expect(ids(next)).toEqual([`learned:${ENVIRONMENT_ID}:http://100.101.102.103:3773`, 'relay', `learned:${ENVIRONMENT_ID}:http://192.168.1.10:3773`]);
  });

  it('labels a learned address Tailscale only while the server reports it as tailnet', () => {
    const tailscale = { kind: 'tailnet', httpBaseUrl: 'http://100.101.102.103:3773/' };
    const mesh = { kind: 'lan', httpBaseUrl: 'http://100.96.0.1:3773/' };
    const first = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [tailscale, mesh], allowInsecure: true })!;
    expect(first.map(connectionRouteLabel)).toEqual(['Tailscale', 'VPN', 'T3 Connect']);
    // The server later finds the address is not on its Tailscale interface.
    const corrected = mergeLearnedRoutes({ entry: entryWithRoutes(relayOnly, first), activeRoute: RELAY, reported: [{ ...tailscale, kind: 'lan' }, mesh], allowInsecure: true })!;
    expect(ids(corrected)).toEqual(ids(first));
    expect(corrected.map(connectionRouteLabel)).toEqual(['VPN', 'VPN', 'T3 Connect']);
    expect(mergeLearnedRoutes({ entry: entryWithRoutes(relayOnly, corrected), activeRoute: RELAY, reported: [{ ...tailscale, kind: 'lan' }, mesh], allowInsecure: true })).toBeNull();
  });

  it('labels a paired numeric Tailscale address Tailscale once the server confirms it', () => {
    const paired = direct('paired', 'http://100.101.102.103:3773/');
    const other = direct('other', 'http://100.96.0.1:3773/');
    const entry = entryWithRoutes(relayOnly, [paired, other, RELAY]);
    expect(connectionRouteLabel(paired)).toBe('VPN');
    const confirmed = mergeLearnedRoutes({ entry, activeRoute: paired, reported: [{ kind: 'tailnet', httpBaseUrl: 'http://100.101.102.103:3773/' }], allowInsecure: true })!;
    // Same routes in the same order; only the label of the confirmed one changes.
    expect(ids(confirmed)).toEqual(['paired', 'other', 'relay']);
    expect(confirmed.map(connectionRouteLabel)).toEqual(['Tailscale', 'VPN', 'T3 Connect']);
  });

  it('leaves user routes alone and does not learn an address already saved', () => {
    const entry = entryOf([LAN, RELAY]);
    expect(mergeLearnedRoutes({ entry, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://192.168.1.10:3773' }], allowInsecure: true })).toBeNull();
    // The server stops reporting the LAN address; the paired route stays.
    expect(mergeLearnedRoutes({ entry, activeRoute: RELAY, reported: [], allowInsecure: true })).toBeNull();
  });

  it('borrows the paired token when learned over a bearer route', () => {
    const routes = mergeLearnedRoutes({ entry: entryOf([LAN]), activeRoute: LAN, reported: [{ httpBaseUrl: 'https://desk.tail1234.ts.net/' }], allowInsecure: true })!;
    const learned = routes.find(route => connectionRouteKind(route) === 'tailnet')!;
    expect(learned).not.toHaveProperty('authorization');
    expect(credentialConnectionId(connectionRouteId(learned))).toBe('lan');
  });

  it('keeps a token paired over HTTPS off plain HTTP addresses outside the tailnet', () => {
    const routes = mergeLearnedRoutes({ entry: entryOf([PUBLIC, RELAY]), activeRoute: PUBLIC, reported: [
      { kind: 'lan', httpBaseUrl: 'http://192.168.1.10:3773/' }, { kind: 'tailnet', httpBaseUrl: 'http://100.101.102.103:3773/' },
      { kind: 'tailnet', httpBaseUrl: 'https://desk.tail1234.ts.net/' }], allowInsecure: true });
    expect(ids(routes)).toEqual([`learned:${ENVIRONMENT_ID}:http://100.101.102.103:3773@public`, `learned:${ENVIRONMENT_ID}:https://desk.tail1234.ts.net@public`, 'public', 'relay']);
  });

  it('drops a plain HTTP LAN route learned earlier over an HTTPS-paired token', () => {
    const lanOrigin = 'http://192.168.1.10:3773';
    const stale = learned(`learned:${ENVIRONMENT_ID}:${lanOrigin}@public`, `${lanOrigin}/`);
    const routes = mergeLearnedRoutes({ entry: entryOf([stale, PUBLIC, RELAY]), activeRoute: PUBLIC, reported: [{ kind: 'lan', httpBaseUrl: `${lanOrigin}/` }], allowInsecure: true });
    expect(ids(routes)).toEqual(['public', 'relay']);
  });

  it('drops a stale plain HTTP route borrowing an HTTPS-paired token while another route is active', () => {
    const lanOrigin = 'http://10.0.0.42:3773';
    const stale = learned(`learned:${ENVIRONMENT_ID}:${lanOrigin}@public`, `${lanOrigin}/`);
    const entry = entryOf([stale, PUBLIC, RELAY]);
    expect(sendsTokenInClear([stale, PUBLIC, RELAY], stale)).toBe(true);
    expect(ids(usableRoutes(entry))).toEqual(['public', 'relay']);
    const routes = mergeLearnedRoutes({ entry, activeRoute: RELAY, reported: [{ kind: 'lan', httpBaseUrl: `${lanOrigin}/` }], allowInsecure: true });
    // The stale route is gone; the address is learned again with T3 Connect's credential.
    expect(ids(routes)).toEqual([`learned:${ENVIRONMENT_ID}:${lanOrigin}`, 'public', 'relay']);
    expect(routes![0]!).toMatchObject({ authorization: 't3-connect' });
  });

  it("keeps plain HTTP routes that don't borrow the active HTTPS-paired token", () => {
    const lanOrigin = 'http://10.0.0.42:3773';
    const overLan = learned(`learned:${ENVIRONMENT_ID}:${lanOrigin}@lan`, `${lanOrigin}/`);
    expect(mergeLearnedRoutes({ entry: entryOf([overLan, LAN, PUBLIC, RELAY]), activeRoute: PUBLIC, reported: [{ kind: 'lan', httpBaseUrl: `${lanOrigin}/` }], allowInsecure: true })).toBeNull();
  });

  it('learns plain HTTP LAN addresses over a token paired over plain HTTP', () => {
    const routes = mergeLearnedRoutes({ entry: entryOf([LAN, RELAY]), activeRoute: LAN, reported: [{ kind: 'lan', httpBaseUrl: 'http://10.0.0.42:3773/' }], allowInsecure: true });
    expect(ids(routes)).toEqual(['lan', `learned:${ENVIRONMENT_ID}:http://10.0.0.42:3773@lan`, 'relay']);
  });

  it('keeps the Tailscale mark through the catalog record', () => {
    const marked: ConnectionRoute = { ...learned(`learned:${ENVIRONMENT_ID}:http://100.101.102.103:3773@lan`, 'http://100.101.102.103:3773/'), network: 'tailscale' };
    const [read] = savedRoutes({ routes: [storedRoute(marked)] });
    expect(read).toMatchObject({ network: 'tailscale', learned: true });
    expect(connectionRouteLabel(read!)).toBe('Tailscale');
  });

  it('skips plain HTTP from an HTTPS page and never learns loopback', () => {
    expect(mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://192.168.1.10:3773/' }, { httpBaseUrl: 'http://127.0.0.1:3773/' }], allowInsecure: false })).toBeNull();
  });

  it('removes learned routes along with the route whose credential they borrow', () => {
    const overRelay = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://192.168.1.10:3773/' }], allowInsecure: true })!;
    expect(routesAfterRemoving(overRelay, 'relay')).toEqual([]);
    const overLan = mergeLearnedRoutes({ entry: entryOf([LAN, RELAY]), activeRoute: LAN, reported: [{ httpBaseUrl: 'https://desk.tail1234.ts.net/' }], allowInsecure: true })!;
    expect(ids(routesAfterRemoving(overLan, 'lan'))).toEqual(['relay']);
    // Removing T3 Connect keeps the paired LAN and what it learned.
    expect(ids(routesAfterRemoving(overLan, 'relay'))).toHaveLength(2);
  });

  it('counts an environment reached only through T3 Connect as removed with it', () => {
    const learned = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://192.168.1.10:3773/' }], allowInsecure: true })!;
    expect(removedWithRelay(relayOnly)).toBe(true);
    expect(removedWithRelay(entryWithRoutes(relayOnly, learned))).toBe(true);
    expect(removedWithRelay(entryWithRoutes(relayOnly, [LAN, RELAY]))).toBe(false);
  });

  it('keeps GitHub routing trust when a route is learned', () => {
    const learned = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://192.168.1.10:3773/' }], allowInsecure: true })!;
    expect(gitHubRoutingConnectionKey(entryOf(learned))).toBe(gitHubRoutingConnectionKey(relayOnly));
  });

  it('keeps the T3 Connect credential when learning over a learned T3 Connect route', () => {
    const first = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://192.168.1.10:3773/' }], allowInsecure: true })!;
    const learnedLan = first[0]!;
    const next = mergeLearnedRoutes({ entry: entryOf(first), activeRoute: learnedLan,
      reported: [{ httpBaseUrl: 'http://192.168.1.10:3773/' }, { httpBaseUrl: 'https://desk.tail1234.ts.net/' }], allowInsecure: true })!;
    for (const route of next.filter(candidate => connectionRouteKind(candidate) !== 'relay')) {
      expect(route).toMatchObject({ authorization: 't3-connect' });
      expect(connectionRouteId(route)).not.toContain('@');
    }
  });

  it('saves a scheme change on the same host as a new address', () => {
    const first = mergeLearnedRoutes({ entry: relayOnly, activeRoute: RELAY, reported: [{ httpBaseUrl: 'http://desk.local:3773/' }], allowInsecure: true })!;
    const moved = mergeLearnedRoutes({ entry: entryOf(first), activeRoute: RELAY, reported: [{ httpBaseUrl: 'https://desk.local:3773/' }], allowInsecure: true });
    expect(moved).not.toBeNull();
    expect(moved![0]!).toMatchObject({ origin: 'https://desk.local:3773' });
  });
});

describe('EnvironmentRegistry routes', () => {
  const LOOPBACK = direct('http://127.0.0.1:16120', 'http://127.0.0.1:16120');
  it('adds a paired LAN route ahead of T3 Connect instead of replacing it', () => {
    expect(ids(registerRoute([RELAY], LAN))).toEqual(['lan', 'relay']);
  });
  it('re-pairing the same address replaces that route rather than adding another', () => {
    const again = direct('lan-again', 'http://192.168.1.10:3773');
    expect(ids(registerRoute([LAN, RELAY], again))).toEqual(['lan-again', 'relay']);
  });
  it('a second SSH host for the same machine adds a route beside the first', () => {
    const one: ConnectionRoute = { id: 'http://127.0.0.1:41001', origin: 'http://127.0.0.1:41001', kind: 'ssh', ssh: { alias: 'devbox', hostname: 'devbox', username: null, port: null } };
    const two: ConnectionRoute = { id: 'http://127.0.0.1:41002', origin: 'http://127.0.0.1:41002', kind: 'ssh', ssh: { alias: 'devbox-ts', hostname: 'devbox.tail.ts.net', username: null, port: null } };
    expect(ids(registerRoute([LOOPBACK, one], two))).toEqual([LOOPBACK.id, one.id, two.id]);
    expect(connectionRouteLabel(two)).toBe('SSH devbox.tail.ts.net');
    expect(connectionRouteAddress(two)).toBe('devbox-ts');
  });
  it('reorders routes and rejects an order that drops one', () => {
    expect(ids(reorderRoutes([LAN, TAILNET, PUBLIC], ['public', 'lan', 'tailnet']))).toEqual(['public', 'lan', 'tailnet']);
    expect(() => reorderRoutes([LAN, TAILNET], ['lan'])).toThrow('The new route order must name every saved route once.');
    expect(() => reorderRoutes([LAN, TAILNET], ['lan', 'lan'])).toThrow();
  });
  it('removing the last route forgets the environment', () => {
    expect(routesAfterRemoving([LAN], 'lan')).toEqual([]);
  });
  it('revokes GitHub routing trust when a route is added but keeps it on reorder', () => {
    const one = gitHubRoutingConnectionKey(entryOf([LAN]));
    const two = gitHubRoutingConnectionKey(entryOf([LAN, TAILNET]));
    expect(one).toBe(singleRouteKey('http://192.168.1.10:3773', ENVIRONMENT_ID));
    expect(two).not.toBe(one);
    expect(gitHubRoutingConnectionKey(entryOf([TAILNET, LAN]))).toBe(two);
    expect(gitHubRoutingConnectionKey(entryOf([LAN, direct('tailnet', 'https://desk2.tail1234.ts.net/')]))).not.toBe(two);
  });
  it('loads saved routes grouped by environment, in saved order', () => {
    expect(savedRoutes({ origin: 'http://127.0.0.1:16120', environmentId: 'e' })).toEqual([LOOPBACK]);
    const stored = [storedRoute(TAILNET), storedRoute({ ...LAN, id: `learned:e:http://192.168.1.10:3773@${TAILNET.id}`, learned: true })];
    expect(stored[1]).toMatchObject({ credential: TAILNET.id, learned: true });
    expect(ids(savedRoutes({ origin: 'x', routes: stored }))).toEqual(['tailnet', `learned:e:http://192.168.1.10:3773@${TAILNET.id}`]);
  });
});
const ids = (routes: ConnectionRoute[] | null) => routes?.map(route => route.id) ?? null;
