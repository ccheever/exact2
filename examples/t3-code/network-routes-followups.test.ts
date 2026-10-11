// network-routes-followups (2026-10-11, the user's decision: match the reference T3 Code 1e2ecbd975; MIT, see LICENSE-T3).
// NR-1: a learned route's write goes to its own environment's transport, which checks it at once with the token it
// borrows (supervisor.ts learnRoutesFrom → requestBetterRouteCheck); the walk and the switch are the AppKit tests
// `testALearnedPlainHttpLanRouteConnectsWithTheTokenItBorrows` and `testALearnedRouteAboveTheOneInUseTakesOverAtOnce`
// (macos/tests/transport/routes.swift).
// NR-2: after a Network access change failed (or any of the section's dialogs closed), no other Settings page opens
// that dialog again (ConnectionsSettings.tsx closes it for good): the page's closed view keeps the dialogs' serials.
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { closedNetworkView, networkUi, resetNetwork, runNetworkOp } from './connections-network';
import { connectionsPage, connectionsProjection } from './connections';
import { learnRoutes } from './connection-routes-ops';
import { createdCredentials } from './auth-access';
import { primary } from './local-primary';
import { parseLocalBackendStatus } from './local-backend';
import { resetPrimary } from './local-primary-fixture';
import { toasts } from './toast';
import { T3Client } from './client';
import type { Obj } from './domain';
import type { Native } from './protocol';

beforeEach(() => resetNetwork());
afterEach(() => { resetNetwork(); resetPrimary(); createdCredentials.clear(); });

function fakeNative(answer: (request: Obj) => Obj | undefined): { native: Native; requests: Obj[] } {
  const requests: Obj[] = [];
  return { requests, native: { available: true, watch: () => {}, later: async request => {
    const value = request as Obj;
    requests.push(value);
    return answer(value) ?? { ok: true, generation: 1, value: {} };
  } } };
}
const ready = () => parseLocalBackendStatus({ state: 'ready', enabled: true, port: 16502, httpBaseUrl: 'http://127.0.0.1:16502', wsBaseUrl: 'ws://127.0.0.1:16502',
  bearerReady: true, environmentId: 'env-local', label: 'Lane Mac' });
const LAN = { en0: [{ address: '192.168.1.20', family: 'IPv4', internal: false }], lo0: [{ address: '127.0.0.1', family: 'IPv4', internal: true }] };
const facts = { ok: true, generation: 1, value: { interfaces: LAN, lanHostOverride: '', httpsEndpointUrls: [], tailscale: { read: true, magicDnsName: null, tailnetIpv4Addresses: [] },
  probe: { url: '', read: true, reachable: false } } };
/** desktop-settings.json in a read-only directory, as the audit's lane had it. */
const readOnlySettings = (request: Obj) => request.op === 'localNetworkFacts' ? facts
  : request.op === 'desktopSettingsSet' ? { ok: false, generation: 1, error: { kind: 'DesktopSettings', message: 'Failed to persist desktop server exposure mode network-accessible.' } } : undefined;

/** NetworkDialogs' open rule (connections-network-dialogs.contract): the root's `connectionRemove` names the dialog and its serial. */
const dialogsSource = readFileSync(join(import.meta.dir, 'connections-network-dialogs.contract'), 'utf8');
const networkDialogOpen = (remove: string, view: { networkSerial: number }) => remove === `network:on:${view.networkSerial}` || remove === `network:off:${view.networkSerial}`;
const createDialogOpen = (remove: string, view: { createSerial: number }) => remove === `pairing:create:${view.createSerial}`;

describe('NR-2: a closed network access dialog stays closed on every Settings page', () => {
  it('is the rule the view draws', () => {
    expect(dialogsSource).toContain('when remove == `network:on:${net.networkSerial}` or remove == `network:off:${net.networkSerial}`');
    expect(dialogsSource).toContain('when remove == `pairing:create:${net.createSerial}`');
  });

  it('after a failed Restart and enable, Appearance (the Connections page closed) does not open "Enable network access?" again', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    const { native } = fakeNative(readOnlySettings);
    const remove = `network:on:${networkUi.networkSerial}`; // the switch asked: the root's connectionRemove
    await runNetworkOp(client, native, 'network-access', '', 'on');
    expect(toasts(client).map(toast => toast.title)).toEqual(['Could not update network access']);
    expect(networkUi.exposureError).toContain('Failed to persist desktop server exposure mode network-accessible.');
    // The Connections page, still open: the serial moved, so the dialog closed (the reference closes it after any outcome).
    const open = await connectionsPage(client, native, true, 0);
    expect(networkDialogOpen(remove, open.network)).toBe(false);
    // Another Settings page: the resource answers for a closed Connections page.
    const appearance = await connectionsPage(client, native, false, 0);
    expect(appearance.network.networkSerial).toBe(1);
    expect(networkDialogOpen(remove, appearance.network)).toBe(false);
    // Without the native module (or before it answers) too.
    const bare = connectionsProjection({ connection: 'disconnected', origin: '', environmentId: '', statusMessage: '', scopes: [], config: {} }, []);
    expect([bare.network.visible, networkDialogOpen(remove, bare.network)]).toEqual([false, false]);
  });

  it('the same for Create pairing link after it created the link', async () => {
    const client = new T3Client();
    const { native } = fakeNative(request => request.op === 'localAccess' ? { ok: true, generation: 1, value: { id: 'link-1', credential: 'SECRET', expiresAt: '2036-04-07T00:05:00.000Z' } } : undefined);
    const remove = `pairing:create:${networkUi.createSerial}`;
    await runNetworkOp(client, native, 'pairing-create', 'iPad', 'orchestration:read');
    expect(createDialogOpen(remove, closedNetworkView())).toBe(false);
    expect(createDialogOpen(remove, (await connectionsPage(client, native, false, 0)).network)).toBe(false);
  });
});

describe('NR-1: a learned route is written to its own environment’s transport', () => {
  const A = 'http://100.81.1.2:16503', LAN_ROUTE = 'http://192.168.1.10:16503';
  const route = (origin: string) => ({ id: origin, origin, kind: '', learned: false, credential: origin });
  function store(saved: Obj[]) {
    const calls: Obj[] = [];
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = input as Obj; calls.push(request);
      if (request.op === 'status') return { ok: true, generation: 1, value: { activeRouteId: A } };
      if (request.op === 'setRoutes') {
        saved.find(item => item.environmentId === request.environmentId)!.routes = request.routes;
        return { ok: true, generation: 1, value: { saved } };
      }
      return { ok: true, generation: 1, value: {} };
    } };
    return { native, calls };
  }
  const reported = { directEndpoints: [{ kind: 'lan', httpBaseUrl: `${LAN_ROUTE}/` }, { kind: 'tailnet', httpBaseUrl: `${A}/` }] };

  it('the focused environment: to the focused transport, the LAN address ahead of the tailnet route in use, borrowing its token', async () => {
    const saved: Obj[] = [{ origin: A, environmentId: 'env-a', label: 'Desk', routes: [route(A)] }];
    const { native, calls } = store(saved);
    await learnRoutes(native, { entries: new Map(), saved }, { environmentId: 'env-a', connection: 'connected', config: reported });
    const write = calls.find(call => call.op === 'setRoutes')!;
    expect(write.fleet).toBeUndefined();
    expect((write.routes as Obj[]).map(item => [item.id, item.credential, item.learned])).toEqual([[`learned:env-a:${LAN_ROUTE}@${A}`, A, true], [A, A, false]]);
  });

  it('a background environment: to its fleet transport, which checks the learned route at once', async () => {
    const saved: Obj[] = [{ origin: A, environmentId: 'env-b', label: 'Studio', routes: [route(A)] }];
    const { native, calls } = store(saved);
    const key = `${A}\nenv-b`;
    const entries = new Map([[key, { environmentId: 'env-b', phase: 'connected', config: reported as Obj, activeRouteId: A }]]);
    await learnRoutes(native, { entries, saved }, { environmentId: '', connection: 'disconnected' });
    const write = calls.find(call => call.op === 'setRoutes')!;
    expect(write.fleet).toBe(key);
    expect((write.routes as Obj[]).map(item => item.id)).toEqual([`learned:env-b:${LAN_ROUTE}@${A}`, A]);
  });
});
