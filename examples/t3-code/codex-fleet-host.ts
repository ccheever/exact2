// A background environment as a provider-setup host (managed-codex-chatgpt): the welcome shows
// every chosen computer's Codex rows (WelcomeWizard.tsx ConnectedAgentsStep, one
// OnboardingCodexSetup per instance and environment), so a computer the app is not focused on
// runs the same streams and commands through its T3Fleet transport. The fleet's drain hands this
// host its provider-auth / provider-install entries (settings-b-fleet.ts).
import { obj, type Obj } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';
import type { FleetEntry } from './settings-b-fleet';
import type { CodexHost } from './codex-setup';

export class FleetSetupHost implements CodexHost {
  local = { favoriteModels: [] as string[] };
  constructor(readonly entry: FleetEntry) {}
  get config(): Obj { return this.entry.config; }
  set config(value: Obj) { this.entry.config = value; }
  get generation(): number { return this.entry.generation; }
  get ready(): boolean { return this.entry.phase === 'connected' && this.entry.synchronized === this.entry.generation; }
  get writable(): boolean { return this.ready && this.entry.scopes.includes('orchestration:operate'); }
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
