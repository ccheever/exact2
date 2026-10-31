// The thread details card's Servers section (T3 Code 6d450a4284, MIT, see LICENSE-T3: components/chat/ThreadServersPanel.tsx,
// portDiscoveryState.ts useThreadDiscoveredPorts, components/preview/openDiscoveredPort.ts): the web servers this thread's
// terminals and agent started, from the focused environment's `subscribeDiscoveredLocalServers` (no configured URLs),
// hidden while none run. A press opens the server in a Browser tab beside the thread, ⌘-press in the system browser;
// Stop ends it through `preview.stopLocalServer` where the environment reports `localServerStop`. The stream lasts
// while the card shows the section (browser-targets.ts, key `details-discovered-servers`).
import type { T3Client } from '../client/client';
import { obj } from '../client/domain';
import type { Native } from '../client/protocol';
import { letGo } from '../client/let-go';
import { wakeShell } from '../client/command-waits';
import { can } from '../connections/command-permissions';
import { pushToast } from '../shell/toast';
import { openExternal } from '../client/external-open';
import { openUrlInPreview } from '../browser/browser-links';
import { recordVisit } from '../browser/browser-history-record';
import { navigationNow } from '../browser/browser-navigation';
import { DETAILS_DISCOVERED_SERVERS_KEY, discoveredServers, preparedConnection, resolveDiscoveredServerUrl, watchDiscoveredServers,
  type DiscoveredLocalServer } from '../browser/browser-targets';

export type ThreadServerRow = { key: string; port: number; label: string; source: string; stoppable: boolean; stopping: boolean };
export type ThreadServersView = { show: boolean; canOpen: boolean; busy: boolean; rows: ThreadServerRow[] };
export const NO_SERVERS: ThreadServersView = { show: false, canOpen: false, busy: false, rows: [] };

const serverKey = (server: DiscoveredLocalServer) => `${server.pid}:${server.port}`;
/** The server being stopped, by client: one at a time, as the reference's `stoppingKey`. */
const stopping = new WeakMap<T3Client, string>();

/** useThreadDiscoveredPorts: the servers whose terminal or agent belongs to the thread, in the stream's order. */
export function threadServers(client: T3Client, threadId: string): DiscoveredLocalServer[] {
  if (!threadId) return [];
  return discoveredServers(client, DETAILS_DISCOVERED_SERVERS_KEY).servers.filter(server => server.terminal?.threadId === threadId || server.agent?.threadId === threadId);
}

/** The section's stream follows it (`wanted`): subscribed while the card shows Servers for a started thread. */
export async function watchThreadServers(client: T3Client, native: Native, wanted: boolean): Promise<void> {
  await watchDiscoveredServers(client, native, wanted && client.connection === 'connected', [], DETAILS_DISCOVERED_SERVERS_KEY);
}

export function threadServersView(client: T3Client, threadId: string): ThreadServersView {
  const servers = threadServers(client, threadId);
  if (servers.length === 0) return NO_SERVERS;
  const canStop = can(client, 'terminal:operate'), supportsStop = obj(obj(client.config.environment).capabilities).localServerStop === true;
  const busy = stopping.get(client) ?? '';
  return {
    show: true, canOpen: can(client, 'preview:operate'), busy: busy !== '',
    rows: servers.map(server => ({
      key: serverKey(server), port: server.port, label: `localhost:${server.port}`,
      source: [server.terminal === null ? 'Agent' : 'Terminal', server.processName].filter(Boolean).join(' · '),
      stoppable: canStop && supportsStop && server.pid !== null, stopping: busy === serverKey(server),
    })),
  };
}

const failure = (client: T3Client, title: string, error: unknown) =>
  pushToast(client, { kind: 'error', title, description: error instanceof Error ? error.message : String(error), stacked: true });

/** A row's press (`open`, or `external` with ⌘) and its Stop (`stop`), by the server's key. */
export async function threadServerLocal(client: T3Client, native: Native, op: string, key: string): Promise<string> {
  const ref = { environmentId: client.environmentId, threadId: client.threadId };
  const server = threadServers(client, ref.threadId).find(entry => serverKey(entry) === key);
  if (!server) return '';
  if (op === 'external') {
    // Like links: a modified press goes to the system browser.
    const url = resolveDiscoveredServerUrl(ref.environmentId, server.url, preparedConnection(client));
    if (!openExternal(url)) failure(client, 'Unable to open in browser', new Error('This address cannot be opened.'));
    return '';
  }
  if (op === 'open') {
    try {
      await openUrlInPreview(client, native, ref, resolveDiscoveredServerUrl(ref.environmentId, server.url, preparedConnection(client)));
      recordVisit(client, ref, server.url, navigationNow(client));
    } catch (error) { if (letGo(error)) throw error; failure(client, 'Unable to open preview', error); }
    return '';
  }
  if (op === 'stop') {
    if (server.pid === null || stopping.has(client)) return '';
    stopping.set(client, key);
    try {
      await wakeShell(native); // the row spins and Stop is disabled while the server stops
      await client.stopLocalServer(native, { pid: server.pid, port: server.port });
    } catch (error) { if (letGo(error)) throw error; failure(client, 'Could not stop server', error); }
    finally { stopping.delete(client); }
  }
  return '';
}
