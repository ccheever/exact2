// Where a managed Codex setup runs (managed-codex-chatgpt): the focused connection, or a computer
// the welcome shows in the background (codex-fleet-host.ts), and the primary that signs a remote
// environment in (CodexSetupSection.tsx primaryAuthEnvironmentId :247-258: a non-loopback
// environment signs in on the connected loopback primary and imports the profile). A command
// names its setup by `target`: the instance id on the focused connection, `<fleet key>\t<id>`
// on a background one. Kept apart from codex-setup-ops.ts so the fleet imports neither.
import type { T3Client } from './client';
import type { Native } from './protocol';
import { fleet, EnvironmentFleet, type FleetEntry } from './settings-b-fleet';
import { focusedOnPrimary, isPrimaryEnvironment, primary } from './local-primary';
import { isLoopbackHost } from './codex-auth-request';
import { codexFlows, codexPicker, type CodexHost } from './codex-setup';
import { codexSetupPrepare, type CodexContext } from './codex-setup-ops';
import { fleetSetupHost, fleetSetupHosts } from './codex-fleet-host';
import { setCodexTargetResolver } from './providers';

const loopbackPrimary = () => { try { return !!primary.target && isLoopbackHost(new URL(primary.target.httpBaseUrl).hostname); } catch { return false; } };

/** The connected primary when `environmentId` is another environment: a background one, or the focus. */
function primaryConnection(client: T3Client, native: Native, environmentId: string): CodexContext['primary'] {
  if (!loopbackPrimary() || isPrimaryEnvironment(environmentId)) return null;
  if (focusedOnPrimary(client)) return client.connection === 'connected' ? { key: '', generation: client.generation, native } : null;
  const entry = [...fleet.entries.values()].find(candidate => candidate.primary === true && candidate.phase === 'connected' && candidate.synchronized === candidate.generation);
  return entry ? { key: entry.key, generation: entry.generation, native: EnvironmentFleet.native(native, entry.key) } : null;
}

/** One instance's setup on the focused connection. */
export function codexContext(client: T3Client, native: Native, instanceId: string): CodexContext {
  return { host: client, native, module: native, instanceId, origin: client.origin, environmentId: client.environmentId, primary: primaryConnection(client, native, client.environmentId) };
}
/** One instance's setup on a background computer. */
export function fleetCodexContext(client: T3Client, native: Native, entry: FleetEntry, instanceId: string): CodexContext {
  return { host: fleetSetupHost(entry), native: EnvironmentFleet.native(native, entry.key), module: native, instanceId, origin: entry.origin, environmentId: entry.environmentId,
    primary: primaryConnection(client, native, entry.environmentId) };
}
export const fleetTarget = (entry: FleetEntry, instanceId: string) => `${entry.key}\t${instanceId}`;
/** A command's target: the focused connection's instance, or a background computer's. */
export function codexTarget(client: T3Client, native: Native, target: string): CodexContext | null {
  const tab = target.indexOf('\t');
  if (tab < 0) return codexContext(client, native, target);
  const entry = fleet.entries.get(target.slice(0, tab));
  return entry ? fleetCodexContext(client, native, entry, target.slice(tab + 1)) : null;
}
// The Reconnect ChatGPT picker reads the target's own methods (providers.ts providerWizard).
setCodexTargetResolver((client, target) => {
  const tab = target.indexOf('\t'), entry = tab < 0 ? null : fleet.entries.get(target.slice(0, tab));
  const host: CodexHost | null = tab < 0 ? client as CodexHost : entry ? fleetSetupHost(entry) : null;
  return host ? codexPicker(host, tab < 0 ? target : target.slice(tab + 1), target) : null;
});

/** The snapshot answer's step (app.ts): every instance with setup state, on the focus and in the background. */
export async function codexPrepare(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available) return;
  const contexts: CodexContext[] = [];
  if (client.connection === 'connected') for (const id of codexFlows(client).keys()) contexts.push(codexContext(client, native, id));
  for (const host of fleetSetupHosts(fleet.entries.values())) {
    if (!host.ready) continue;
    for (const id of codexFlows(host).keys()) contexts.push(fleetCodexContext(client, native, host.entry, id));
  }
  if (contexts.length) await codexSetupPrepare(contexts);
}
