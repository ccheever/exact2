// network-routes-followups (2026-10-11, the user's decision: match the reference T3 Code 1e2ecbd975; MIT, see LICENSE-T3).
// NR-1: a learned route's write goes to its own environment's transport, which checks it at once with the token it
// borrows (supervisor.ts learnRoutesFrom → requestBetterRouteCheck), and one environment's failed write leaves the others
// to learn; the walk and the switch are the AppKit tests
// `testALearnedPlainHttpLanRouteConnectsWithTheTokenItBorrows` and `testALearnedRouteAboveTheOneInUseTakesOverAtOnce`
// (macos/tests/transport/routes.swift).
// NR-2: after a Network access change failed (or any of the section's dialogs closed), no other Settings page opens
// that dialog again (ConnectionsSettings.tsx closes it for good): the page's closed view keeps the dialogs' serials, checked
// by running NetworkDialogs' own open rules from the contract over the views the pages answer.
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { closedNetworkView, emptyNetworkView, networkUi, resetNetwork, runNetworkOp, type NetworkView } from './connections-network';
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

/**
 * NetworkDialogs' own open rules, run: each `if <rule>` of its view (connections-network-dialogs.contract) and the dialog
 * it draws, the rule as JavaScript (Contract's `==`, `!=`, `and`, `or`, `not` outside strings; a template literal is one in
 * both) over the props `remove` (the root's `connectionRemove`) and `net` (the section's view). A reworded rule runs as written.
 */
const dialogRules = (() => {
  const lines = readFileSync(join(import.meta.dir, 'connections-network-dialogs.contract'), 'utf8').split('\n');
  const start = lines.indexOf('component NetworkDialogs'), view = lines.indexOf('  view', start);
  const end = lines.findIndex((line, index) => index > view && /^\S/.test(line));
  const rules: { dialog: string; open: (remove: string, net: NetworkView) => boolean }[] = [];
  for (let index = view + 1; index < end; index++) {
    const rule = /^    if (.+)$/.exec(lines[index]!)?.[1];
    const dialog = /^      (\w+)\(/.exec(lines[index + 1] ?? '')?.[1];
    if (!rule || !dialog) continue;
    const js = rule.split(/(`[^`]*`|"[^"]*")/).map((part, at) => at % 2 ? part
      : part.replace(/==/g, '===').replace(/!=/g, '!==').replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!')).join('');
    rules.push({ dialog, open: new Function('remove', 'net', `return (${js});`) as (remove: string, net: NetworkView) => boolean });
  }
  return rules;
})();
/** The dialogs NetworkDialogs draws for this `connectionRemove` over this view. */
const openDialogs = (remove: string, net: NetworkView) => dialogRules.filter(rule => rule.open(remove, net)).map(rule => rule.dialog);

describe('NR-2: a closed network access dialog stays closed on every Settings page', () => {
  it('the view opens each dialog on its own name and serial, and the old closed page (serials 0) opened it again', () => {
    const at = { ...emptyNetworkView(), networkSerial: 2, tailscaleSerial: 3, createSerial: 4 };
    expect(dialogRules.map(rule => rule.dialog)).toEqual(['NetworkAccessDialog', 'TailscaleDisableDialog', 'TailscaleSetupDialog', 'CreatePairingDialog', 'PairingRevealDialog']);
    expect(openDialogs('network:on:2', at)).toEqual(['NetworkAccessDialog']);
    expect(openDialogs('network:off:2', at)).toEqual(['NetworkAccessDialog']);
    expect(openDialogs('tailscale:disable:3', at)).toEqual(['TailscaleDisableDialog']);
    expect(openDialogs('tailscale:setup:3', at)).toEqual(['TailscaleSetupDialog']);
    expect(openDialogs('pairing:create:4', at)).toEqual(['CreatePairingDialog']);
    for (const remove of ['network:on:1', 'network:off:3', 'pairing:create:2', '']) expect(openDialogs(remove, at)).toEqual([]);
    // The bug: a closed Connections page answered with emptyNetworkView(), so the first dialog's name matched again.
    expect(openDialogs('network:on:0', emptyNetworkView())).toEqual(['NetworkAccessDialog']);
    expect(openDialogs('pairing:create:0', emptyNetworkView())).toEqual(['CreatePairingDialog']);
  });

  it('after a failed Restart and enable, Appearance (the Connections page closed) does not open "Enable network access?" again', async () => {
    primary.update(ready(), true);
    const client = new T3Client();
    const { native } = fakeNative(readOnlySettings);
    const remove = `network:on:${networkUi.networkSerial}`; // the switch asked: the root's connectionRemove
    expect(openDialogs(remove, (await connectionsPage(client, native, true, 0)).network)).toEqual(['NetworkAccessDialog']);
    expect((await runNetworkOp(client, native, 'network-access', '', 'on')).relaunch).toBeUndefined();
    expect(toasts(client).map(toast => toast.title)).toEqual(['Could not update network access']);
    expect(networkUi.exposureError).toContain('Failed to persist desktop server exposure mode network-accessible.');
    // The Connections page, still open: the serial moved, so the dialog closed (the reference closes it after any outcome).
    const open = await connectionsPage(client, native, true, 0);
    expect(openDialogs(remove, open.network)).toEqual([]);
    // Another Settings page: the resource answers for a closed Connections page.
    const appearance = await connectionsPage(client, native, false, 0);
    expect(openDialogs(remove, appearance.network)).toEqual([]);
    expect(appearance.network.networkSerial).toBe(1);
    // Without the native module (or before it answers) too.
    const bare = connectionsProjection({ connection: 'disconnected', origin: '', environmentId: '', statusMessage: '', scopes: [], config: {} }, []);
    expect([bare.network.visible, openDialogs(remove, bare.network)]).toEqual([false, []]);
  });

  it.each([
    { op: 'network-access', value: 'on', remove: 'network:on:0' },
    { op: 'tailscale-serve', value: 'on:443', remove: 'tailscale:setup:0' },
    { op: 'tailscale-serve', value: 'off', remove: 'tailscale:disable:0' },
  ])('an accepted $op $value relaunch request keeps its completed dialog closed', async ({ op, value, remove }) => {
    primary.update(ready(), true);
    const client = new T3Client();
    const { native } = fakeNative(request => request.op === 'localNetworkFacts' ? facts
      : request.op === 'desktopSettingsSet' ? { ok: true, generation: 1, value: { changed: true, settings: { ...client.localBackend.settings,
        ...(op === 'network-access' ? { serverExposureMode: 'network-accessible' } : { tailscaleServeEnabled: value !== 'off', tailscaleServePort: 443 }) } } } : undefined);
    expect(openDialogs(remove, (await connectionsPage(client, native, true, 0)).network)).toHaveLength(1);
    expect((await runNetworkOp(client, native, op, '', value)).relaunch).toBe(true);
    expect(openDialogs(remove, closedNetworkView())).toEqual([]);
    expect(openDialogs(remove, (await connectionsPage(client, native, false, 0)).network)).toEqual([]);
    expect(openDialogs(remove, (await connectionsPage(client, native, true, 0)).network)).toEqual([]);
  });

  it('the same for Create pairing link after it created the link', async () => {
    const client = new T3Client();
    const { native } = fakeNative(request => request.op === 'localAccess' ? { ok: true, generation: 1, value: { id: 'link-1', credential: 'SECRET', expiresAt: '2036-04-07T00:05:00.000Z' } } : undefined);
    const remove = `pairing:create:${networkUi.createSerial}`;
    expect(openDialogs(remove, (await connectionsPage(client, native, true, 0)).network)).toEqual(['CreatePairingDialog']);
    await runNetworkOp(client, native, 'pairing-create', 'iPad', 'orchestration:read');
    expect(openDialogs(remove, closedNetworkView())).toEqual([]);
    expect(openDialogs(remove, (await connectionsPage(client, native, false, 0)).network)).toEqual([]);
  });
});

describe('NR-1: a learned route is written to its own environment’s transport', () => {
  const A = 'http://100.81.1.2:16503', LAN_ROUTE = 'http://192.168.1.10:16503';
  const route = (origin: string) => ({ id: origin, origin, kind: '', learned: false, credential: origin });
  function store(saved: Obj[], stopped = '') {
    const calls: Obj[] = [];
    const native: Native = { available: true, watch() {}, async later(input) {
      const request = input as Obj; calls.push(request);
      if (request.op === 'status') return { ok: true, generation: 1, value: { activeRouteId: A } };
      // A fleet transport stopped since the pass read its status (T3Fleet.perform).
      if (stopped && request.fleet === stopped) return { ok: false, generation: 1, error: { kind: 'Disconnected', message: 'That environment is not connected.', uncertain: false } };
      if (request.op === 'setRoutes') {
        saved.find(item => item.environmentId === request.environmentId)!.routes = request.routes;
        return { ok: true, generation: 1, value: { saved } };
      }
      return { ok: true, generation: 1, value: {} };
    } };
    return { native, calls };
  }
  const savedRoutesOf = (saved: Obj[], environmentId: string) => (saved.find(item => item.environmentId === environmentId)!.routes as Obj[]).map(item => item.id);
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

  it('one background environment whose transport stopped since the pass read it does not keep the next from learning', async () => {
    const B = 'http://100.81.1.3:16503';
    const saved: Obj[] = [{ origin: A, environmentId: 'env-b', label: 'Studio', routes: [route(A)] }, { origin: B, environmentId: 'env-c', label: 'Lab', routes: [route(B)] }];
    const keyB = `${A}\nenv-b`, keyC = `${B}\nenv-c`;
    const { native, calls } = store(saved, keyB);
    const entries = new Map([
      [keyB, { environmentId: 'env-b', phase: 'connected', config: reported as Obj, activeRouteId: A }],
      [keyC, { environmentId: 'env-c', phase: 'connected', config: { directEndpoints: [{ kind: 'lan', httpBaseUrl: 'http://192.168.1.11:16503/' }] } as Obj, activeRouteId: B }],
    ]);
    const fleet = { entries, saved };
    await learnRoutes(native, fleet, { environmentId: '', connection: 'disconnected' });
    expect(calls.filter(call => call.op === 'setRoutes').map(call => call.fleet)).toEqual([keyB, keyC]);
    expect(savedRoutesOf(fleet.saved, 'env-c')).toEqual(['learned:env-c:http://192.168.1.11:16503@' + B, B]);
    expect(savedRoutesOf(fleet.saved, 'env-b')).toEqual([A]);
  });
});
