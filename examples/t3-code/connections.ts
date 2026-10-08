// Settings → Connections (ConnectionsSettings.tsx, EnvironmentRow.tsx,
// EnvironmentIconPicker.tsx, LoadBalancingSettings.tsx, GitHubRoutingSettings.tsx).
// The primary environment is the embedded server ("This machine", local-primary.ts,
// this-machine.ts): it has its own section and no row under Environments, and it
// leads Load balancing and GitHub sharing. Every server this client pairs with is a
// saved environment, listed under Environments with its own switch and row menu
// (lane r9-connect). Several stay connected at once: the focused one is T3Client's
// connection, the rest are background transports (settings-b-fleet.ts / T3Fleet.swift).
import { obj, str, arr, type Obj } from './domain';
import { ClientError, bridgeReply, parsePairing, type Native } from './protocol';
import { fleet, environmentKey, phaseOf, trimOrigin, EnvironmentFleet, type FleetPhase, type FleetEntry } from './settings-b-fleet';
import { isPrimaryEnvironment, primary, primaryEntry, primaryPhase, sessionScopes, withoutPrimaryDuplicates } from './local-primary';
import { applyLocalSetting, thisMachine } from './this-machine';
import { emptyNetworkView, networkPage, runNetworkOp, NETWORK_OPS } from './connections-network'; // this-machine-network-access
import { pushToast } from './toast';
import { runSshOp, sshTargets, formatSshTarget, SSH_OPS, type SshTarget } from './settings-b-ssh';
import type { T3Client } from './client';
import { forgetProbes, jobFor, outdatedRow, pairOutdated, probeDescriptors, probedDescriptor, readJobs, startOutdatedUpdate } from './settings-b-outdated';
import { runOnEnvironment } from './r4-git-env';
import { balanceSources, balanceSubtitle } from './r11-misc-connections';
import { environmentRows } from './r12-sidebar-connections';
import { withStandardScope } from './remote-scopes';
import { resolveRemotePairingTarget } from './remote';
import { CLIENT_VERSION, versionMismatch } from './version-skew';
import { nativeUpdateDeps, updateEnvironment, updateTargetFromConfig } from './server-update';
import { confirms } from './server-update-notices';
import { configInstallation, desktopManagedOnly, manualUpdateCopy, serverUpdateActionLabel, DESKTOP_MANAGED_NOTE } from './server-installation';
import { gitHubRoutingConnectionKey, savedRoutes, singleRouteKey, type ConnectionRoute } from './connection-routes';
import { moveSavedRoute, placeRoute, removeSavedRoute, routeCountLabel, routeRows, routesTransportLabel, savedEntry, savedList } from './connection-routes-ops';
import { letGo } from './let-go';

export interface ConnectionHost {
  connection: string; origin: string; environmentId: string; statusMessage: string; scopes: string[]; config: Obj;
}
export { CLIENT_VERSION, compareSemver, versionMismatch } from './version-skew';
export const MACHINE_KINDS: [string, string][] = [['server', 'Server'], ['cloud', 'Cloud VM'], ['linux', 'Linux/WSL'], ['desktop', 'Desktop'],
  ['laptop', 'Laptop'], ['mac-mini', 'Mini PC'], ['mac-studio', 'Workstation']];
const KIND_IDS = MACHINE_KINDS.map(([kind]) => kind);
const PREFERENCES = [[100, 'Prefer'], [50, 'Normal'], [25, 'Less often'], [0, 'Manual only']] as const;
const ROUTING = [['off', 'Off'], ['read', 'Read PRs'], ['read-write', 'Read and act']] as const;
const hostOf = (origin: string) => { try { return new URL(origin).host; } catch { return origin; } };
const displayUrl = (origin: string) => `${trimOrigin(origin)}/`;
const splitKey = (key: string) => { const index = key.indexOf('\n'); return index < 0 ? ['', ''] : [key.slice(0, index), key.slice(index + 1)]; };

// ── Status (savedBackendStatus, connectionStatusText) ─────────────────────
export function savedStatus(enabled: boolean, phase: FleetPhase, error: string): { text: string; tone: string } {
  if (!enabled && phase !== 'unsupported') return { text: 'Off', tone: 'muted' };
  if (phase === 'connected') return { text: 'Connected', tone: 'muted' };
  if (phase === 'connecting') return { text: 'Connecting', tone: 'muted' };
  if (phase === 'reconnecting') return { text: error ? `Reconnecting: ${error}` : 'Reconnecting', tone: 'error' };
  if (phase === 'unsupported') return { text: 'Client not supported', tone: 'muted' };
  if (phase === 'error') return { text: error ? `Connection failed: ${error}` : 'Connection failed', tone: 'error' };
  return { text: 'Not connected', tone: 'muted' };
}
export function statusText(phase: FleetPhase, error: string): string {
  if (phase === 'available') return 'Available';
  if (phase === 'connecting') return 'Connecting...';
  if (phase === 'reconnecting') return error ? `Failed to connect. Reconnecting... Reason: ${error}` : 'Reconnecting...';
  if (phase === 'connected') return 'Connected';
  if (phase === 'unsupported') return 'Client not supported';
  return error ? `Connection failed. Reason: ${error}` : 'Connection failed';
}
/** Kept for the focused connection, whose facts arrive as T3Client fields. */
export function environmentStatus(active: boolean, connection: string, message: string): { text: string; tone: string } {
  const phase = phaseOf({ state: active ? connection : 'disconnected', message });
  return savedStatus(true, phase.phase, phase.message);
}

/** resolveEnvironmentMachineKind: the server's icon override, else its detection, else server. */
export function machineKind(config: Obj, fallback = ''): string {
  const icon = str(obj(config.settings).environmentIcon), machine = str(obj(obj(config.environment).platform).machine) || fallback;
  return KIND_IDS.includes(icon) ? icon : KIND_IDS.includes(machine) ? machine : 'server';
}

// ── Device preferences (LoadBalancingSettings, githubRoutingPermissions) ──
export type ConnectionPrefs = { loadBalancingEnabled: boolean; loadBalancingWeights: Record<string, number>; githubRouting: Record<string, string> };
export function decodePrefs(text: string): ConnectionPrefs {
  let raw: Obj = {};
  try { raw = obj(JSON.parse(text || '{}')); } catch { raw = {}; }
  const weights: Record<string, number> = {}, routing: Record<string, string> = {};
  for (const [id, weight] of Object.entries(obj(raw.loadBalancingWeights))) if (typeof weight === 'number' && Number.isFinite(weight)) weights[id] = weight;
  for (const [key, permission] of Object.entries(obj(raw.githubRouting))) if (permission === 'read' || permission === 'read-write') routing[key] = permission;
  return { loadBalancingEnabled: raw.loadBalancingEnabled === true, loadBalancingWeights: weights, githubRouting: routing };
}
export function loadPreference(weight: number | undefined): number {
  if (weight === undefined || weight === 50) return 50;
  if (weight === 0) return 0;
  return weight < 50 ? 25 : 100;
}
const preferenceLabel = (value: number) => PREFERENCES.find(([weight]) => weight === value)![1];
/** Trust belongs to the saved endpoint, not to an environment id alone (one route; several: gitHubRoutingConnectionKey). */
export const routingKey = singleRouteKey;
/** The GitHub sharing key of a saved environment: its routes' endpoints (lane environment-routes). */
const sharingKey = (source: { origin: string; environmentId: string; routes?: ConnectionRoute[] }) =>
  source.routes?.length ? gitHubRoutingConnectionKey({ environmentId: source.environmentId, label: '', routes: source.routes }) ?? routingKey(source.origin, source.environmentId) : routingKey(source.origin, source.environmentId);
/**
 * pr-links-previews-and-routing (githubRoutingPermissions.ts get): each environment's GitHub sharing as the
 * pull request routing reads it, over the key of its current saved endpoints (a changed address or SSH
 * profile no longer matches the stored key, so it reads "off"), with whether it is local (the primary, or a
 * loopback origin: pullRequestRouting.ts isLocal) and switched on.
 */
export function githubSharing(host: ConnectionHost, saved: Obj[], entries: Map<string, FleetEntry>, prefs: ConnectionPrefs): Map<string, { permission: string; local: boolean; enabled: boolean }> {
  const loopback = (origin: string) => { try { return ['localhost', '127.0.0.1', '[::1]'].includes(new URL(origin).hostname); } catch { return false; } };
  return new Map(environmentSources(host, saved, entries).map(source => [source.environmentId,
    { permission: prefs.githubRouting[sharingKey(source)] ?? 'off', local: source.primary || loopback(source.origin), enabled: source.enabled }]));
}
export function summarizeLoad(machines: { environmentId: string; label: string }[], weights: Record<string, number>): string {
  return machines.flatMap(machine => { const value = loadPreference(weights[machine.environmentId]); return value === 50 ? [] : [`${machine.label} ${preferenceLabel(value).toLowerCase()}`]; }).join(' · ');
}
export function summarizeRouting(entries: { label: string; permission: string }[]): string {
  const words: Record<string, string> = { 'read-write': 'read and act', read: 'read PRs' };
  return (['read-write', 'read'] as const).flatMap(permission => {
    const labels = entries.filter(entry => entry.permission === permission).map(entry => entry.label);
    return labels.length ? [`${labels.join(', ')} ${words[permission]}`] : [];
  }).join(' · ');
}

// ── Projection ────────────────────────────────────────────────────────────
export type Source = { key: string; origin: string; environmentId: string; label: string; machine: string; enabled: boolean;
  phase: FleetPhase; error: string; traceId: string; config: Obj; scopes: string[]; focused: boolean; routes: ConnectionRoute[]; activeRouteId: string;
  /** The primary environment (local-primary.ts): no row under Environments, first in Load balancing. */
  primary: boolean };

/** The primary first (once its server named itself), then every saved environment, with live facts: the focused one from T3Client, the rest from the fleet. */
export function environmentSources(host: ConnectionHost, saved: Obj[], entries: Map<string, FleetEntry>, focusedFailure: Obj = {}): Source[] {
  // One saved entry per environment: the focus (whose origin is the route in use) is matched by id. A saved
  // duplicate of the primary (decision U6) is left out; settings-b-fleet.ts removes it.
  const local = primaryEntry();
  const list = [...(local ? [local] : []), ...withoutPrimaryDuplicates(saved).filter(entry => str(entry.origin) && str(entry.environmentId))];
  if (host.environmentId && host.connection !== 'disconnected' && !list.some(entry => str(entry.environmentId) === host.environmentId)) {
    const environment = obj(host.config.environment);
    list.unshift({ origin: host.origin, environmentId: host.environmentId, label: str(environment.label), machine: str(obj(environment.platform).machine), enabled: true });
  }
  return list.map(entry => {
    const origin = str(entry.origin), environmentId = str(entry.environmentId), key = environmentKey(origin, environmentId);
    const focused = !!host.environmentId && environmentId === host.environmentId && host.connection !== 'disconnected', live = entries.get(key);
    const facts = focused ? phaseOf({ state: host.connection, message: host.statusMessage, failureKind: str(focusedFailure.failureKind), traceId: str(focusedFailure.traceId) })
      : live ? { phase: live.phase, message: live.message, traceId: live.traceId } : { phase: 'available' as FleetPhase, message: '', traceId: '' };
    const config = focused ? host.config : live?.config ?? {};
    const isPrimary = entry.primary === true || isPrimaryEnvironment(environmentId);
    // The primary's phase follows its server first (installing and starting connect, a restart reconnects).
    const shown = isPrimary ? primaryPhase(primary.status, { phase: facts.phase, message: facts.message }) : facts;
    return { key, origin, environmentId, label: str(obj(config.environment).label) || str(entry.label) || (isPrimary ? 'This machine' : hostOf(origin)), machine: str(entry.machine),
      enabled: entry.enabled !== false, phase: shown.phase, error: shown.message, traceId: facts.traceId, config, scopes: focused ? host.scopes : live?.scopes ?? [], focused,
      routes: isPrimary ? [] : savedRoutes(entry), activeRouteId: focused ? str(focusedFailure.activeRouteId) : live?.activeRouteId ?? '', primary: isPrimary };
  });
}

/** environmentTransportLabel: "SSH user@host" for a tunnelled environment, else its URL. */
const transportLabel = (origin: string, ssh: Record<string, SshTarget>) => { const target = ssh[trimOrigin(origin)] ?? ssh[`${trimOrigin(origin)}/`]; return target ? `SSH ${formatSshTarget(target)}` : displayUrl(origin); };
/** Lane environment-routes: with several routes, "via <route>" for the one in use, else the first route's label. */
const sourceTransport = (source: Source, ssh: Record<string, SshTarget>) =>
  routesTransportLabel(source.routes, source.activeRouteId, source.phase === 'connected') ?? transportLabel(source.routes[0]?.origin || source.origin, ssh);

/** The icon menu's row while there is no primary (the shape is fixed; the menu is not shown). */
const PLACEHOLDER: Source = { key: '', origin: '', environmentId: '', label: 'This machine', machine: 'desktop', enabled: false, phase: 'available', error: '', traceId: '',
  config: {}, scopes: [], focused: false, routes: [], activeRouteId: '', primary: true };
/** One Environments row's projection (and the This machine icon menu's). */
export type SavedRowView = ReturnType<typeof savedRow>;
export const placeholderRow = (): SavedRowView => savedRow(PLACEHOLDER, 0);
export function savedRow(source: Source, index: number, ssh: Record<string, SshTarget> = {}, probes: Map<string, Obj> = new Map()) {
  // 22e9d35613: a probed descriptor names the protocol direction and whether the host can update itself.
  const outdated = outdatedRow(source.key, source.label, probes.get(source.key), jobFor(source.key));
  const unsupported = source.phase === 'unsupported' || outdated.blocked !== null, enabled = source.enabled && !unsupported, connected = source.phase === 'connected';
  const status = savedStatus(source.enabled, unsupported ? 'unsupported' : source.phase, source.error);
  const serverVersion = str(obj(source.config.environment).serverVersion);
  const mismatch = Object.keys(source.config).length ? versionMismatch(serverVersion) : null;
  const subtitle = [sourceTransport(source, ssh), outdated.resuming ? 'Restarting' : status.text, enabled && mismatch ? serverVersion : ''].filter(Boolean).join(' · ');
  const tooltip = `${unsupported ? outdated.blocked?.message || source.error || statusText('unsupported', '') : enabled ? statusText(source.phase, source.error) : 'Switched off'}${mismatch ? `\nUpdate available: ${mismatch.serverVersion} → ${mismatch.clientVersion}` : ''}`;
  const capabilities = obj(obj(source.config.environment).capabilities);
  const selfUpdate = str(capabilities.serverSelfUpdate);
  const lock = !Object.keys(source.config).length || !connected ? 'Connect to this environment to change its icon.'
    : capabilities.environmentIcon !== true ? "This environment's server is too old to keep an icon. Update it to choose one."
      : source.scopes.length && !source.scopes.includes('orchestration:operate') ? 'Your session on this environment cannot change its settings.' : '';
  const showUpdate = enabled && connected && mismatch !== null;
  // server-update.ts: a running update (the banner's or this row's) shows its progress here instead of the button.
  const updateLabel = showUpdate && !desktopManagedOnly(capabilities) && !outdated.running
    ? serverUpdateActionLabel(selfUpdate, configInstallation(source.config)) : '';
  const kind = Object.keys(source.config).length ? machineKind(source.config) : machineKind({}, source.machine);
  const detected = str(obj(obj(source.config.environment).platform).machine);
  return {
    key: source.key, first: index === 0, origin: source.origin, environmentId: source.environmentId, label: source.label, kind,
    subtitle, tooltip, errorTone: enabled && status.tone === 'error' && !outdated.resuming, enabled, dimmed: !enabled, unsupported,
    outdatedAction: outdated.action, outdatedFrom: outdated.fromVersion, progress: outdated.progress, progressFailed: outdated.failure,
    switchTip: unsupported ? 'Client not supported' : enabled ? 'Switch off' : 'Switch on', active: source.focused, traceId: source.traceId,
    // ServerUpdateAction (3b0093d716): the manual label follows the install; a desktop app that cannot
    // update from here gets the sentence instead of a button.
    update: updateLabel,
    updateNote: showUpdate && desktopManagedOnly(capabilities) ? DESKTOP_MANAGED_NOTE : '',
    updateVersion: mismatch?.clientVersion ?? '',
    iconLock: lock,
    icons: MACHINE_KINDS.map(([id, label]) => ({ kind: id, label, selected: id === kind, note: id === (detected || 'server') ? (detected ? 'detected' : 'default') : '' })),
    // Lane environment-routes (EnvironmentRoutesList): the route count control and the list under the row.
    routeCount: routeCountLabel(source.routes.length), routesLabel: `Routes to ${source.label}, preferred first`,
    routes: routeRows(source.environmentId, source.routes, source.activeRouteId, connected),
  };
}

export function connectionsProjection(host: ConnectionHost, saved: Obj[], entries: Map<string, FleetEntry> = new Map(), prefsText = '{}', focusedFailure: Obj = {}, ssh: Record<string, SshTarget> = {}, probes: Map<string, Obj> = new Map()) {
  // An environment is listed once it is saved (paired); the focus joins before the catalog catches up only
  // once it connected, so a pairing that failed lists nothing, as connectPairing registers nothing. The
  // primary is listed whenever it exists (loadBalancingEnvironments puts it first).
  const savedIds = new Set(saved.map(entry => str(entry.environmentId)));
  const sources = environmentSources(host, saved, entries, focusedFailure);
  const listed = sources.filter(source => source.primary || savedIds.has(source.environmentId) || source.phase === 'connected');
  const prefs = decodePrefs(prefsText);
  // r11-misc: the reference's counting rule (the primary always counts, then switched-on saved ones).
  const machines = balanceSources(listed).map(({ source, primary }, index) => {
    const weight = loadPreference(prefs.loadBalancingWeights[source.environmentId]), routing = prefs.githubRouting[sharingKey(source)] ?? 'off';
    return { key: source.key, first: index === 0, environmentId: source.environmentId, label: source.label,
      kind: Object.keys(source.config).length ? machineKind(source.config) : machineKind({}, source.machine),
      subtitle: balanceSubtitle(primary, sourceTransport(source, ssh)),
      weight: String(weight), weightLabel: preferenceLabel(weight), routing, routingLabel: ROUTING.find(([value]) => value === routing)![1],
      weights: PREFERENCES.map(([value, label]) => ({ value: String(value), label, selected: value === weight })),
      routings: ROUTING.map(([value, label]) => ({ value, label, selected: value === routing })) };
  });
  const rows = environmentRows(listed).map((source, index) => savedRow(source, index, ssh, probes)); // the primary has no row (r12-sidebar-connections.ts)
  const local = sources.find(source => source.primary);
  return {
    connected: host.connection === 'connected', state: host.connection, adminAccess: sessionScopes(host.environmentId, host.scopes).includes('access:write'),
    activeLabel: str(obj(host.config.environment).label) || hostOf(host.origin),
    thisMachine: thisMachine(local, local ? savedRow(local, 0, ssh, probes) : null), // this-machine.ts: the "This machine" section
    network: emptyNetworkView(), // connections-network.ts: Network access, Tailscale HTTPS, Authorized clients (connectionsPage fills it)
    environments: rows, updateCount: rows.filter(row => row.update === 'Update').length,
    machines: machines.length >= 2 ? machines : [],
    loadBalancing: prefs.loadBalancingEnabled,
    loadSummary: prefs.loadBalancingEnabled ? summarizeLoad(machines, prefs.loadBalancingWeights) : 'Off',
    githubSummary: summarizeRouting(machines.map(machine => ({ label: machine.label, permission: machine.routing }))) || 'Off',
  };
}

/** The resource behind the Connections page. */
export async function connectionsPage(host: ConnectionHost, native: Native | null | undefined, open: boolean, now = 0) {
  if (!open || !native?.available) {
    forgetProbes();
    if (native?.available && 'local' in host) await networkPage(host as T3Client, native, false, now, false);
    return connectionsProjection(host, []);
  }
  native.watch('t3.fleet');
  const [listed, prefs, status, ssh] = await Promise.all([bridgeReply(native, { op: 'environments' }), bridgeReply(native, { op: 'connectionPreferences' }),
    bridgeReply(native, { op: 'status' }), sshTargets(native)]);
  const saved = listed.ok ? arr(obj(listed.value).saved) : [], focusStatus = status.ok ? obj(status.value) : {};
  // Switched-off or unsupported environments: their descriptors say whether an outdated host can be updated from here.
  const focusKey = host.environmentId ? environmentKey(str(savedEntry(saved, host.environmentId)?.origin, host.origin), host.environmentId) : '';
  const blocked = saved.map(entry => environmentKey(str(entry.origin), str(entry.environmentId))).filter(key => !key.endsWith('\n')
    && (saved.some(entry => entry.enabled === false && environmentKey(str(entry.origin), str(entry.environmentId)) === key) || fleet.entries.get(key)?.phase === 'unsupported'
      || (key === focusKey && str(focusStatus.failureKind) === 'Protocol')));
  const [probes] = await Promise.all([probeDescriptors(native, blocked), readJobs(native)]);
  const page = connectionsProjection(host, saved, fleet.entries, prefs.ok ? str(obj(prefs.value).text, '{}') : '{}', focusStatus, ssh, probes);
  if ('local' in host) page.network = await networkPage(host as T3Client, native, true, now, page.thisMachine.canManage);
  return page;
}

// ── Writes ────────────────────────────────────────────────────────────────
type Result = { status: Obj | null; generation: number };
async function call(native: Native, request: Obj) {
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  return reply;
}
const focusOf = (client: Pick<T3Client, 'origin' | 'environmentId' | 'connection'> | undefined) =>
  client ? { origin: client.origin, environmentId: client.environmentId, connection: client.connection } : { origin: '', environmentId: '', connection: 'disconnected' };
// A let-go request is rethrown, never toasted (let-go.ts).
const failed = (client: T3Client | undefined, title: string, error: unknown) => {
  if (letGo(error)) throw error;
  if (client) pushToast(client, { kind: 'error', title, description: error instanceof Error ? error.message : String(error), stacked: true });
};

/** ServerUpdatesAction's eligible targets: connected, behind this client, self-updatable. */
export function updateTargets(saved: Obj[], focus: { origin: string; environmentId: string; connection: string }, client: Pick<T3Client, 'config' | 'generation'> | undefined, native?: Native, source: EnvironmentFleet = fleet) {
  return saved.filter(entry => entry.enabled !== false).flatMap(entry => {
    const key = environmentKey(str(entry.origin), str(entry.environmentId)), focused = !!focus.environmentId && str(entry.environmentId) === focus.environmentId && focus.connection === 'connected';
    const live = source.entries.get(key), config = focused ? obj(client?.config) : live?.phase === 'connected' ? live.config : {};
    const environment = obj(config.environment), capabilities = obj(environment.capabilities), selfUpdate = str(capabilities.serverSelfUpdate);
    if (!Object.keys(config).length || !versionMismatch(str(environment.serverVersion)) || !selfUpdate) return [];
    if (selfUpdate === 'desktop-managed' && capabilities.desktopAppUpdate !== true) return [];
    const target = native ? (focused ? native : EnvironmentFleet.native(native, key)) : (undefined as unknown as Native);
    return [{ key, label: `${str(environment.label, 'Environment')} server`, selfUpdate, native: target, generation: focused ? client!.generation : live!.generation,
      environmentId: str(entry.environmentId), config }];
  });
}

async function writePrefs(native: Native, change: (prefs: ConnectionPrefs) => void) {
  const current = decodePrefs(str(obj((await call(native, { op: 'connectionPreferences' })).value).text, '{}'));
  change(current);
  await call(native, { op: 'setConnectionPreferences', text: JSON.stringify(current) });
}

/**
 * r9-connect: a pairing that failed while nothing was connected saves nothing (connectPairing
 * registers only a validated connection). The transport tried the new origin as its focus, so
 * it goes back: to the saved environment it was focused on, else to no connection at all.
 */
async function abandonPairing(native: Native, before: { origin: string; environmentId: string }, client?: T3Client) {
  const saved = await bridgeReply(native, { op: 'environments' }).then(reply => reply.ok ? arr(obj(reply.value).saved) : []).catch(() => [] as Obj[]);
  const back = before.environmentId ? saved.find(entry => entry.enabled !== false && str(entry.environmentId) === before.environmentId) : undefined;
  // The reconnect completes when its socket opens; its progress arrives as t3.status changes.
  if (back) void native.later({ op: 'connect', origin: str(back.origin), credential: '' }).catch(() => undefined);
  else {
    // r10-connect: nothing to go back to. The transport drops the failed origin (`abandon`), and the
    // client's origin returns to the one it named before, so no state names the failed host.
    await native.later({ op: 'disconnect', forget: false, abandon: true }).catch(() => undefined);
    if (client) client.origin = before.origin;
  }
}

/**
 * One Connections write. Returns the native status reply for T3Client to adopt
 * (the focused connection changed), or a null status.
 */
export async function runConnectionOp(native: Native, op: string, id: string, value: string, connected: boolean, client?: T3Client, options: { failureTitle?: string } = {}): Promise<Result> {
  if (SSH_OPS.includes(op)) {
    // Add Environment → SSH (settings-b-ssh.ts): tunnel, pair, then keep it connected beside the focus.
    // A failure is the dialog's Alert (setSavedBackendError), not a toast.
    const result = await runSshOp(native, op, id, value, connected, client);
    if (connected) await fleet.sync(native, focusOf(client));
    return result;
  }
  // lane r4-git: "Run on" moves a draft to another connected environment (r4-git-env.ts).
  if (op === 'environment-run-on') { if (!client) throw new ClientError('Open a draft first.'); return runOnEnvironment(client, native, value); }
  if (op === 'environment-add' || op === 'welcome-pair') {
    const host = id.trim(), code = value.trim();
    const welcome = op === 'welcome-pair';
    try {
      if (!welcome && !host && !/^(https?|wss?):\/\//i.test(code)) throw new ClientError('Enter a backend host.');
      const target = welcome ? resolveRemotePairingTarget({ pairingUrl: code }) : parsePairing(host || code, code);
      if (!target.credential && connected) throw new ClientError('Enter a pairing code.');
      if (connected) {
        // 22e9d35613 preparePairingRegistration: an outdated host that can update itself is still saved (switched off).
        // U6: pairing this machine's own server saves nothing (the reference's register is a no-op for the primary's id).
        const paired = await call(native, { op: 'pairEnvironment', ...withStandardScope(target), primaryEnvironmentId: primary.target?.environmentId ?? '' }).catch(async error => {
          if (!(error instanceof ClientError) || error.kind !== 'Protocol') throw error;
          try { await pairOutdated(native, target.origin, target.credential); }
          catch (outdated) {
            // A compatible host failed for another reason: keep the transport's own message.
            if (outdated instanceof ClientError && outdated.kind === 'Compatible') throw error;
            throw new ClientError(outdated instanceof Error ? outdated.message : String(outdated), 'Protocol');
          }
        });
        // Lane environment-routes: pairing a saved machine at another address adds a route, placed by kind.
        const added = obj(paired?.value);
        if (str(added.environmentId) && added.primary !== true) await placeRoute(native, str(added.environmentId), str(added.origin)).catch(() => {});
        if (client && !welcome) pushToast(client, { kind: 'success', title: 'Backend added', description: 'The environment is saved and will reconnect on app startup.' });
        await fleet.sync(native, focusOf(client));
        return { status: null, generation: -1 };
      }
      const before = focusOf(client);
      try {
        const reply = await call(native, { op: 'connect', ...withStandardScope(target) });
        const opened = obj(reply.value);
        if (str(opened.environmentId)) await placeRoute(native, str(opened.environmentId), str(opened.origin)).catch(() => {});
        // r10-connect: handleAddSavedBackend's success is the same with or without a connection: the dialog
        // closes over Settings › Connections and the toast says so (app.contract keeps Settings open).
        if (client && !welcome) pushToast(client, { kind: 'success', title: 'Backend added', description: 'The environment is saved and will reconnect on app startup.' });
        return { status: obj(reply.value), generation: reply.generation };
      } catch (error) { if (!letGo(error)) await abandonPairing(native, before, client); throw error; }
    } catch (error) { if (!welcome) failed(client, 'Could not add backend', error); throw error; }
  }
  if (op === 'environment-route-add') {
    // "Add a route to <label>" (ConnectionsSettings.tsx routeTarget): pair the same machine at another address.
    // id is `<environment id> <host>`, value the pairing code; the code is not spent on a different machine.
    const space = id.indexOf(' '), environmentId = id.slice(0, Math.max(0, space)), host = id.slice(space + 1).trim(), code = value.trim();
    try {
      if (!environmentId) throw new ClientError('Choose a saved environment.');
      if (!host && !/^(https?|wss?):\/\//i.test(code)) throw new ClientError('Enter a backend host.');
      const target = parsePairing(host || code, code);
      if (!target.credential) throw new ClientError('Enter a pairing code.');
      const label = str(savedEntry(await savedList(native), environmentId)?.label, 'This environment');
      const reply = await call(native, { op: 'pairEnvironment', ...withStandardScope(target), expectedEnvironmentId: environmentId });
      await placeRoute(native, environmentId, str(obj(reply.value).origin, target.origin));
      if (client) pushToast(client, { kind: 'success', title: 'Route added', description: `${label} now has another way to connect.` });
      if (connected) await fleet.sync(native, focusOf(client));
      return { status: null, generation: -1 };
    } catch (error) { failed(client, 'Could not add route', error); throw error; }
  }
  if (op === 'environment-route-move' || op === 'environment-route-remove') {
    // EnvironmentRoutesList: reorder (drag or keyboard) and remove; id is the row key.
    const [, environmentId] = splitKey(id);
    try {
      if (op === 'environment-route-move') await moveSavedRoute(native, environmentId, value);
      else await removeSavedRoute(native, environmentId, value);
      // A background environment walks its routes again on its next connection.
      const key = [...fleet.entries.keys()].find(candidate => candidate.endsWith(`\n${environmentId}`));
      if (op === 'environment-route-remove' && key && fleet.entries.get(key)?.activeRouteId === value) {
        fleet.forget(key); await native.later({ op: 'fleetStop', fleet: key }).catch(() => {});
      }
      if (connected) await fleet.sync(native, focusOf(client));
    } catch (error) { failed(client, op === 'environment-route-move' ? 'Could not reorder routes' : 'Could not remove route', error); throw error; }
    return { status: null, generation: -1 };
  }
  if (op === 'environment-enabled' || op === 'environment-switch') {
    // The row switch: id is the environment key (or an origin for the legacy op).
    const enabled = value === 'on';
    try {
      const [origin, environmentId] = id.includes('\n') ? splitKey(id) : [id, str((client && trimOrigin(id) === trimOrigin(client.origin)) ? client.environmentId : '')];
      if (!environmentId) {
        // Legacy: an origin without an identity switches the focused connection.
        const reply = enabled ? await call(native, { op: 'connect', origin: id, credential: '' }) : await call(native, { op: 'disconnect', forget: false });
        return { status: obj(reply.value), generation: reply.generation };
      }
      const key = environmentKey(origin, environmentId), focus = focusOf(client);
      const saved = arr(obj((await call(native, { op: 'setEnvironmentEnabled', origin, environmentId, enabled })).value).saved);
      fleet.forget(key);
      let result: Result = { status: null, generation: -1 };
      if (!enabled && focus.environmentId === environmentId && focus.connection !== 'disconnected') {
        // Switching the focused environment off hands focus to the primary when it is ready, else another switched-on saved one.
        let reply = await call(native, { op: 'disconnect', forget: false });
        const local = primary.connectable ? primaryEntry() : null;
        const next = local ?? saved.find(entry => entry.enabled !== false && str(entry.environmentId) && str(entry.environmentId) !== environmentId);
        if (next) {
          fleet.forget(environmentKey(str(next.origin), str(next.environmentId)));
          await native.later({ op: 'fleetStop', fleet: environmentKey(str(next.origin), str(next.environmentId)) }).catch(() => {});
          reply = await call(native, { op: 'connect', origin: str(next.origin), ...(next.primary ? { primary: true } : { credential: '' }) });
        }
        result = { status: obj(reply.value), generation: reply.generation };
      } else if (enabled && (focus.connection === 'disconnected' || focus.connection === 'error' || !focus.environmentId)) {
        const reply = await call(native, { op: 'connect', origin, credential: '' });
        result = { status: obj(reply.value), generation: reply.generation };
      }
      const nextFocus = result.status ? { origin: str(result.status.origin), environmentId: str(result.status.environmentId), connection: str(result.status.state) } : focus;
      await native.later({ op: 'fleetStop', fleet: key }).catch(() => {});
      await fleet.sync(native, nextFocus);
      return result;
    } catch (error) { failed(client, options.failureTitle ?? `Could not switch backend ${enabled ? 'on' : 'off'}`, error); throw error; }
  }
  if (op === 'environment-forget') {
    try {
      const key = environmentKey(id, value);
      const reply = await call(native, { op: 'forgetEnvironment', origin: id, environmentId: value });
      fleet.forget(key);
      await native.later({ op: 'sshForget', origin: trimOrigin(id) }).catch(() => {}); // an SSH environment's tunnel goes with it
      await native.later({ op: 'fleetStop', fleet: key }).catch(() => {});
      return { status: obj(reply.value), generation: reply.generation };
    } catch (error) { failed(client, 'Could not remove backend', error); throw error; }
  }
  if (op === 'environment-trace') {
    // "Copy trace ID" (useCopyToClipboard): value is the trace from the row.
    try {
      if (!value || value.length > 256) throw new ClientError('That trace ID is unavailable.');
      await call(native, { op: 'copyText', text: value });
      if (client) pushToast(client, { kind: 'success', title: 'Trace ID copied', description: value });
    } catch (error) { failed(client, 'Could not copy trace ID', error); throw error; }
    return { status: null, generation: -1 };
  }
  if (op === 'environment-icon') {
    // EnvironmentIconMenu: picking the detected kind clears the override.
    const [origin, environmentId] = splitKey(id);
    const key = environmentKey(origin, environmentId), focus = focusOf(client);
    const focused = !!focus.environmentId && focus.environmentId === environmentId;
    const live = fleet.entries.get(key);
    const config = focused ? obj(client?.config) : live?.config ?? {};
    if (!KIND_IDS.includes(value)) throw new ClientError('Choose one of the listed icons.');
    if (!Object.keys(config).length) throw new ClientError('Connect to this environment to change its icon.');
    const detected = str(obj(obj(config.environment).platform).machine) || 'server';
    const target = focused ? native : EnvironmentFleet.native(native, key);
    const generation = focused ? client!.generation : live!.generation;
    await call(target, { op: 'request', method: 'server.updateSettings', payload: { patch: { environmentIcon: value === detected ? null : value } }, generation });
    if (focused && client) client.config = obj((await call(native, { op: 'request', method: 'server.getConfig', payload: {}, generation })).value);
    else if (live) live.config = obj((await call(target, { op: 'request', method: 'server.getConfig', payload: {}, generation })).value);
    return { status: null, generation: -1 };
  }
  if (op === 'environment-update') {
    // ServerUpdateAction: a server without self-update gets the manual command.
    const [origin, environmentId] = splitKey(id), key = environmentKey(origin, environmentId);
    const focus = focusOf(client), focused = !!focus.environmentId && focus.environmentId === environmentId;
    const live = fleet.entries.get(key), config = focused ? obj(client?.config) : live?.config ?? {};
    const label = `${str(obj(config.environment).label, 'Environment')} server`;
    const selfUpdate = str(obj(obj(config.environment).capabilities).serverSelfUpdate);
    try {
      if (!versionMismatch(str(obj(config.environment).serverVersion))) throw new ClientError('This server is already up to date.');
      if (!selfUpdate) {
        // ServerUpdateAction's manual path: copy the command that matches the install; no remote call.
        const manual = manualUpdateCopy(CLIENT_VERSION, configInstallation(config), label);
        try { await call(native, { op: 'copyText', text: manual.command }); }
        catch (error) { if (letGo(error)) throw error; throw new ClientError(manual.failureMessage); }
        if (client) pushToast(client, { kind: 'success', title: manual.title, description: manual.description });
        return { status: null, generation: -1 };
      }
      // server-update.ts: the shared, single-flight update state the composer banner shows too; its
      // result toasts once the connection reports the new version. A desktop app asks first.
      const target = updateTargetFromConfig(config, key, environmentId, label);
      if (target.selfUpdate === 'desktop-managed' && target.desktopAppUpdate && client) { confirms.set(client, target); return { status: null, generation: -1 }; }
      await updateEnvironment(target, nativeUpdateDeps(native, client ?? null));
    } catch (error) { failed(client, selfUpdate ? 'Server update failed' : 'Could not copy update command', error); throw error; }
    return { status: null, generation: -1 };
  }
  if (op === 'environment-update-outdated') {
    // OutdatedServerUpdateAction: single-flight per environment; progress and toasts follow "t3.fleet".
    const [origin, environmentId] = splitKey(id), key = environmentKey(origin, environmentId);
    try {
      if (!environmentId) throw new ClientError('Choose a saved environment to update.');
      // The version comes from the host descriptor: an outdated host never delivers a server config.
      await startOutdatedUpdate(native, key, value, str(probedDescriptor(key)?.serverVersion), CLIENT_VERSION);
      await readJobs(native);
    } catch (error) { failed(client, 'Server update failed', error); throw error; }
    return { status: null, generation: -1 };
  }
  if (op === 'environment-update-all') {
    // ServerUpdatesAction "Update all": every connected, self-updatable saved
    // environment behind this client, updated independently.
    const focus = focusOf(client), saved = arr(obj((await call(native, { op: 'environments' })).value).saved);
    const targets = updateTargets(saved, focus, client, native);
    if (!targets.length) throw new ClientError('No saved environment can update itself right now.');
    // Each through the shared single-flight update (server-update.ts), its failure named by its label.
    await Promise.all(targets.map(target => updateEnvironment(updateTargetFromConfig(target.config, target.key, target.environmentId, target.label),
      nativeUpdateDeps(native, client ?? null), `${target.label} update failed`)));
    return { status: null, generation: -1 };
  }
  if (op === 'local-environment') {
    // "This machine" › Local environment, after its confirm dialog (this-machine.ts); the reason of a failure shows in the dialog.
    if (!client) throw new ClientError('Open this app on macOS to change this setting.');
    if (value !== 'on' && value !== 'off') throw new ClientError("Couldn't change this setting.");
    return applyLocalSetting(client, native, { localEnvironmentEnabled: value === 'on' });
  }
  // this-machine-network-access: Network access, Tailscale HTTPS, pairing links and clients (connections-network.ts).
  if (NETWORK_OPS.includes(op)) { if (!client) throw new ClientError('Open this app on macOS to change this setting.'); return runNetworkOp(client, native, op, id, value); }
  if (op === 'load-balancing') {
    await writePrefs(native, prefs => { prefs.loadBalancingEnabled = value === 'true'; });
    return { status: null, generation: -1 };
  }
  if (op === 'load-weight') {
    const weight = Number(value);
    if (![100, 50, 25, 0].includes(weight) || !id) throw new ClientError('Choose a load preference.');
    await writePrefs(native, prefs => { prefs.loadBalancingWeights = { ...prefs.loadBalancingWeights, [id]: weight }; });
    return { status: null, generation: -1 };
  }
  if (op === 'github-routing') {
    const [origin, environmentId] = splitKey(id);
    try {
      if (!['off', 'read', 'read-write'].includes(value) || !environmentId) throw new ClientError('Choose a GitHub routing permission.');
      const entry = savedEntry(await savedList(native), environmentId);
      const key = sharingKey({ origin: str(entry?.origin, origin), environmentId, routes: entry ? savedRoutes(entry) : [] });
      await writePrefs(native, prefs => {
        const next = { ...prefs.githubRouting };
        if (value === 'off') delete next[key]; else next[key] = value;
        prefs.githubRouting = next;
      });
    } catch (error) { if (client && !letGo(error)) pushToast(client, { kind: 'error', title: 'Could not save GitHub routing permission' }); throw error; }
    return { status: null, generation: -1 };
  }
  throw new ClientError(`Unknown action: ${op}`);
}
export const CONNECTION_OPS = ['environment-add', 'welcome-pair', 'environment-switch', 'environment-enabled', 'environment-forget', 'environment-trace', 'environment-icon',
  'environment-update', 'environment-update-outdated', 'environment-update-all', 'environment-ssh-add', 'environment-ssh-pick', 'load-balancing', 'load-weight', 'github-routing', 'environment-run-on',
  'environment-route-add', 'environment-route-move', 'environment-route-remove', 'local-environment',
  // connections-network.ts NETWORK_OPS (spelled out: the two modules import each other)
  'network-access', 'tailscale-serve', 'endpoint-default', 'pairing-create', 'pairing-revoke', 'client-revoke', 'clients-revoke-others', 'pairing-copy', 'pairing-reveal-close'];
