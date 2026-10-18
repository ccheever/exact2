// Background environments. The reference keeps every
// switched-on saved environment connected at once (ConnectionsSettings.tsx
// SavedBackendListRow, environmentCatalog.setEnabled). The focused environment
// stays T3Client's own connection; every other switched-on environment is a
// background session of the transport (transport.ts), addressed by `fleet: <origin\nid>`.
// This module keeps each one's connection facts, server config and shell
// (projects and threads) so the sidebar, scope menus and Connections page can
// cover them all. The primary environment (local-primary.ts, the embedded server)
// is one of them whenever it is not the focus: it connects with `primary: true`
// (its bearer is the embedded server's, in memory) and is never saved.
import { obj, str, num, arr, initialShell, applyShell, type Obj, type Shell } from '../client/domain';
import { bridgeReply, applyConfig, type Native } from '../client/protocol';
import { announceJobs } from './settings-b-outdated';
import { announceServerUpdates } from './server-update';
import type { T3Client } from '../client/client';
import { learnRoutes } from './connection-routes-ops';
import { liveFleetEvent, liveFleetPass } from '../client/live-streams';
import { applyTerminalMetadataStreamEvent, type TerminalSummary, type TerminalMetadataStreamEvent } from '../terminal/terminal-session';
import { letGo } from '../client/let-go';
import { dropPrimaryDuplicates, focusedOnPrimary, isPrimaryEnvironment, primary, primaryEntry, withoutPrimaryDuplicates, type LocalPrimary } from './local-primary';
import { usageFleetEvent, usageFleetReset, usageFleetSynced } from '../usage/usage-replies';
import { handoffKeptThread, keepAliveFleetEvent, keepAliveFleetPass } from '../client/keep-alive';
import { automationFleetEvent } from '../browser/browser-automation';
import { codexHandoffEvent } from '../providers/codex-handoff-events'; // the primary's handoff stream
import { providerSetupEvent } from '../providers/provider-setup';
import { fleetSetupHost } from '../providers/codex-fleet-host'; // a background computer's Codex setup streams
import { fleetPortsEvent, fleetPortsPass } from '../sidebar/legacy-sidebar-ports'; // a background environment's discovered ports

export type FleetPhase = 'available' | 'connecting' | 'reconnecting' | 'connected' | 'error' | 'unsupported';
export interface FleetEntry {
  key: string; origin: string; environmentId: string;
  phase: FleetPhase; message: string; traceId: string;
  generation: number; synchronized: number; lastEvent: number; subscriptions: Record<string, string>;
  config: Obj; shell: Shell; scopes: string[]; error: string; requested: boolean; busy?: boolean;
  /** The route this environment's transport connected over. */
  activeRouteId?: string;
  /** The primary environment (local-primary.ts): connected with the embedded server's bearer. */
  primary?: boolean;
  terminalMetadata?: ReadonlyArray<TerminalSummary> | null;
  terminalAttempted?: boolean;
}
export interface FocusedHost { origin: string; environmentId: string; connection: string }

export const trimOrigin = (origin: string) => origin.trim().replace(/\/+$/, '');
/**
 * A saved environment's key: its home origin (the address it was first saved under,
 * the connection catalog) and its id. One entry per environment id holds every route, so the
 * focused connection (whose origin is the route in use) is matched by id, never by this key.
 */
export const environmentKey = (origin: string, environmentId: string) => `${trimOrigin(origin)}\n${environmentId}`;

/** The facts a session's status reports, as the reference's connection phases. */
export function phaseOf(status: Obj): { phase: FleetPhase; message: string; traceId: string } {
  const state = str(status.state), kind = str(status.failureKind), raw = str(status.message);
  const message = raw.replace(/\s*Reconnecting in \d+s…$/, '').trim();
  if (state === 'connected') return { phase: 'connected', message: '', traceId: '' };
  if (state === 'connecting') return { phase: 'connecting', message: '', traceId: '' };
  if (state === 'reconnecting') return { phase: 'reconnecting', message, traceId: str(status.traceId) };
  if (state === 'error') return { phase: kind === 'Protocol' ? 'unsupported' : 'error', message, traceId: str(status.traceId) };
  return { phase: 'available', message: '', traceId: '' };
}

const fresh = (key: string, origin: string, environmentId: string): FleetEntry => ({
  key, origin, environmentId, phase: 'available', message: '', traceId: '', generation: -1, synchronized: -1, lastEvent: 0,
  subscriptions: {}, config: {}, shell: initialShell(), scopes: [], error: '', requested: false,
});

export class EnvironmentFleet {
  entries = new Map<string, FleetEntry>();
  saved: Obj[] = [];
  revision = 0;
  /** The last sync dropped a saved duplicate of the primary that was the focus (client.ts hands the focus to the primary). */
  focusDropped = false;
  takeFocusDropped(): boolean { const dropped = this.focusDropped; this.focusDropped = false; return dropped; }
  private epoch = 0;
  private retries = new WeakMap<FleetEntry, Set<'config' | 'shell'>>();

  /** The fleet's native view of one background environment. */
  static native(native: Native, key: string): Native {
    return { available: native.available, watch: topic => native.watch(topic), later: request => native.later({ ...obj(request), fleet: key }) };
  }

  /** Saved environments this device keeps switched on, and the primary once its server is ready, other than the focused one. */
  wanted(focused: FocusedHost): Obj[] {
    const local = primary.connectable ? primaryEntry() : null;
    // The focus may be any of its environment's routes, so it is matched by id.
    return [...(local ? [local] : []), ...this.saved].filter(entry => entry.enabled !== false && str(entry.environmentId)
      && !(focused.environmentId && str(entry.environmentId) === focused.environmentId)
      && !(!focused.environmentId && focused.origin && trimOrigin(str(entry.origin)) === trimOrigin(focused.origin))).slice(0, 8);
  }

  /**
   * One pass: read the saved catalog, stop transports nobody wants, start the
   * switched-on ones, then bootstrap and drain each connected one. Never throws;
   * a failing environment keeps its own error.
   */
  async sync(native: Native | null | undefined, focused: FocusedHost): Promise<void> {
    if (!native?.available) return;
    const epoch = ++this.epoch;
    try {
      const listed = await bridgeReply(native, { op: 'environments' });
      if (epoch !== this.epoch || !listed.ok) return;
      // As the reference: a saved entry with the primary's id is removed and its credential forgotten.
      const all = arr(obj(listed.value).saved);
      this.saved = withoutPrimaryDuplicates(all);
      if (this.saved.length !== all.length) {
        const dropped = await dropPrimaryDuplicates(native, all).catch(() => ({ origins: [], focusDropped: false }));
        if (dropped.focusDropped) this.focusDropped = true;
      }
    } catch { return; }
    const wanted = new Map(this.wanted(focused).map(entry => [environmentKey(str(entry.origin), str(entry.environmentId)), entry]));
    for (const key of [...this.entries.keys()]) {
      if (wanted.has(key)) continue;
      this.entries.delete(key); this.revision++;
      await native.later({ op: 'fleetStop', fleet: key }).catch(() => {});
    }
    for (const [key, saved] of wanted) {
      let entry = this.entries.get(key);
      if (!entry) { entry = fresh(key, str(saved.origin), str(saved.environmentId)); entry.primary = saved.primary === true; this.entries.set(key, entry); this.revision++; }
      await this.syncOne(native, entry, epoch).catch(error => { if (letGo(error)) throw error; entry!.error = error instanceof Error ? error.message : 'The environment failed.'; });
    }
    // Each connected environment's reported addresses become learned routes.
    await learnRoutes(native, this, focused).catch(() => {});
    // Outdated-host updates (settings-b-outdated.ts) finish in the background: toast each result once.
    await announceJobs(native, 'local' in focused ? focused as unknown as T3Client : null).catch(() => {});
    // A connected server's update (server-update.ts) toasts once its connection reports the new version.
    const client = 'local' in focused ? focused as unknown as T3Client : null;
    await announceServerUpdates(native, client, (_key, environmentId) => {
      if (client && environmentId === client.environmentId) return client.connection === 'connected' ? str(obj(client.config.environment).serverVersion) || null : null;
      const entry = [...this.entries.values()].find(candidate => candidate.environmentId === environmentId);
      return entry?.phase === 'connected' ? str(obj(entry.config.environment).serverVersion) || null : null;
    }).catch(() => {});
    usageFleetSynced(this.revision); // usage-replies.ts: a background change redraws the open Usage page
  }

  private async syncOne(native: Native, entry: FleetEntry, epoch: number): Promise<void> {
    // Two answers can overlap; only one may bootstrap or drain an environment.
    if (entry.busy) return;
    entry.busy = true;
    try { await this.syncEntry(native, entry, epoch); } finally { entry.busy = false; }
  }
  private async syncEntry(native: Native, entry: FleetEntry, epoch: number): Promise<void> {
    const remote = EnvironmentFleet.native(native, entry.key);
    const status = await bridgeReply(remote, { op: 'status' });
    if (epoch !== this.epoch || !status.ok) return;
    const value = obj(status.value), next = phaseOf(value);
    if (str(value.state) === 'disconnected' && !entry.requested) {
      // Opening completes only when the socket opens; its progress arrives as
      // fleet news (the snapshot is asked again), so this answer never waits on the network.
      entry.requested = true; entry.phase = 'connecting'; this.revision++;
      void native.later({ op: 'connect', fleet: entry.key, origin: entry.origin, ...(entry.primary ? { primary: true } : { credential: '' }) }).catch(() => {});
      return;
    }
    if (next.phase !== entry.phase || next.message !== entry.message || next.traceId !== entry.traceId || str(value.activeRouteId) !== (entry.activeRouteId ?? '')) {
      entry.phase = next.phase; entry.message = next.message; entry.traceId = next.traceId; entry.activeRouteId = str(value.activeRouteId); this.revision++;
    }
    if (status.generation !== entry.generation) {
      entry.generation = status.generation; entry.synchronized = -1; entry.lastEvent = 0; entry.subscriptions = {}; entry.terminalMetadata = null; entry.terminalAttempted = false;
      this.retries.delete(entry);
    }
    if (entry.phase !== 'connected') return;
    if (entry.synchronized !== entry.generation) await this.bootstrap(remote, entry);
    await this.drain(remote, entry);
    await this.watchTerminals(remote, entry);
    await liveFleetPass(request => this.call(remote, entry, request), entry); // live-streams.ts: scheduled tasks and clones
    await keepAliveFleetPass(request => this.call(remote, entry, request), entry); // keep-alive.ts: running threads' detail streams, the primary's lifecycle
    await fleetPortsPass(request => this.call(remote, entry, request), entry); // legacy-sidebar-ports.ts: the legacy rows' discovered ports
  }

  private async call(remote: Native, entry: FleetEntry, request: Obj): Promise<Obj> {
    const reply = await bridgeReply(remote, { ...request, generation: entry.generation });
    if (!reply.ok) throw new Error(reply.error!.message);
    if (reply.generation !== entry.generation) throw new Error('The connection changed.');
    return obj(reply.value);
  }

  private async bootstrap(remote: Native, entry: FleetEntry): Promise<void> {
    const generation = entry.generation;
    const auth = await this.call(remote, entry, { op: 'http', path: '/api/auth/session' });
    entry.scopes = Array.isArray(auth.scopes) ? auth.scopes.filter((scope): scope is string => typeof scope === 'string') : [];
    entry.config = await this.call(remote, entry, { op: 'request', method: 'server.getConfig', payload: {} });
    entry.subscriptions.config = str((await this.call(remote, entry, { op: 'subscribe', key: 'config', method: 'subscribeServerConfig', payload: { usageLimitSources: true } })).id); // hub sources, as the focused client asks
    entry.shell = applyShell(initialShell(), await this.call(remote, entry, { op: 'http', path: '/api/orchestration/shell' }));
    entry.subscriptions.shell = str((await this.call(remote, entry, { op: 'subscribe', key: 'shell', method: 'orchestration.subscribeShell', payload: { afterSequence: entry.shell.sequence } })).id);
    if (generation === entry.generation) { this.retries.delete(entry); entry.synchronized = generation; entry.error = ''; this.revision++; }
  }

  /** Each background transport already has its own inbox and generation; metadata uses that lifecycle. */
  private async watchTerminals(remote: Native, entry: FleetEntry): Promise<void> {
    if (!entry.scopes.includes('terminal:operate') || entry.terminalAttempted || entry.subscriptions['terminal-metadata']) return;
    entry.terminalAttempted = true;
    try {
      entry.subscriptions['terminal-metadata'] = str((await this.call(remote, entry, { op: 'subscribe', key: 'terminal-metadata', method: 'subscribeTerminalMetadata', payload: {} })).id);
    } catch { entry.terminalAttempted = false; }
  }

  private async drain(remote: Native, entry: FleetEntry): Promise<void> {
    // A stream the transport scheduled to resubscribe on this session (`_retryDue`:
    // the app became active again, the transport's `foreground`, or a failed stream's backoff came due).
    for (let pass = 0; pass < 8; pass++) {
      const batch = await this.call(remote, entry, { op: 'events', after: entry.lastEvent });
      if (batch.reset === true) { if (entry.lastEvent > 0) usageFleetReset(entry); entry.synchronized = -1; } // usage-replies.ts: an overflowed inbox may have dropped a reply
      let through = entry.lastEvent;
      for (const event of arr(batch.events)) {
        const seq = num(event.seq);
        if (seq <= entry.lastEvent) continue;
        through = Math.max(through, seq);
        if (liveFleetEvent(entry, event)) { this.revision++; continue; } // live-streams.ts
        if (fleetPortsEvent(entry, event)) { this.revision++; continue; } // legacy-sidebar-ports.ts
        if (keepAliveFleetEvent(entry, event)) { this.revision++; continue; } // keep-alive.ts
        if (automationFleetEvent(entry, event)) { this.revision++; continue; } // browser-automation.ts: this environment's previewAutomation host
        if (usageFleetEvent(entry, event)) { this.revision++; continue; } // usage-replies.ts: the Usage page's detached requests
        if (num(event.generation, -1) === entry.generation && (codexHandoffEvent(event) || providerSetupEvent(fleetSetupHost(entry), event))) { this.revision++; continue; } // codex-handoff-events.ts, codex-fleet-host.ts
        const key = str(event.key), item = obj(event.value);
        if (num(event.generation, -1) !== entry.generation || str(event.subscriptionId) !== entry.subscriptions[key]) continue;
        if (key === 'terminal-metadata') {
          // Keep terminal failures isolated from shell/config. Authorization waits for a new
          // generation; other failures wait for the transport's bounded retry notification.
          if (item._retryDue) { entry.subscriptions[key] = ''; entry.terminalAttempted = false; continue; }
          if (item._transportError || item._streamEnded) continue;
          const metadataEvent = terminalMetadataEvent(item);
          if (metadataEvent) { entry.terminalMetadata = applyTerminalMetadataStreamEvent(entry.terminalMetadata ?? [], metadataEvent); this.revision++; }
          continue;
        }
        if (item._retryDue) {
          if (key === 'config' || key === 'shell') {
            // Keep the retry before acknowledging its event. Either the ack or subscribe reply can be superseded
            // after native has applied it, so a later answer must retry until it adopts the new subscription id.
            let retry = this.retries.get(entry);
            if (!retry) { retry = new Set(); this.retries.set(entry, retry); }
            retry.add(key);
          }
          continue;
        }
        if (item._transportError || item._streamEnded) { entry.synchronized = -1; continue; }
        try {
          if (key === 'config') entry.config = applyConfig(entry.config, item);
          else if (key === 'shell' && item.kind !== 'synchronized') entry.shell = applyShell(entry.shell, item);
          this.revision++;
        } catch { entry.synchronized = -1; }
      }
      if (batch.reset === true && arr(batch.events).length === 0) through = Math.max(through, num(batch.latest));
      entry.lastEvent = through;
      const ack = through > 0 ? await this.call(remote, entry, { op: 'ack', through }) : {};
      if (through >= Math.max(num(batch.latest), num(ack.latest))) break;
    }
    await this.resubscribe(remote, entry);
  }

  /**
   * The reference's foreground resubscription (client-runtime state/shell.ts `foregroundResubscriptions`): on the same
   * session the shell resumes from its in-memory cursor and keeps its snapshot, so the environment stays synchronized and
   * every surface reading it (the Pull Requests page's server set, the sidebar's threads) keeps its rows. Before, the
   * marker reached applyShell as a snapshot without a sequence, which threw and dropped the environment until a fresh
   * bootstrap: on every return to the app the Pull Requests list emptied and came back at its top. A resubscription that fails asks for that bootstrap instead.
   */
  private async resubscribe(remote: Native, entry: FleetEntry): Promise<void> {
    const retry = this.retries.get(entry);
    if (!retry?.size || entry.synchronized !== entry.generation) return;
    try {
      if (retry.has('config')) {
        entry.subscriptions.config = str((await this.call(remote, entry, { op: 'subscribe', key: 'config', method: 'subscribeServerConfig', payload: { usageLimitSources: true } })).id);
        retry.delete('config');
      }
      if (retry.has('shell')) {
        entry.subscriptions.shell = str((await this.call(remote, entry, { op: 'subscribe', key: 'shell', method: 'orchestration.subscribeShell', payload: { afterSequence: entry.shell.sequence } })).id);
        retry.delete('shell');
      }
    } catch (error) { if (letGo(error)) throw error; entry.synchronized = -1; }
  }

  /** A switch flip or removal: re-read the catalog on the next pass. */
  forget(key: string): void { if (this.entries.delete(key)) this.revision++; }
  /**
   * EnvironmentRegistry.register → installEntryLocked: registering a saved environment again (a new pairing) closes its
   * connection and opens it afresh, so a transport that failed for want of a credential reads the new one. This stops
   * each of its background transports; the next sync connects them as a switch-on does.
   */
  async restart(native: Native, environmentId: string): Promise<void> {
    for (const [key, entry] of [...this.entries]) {
      if (entry.environmentId !== environmentId || entry.primary) continue;
      this.forget(key);
      await native.later({ op: 'fleetStop', fleet: key }).catch(() => {});
    }
  }
  reconnect(key: string): void { const entry = this.entries.get(key); if (entry) { entry.requested = false; entry.phase = 'available'; this.revision++; } }
}

/** Validate the metadata boundary before shared terminal reducers consume a remote event. */
function terminalSummary(value: unknown): TerminalSummary | null {
  const item = obj(value), status = item.status;
  if (typeof item.threadId !== 'string' || typeof item.terminalId !== 'string' || typeof item.cwd !== 'string'
      || (item.worktreePath !== null && typeof item.worktreePath !== 'string')
      || !['starting', 'running', 'exited', 'error'].includes(str(status)) || typeof item.hasRunningSubprocess !== 'boolean'
      || typeof item.label !== 'string' || typeof item.updatedAt !== 'string'
      || ![item.pid, item.exitCode, item.exitSignal].every(value => value === null || typeof value === 'number' && Number.isFinite(value))) return null;
  if (status !== 'starting' && status !== 'running' && status !== 'exited' && status !== 'error') return null;
  return { threadId: item.threadId, terminalId: item.terminalId, cwd: item.cwd, worktreePath: item.worktreePath,
    status, pid: typeof item.pid === 'number' ? item.pid : null, exitCode: typeof item.exitCode === 'number' ? item.exitCode : null,
    exitSignal: typeof item.exitSignal === 'number' ? item.exitSignal : null, hasRunningSubprocess: item.hasRunningSubprocess,
    label: item.label, updatedAt: item.updatedAt };
}
function terminalMetadataEvent(item: Obj): TerminalMetadataStreamEvent | null {
  if (item.type === 'snapshot' && Array.isArray(item.terminals)) {
    const terminals = item.terminals.map(terminalSummary);
    return terminals.every((terminal): terminal is TerminalSummary => terminal !== null) ? { type: 'snapshot', terminals } : null;
  }
  if (item.type === 'upsert') { const terminal = terminalSummary(item.terminal); return terminal ? { type: 'upsert', terminal } : null; }
  return item.type === 'remove' && typeof item.threadId === 'string' && typeof item.terminalId === 'string'
    ? { type: 'remove', threadId: item.threadId, terminalId: item.terminalId } : null;
}

/** Running subprocesses in one background environment, never a same-named local thread. */
export function fleetTerminalProcessCount(environmentId: string, threadId: string, source: EnvironmentFleet = fleet): number {
  return fleetRunningTerminalIds(environmentId, threadId, source).length;
}
/** The background environment's terminals of the thread that run a child process, in the server's order. */
export function fleetRunningTerminalIds(environmentId: string, threadId: string, source: EnvironmentFleet = fleet): string[] {
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  if (!entry || entry.phase !== 'connected' || entry.synchronized !== entry.generation) return [];
  return (entry.terminalMetadata ?? []).filter(terminal => terminal.threadId === threadId && terminal.hasRunningSubprocess).map(terminal => terminal.terminalId);
}

/** The app's one fleet: background environments outlive any single answer. */
export const fleet = new EnvironmentFleet();

// ── Sidebar (all environments) ────────────────────────────────────────────
// Background environments' threads join the sidebar under a qualified id, so
// two environments that share thread ids stay distinct rows. Opening one
// focuses its environment: T3Client reconnects there and the previous focus
// becomes a background environment on the next pass.
const MACHINES = ['server', 'cloud', 'linux', 'desktop', 'laptop', 'mac-mini', 'mac-studio'];
const kindOf = (config: Obj) => {
  const icon = str(obj(config.settings).environmentIcon), machine = str(obj(obj(config.environment).platform).machine);
  return MACHINES.includes(icon) ? icon : MACHINES.includes(machine) ? machine : 'server';
};
export const fleetThreadId = (environmentId: string, threadId: string) => `fleet:${environmentId}:${threadId}`;
export function parseFleetThreadId(id: string): { environmentId: string; threadId: string } | null {
  const match = /^fleet:([^:]+):(.+)$/.exec(id);
  return match ? { environmentId: match[1]!, threadId: match[2]! } : null;
}
/**
 * Whether a sidebar thread lives off the primary environment, and that environment's machine kind and
 * label: isRemoteThread (`thread.environmentId !== primaryEnvironmentId`, LegacySidebar.tsx and
 * Sidebar.tsx). The focused environment is remote too when it is not the primary, as after opening a
 * background environment's thread; with no primary every thread is remote.
 */
export function remoteThread(client: T3Client, thread: Obj, source: LocalPrimary = primary): { machine: string; label: string } | null {
  const background = parseFleetThreadId(str(thread.id));
  if (background ? isPrimaryEnvironment(background.environmentId, source) : focusedOnPrimary(client, source)) return null;
  return background ? { machine: str(thread.fleetMachine, 'server'), label: str(thread.fleetEnvironmentLabel) }
    : { machine: kindOf(client.config), label: str(obj(client.config.environment).label) };
}
/** Every synchronized background environment's shell threads, decorated for the sidebar. */
export function fleetThreads(source: EnvironmentFleet = fleet): Obj[] {
  const threads: Obj[] = [];
  for (const entry of source.entries.values()) {
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation) continue;
    const label = str(obj(entry.config.environment).label), kind = kindOf(entry.config);
    const titles = new Map(entry.shell.projects.map(project => [str(project.id), str(project.title)]));
    for (const thread of entry.shell.threads) {
      threads.push({ ...thread, id: fleetThreadId(entry.environmentId, str(thread.id)), projectId: `fleet:${entry.environmentId}:${str(thread.projectId)}`,
        fleetKey: entry.key, fleetProjectTitle: titles.get(str(thread.projectId)) ?? '', fleetEnvironmentLabel: label, fleetMachine: kind });
    }
  }
  return threads;
}
/**
 * Open a background environment's thread: remember it as that environment's
 * selection, then reconnect T3Client there. Returns the status to adopt.
 */
export async function focusFleetThread(client: { local: { selections: Record<string, { projectId: string; threadId: string }> } }, native: Native, id: string, source: EnvironmentFleet = fleet): Promise<{ value: Obj; generation: number }> {
  const parsed = parseFleetThreadId(id);
  const entry = parsed ? [...source.entries.values()].find(candidate => candidate.environmentId === parsed.environmentId) : undefined;
  if (!parsed || !entry) throw new Error('That environment is no longer connected.');
  const thread = entry.shell.threads.find(candidate => str(candidate.id) === parsed.threadId);
  if (!thread) throw new Error('That thread is no longer available.');
  client.local.selections[parsed.environmentId] = { projectId: str(thread.projectId), threadId: parsed.threadId };
  handoffKeptThread(client, entry, parsed.threadId); // keep-alive.ts: a running thread opens from its kept detail, no replay
  return refocus(native, entry, source);
}
/**
 * Open a background environment's project on its new-thread draft (useNewThreadHandler with another environment's
 * project ref: a clone toast's Open project): that environment's selection is the
 * project's draft, then T3Client reconnects there. The project need not be in the entry's shell yet: a clone's project
 * is the server's from projectClone.start on, and the reconnected shell reads it.
 */
export async function focusFleetProject(client: { local: { selections: Record<string, { projectId: string; threadId: string }> } }, native: Native, environmentId: string, projectId: string, source: EnvironmentFleet = fleet): Promise<{ value: Obj; generation: number }> {
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  if (!entry) throw new Error('That environment is no longer connected.');
  client.local.selections[environmentId] = { projectId, threadId: '' };
  return refocus(native, entry, source);
}
/** The window moves to a background environment: its fleet transport stops and T3Client connects there. */
async function refocus(native: Native, entry: FleetEntry, source: EnvironmentFleet): Promise<{ value: Obj; generation: number }> {
  source.forget(entry.key);
  await native.later({ op: 'fleetStop', fleet: entry.key }).catch(() => {});
  const reply = await bridgeReply(native, { op: 'connect', origin: entry.origin, ...(entry.primary ? { primary: true } : { credential: '' }) });
  if (!reply.ok) throw new Error(reply.error!.message);
  return { value: obj(reply.value), generation: reply.generation };
}
