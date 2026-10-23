// The routes under a saved environment's row (connection-routes-ops.ts,
// connections.ts), against a stand-in that keeps the catalog's shape (transport-catalog.ts).
import { describe, expect, test } from 'bun:test';
import type { Native } from '../../src/client/protocol';
import { connectionsProjection, runConnectionOp, type ConnectionHost } from '../../src/connections/connections';
import { dropBefore, learnRoutes, placeRoute, routeRows, routesTransportLabel } from '../../src/connections/connection-routes-ops';
import { gitHubRoutingConnectionKey, savedRoutes, singleRouteKey } from '../../src/connections/connection-routes';
import { environmentKey } from '../../src/connections/settings-b-fleet';
import type { Obj } from '../../src/client/domain';

const A = 'http://127.0.0.1:16120', B = 'http://localhost:16120';
const route = (origin: string, extra: Obj = {}) => ({ id: origin, origin, kind: '', learned: false, credential: origin, ...extra });
function store(saved: Obj[], setRoutesError = '') {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = input as Obj; calls.push(request);
    if (request.op === 'environments') return { ok: true, generation: 1, value: { saved } };
    if (request.op === 'status') return { ok: true, generation: 1, value: { activeRouteId: A } };
    if (request.op === 'setRoutes') {
      if (setRoutesError) return { ok: false, generation: 1, error: { kind: 'Persistence', message: setRoutesError } };
      const entry = saved.find(item => item.environmentId === request.environmentId)!;
      entry.routes = request.routes;
      return { ok: true, generation: 1, value: { saved } };
    }
    if (request.op === 'pairEnvironment') {
      if (request.expectedEnvironmentId === 'env-a' && request.origin === 'http://127.0.0.1:16125') return { ok: false, generation: 1, error: { kind: 'Pairing', message: 'That address reaches Studio, a different machine. Add it as its own environment instead.' } };
      (saved[0]!.routes as Obj[]).push(route(String(request.origin)));
      return { ok: true, generation: 1, value: { origin: request.origin, environmentId: 'env-a', label: 'Desk' } };
    }
    return { ok: true, generation: 1, value: {} };
  } };
  return { native, calls };
}
const host = (connection = 'connected'): ConnectionHost => ({ connection, origin: B, environmentId: 'env-a', statusMessage: '', scopes: [], config: { environment: { label: 'Desk' } } });

describe('environment routes', () => {
  test('one row per environment with its route count, "via" the route in use and the list', () => {
    const saved = [{ origin: A, environmentId: 'env-a', label: 'Desk', enabled: true, routes: [route(A), route(B), route('http://192.168.1.10:16120', { id: 'learned:env-a:http://192.168.1.10:16120@' + A, learned: true, credential: A })] }];
    const page = connectionsProjection(host(), saved, new Map(), '{}', { activeRouteId: B });
    expect(page.environments).toHaveLength(1);
    const row = page.environments[0]!;
    expect(row.key).toBe(environmentKey(A, 'env-a'));
    expect(row.subtitle).toBe('via This device · Connected');
    expect(row.routeCount).toBe('3 routes');
    expect(row.routesLabel).toBe('Routes to Desk, preferred first');
    expect(row.routes.map(item => [item.label, item.inUse, item.removable, item.address])).toEqual([
      ['This device', false, true, `${A}/`], ['This device', true, true, `${B}/`], ['LAN', false, false, 'http://192.168.1.10:16120/ · found automatically']]);
    // Disconnected: the first route's label, nothing in use.
    const off = connectionsProjection(host('disconnected'), saved, new Map(), '{}', {}).environments[0]!;
    expect(off.subtitle.startsWith('This device · ')).toBe(true);
    expect(off.routes.some(item => item.inUse)).toBe(false);
  });

  test('a single route keeps the URL subtitle and the plain "Routes" control', () => {
    const page = connectionsProjection(host(), [{ origin: A, environmentId: 'env-a', label: 'Desk', routes: [route(A)] }], new Map(), '{}', { activeRouteId: A });
    expect(page.environments[0]!.subtitle).toBe(`${A}/ · Connected`);
    expect(page.environments[0]!.routeCount).toBe('Routes');
    expect(page.environments[0]!.routes[0]!.removable).toBe(false);
    expect(routesTransportLabel(savedRoutes({ origin: A }), A, true)).toBeNull();
  });

  test('GitHub sharing trust follows the route set: a second route revokes, the learned one does not', () => {
    const one = { environmentId: 'env-a', label: '', routes: [route(A)] };
    expect(gitHubRoutingConnectionKey(one)).toBe(singleRouteKey(A, 'env-a'));
    const two = { ...one, routes: [route(A), route(B)] };
    expect(gitHubRoutingConnectionKey(two)).not.toBe(gitHubRoutingConnectionKey(one));
    expect(gitHubRoutingConnectionKey({ ...one, routes: [route(B), route(A)] })).toBe(gitHubRoutingConnectionKey(two));
  });

  test('a drop lands a route before another, or at the end', () => {
    expect(dropBefore(['a', 'b', 'c'], 'c', 'a')).toEqual(['c', 'a', 'b']);
    expect(dropBefore(['a', 'b', 'c'], 'a', '')).toEqual(['b', 'c', 'a']);
    expect(dropBefore(['a', 'b', 'c'], 'b', 'b')).toEqual(['a', 'b', 'c']);
    expect(routeRows('e', [], '', false)).toEqual([]);
  });

  test('a keyboard drop writes its final order once, preserving learned credentials, trust and the other environment', async () => {
    const learned = route('http://192.168.1.10:16120', { id: `learned:env-a:http://192.168.1.10:16120@${A}`, learned: true, credential: A });
    const first = route(A), second = route(B);
    const target = { origin: A, environmentId: 'env-a', label: 'Desk', enabled: true, routes: [first, second, learned] };
    const other = { origin: B, environmentId: 'env-b', label: 'Studio', enabled: false, routes: [route(B), route(A)] };
    const saved: Obj[] = [other, target], before = structuredClone(saved);
    const trust = gitHubRoutingConnectionKey({ ...target, routes: savedRoutes(target) });
    const { native, calls } = store(saved);

    await runConnectionOp(native, 'environment-route-move', environmentKey(A, 'env-a'), `${learned.id}>${A}\n${A}\n${B}\n${learned.id}`, false);

    const finalRoutes = [learned, first, second];
    expect(calls.filter(call => call.op === 'setRoutes')).toEqual([{ op: 'setRoutes', environmentId: 'env-a', routes: finalRoutes }]);
    expect(saved).toEqual([before[0], { ...before[1], routes: finalRoutes }]);
    expect(trust).not.toBeNull();
    expect(gitHubRoutingConnectionKey({ ...target, routes: savedRoutes(target) })).toBe(trust);
  });

  test('keyboard drops that leave the saved order unchanged make no writes', async () => {
    const C = 'http://192.168.1.10:16120';
    const saved: Obj[] = [{ origin: A, environmentId: 'env-a', label: 'Desk', routes: [route(A), route(B), route(C)] }];
    const before = structuredClone(saved), { native, calls } = store(saved);
    for (const drop of [`${A}>${B}`, `${B}>${B}`, `${C}>`]) {
      await runConnectionOp(native, 'environment-route-move', environmentKey(A, 'env-a'), `${drop}\n${A}\n${B}\n${C}`, false);
      expect(calls.filter(call => call.op === 'setRoutes')).toEqual([]);
      expect(saved).toEqual(before);
    }
  });

  const C = 'http://192.168.1.10:16120', missing = 'https://removed.example.com';
  for (const { name, baseline, drop } of [
    { name: 'changed order', baseline: [B, A, C], drop: `${C}>${A}` },
    { name: 'new route since pickup', baseline: [A, B], drop: `${B}>${A}` },
    { name: 'removed route since pickup', baseline: [A, B, C, missing], drop: `${C}>${A}` },
    { name: 'replaced route since pickup', baseline: [A, B, missing], drop: `${B}>${A}` },
    { name: 'duplicate baseline ID', baseline: [A, A, C], drop: `${C}>${A}` },
    { name: 'missing dragged ID', baseline: [A, B, C], drop: `${missing}>${A}` },
    { name: 'missing destination ID', baseline: [A, B, C], drop: `${A}>${missing}` },
  ]) test(`a keyboard drop rejects ${name} without writing`, async () => {
    const saved: Obj[] = [{ origin: A, environmentId: 'env-a', label: 'Desk', routes: [route(A), route(B), route(C)] }];
    const before = structuredClone(saved), { native, calls } = store(saved);

    await expect(runConnectionOp(native, 'environment-route-move', environmentKey(A, 'env-a'), `${drop}\n${baseline.join('\n')}`, false))
      .rejects.toThrow('Routes changed while reordering. Try again with the current route order.');

    expect(calls.filter(call => call.op === 'setRoutes')).toEqual([]);
    expect(saved).toEqual(before);
  });

  test('a rejected keyboard reorder keeps the saved order and propagates the persistence error', async () => {
    const first = route(A), second = route(B), third = route(C);
    const saved: Obj[] = [{ origin: A, environmentId: 'env-a', label: 'Desk', routes: [first, second, third] }];
    const before = structuredClone(saved), message = 'Route order could not be saved.';
    const { native, calls } = store(saved, message);

    await expect(runConnectionOp(native, 'environment-route-move', environmentKey(A, 'env-a'), `${C}>${A}\n${A}\n${B}\n${C}`, false))
      .rejects.toMatchObject({ kind: 'Persistence', message });

    expect(calls.filter(call => call.op === 'setRoutes')).toEqual([{ op: 'setRoutes', environmentId: 'env-a', routes: [third, first, second] }]);
    expect(saved).toEqual(before);
  });

  test('a second address joins by kind, and a loopback-paired token never learns plain HTTP LAN routes', async () => {
    const tail = 'https://desk.tail1.ts.net';
    const saved: Obj[] = [{ origin: tail, environmentId: 'env-a', label: 'Desk', routes: [route(tail), route(A)] }];
    const { native } = store(saved);
    await placeRoute(native, 'env-a', A);
    expect((saved[0]!.routes as Obj[]).map(item => item.id)).toEqual([A, tail], 'loopback goes ahead of tailnet');
    const fleet = { entries: new Map(), saved };
    await learnRoutes(native, fleet, { environmentId: 'env-a', connection: 'connected', config: { directEndpoints: [{ kind: 'lan', httpBaseUrl: 'http://192.168.1.10:16120' }] } });
    expect((saved[0]!.routes as Obj[]).map(item => [item.id, item.credential])).toEqual([[A, A], [tail, tail]]);
  });

  test('Add route pairs the same machine, and a different machine is refused', async () => {
    const saved: Obj[] = [{ origin: A, environmentId: 'env-a', label: 'Desk', routes: [route(A)] }];
    const { native, calls } = store(saved);
    await runConnectionOp(native, 'environment-route-add', `env-a ${B}`, 'PAIRCODE', false);
    expect(calls.find(call => call.op === 'pairEnvironment')).toMatchObject({ origin: B, expectedEnvironmentId: 'env-a' });
    expect((saved[0]!.routes as Obj[]).map(item => item.id)).toEqual([A, B]);
    await expect(runConnectionOp(native, 'environment-route-add', 'env-a http://127.0.0.1:16125', 'PAIRCODE', false)).rejects.toThrow('a different machine');
    await runConnectionOp(native, 'environment-route-move', environmentKey(A, 'env-a'), `${B}>${A}`, false);
    expect((saved[0]!.routes as Obj[]).map(item => item.id)).toEqual([B, A]);
    await runConnectionOp(native, 'environment-route-remove', environmentKey(A, 'env-a'), A, false);
    expect((saved[0]!.routes as Obj[]).map(item => item.id)).toEqual([B]);
    await expect(runConnectionOp(native, 'environment-route-remove', environmentKey(A, 'env-a'), B, false)).rejects.toThrow('The last route goes with the machine');
  });
});
