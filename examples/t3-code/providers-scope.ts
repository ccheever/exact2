// Settings › Providers shows one environment at a time, the one the settings scope selector chose
// (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: apps/web/src/routes/settings.providers.tsx:7-30, "Providers
// are machine state"; SettingsScopeContext.tsx's `environment`, the connected target with the primary
// first; ProviderSettingsPanel `environmentId … scoped`). The focused environment is T3Client's own
// connection; any other is a background transport (settings-b-fleet.ts) whose provider host is its
// FleetSetupHost (codex-fleet-host.ts). The page, the Add provider wizard, the ACP Registry search and
// every action the page sends (sign-in and install, updates, instance writes) read and write the
// environment the page names in `environment`. Update all stays every connected environment's.
import type { T3Client } from './client';
import type { ProvidersRoute } from './client-ops';
import { obj, str } from './domain';
import { ClientError, type Native } from './protocol';
import { EnvironmentFleet, fleet as appFleet, type FleetEntry } from './settings-b-fleet';
import { FleetSetupHost, fleetSetupHost } from './codex-fleet-host';
import { acpRegistry, providerPage, providerSetupStreams, providerWizard, wizardSetupStreams, type ProviderHost } from './providers';
import { providerUpkeepOp, withUpkeep, type UpkeepHost } from './providers-upkeep';
import { providerSetupOp, watchProviderSetup } from './provider-setup';
import { settingsScopeOf } from './settings-core-view';
import { isPrimaryEnvironment } from './local-primary';

const EMPTY = { auth: [] as string[], install: [] as string[] };
/**
 * `environment` while Settings shows Providers but the scope names no connected environment: a dialog the page
 * opened before (sign out, remove, the Add provider wizard) then writes nowhere, never to the focused one.
 */
export const NO_ENVIRONMENT = '-';

/**
 * SettingsProvidersRoute: the connected environment the scope resolves (the primary first), or the
 * route's own words when there is none. A removed target and a disconnected chosen environment are
 * SettingsScopeBoundary's to explain (settings-core-view.ts), so the page then says nothing.
 */
export function providersEnvironment(client: T3Client, machine: string, projectKey: string, checkout: string, legacyProjectId: string): { environmentId: string; message: string } {
  const { scope } = settingsScopeOf(client, 'providers', machine, projectKey, checkout, legacyProjectId);
  const connected = scope.selected.filter(environment => environment.connection.phase === 'connected' && environment.serverConfig !== null);
  const environment = connected.find(candidate => isPrimaryEnvironment(candidate.environmentId)) ?? connected[0];
  if (environment) return { environmentId: environment.environmentId, message: '' };
  return { environmentId: '', message: scope.kind === 'unavailable' || scope.kind === 'environment' ? '' : 'Connect an environment to set up its providers.' };
}

/** A background environment's own transport for its requests and streams; this window's module for the rest (opening a link, copying). */
const TRANSPORT_OPS = new Set(['http', 'request', 'subscribe', 'unsubscribe', 'events', 'ack', 'ids']);
export function environmentNative(native: Native, key: string): Native {
  const remote = EnvironmentFleet.native(native, key);
  return { available: native.available, watch: topic => native.watch(topic), later: request => (TRANSPORT_OPS.has(str(obj(request).op)) ? remote : native).later(request) };
}
/** The native a host's work goes through. */
export const routedNative = (host: object, native: Native): Native => host instanceof FleetSetupHost ? environmentNative(native, host.entry.key) : native;

const ownedHost = (client: T3Client, entry: FleetEntry): FleetSetupHost => { const host = fleetSetupHost(entry); host.owner = client; return host; };
/** An environment that left this device: nothing to show, and every write is refused. */
const offline = (client: T3Client): UpkeepHost => ({ config: {}, ready: false, writable: false, local: client.local,
  rpc: async () => { throw new ClientError('Reconnect before making changes.'); } });

/** The provider host of one environment: '' (Settings closed) or the focused one is the client; NO_ENVIRONMENT none. */
export function providersHost(client: T3Client, environmentId: string, source: EnvironmentFleet = appFleet): UpkeepHost {
  if (environmentId === NO_ENVIRONMENT) return offline(client);
  if (!environmentId || environmentId === client.environmentId) return client;
  const entry = [...source.entries.values()].find(candidate => candidate.environmentId === environmentId);
  return entry ? ownedHost(client, entry) : offline(client);
}

/** Each owner's streams live on one host: moving to another environment closes them on the previous one. */
const watching = new Map<string, ProviderHost>();
async function watchOn(owner: string, host: ProviderHost | null, native: Native, wanted: { auth: string[]; install: string[] }): Promise<void> {
  const previous = watching.get(owner);
  if (previous && previous !== host) await watchProviderSetup(previous, routedNative(previous, native), owner, EMPTY);
  if (!host) { watching.delete(owner); return; }
  watching.set(owner, host);
  await watchProviderSetup(host, routedNative(host, native), owner, wanted);
}

/**
 * The providerPage answer. args: selection, active, revision, clock, then the settings scope (machine,
 * project, checkout, legacy project id). Closed, the page is the focused environment's, as before.
 */
export async function scopedProviderPage(client: T3Client, native: Native | null | undefined, args: unknown[]) {
  const selection = String(args[0] || ''), active = args[1] === true, now = Number(args[3]) || 0;
  const chosen = active ? providersEnvironment(client, String(args[4] ?? ''), String(args[5] ?? ''), String(args[6] ?? ''), String(args[7] ?? ''))
    : { environmentId: client.environmentId, message: '' };
  const host = !active || chosen.environmentId ? providersHost(client, chosen.environmentId) : null;
  if (native?.available) await watchOn('page', host, native, active && host ? providerSetupStreams(host, selection) : EMPTY);
  if (!host) return { ...withUpkeep(client, { ...providerPage(offline(client), '', now), message: chosen.message }), environment: NO_ENVIRONMENT };
  return { ...withUpkeep(host, providerPage(host, selection, now), client), environment: active ? chosen.environmentId : '' };
}

/** The providerWizard answer on the page's environment (args[10]); the Reconnect ChatGPT picker reads its own target's streams. */
export async function scopedProviderWizard(client: T3Client, native: Native | null | undefined, args: unknown[]) {
  const host = providersHost(client, String(args[10] ?? ''));
  const open = args[0] === true, serial = Number(args[1]) || 0, dialog = String(args[8] || ''), target = String(args[9] || '');
  const tab = target.indexOf('\t'), pickerEntry = dialog === 'codex-picker' && tab >= 0 ? appFleet.entries.get(target.slice(0, tab)) : undefined;
  const streamHost = pickerEntry ? ownedHost(client, pickerEntry) : host;
  if (native?.available) await watchOn('wizard', streamHost, native, wizardSetupStreams(streamHost, open, serial, dialog, pickerEntry ? target.slice(tab + 1) : target));
  return providerWizard(host, open, serial, String(args[2] || 'codex'), args[3] === true, String(args[4] || ''), args[5] === true, String(args[6] || ''), dialog, target);
}

/** The ACP Registry search on the page's environment (args[3]), marking the agents it already has. */
export function scopedAcpRegistry(client: T3Client, native: Native | null | undefined, args: unknown[]) {
  const host = providersHost(client, String(args[3] ?? ''));
  const configured = Object.values(obj(obj(host.config.settings).providerInstances)).filter(entry => obj(entry).driver === 'acpRegistry').map(entry => str(obj(obj(entry).config).agentId));
  return acpRegistry(host, native?.available ? routedNative(host, native) : native, String(args[0] || ''), args[1] === true, configured);
}

/** A provider command's route to the page's environment (client.command), none for the focused one. */
export function providersRoute(client: T3Client, environmentId: string): ProvidersRoute | undefined {
  const host = providersHost(client, environmentId);
  if (host === client) return undefined;
  return { host, native: native => routedNative(host, native),
    requireWrite: () => { if (!host.writable) throw new ClientError(host.ready ? 'Wait for synchronization and check your connection permissions.' : 'Reconnect before making changes.'); } };
}

/** The page's `upkeep:` ops (args[4] names its environment); Update all and the launch prompt cover every environment. */
export async function scopedUpkeepOp(client: T3Client, native: Native, args: unknown[]): Promise<void> {
  const op = String(args[0] || ''), host = op === 'upkeep:update-all' || op === 'upkeep:update-launch' ? client : providersHost(client, String(args[4] ?? ''));
  await providerUpkeepOp(host, routedNative(host, native), op, String(args[1] || ''), String(args[2] ?? ''), String(args[3] ?? ''));
}

/** The page's sign-in and install commands (`setup:`), on its environment (args[4]). */
export async function scopedSetupOp(client: T3Client, native: Native, args: unknown[]): Promise<void> {
  const host = providersHost(client, String(args[4] ?? ''));
  await providerSetupOp(host, routedNative(host, native), String(args[0] || ''), String(args[1] || ''), String(args[2] ?? ''), String(args[3] ?? ''));
}
