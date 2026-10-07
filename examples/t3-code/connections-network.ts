// Settings → Connections › This machine: Network access, its endpoints, Tailscale HTTPS and the
// Authorized clients section (20261005-this-machine-network-access items 3-5), after T3 Code (MIT,
// see LICENSE-T3; reference 1e2ecbd975) ConnectionsSettings.tsx (renderNetworkAccessRow,
// renderEndpointRows, renderTailscaleRow, AuthorizedClientsHeaderAction, PairingClientsList,
// PairingLinkListRow, ConnectedClientListRow, the four dialogs) and state/desktopNetworkAccess.ts.
//  - The exposure state and endpoints are server-exposure.ts over the native facts
//    (`localNetworkFacts`: interfaces, the cached `tailscale status`, the HTTPS probe). The native
//    side answers at once from its caches and refreshes in the background, announcing `t3.local`;
//    until the first complete read the rows say "Loading…", afterwards the last one stays.
//  - A change restarts the embedded server in place through `applyLocalSetting` (this-machine.ts):
//    the reference relaunches the app (decision U4), which exact2 cannot (issue X45, exact2 #122).
//  - Dialogs open on `connectionRemove` (`network:on:<serial>` …) and close when their serial moves:
//    network access after any outcome (the reference closes it and shows the error in the row),
//    Tailscale and Create link only after success (they stay open with a toast on failure).
//  - Expiry labels tick each second while the section shows (the root's clock, X19).
import { obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import type { AdvertisedEndpoint } from './advertised-endpoint';
import { DesktopServerExposure, decodeExposureSettings, defaultEndpointKey, exposureSettings, setDefaultEndpointKey, withServerExposureMode, withTailscaleServe, writeExposureSettings,
  type DesktopServerExposureMode, type DesktopServerExposureState } from './server-exposure';
import type { NetworkInterfaces, NetworkInterfaceInfo } from './tailscale';
import { endpointDefaultPreferenceKey, endpointShareHint, isHostedAppPairingUrl, isQrShareableEndpoint, isTailscaleHttpsEndpoint, resolveAdvertisedEndpointPairingUrl,
  resolveDesktopPairingUrl, resolveHostedPairingUrl, selectPairingEndpoint, selectQrEndpointOption } from './pairing-urls';
import { access, createdCredentials, createPairingCredential, revokeClientSession, revokeOtherClientSessions, revokePairingLink, sortClientSessions, sortPairingLinks,
  summarizeAuthorizedClients, watchAccess, type AuthClientSession, type AuthPairingLink } from './auth-access';
import { qrCells, type QrCell } from './settings-qr';
import { focusedOnPrimary, primary } from './local-primary';
import { applyLocalSetting } from './this-machine';
import { pushToast } from './toast';
import { letGo } from './let-go';
import type { T3Client } from './client';

export const PAIRING_SCOPE_OPTIONS = [
  { scope: 'orchestration:read', title: 'View environment', description: 'Read threads, status, diffs, and configuration.' },
  { scope: 'orchestration:operate', title: 'Operate tasks', description: 'Start tasks and perform changes in the environment.' },
  { scope: 'terminal:operate', title: 'Use terminals', description: 'Create terminals and send input to running shells.' },
  { scope: 'review:write', title: 'Write reviews', description: 'Create comments while reviewing changes.' },
  { scope: 'access:read', title: 'View access', description: 'Inspect pairing links and authorized clients.' },
  { scope: 'access:write', title: 'Manage access', description: 'Issue and revoke credentials for other clients.' },
  { scope: 'relay:read', title: 'View relay', description: 'Inspect managed relay connectivity.' },
  { scope: 'relay:write', title: 'Manage relay', description: 'Change managed tunnel connectivity.' },
] as const;

// ── Time labels (timestampFormat.ts) ─────────────────────────────────────
const accessTimestampFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });
export function formatAccessTimestamp(value: string): string { const parsed = new Date(value); return Number.isNaN(parsed.getTime()) ? value : accessTimestampFormatter.format(parsed); }
/** formatExpiresInLabel: "Expires in 4m 12s", second precision under a day. */
export function formatExpiresInLabel(isoDate: string, nowMs: number): string {
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) return '';
  const diffMs = date.getTime() - nowMs;
  if (diffMs <= 0) return 'Expired';
  const totalSeconds = Math.floor(diffMs / 1000);
  if (totalSeconds < 5) return 'Expires in a moment';
  if (totalSeconds < 60) return `Expires in ${totalSeconds}s`;
  if (totalSeconds < 3600) { const minutes = Math.floor(totalSeconds / 60), seconds = totalSeconds % 60; return seconds === 0 ? `Expires in ${minutes}m` : `Expires in ${minutes}m ${seconds}s`; }
  if (totalSeconds < 86_400) {
    const hours = Math.floor(totalSeconds / 3600), rem = totalSeconds % 3600, minutes = Math.floor(rem / 60), seconds = rem % 60;
    return `Expires in ${[`${hours}h`, ...(minutes > 0 ? [`${minutes}m`] : []), ...(seconds > 0 ? [`${seconds}s`] : [])].join(' ')}`;
  }
  const days = Math.floor(totalSeconds / 86_400), hours = Math.floor((totalSeconds % 86_400) / 3600);
  return hours === 0 ? `Expires in ${days}d` : `Expires in ${days}d ${hours}h`;
}
/** formatElapsedDurationLabel: "Connected for 3m". */
export function formatElapsedDurationLabel(isoDate: string, nowMs: number): string {
  const date = new Date(isoDate);
  if (Number.isNaN(date.getTime())) return '';
  const diffMs = nowMs - date.getTime();
  if (diffMs <= 0) return 'just now';
  const seconds = Math.floor(diffMs / 1000);
  if (seconds < 5) return 'just now';
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  return hours < 24 ? `${hours}h` : `${Math.floor(hours / 24)}d`;
}

// ── The view (connections-network.contract) ──────────────────────────────
export type NetText = { key: string; text: string };
export type NetEndpointRow = { key: string; preferenceKey: string; label: string; url: string; available: boolean; isDefault: boolean; showUrl: boolean;
  needsSetup: boolean; canDisable: boolean; canSetDefault: boolean };
export type NetShareOption = { id: string; label: string; detail: string; url: string; qr: boolean; cells: QrCell[] };
export type NetPairingRow = { id: string; label: string; createdTip: string; expires: string; expiresTip: string; scopeCount: string; scopeLabel: string; scopes: NetText[];
  note: string; share: boolean; copyCode: boolean; options: NetShareOption[]; multiple: boolean; defaultOption: string };
export type NetClientRow = { id: string; label: string; live: boolean; statusTip: string; current: boolean; device: string; scopeCount: string; scopeLabel: string; scopes: NetText[] };
export type NetReveal = { open: boolean; linkId: string; title: string; description: string; value: string; rows: number; qr: boolean; cells: QrCell[] };
export type NetworkView = {
  visible: boolean; loading: boolean; networkOn: boolean; switchReady: boolean; summaryUrl: string; fallback: string; hidden: number; error: string;
  endpoints: NetEndpointRow[]; tailscale: boolean; tailscaleOn: boolean; tailscaleDescription: string; tailscaleUrl: string; tailscaleHost: string; servePort: number;
  clientsVisible: boolean; summary: string; accessError: string; accessLoading: boolean; pairings: NetPairingRow[]; clients: NetClientRow[]; empty: boolean;
  othersDisabled: boolean; networkSerial: number; tailscaleSerial: number; createSerial: number; settled: number; reveal: NetReveal; ticking: boolean;
};

const closedReveal = (): NetReveal => ({ open: false, linkId: '', title: '', description: '', value: '', rows: 3, qr: false, cells: [] });
export function emptyNetworkView(): NetworkView {
  return { visible: false, loading: true, networkOn: false, switchReady: false, summaryUrl: '', fallback: '', hidden: 0, error: '', endpoints: [], tailscale: false, tailscaleOn: false,
    tailscaleDescription: 'Start Tailscale to set up HTTPS access through MagicDNS.', tailscaleUrl: '', tailscaleHost: '', servePort: 443, clientsVisible: false, summary: '',
    accessError: '', accessLoading: true, pairings: [], clients: [], empty: false, othersDisabled: true, networkSerial: 0, tailscaleSerial: 0, createSerial: 0, settled: 0,
    reveal: closedReveal(), ticking: false };
}

/** What the commands leave for the page: mutation errors, the dialogs' serials, the reveal dialog. */
export const networkUi = { exposureError: '', accessError: '', networkSerial: 0, tailscaleSerial: 0, createSerial: 0, settled: 0,
  reveal: null as null | { linkId: string; value: string; kind: 'code' | 'link' } };

const qrCache = new Map<string, QrCell[]>();
function cellsFor(url: string): QrCell[] {
  let cells = qrCache.get(url);
  if (!cells) { cells = qrCells(url, 168, 1, 'M'); if (qrCache.size > 32) qrCache.clear(); qrCache.set(url, cells); }
  return cells;
}
const scopeTexts = (scopes: readonly string[]): NetText[] => scopes.map(scope => ({ key: scope, text: scope }));
const scopeCount = (scopes: readonly string[]) => `${scopes.length} ${scopes.length === 1 ? 'scope' : 'scopes'}`;

export type NetworkInput = { canManage: boolean; state: DesktopServerExposureState | null; endpoints: readonly AdvertisedEndpoint[]; defaultKey: string | null;
  access: { loaded: boolean; error: string; pairingLinks: readonly AuthPairingLink[]; clientSessions: readonly AuthClientSession[] }; credentials: ReadonlyMap<string, string>; now: number;
  loadError?: string };

function pairingRow(link: AuthPairingLink, input: NetworkInput, visibleAll: readonly AdvertisedEndpoint[], defaultKey: string | null): NetPairingRow {
  const credential = input.credentials.get(link.id);
  const endpointUrl = input.state?.endpointUrl ?? null;
  const endpoint = selectPairingEndpoint(visibleAll, defaultKey);
  const endpointPairingUrl = endpoint && credential ? resolveAdvertisedEndpointPairingUrl(endpoint, credential) : null;
  const options = !credential ? [] : visibleAll.filter(candidate => candidate.status !== 'unavailable').map(candidate => {
    const url = resolveAdvertisedEndpointPairingUrl(candidate, credential);
    const qr = isQrShareableEndpoint(candidate);
    return { id: candidate.id, preferenceKey: endpointDefaultPreferenceKey(candidate), label: candidate.label, url, detail: endpointShareHint(candidate, url), qr, cells: qr ? cellsFor(url) : [] };
  });
  // The desktop renders from loopback, so its current origin never makes a shareable link (isLoopbackHostname).
  const shareable = endpointPairingUrl ?? (credential && endpointUrl ? (resolveHostedPairingUrl(endpointUrl, credential) ?? resolveDesktopPairingUrl(endpointUrl, credential)) : null);
  // With no endpoint list the shareable link is the only option the panel can show.
  const shown = options.length ? options : shareable ? [{ id: 'fallback', preferenceKey: '', label: '', url: shareable, detail: '', qr: true, cells: cellsFor(shareable) }] : [];
  return {
    id: link.id, label: link.label ?? 'Pairing link', createdTip: `Link created at ${formatAccessTimestamp(link.createdAt)}`,
    expires: formatExpiresInLabel(link.expiresAt, input.now), expiresTip: formatAccessTimestamp(link.expiresAt),
    scopeCount: scopeCount(link.scopes), scopeLabel: `Pairing link scopes: show ${scopeCount(link.scopes)}`, scopes: scopeTexts(link.scopes),
    note: !credential ? 'Create a new link to share from this client.' : shareable === null ? "Copy the token and pair from another client using this backend's reachable host." : '',
    share: shareable !== null, copyCode: !!credential && shareable === null,
    options: shown.map(({ preferenceKey: _key, ...option }) => option), multiple: options.length > 1,
    defaultOption: selectQrEndpointOption(shown.map(option => ({ ...option, qrShareable: option.qr })), null, defaultKey)?.id ?? '',
  };
}

function clientRow(session: AuthClientSession, now: number): NetClientRow {
  const live = session.current || session.connected, last = session.lastConnectedAt;
  const statusTip = live ? (last ? `Connected for ${formatElapsedDurationLabel(last, now)}` : 'Connected') : last ? `Last connected at ${formatAccessTimestamp(last)}` : 'Not connected yet.';
  const device = [session.client.deviceType !== 'unknown' ? (session.client.deviceType[0]?.toUpperCase() ?? '') + session.client.deviceType.slice(1) : null,
    session.client.os ?? null, session.client.browser ?? null, session.client.ipAddress ?? null].filter((value): value is string => value !== null);
  const label = session.client.label ?? ([session.client.os, session.client.browser].filter(Boolean).join(' · ') || session.subject);
  return { id: session.sessionId, label, live, statusTip, current: session.current, device: device.join(' · '),
    scopeCount: scopeCount(session.scopes), scopeLabel: `Client scopes: show ${scopeCount(session.scopes)}`, scopes: scopeTexts(session.scopes) };
}

/** The section's projection (ConnectionsSettings.tsx), pure for the tests. */
export function networkProjection(input: NetworkInput): NetworkView {
  const view = emptyNetworkView();
  Object.assign(view, { networkSerial: networkUi.networkSerial, tailscaleSerial: networkUi.tailscaleSerial, createSerial: networkUi.createSerial, settled: networkUi.settled });
  if (!input.canManage) return view;
  const state = input.state;
  const networkOn = state?.mode === 'network-accessible';
  const tailscale = input.endpoints.find(isTailscaleHttpsEndpoint) ?? null;
  const visibleNetwork = networkOn ? input.endpoints.filter(endpoint => !isTailscaleHttpsEndpoint(endpoint)) : [];
  const visibleAll = tailscale ? [...visibleNetwork, tailscale] : visibleNetwork;
  const reachable = networkOn || tailscale?.status === 'available';
  const defaultNetwork = selectPairingEndpoint(visibleNetwork, input.defaultKey);
  const defaultAll = defaultNetwork ?? selectPairingEndpoint(tailscale ? [tailscale] : [], input.defaultKey);
  const defaultKey = defaultAll ? endpointDefaultPreferenceKey(defaultAll) : null;
  const links = sortPairingLinks(input.access.pairingLinks).filter(link => new Date(link.expiresAt).getTime() > input.now);
  const sessions = sortClientSessions(input.access.clientSessions);
  let tailscaleHost = '';
  try { tailscaleHost = tailscale ? new URL(tailscale.httpBaseUrl).hostname : ''; } catch { tailscaleHost = ''; }
  return {
    ...view, visible: true, loading: state === null, networkOn, switchReady: state !== null,
    summaryUrl: networkOn ? defaultNetwork?.httpBaseUrl ?? '' : '',
    fallback: !state ? 'Loading…' : !networkOn ? 'Limited to this machine.'
      : state.endpointUrl ? `Reachable at ${state.endpointUrl}` : state.advertisedHost ? `Exposed on all interfaces. Pairing links use ${state.advertisedHost}.` : 'Exposed on all interfaces.',
    hidden: networkOn ? Math.max(visibleNetwork.length - 1, 0) : 0,
    error: networkUi.exposureError || input.loadError || '',
    endpoints: visibleNetwork.map(endpoint => {
      const needsSetup = isTailscaleHttpsEndpoint(endpoint) && endpoint.status !== 'available', isDefault = endpointDefaultPreferenceKey(endpoint) === defaultKey;
      return { key: endpoint.id, preferenceKey: endpointDefaultPreferenceKey(endpoint), label: endpoint.label, url: endpoint.httpBaseUrl, available: endpoint.status === 'available', isDefault,
        showUrl: !needsSetup, needsSetup, canDisable: isTailscaleHttpsEndpoint(endpoint) && endpoint.status === 'available', canSetDefault: !needsSetup && !isDefault };
    }),
    tailscale: tailscale !== null, tailscaleOn: tailscale?.status === 'available',
    tailscaleDescription: tailscale ? (tailscale.status === 'available' ? tailscale.httpBaseUrl : 'Use Tailscale Serve to expose this backend through a MagicDNS HTTPS URL.')
      : 'Start Tailscale to set up HTTPS access through MagicDNS.',
    tailscaleUrl: tailscale?.httpBaseUrl ?? '', tailscaleHost, servePort: state?.tailscaleServePort ?? 443,
    clientsVisible: reachable, summary: summarizeAuthorizedClients(sessions, links),
    accessError: networkUi.accessError || input.access.error, accessLoading: !input.access.loaded,
    pairings: links.map(link => pairingRow(link, input, visibleAll, defaultKey)), clients: sessions.map(session => clientRow(session, input.now)),
    empty: input.access.loaded && links.length === 0 && sessions.length === 0,
    othersDisabled: sessions.every(session => session.current),
    reveal: revealView(networkUi.reveal, visibleAll),
    ticking: reachable,
  };
}

function revealView(reveal: typeof networkUi.reveal, endpoints: readonly AdvertisedEndpoint[]): NetReveal {
  if (!reveal) return closedReveal();
  const url = reveal.kind === 'link', hosted = url && isHostedAppPairingUrl(reveal.value);
  // Never a QR for a loopback URL, even here.
  const loopback = url && endpoints.some(endpoint => !isQrShareableEndpoint(endpoint) && reveal.value.startsWith(endpoint.httpBaseUrl));
  return { open: true, linkId: reveal.linkId, title: url ? (hosted ? 'Hosted app pairing link' : 'Pairing link') : 'Pairing code',
    description: url ? (hosted ? 'Clipboard copy is unavailable here. Open or manually copy this hosted app link on the device you want to connect.'
      : 'Clipboard copy is unavailable here. Open or manually copy this full pairing URL on the device you want to connect.')
      : 'Clipboard copy is unavailable here. Manually copy this code into another client.',
    value: reveal.value, rows: url ? 4 : 3, qr: url && !loopback, cells: url && !loopback ? qrCells(reveal.value, 132, 2, 'M') : [] };
}

// ── The live state behind it ────────────────────────────────────────────
type Local = { local: object };
const ctx = { native: null as Native | null, owner: null as Local | null, pending: false, refresh: false, env: { lanHost: '', httpsEndpoints: [] as string[] }, facts: null as Obj | null };
async function facts(tailscale: boolean, probe = ''): Promise<Obj> {
  if (!ctx.native) throw new Error('Open this app on macOS to read network access.');
  if (!tailscale && ctx.facts) return ctx.facts;
  const reply = await bridgeReply(ctx.native, { op: 'localNetworkFacts', tailscale, probe, refresh: ctx.refresh });
  if (!reply.ok) throw new Error(reply.error!.message);
  const value = obj(reply.value);
  ctx.facts = value;
  ctx.env = { lanHost: str(value.lanHostOverride), httpsEndpoints: (Array.isArray(value.httpsEndpointUrls) ? value.httpsEndpointUrls : []).filter((url): url is string => typeof url === 'string' && !!url.trim()) };
  return value;
}
export function decodeInterfaces(value: unknown): NetworkInterfaces {
  const result: Record<string, NetworkInterfaceInfo[]> = {};
  for (const [name, list] of Object.entries(obj(value))) {
    result[name] = (Array.isArray(list) ? list : []).map(entry => obj(entry)).filter(entry => typeof entry.address === 'string')
      .map(entry => ({ address: str(entry.address), family: str(entry.family), internal: entry.internal === true }));
  }
  return result;
}

const makeExposure = () => new DesktopServerExposure({
  settings: {
    get: () => (ctx.owner ? exposureSettings(ctx.owner) : decodeExposureSettings({})),
    setServerExposureMode: async (mode: DesktopServerExposureMode) => {
      if (!ctx.owner) throw new Error('No preference file.');
      const before = exposureSettings(ctx.owner), next = withServerExposureMode(before, mode);
      writeExposureSettings(ctx.owner, next); // T3Client saves the preference file after the command
      return { changed: next !== before };
    },
    setTailscaleServe: async (input: { enabled: boolean; port?: number }) => {
      if (!ctx.owner) throw new Error('No preference file.');
      const before = exposureSettings(ctx.owner), next = withTailscaleServe(before, input);
      writeExposureSettings(ctx.owner, next);
      return { settings: next, changed: next !== before };
    },
  },
  readNetworkInterfaces: async () => decodeInterfaces((await facts(false)).interfaces),
  readMagicDnsName: async () => { const tailscale = obj((await facts(true)).tailscale); if (tailscale.read !== true) ctx.pending = true; return str(tailscale.magicDnsName) || null; },
  probe: async (url: string) => { const probe = obj((await facts(true, url)).probe); if (probe.read !== true || str(probe.url) !== url) { ctx.pending = true; return false; } return probe.reachable === true; },
  config: { get desktopLanHostOverride() { return ctx.env.lanHost || undefined; }, get desktopHttpsEndpointUrls() { return ctx.env.httpsEndpoints; } },
});
let exposure = makeExposure();
const live = { port: 0, snapshot: null as null | { state: DesktopServerExposureState; endpoints: AdvertisedEndpoint[] }, open: false, error: '' };
/** Tests: forget the exposure state, the last snapshot and the dialogs' serials. */
export function resetNetwork(): void {
  exposure = makeExposure();
  Object.assign(live, { port: 0, snapshot: null, open: false, error: '' });
  Object.assign(networkUi, { exposureError: '', accessError: '', networkSerial: 0, tailscaleSerial: 0, createSerial: 0, settled: 0, reveal: null });
  Object.assign(ctx, { native: null, owner: null, pending: false, refresh: false, facts: null, env: { lanHost: '', httpsEndpoints: [] } });
}

/** Bind the exposure service to this client and read the native facts (each answer starts over). */
function bind(client: Local, native: Native): void { ctx.native = native; ctx.owner = client; ctx.facts = null; ctx.pending = false; }

async function readNetwork(client: Local, native: Native, revalidate: boolean): Promise<void> {
  bind(client, native);
  const port = primary.status.port;
  if (!port || primary.status.state !== 'ready') return;
  ctx.refresh = revalidate;
  try {
    if (live.port !== port) { await exposure.configureFromSettings({ port }); live.port = port; }
    const state = exposure.getState(), endpoints = await exposure.getAdvertisedEndpoints();
    if (!ctx.pending) live.snapshot = { state, endpoints };
    live.error = '';
  } catch (error) { if (letGo(error)) throw error; live.error = error instanceof Error ? error.message : String(error); }
  finally { ctx.refresh = false; }
}

/** The Connections page's network part (connections.ts connectionsPage). */
export async function networkPage(client: T3Client, native: Native, open: boolean, now: number, canManage: boolean): Promise<NetworkView> {
  const opened = open && !live.open;
  live.open = open;
  access.wanted = open && canManage && primary.connectable;
  access.changed = () => { client.revision++; };
  if (!open || !canManage) return networkProjection({ canManage: false, state: null, endpoints: [], defaultKey: null, access: { loaded: false, error: '', pairingLinks: [], clientSessions: [] }, credentials: createdCredentials, now });
  native.watch('t3.local');
  // Subscribe at once when this machine is the focus (the fleet's pass follows its own sync).
  if (focusedOnPrimary(client) && client.connection === 'connected') await watchAccess(client, request => client.restAccess(native).call(request)).catch(error => { if (letGo(error)) throw error; });
  await readNetwork(client, native, opened);
  const sameMachine = access.environmentId === primary.target?.environmentId;
  return networkProjection({ canManage, state: live.snapshot?.state ?? null, endpoints: live.snapshot?.endpoints ?? [], defaultKey: defaultEndpointKey(client),
    access: { loaded: sameMachine && access.loaded, error: sameMachine ? access.error : '', pairingLinks: sameMachine ? access.snapshot.pairingLinks : [],
      clientSessions: sameMachine ? access.snapshot.clientSessions : [] }, credentials: createdCredentials, now, loadError: live.error });
}

// ── Commands ─────────────────────────────────────────────────────────────
export const NETWORK_OPS = ['network-access', 'tailscale-serve', 'endpoint-default', 'pairing-create', 'pairing-revoke', 'client-revoke', 'clients-revoke-others', 'pairing-copy', 'pairing-reveal-close'];
type Result = { status: Obj | null; generation: number };
const none: Result = { status: null, generation: -1 };
const messageOf = (error: unknown, fallback: string) => (error instanceof Error && error.message ? error.message : fallback);

/** Restart the embedded server with the exposure's envelope (applyLocalSetting, the U4 stopgap), and read the facts afresh. */
async function restart(client: T3Client, native: Native): Promise<Result> {
  const config = exposure.backendConfig();
  const result = await applyLocalSetting(client, native, { serverExposure: { host: config.bindHost, tailscaleServeEnabled: config.tailscaleServeEnabled, tailscaleServePort: config.tailscaleServePort } });
  live.snapshot = null; live.port = 0;
  await readNetwork(client, native, true);
  return result;
}

export async function runNetworkOp(client: T3Client, native: Native, op: string, id: string, value: string): Promise<Result> {
  bind(client, native);
  if (op === 'network-access') {
    // handleDesktopServerExposureChange: the dialog closes either way; a failure is the row's red text and a toast.
    networkUi.exposureError = '';
    const previous = exposure.getState().mode;
    try {
      if (live.port !== primary.status.port && primary.status.port) { await exposure.configureFromSettings({ port: primary.status.port }); live.port = primary.status.port; }
      const change = await exposure.setMode(value === 'on' ? 'network-accessible' : 'local-only');
      if (!change.requiresRelaunch) return none;
      try { return await restart(client, native); }
      catch (error) {
        if (letGo(error)) throw error;
        // The server could not come back with the new envelope: put the setting back and restart as it was.
        await exposure.setMode(previous).catch(() => undefined);
        await restart(client, native).catch(() => undefined);
        throw error;
      }
    } catch (error) {
      if (letGo(error)) throw error;
      networkUi.exposureError = messageOf(error, 'Failed to update network exposure.');
      pushToast(client, { kind: 'error', title: 'Could not update network access', description: networkUi.exposureError, stacked: true });
      return none;
    } finally { networkUi.networkSerial++; }
  }
  if (op === 'tailscale-serve') {
    // handleConfirmTailscaleServeSetup / Disable: the dialog closes on success only.
    const enabled = value.startsWith('on');
    networkUi.exposureError = '';
    try {
      const port = enabled ? Number(value.slice(3)) : exposure.getState().tailscaleServePort;
      if (enabled && (!/^\d+$/u.test(value.slice(3)) || !Number.isInteger(port) || port < 1 || port > 65_535)) throw new Error('Enter a port from 1 to 65535.');
      const change = await exposure.setTailscaleServeEnabled({ enabled, port });
      const result = change.requiresRelaunch ? await restart(client, native) : none;
      networkUi.tailscaleSerial++;
      return result;
    } catch (error) {
      if (letGo(error)) throw error;
      networkUi.exposureError = messageOf(error, enabled ? 'Failed to configure Tailscale HTTPS.' : 'Failed to disable Tailscale HTTPS.');
      pushToast(client, { kind: 'error', title: enabled ? 'Could not set up Tailscale HTTPS' : 'Could not disable Tailscale HTTPS', description: networkUi.exposureError, stacked: true });
      return none;
    }
  }
  if (op === 'endpoint-default') {
    if (value) setDefaultEndpointKey(client, value);
    return none;
  }
  if (op === 'pairing-create') {
    // handleCreatePairingLink: the dialog closes on success; its credential stays in memory for this row.
    try {
      const scopes = value.split(' ').filter(scope => PAIRING_SCOPE_OPTIONS.some(option => option.scope === scope));
      if (!scopes.length) throw new Error('Select at least one permission.');
      const created = await createPairingCredential(native, { label: id, scopes });
      createdCredentials.set(created.id, created.credential);
      networkUi.createSerial++;
    } catch (error) {
      if (letGo(error)) throw error;
      pushToast(client, { kind: 'error', title: 'Could not create pairing URL', description: messageOf(error, 'Failed to create pairing URL.'), stacked: true });
    }
    return none;
  }
  if (op === 'pairing-revoke' || op === 'client-revoke' || op === 'clients-revoke-others') {
    networkUi.accessError = '';
    try {
      if (op === 'pairing-revoke') { await revokePairingLink(native, id); createdCredentials.delete(id); }
      else if (op === 'client-revoke') await revokeClientSession(native, id);
      else {
        const revoked = await revokeOtherClientSessions(native);
        pushToast(client, { kind: 'success', title: revoked === 1 ? 'Revoked 1 other client' : `Revoked ${revoked} clients`, description: 'Other paired clients will need a new pairing link before reconnecting.' });
      }
    } catch (error) {
      if (letGo(error)) throw error;
      const [title, fallback] = op === 'pairing-revoke' ? ['Could not revoke pairing link', 'Failed to revoke pairing link.']
        : op === 'client-revoke' ? ['Could not revoke client access', 'Failed to revoke client access.'] : ['Could not revoke other clients', 'Failed to revoke other clients.'];
      networkUi.accessError = messageOf(error, fallback);
      pushToast(client, { kind: 'error', title, description: networkUi.accessError, stacked: true });
    } finally { networkUi.settled++; }
    return none;
  }
  if (op === 'pairing-copy') {
    // useCopyToClipboard: id is the link, value `code` or the option id whose URL to copy. A failed
    // write shows exactly that value in the reveal dialog.
    const credential = createdCredentials.get(id);
    if (!credential) return none;
    let text = credential, kind: 'code' | 'hosted-link' | 'link' = 'code';
    if (value !== 'code') {
      const row = networkProjection({ canManage: true, state: live.snapshot?.state ?? null, endpoints: live.snapshot?.endpoints ?? [], defaultKey: defaultEndpointKey(client),
        access: { loaded: true, error: '', pairingLinks: access.snapshot.pairingLinks.filter(link => link.id === id), clientSessions: [] }, credentials: createdCredentials, now: 0 })
        .pairings.find(pairing => pairing.id === id);
      const url = row?.options.find(option => option.id === value)?.url;
      if (!url) return none;
      text = url; kind = isHostedAppPairingUrl(url) ? 'hosted-link' : 'link';
    }
    const reply = await bridgeReply(native, { op: 'copyText', text });
    if (reply.ok) {
      pushToast(client, { kind: 'success', title: kind === 'hosted-link' ? 'Hosted app link copied' : kind === 'link' ? 'Pairing URL copied' : 'Pairing code copied',
        description: kind === 'hosted-link' ? 'Open it in the browser on the device you want to connect.' : kind === 'link' ? 'Open it in the client you want to pair to this environment.'
          : 'Paste it into another client to finish pairing.' });
    } else {
      networkUi.reveal = { linkId: id, value: text, kind: kind === 'code' ? 'code' : 'link' };
      pushToast(client, { kind: 'error', title: kind === 'hosted-link' ? 'Could not copy hosted app link' : kind === 'link' ? 'Could not copy pairing URL' : 'Could not copy pairing code',
        description: reply.error?.message ?? '', stacked: true });
    }
    return none;
  }
  if (op === 'pairing-reveal-close') { networkUi.reveal = null; return none; }
  return none;
}
