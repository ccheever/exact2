// Where a managed Codex setup runs: the focused connection, or a computer
// the welcome shows in the background (codex-fleet-host.ts), and the primary that signs a remote
// environment in (CodexSetupSection.tsx primaryAuthEnvironmentId :247-258: a non-loopback
// environment signs in on the connected loopback primary and imports the profile). A command
// names its setup by `target`: the instance id on the focused connection, `<fleet key>\t<id>`
// on a background one. Kept apart from codex-setup-ops.ts so the fleet imports neither.
import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';
import { fleet, EnvironmentFleet, type FleetEntry } from '../connections/settings-b-fleet';
import { focusedOnPrimary, isPrimaryEnvironment, primary } from '../connections/local-primary';
import { isLoopbackHost } from './codex-auth-request';
import { codexFlows, codexPicker, type CodexHost } from './codex-setup';
import { codexSetupPrepare, receiveCallback, type CodexContext } from './codex-setup-ops';
import { composerNow } from '../composer/composer-controls';
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

/**
 * Every instance with setup state, on the focus and in the background: those whose connection is up for the effects,
 * and every one for the loopback receiver, which keeps listening through a reconnect as the reference's receive does
 * (CodexSetupSection's effect depends on the URL alone).
 */
function setupContexts(client: T3Client, native: Native, connected = true): CodexContext[] {
  const contexts: CodexContext[] = [];
  if (!connected || client.connection === 'connected') for (const id of codexFlows(client).keys()) contexts.push(codexContext(client, native, id));
  for (const host of fleetSetupHosts(fleet.entries.values())) {
    if (connected && !host.ready) continue;
    for (const id of codexFlows(host).keys()) contexts.push(fleetCodexContext(client, native, host.entry, id));
  }
  return contexts;
}
const receivingOf = (context: CodexContext) => codexFlows(context.host).get(context.instanceId)?.receiving ?? '';

/** The snapshot answer's step (app.ts): each setup's effects, at the snapshot's clock. */
export async function codexPrepare(client: T3Client, native: Native | null | undefined): Promise<void> {
  if (!native?.available) return;
  const contexts = setupContexts(client, native);
  if (contexts.length) await codexSetupPrepare(contexts, composerNow(client));
}
/**
 * The window's loopback receivers (app.contract `codexLoopback`, `codexLoopbackSecond`): two sign-ins wait at once, each
 * on its own port, as the reference's do ("allows two accounts to complete independently", CodexAuthCallback.test.ts;
 * LLP 1133.005 D9 "Several at once"). A sign-in keeps its slot while it waits, since a new URL in a slot lets its
 * receiver go and binds again; a third waits for a free one.
 */
export const LOOPBACK_SLOTS = 2;
const slots: string[] = Array.from({ length: LOOPBACK_SLOTS }, () => '');
/** The authorization URL each slot's sign-in waits on the loopback for (the snapshot's `codexReceiving`), '' when free. */
export function codexReceiving(client: T3Client, native: Native | null | undefined): string[] {
  const waiting = native?.available ? setupContexts(client, native, false).map(receivingOf).filter(url => url !== '') : [];
  for (let slot = 0; slot < LOOPBACK_SLOTS; slot++) if (!waiting.includes(slots[slot]!)) slots[slot] = '';
  for (const url of waiting) {
    const free = slots.indexOf('');
    if (!slots.includes(url) && free >= 0) slots[free] = url;
  }
  return [...slots];
}
/** The snapshot's fields for the two slots. */
export function codexReceivingSlots(client: T3Client, native: Native | null | undefined): { codexReceiving: string; codexReceivingSecond: string } {
  const [first = '', second = ''] = codexReceiving(client, native);
  return { codexReceiving: first, codexReceivingSecond: second };
}
/** A slot's resource: that sign-in's callback, received and completed. */
export async function codexReceive(client: T3Client, native: Native | null | undefined, url: string): Promise<{ phase: string }> {
  if (!url || !native?.available) return { phase: 'idle' };
  const context = setupContexts(client, native, false).find(entry => receivingOf(entry) === url);
  return { phase: context ? await receiveCallback(context, url) : 'gone' };
}
