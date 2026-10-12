// The legacy sidebar row's discovered-port button (T3 Code 1e2ecbd975; MIT, see LICENSE-T3): LegacySidebar.tsx
// handleOpenDiscoveredPort over portDiscoveryState.ts useThreadDiscoveredPorts (each row's environment's
// `subscribeDiscoveredLocalServers` with no configured URLs, the servers whose terminal is the row's thread) and
// components/preview/openDiscoveredPort.ts (the port's URL for that environment in a Browser tab beside the
// thread, recorded in its history; "Unable to open preview" on failure). The focused environment's stream is
// the client's own (browser-targets.ts, key `sidebar-discovered-servers`); each background environment the
// fleet keeps connected carries one too (settings-b-fleet.ts syncEntry and drain). A background row's button
// focuses that environment first (the window shows one), and the tab opens once its thread is open there.
import { obj, str, type Obj } from './domain';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { letGo } from './let-go';
import { pushToast } from './toast';
import { openUrlInPreview } from './browser-links';
import { browserHistory } from './browser-history';
import { navigationNow } from './browser-navigation';
import { SIDEBAR_DISCOVERED_SERVERS_KEY, discoveredServers, environmentHostname, preparedConnection, readDiscoveredServers, resolveDiscoveredServerUrl,
  watchDiscoveredServers, type DiscoveredLocalServer } from './browser-targets';
import { subscriptionSerial } from './shell-vcs';
import { fleet, parseFleetThreadId, type EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { legacyEnabled } from './legacy-sidebar-view';
import { goTo, rowThread } from './legacy-sidebar-links';

/** Whether the legacy sidebar shows rows (the last refresh's answer): the background streams follow it. */
let wanted = false;
/** The sidebar's own stream on the focused environment, live while the legacy sidebar shows its rows. */
export async function watchRowPorts(client: T3Client, native: Native): Promise<void> {
  wanted = legacyEnabled(client) && client.local.sidebarOpen !== false;
  await watchDiscoveredServers(client, native, wanted && client.connection === 'connected', [], SIDEBAR_DISCOVERED_SERVERS_KEY);
}

// ── A background environment's stream ─────────────────────────────────────────────────────────
type Stream = { generation: number; id: string; tried: boolean; floor: number; maxSeen: number; servers: DiscoveredLocalServer[] };
const streams = new WeakMap<FleetEntry, Stream>();
function streamOf(entry: FleetEntry): Stream {
  let stream = streams.get(entry);
  if (!stream || stream.generation !== entry.generation) {
    stream = { generation: entry.generation, id: '', tried: false, floor: stream?.maxSeen ?? 0, maxSeen: stream?.maxSeen ?? 0, servers: [] };
    streams.set(entry, stream);
  }
  return stream;
}
/** settings-b-fleet.ts syncEntry, after its drain: subscribed while the legacy sidebar shows rows, released when it does not. */
export async function fleetPortsPass(call: (request: Obj) => Promise<Obj>, entry: FleetEntry): Promise<void> {
  const stream = streamOf(entry);
  if (!wanted) {
    if (!stream.id) return;
    stream.id = ''; stream.tried = false; stream.floor = stream.maxSeen; stream.servers = [];
    try { await call({ op: 'unsubscribe', key: SIDEBAR_DISCOVERED_SERVERS_KEY }); } catch (error) { if (letGo(error)) throw error; }
    return;
  }
  if (stream.id || stream.tried) return;
  stream.tried = true; stream.floor = stream.maxSeen;
  try {
    const reply = await call({ op: 'subscribe', key: SIDEBAR_DISCOVERED_SERVERS_KEY, method: 'subscribeDiscoveredLocalServers', payload: {} });
    const serial = subscriptionSerial(str(reply.id));
    stream.maxSeen = Math.max(stream.maxSeen, serial);
    if (serial > stream.floor && (!stream.id || serial > subscriptionSerial(stream.id))) stream.id = str(reply.id);
  } catch (error) { stream.tried = false; if (letGo(error)) throw error; }
}
/** settings-b-fleet.ts drain: the stream's events (true when the event was this stream's). */
export function fleetPortsEvent(entry: FleetEntry, event: Obj): boolean {
  if (str(event.key) !== SIDEBAR_DISCOVERED_SERVERS_KEY) return false;
  if (Number(event.generation) !== entry.generation) return true;
  const stream = streamOf(entry), id = str(event.subscriptionId), serial = subscriptionSerial(id);
  stream.maxSeen = Math.max(stream.maxSeen, serial);
  if (serial <= stream.floor || (stream.id && serial < subscriptionSerial(stream.id))) return true;
  stream.id = id;
  const item = obj(event.value);
  if (item._retryDue || item._streamEnded) { stream.id = ''; stream.tried = false; return true; }
  if (!item._transportError) stream.servers = readDiscoveredServers(item).servers;
  return true;
}

/** useThreadDiscoveredPorts: the row's environment's servers whose terminal belongs to the row's thread, in the stream's order. */
export function rowPorts(client: T3Client, id: string, source: EnvironmentFleet = fleet): DiscoveredLocalServer[] {
  const ref = parseFleetThreadId(id);
  if (ref && ref.environmentId !== client.environmentId) {
    const entry = [...source.entries.values()].find(candidate => candidate.environmentId === ref.environmentId && candidate.phase === 'connected');
    return entry ? streamOf(entry).servers.filter(server => server.terminal?.threadId === ref.threadId) : [];
  }
  const threadId = ref?.threadId ?? id;
  return discoveredServers(client, SIDEBAR_DISCOVERED_SERVERS_KEY).servers.filter(server => server.terminal?.threadId === threadId);
}
/** The button's label and tooltip ("Open localhost:5173", "Open localhost:5173 (+1)"); empty without a port. */
export function portLabels(ports: readonly DiscoveredLocalServer[]): { portLabel: string; portTip: string } {
  const first = ports[0];
  if (!first) return { portLabel: '', portTip: '' };
  return { portLabel: `Open localhost:${first.port}`, portTip: `Open localhost:${first.port}${ports.length > 1 ? ` (+${ports.length - 1})` : ''}` };
}

// ── The button ───────────────────────────────────────────────────────────────────────────────
/** A background row's port, opened once its environment is the focus and its thread is open there. */
const pendingOpens = new WeakMap<T3Client, { environmentId: string; threadId: string; url: string }>();
async function openBeside(client: T3Client, native: Native, ref: { environmentId: string; threadId: string }, url: string): Promise<void> {
  try {
    await openUrlInPreview(client, native, ref, resolveDiscoveredServerUrl(ref.environmentId, url, preparedConnection(client)));
    browserHistory(client.local).recordVisitForThread(ref, url, navigationNow(client), environmentHostname(client));
  } catch (error) {
    if (letGo(error)) throw error;
    pushToast(client, { kind: 'error', title: 'Unable to open preview', description: error instanceof Error && error.message ? error.message : 'The preview could not be opened.', stacked: true });
  }
}
/** handleOpenDiscoveredPort: go to the thread, then open its first port beside it (openDiscoveredPort). */
export async function openRowPort(client: T3Client, native: Native, id: string): Promise<string> {
  const row = rowThread(client, id), port = rowPorts(client, id)[0];
  if (!row || !port) return '';
  if (!row.focused) pendingOpens.set(client, { ...row.ref, url: port.url });
  let result = '';
  // A focus switch that fails leaves no port to open later, when that environment is the focus by other means.
  try { result = await goTo(client, native, id, row.ref); } catch (error) { if (!row.focused) pendingOpens.delete(client); throw error; }
  if (row.focused) await openBeside(client, native, row.ref, port.url);
  return result;
}
/** After each refresh (sidebar-commands.ts sidebarRefreshed): a background row's port, once its thread is the open one here. */
export async function openPendingPort(client: T3Client, native: Native): Promise<void> {
  const pending = pendingOpens.get(client);
  if (!pending || !client.ready || client.environmentId !== pending.environmentId) return;
  pendingOpens.delete(client);
  // Its environment is the focus now: the tab opens beside its thread, unless another thread is open by then.
  if (client.threadId === pending.threadId) await openBeside(client, native, pending, pending.url);
}
