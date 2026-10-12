// A background environment as a provider-setup host (managed-codex-chatgpt): the welcome shows
// every chosen computer's Codex rows (WelcomeWizard.tsx ConnectedAgentsStep, one
// OnboardingCodexSetup per instance and environment), so a computer the app is not focused on
// runs the same streams and commands through its T3Fleet transport. The fleet's drain hands this
// host its provider-auth / provider-install entries (settings-b-fleet.ts). Settings › Providers
// shows a background environment through this host too (providers-scope.ts): `owner` is then the
// window's client, whose device preferences (model favorites and order) and toasts the page uses.
import { obj, type Obj } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';
import type { FleetEntry } from './settings-b-fleet';
import type { CodexHost } from './codex-setup';
import type { T3Client } from './client';

export class FleetSetupHost implements CodexHost {
  /** The window's client while Settings › Providers shows this environment (providers-scope.ts). */
  owner: T3Client | null = null;
  private readonly own = { favoriteModels: [] as string[] };
  constructor(readonly entry: FleetEntry) {}
  /** Model favorites and order are client settings, one set for every environment (settings.ts favorites, providerModelPreferences). */
  get local(): { favoriteModels: string[] } { return this.owner?.local ?? this.own; }
  get config(): Obj { return this.entry.config; }
  set config(value: Obj) { this.entry.config = value; }
  get generation(): number { return this.entry.generation; }
  get environmentId(): string { return this.entry.environmentId; }
  get shell() { return this.entry.shell; }
  get ready(): boolean { return this.entry.phase === 'connected' && this.entry.synchronized === this.entry.generation; }
  get writable(): boolean { return this.ready && this.entry.scopes.includes('orchestration:operate'); }
  /** A managed Codex setup's command target on this environment (codex-setup-host.ts codexTarget). */
  setupTarget(instanceId: string): string { return `${this.entry.key}\t${instanceId}`; }
  /** New ids from this environment's transport (T3Transport `ids`), as T3Client.ids. */
  async ids(native: Native, count: number): Promise<string[]> {
    const reply = await bridgeReply(native, { op: 'ids', count });
    if (!reply.ok || !Array.isArray(reply.value)) throw new ClientError(reply.error?.message || 'Could not allocate request identifiers.');
    return reply.value.map(String);
  }
  /** `native` is this environment's fleet native (EnvironmentFleet.native). */
  async rpc(native: Native, method: string, payload: Obj): Promise<Obj> {
    const generation = this.entry.generation;
    const reply = await bridgeReply(native, { op: 'request', method, payload, generation });
    if (!reply.ok) throw new ClientError(reply.error?.message || 'The request failed.', reply.error?.kind || 'transport', reply.error?.uncertain === true);
    if (reply.generation !== generation || generation !== this.entry.generation) throw new ClientError('The connection changed. Refresh before continuing.', 'stale');
    return obj(reply.value);
  }
}
const hosts = new WeakMap<FleetEntry, FleetSetupHost>();
export function fleetSetupHost(entry: FleetEntry): FleetSetupHost {
  let host = hosts.get(entry);
  if (!host) hosts.set(entry, host = new FleetSetupHost(entry));
  return host;
}
/** The hosts made so far, for the snapshot answer's effects. */
export const fleetSetupHosts = (entries: Iterable<FleetEntry>) => [...entries].map(entry => hosts.get(entry)).filter((host): host is FleetSetupHost => !!host);
/** Where a provider host's toasts go: the window's client, also for a background environment's host. */
export const hostClient = (host: object): T3Client => (host instanceof FleetSetupHost && host.owner ? host.owner : host) as T3Client;
