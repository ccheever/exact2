// Background environments (lane settings-b). The reference keeps every
// switched-on saved environment connected at once (ConnectionsSettings.tsx
// SavedBackendListRow, environmentCatalog.setEnabled). The focused environment
// stays T3Client's own connection; every other switched-on environment is a
// background transport in T3Fleet.swift, addressed by `fleet: <origin\nid>`.
// This module keeps each one's connection facts, server config and shell
// (projects and threads) so the sidebar, scope menus and Connections page can
// cover them all.
import { obj, str, num, arr, initialShell, applyShell, type Obj, type Shell } from './domain';
import { bridgeReply, applyConfig, type Native } from './protocol';
import { announceJobs } from './settings-b-outdated';
import { announceServerUpdates } from './server-update';
import type { T3Client } from './client';
import { learnRoutes } from './connection-routes-ops';
import { liveFleetEvent, liveFleetPass } from './live-streams';
import { applyTerminalMetadataStreamEvent, type TerminalSummary, type TerminalMetadataStreamEvent } from './terminal-session';
import { letGo } from './let-go';

export type FleetPhase = 'available' | 'connecting' | 'reconnecting' | 'connected' | 'error' | 'unsupported';
export interface FleetEntry {
  key: string; origin: string; environmentId: string;
  phase: FleetPhase; message: string; traceId: string;
  generation: number; synchronized: number; lastEvent: number; subscriptions: Record<string, string>;
  config: Obj; shell: Shell; scopes: string[]; error: string; requested: boolean; busy?: boolean;
  /** The route this environment's transport connected over (lane environment-routes). */
  activeRouteId?: string;
  terminalMetadata?: ReadonlyArray<TerminalSummary> | null;
  terminalAttempted?: boolean;
}
export interface FocusedHost { origin: string; environmentId: string; connection: string }

export const trimOrigin = (origin: string) => origin.trim().replace(/\/+$/, '');
/**
 * A saved environment's key: its home origin (the address it was first saved under,
 * T3SavedEnvironments) and its id. One entry per environment id holds every route, so the
 * focused connection (whose origin is the route in use) is matched by id, never by this key.
 */
export const environmentKey = (origin: string, environmentId: string) => `${trimOrigin(origin)}\n${environmentId}`;
const LOOPBACK = /^(localhost|127(?:\.\d{1,3}){3}|\[::1\]|::1)$/i;
/** A loopback origin is this machine: the reference's primary environment. */
export function isLoopback(origin: string): boolean {
  try { return LOOPBACK.test(new URL(origin).hostname); } catch { return false; }
}

/** The transport facts T3Transport reports, as the reference's connection phases. */
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
  private epoch = 0;

  /** The fleet's native view of one background environment. */
  static native(native: Native, key: string): Native {
    return { available: native.available, watch: topic => native.watch(topic), later: request => native.later({ ...obj(request), fleet: key }) };
  }

  /** Saved environments this device keeps switched on, other than the focused one. */
  wanted(focused: FocusedHost): Obj[] {
    // The focus may be any of its environment's routes, so it is matched by id (lane environment-routes).
    return this.saved.filter(entry => entry.enabled !== false && str(entry.environmentId)
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
    native.watch('t3.fleet');
    try {
      const listed = await bridgeReply(native, { op: 'environments' });
      if (epoch !== this.epoch || !listed.ok) return;
      this.saved = arr(obj(listed.value).saved);
    } catch { return; }
    const wanted = new Map(this.wanted(focused).map(entry => [environmentKey(str(entry.origin), str(entry.environmentId)), entry]));
    for (const key of [...this.entries.keys()]) {
      if (wanted.has(key)) continue;
      this.entries.delete(key); this.revision++;
      await native.later({ op: 'fleetStop', fleet: key }).catch(() => {});
    }
    for (const [key, saved] of wanted) {
      let entry = this.entries.get(key);
      if (!entry) { entry = fresh(key, str(saved.origin), str(saved.environmentId)); this.entries.set(key, entry); this.revision++; }
      await this.syncOne(native, entry, epoch).catch(error => { if (letGo(error)) throw error; entry!.error = error instanceof Error ? error.message : 'The environment failed.'; });
    }
    // Lane environment-routes: each connected environment's reported addresses become learned routes.
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
      // t3.fleet changes, so this answer never waits on the network.
      entry.requested = true; entry.phase = 'connecting'; this.revision++;
      void native.later({ op: 'connect', fleet: entry.key, origin: entry.origin, credential: '' }).catch(() => {});
      return;
    }
    if (next.phase !== entry.phase || next.message !== entry.message || next.traceId !== entry.traceId || str(value.activeRouteId) !== (entry.activeRouteId ?? '')) {
      entry.phase = next.phase; entry.message = next.message; entry.traceId = next.traceId; entry.activeRouteId = str(value.activeRouteId); this.revision++;
    }
    if (status.generation !== entry.generation) {
      entry.generation = status.generation; entry.synchronized = -1; entry.lastEvent = 0; entry.subscriptions = {}; entry.terminalMetadata = null; entry.terminalAttempted = false;
    }
    if (entry.phase !== 'connected') return;
    if (entry.synchronized !== entry.generation) await this.bootstrap(remote, entry);
    await this.drain(remote, entry);
    await this.watchTerminals(remote, entry);
    await liveFleetPass(request => this.call(remote, entry, request), entry); // live-streams.ts: scheduled tasks and clones
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
    entry.subscriptions.config = str((await this.call(remote, entry, { op: 'subscribe', key: 'config', method: 'subscribeServerConfig', payload: {} })).id);
    entry.shell = applyShell(initialShell(), await this.call(remote, entry, { op: 'http', path: '/api/orchestration/shell' }));
    entry.subscriptions.shell = str((await this.call(remote, entry, { op: 'subscribe', key: 'shell', method: 'orchestration.subscribeShell', payload: { afterSequence: entry.shell.sequence } })).id);
    if (generation === entry.generation) { entry.synchronized = generation; entry.error = ''; this.revision++; }
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
    for (let pass = 0; pass < 8; pass++) {
      const batch = await this.call(remote, entry, { op: 'events', after: entry.lastEvent });
      if (batch.reset === true) entry.synchronized = -1;
      let through = entry.lastEvent;
      for (const event of arr(batch.events)) {
        const seq = num(event.seq);
        if (seq <= entry.lastEvent) continue;
        through = Math.max(through, seq);
        if (liveFleetEvent(entry, event)) { this.revision++; continue; } // live-streams.ts
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
  }

  /** A switch flip or removal: re-read the catalog on the next pass. */
  forget(key: string): void { if (this.entries.delete(key)) this.revision++; }
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
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  if (!entry || entry.phase !== 'connected' || entry.synchronized !== entry.generation) return 0;
  return (entry.terminalMetadata ?? []).filter(terminal => terminal.threadId === threadId && terminal.hasRunningSubprocess).length;
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
  source.forget(entry.key);
  await native.later({ op: 'fleetStop', fleet: entry.key }).catch(() => {});
  const reply = await bridgeReply(native, { op: 'connect', origin: entry.origin, credential: '' });
  if (!reply.ok) throw new Error(reply.error!.message);
  return { value: obj(reply.value), generation: reply.generation };
}
