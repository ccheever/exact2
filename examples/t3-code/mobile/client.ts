import { mobileGitEvents } from './git-overview';
import { mobileDevicesEvents } from './devices-mobile-data';
import { mobileBrowserEvents } from './browser-mobile-data';
import { mobileVoiceObserveDraft } from './voice-data';
// upstream 365aa87982 mobile pairing.ts and connection/platform.ts; shared reducers remain unchanged.
// @ref llp/1106.003-pairing-and-transport.decision.md#mobile-adaptations
import { T3Client } from './shared/client';
import { applyMobileComposerBehavior, mobileSend } from './composer-behavior';
import { mobileDraftChanged } from './draft';
import { decodePrefs, environmentSources, machineKind, savedStatus } from './shared/connections';
import { connectionRouteAddress, connectionRouteKind, connectionRouteLabel, gitHubRoutingConnectionKey, hasRelayRoute } from './shared/connection-routes';
import { arr, obj, str, type Obj } from './shared/domain';
import { letGo, letGoAware } from './shared/let-go';
import { bridgeReply, ClientError, nativeFiles, parsePairing, type Files, type Native } from './shared/protocol';
import { pairingFields } from './shared/r10-connect-pairing';
import { EnvironmentFleet, fleet } from './shared/settings-b-fleet';

export const mobileClient = new T3Client();

/** Mobile QR links wrap a normal hosted/direct pairing URL. The normal parser still validates it. */
export function mobilePairingUrl(input: string): string {
  const trimmed = input.trim();
  try {
    const url = new URL(trimmed);
    if (url.protocol === 't3code:') return url.searchParams.get('pairingUrl')?.trim() || trimmed;
  } catch { /* Raw host text is also accepted by the form. */ }
  return trimmed;
}

export function mobilePairingFields(input: string): { source: string; host: string; code: string } {
  const raw = mobilePairingUrl(input);
  if (!raw) return { source: input, host: '', code: '' };
  const shared = pairingFields(raw);
  if (shared.host) return { ...shared, source: input };
  try {
    const url = new URL(raw);
    url.hash = ''; url.search = ''; url.pathname = '/';
    return { source: input, host: url.toString().replace(/\/$/, ''), code: '' };
  } catch { return { source: input, host: raw, code: '' }; }
}

function isIpLiteral(host: string): boolean {
  try {
    const name = new URL(`http://${host}`).hostname.replace(/^\[|\]$/g, '');
    if (name.includes(':')) return true;
    const parts = name.split('.');
    return parts.length === 4 && parts.every(part => /^\d{1,3}$/.test(part) && Number(part) <= 255);
  } catch { return false; }
}

/** The same host+code rule as upstream buildPairingUrl, followed by the shared validation. */
export function mobilePairingTarget(host: string, code: string): { origin: string; credential: string } {
  const credentialInput = mobilePairingUrl(code);
  const input = /^(https?|wss?|t3code):\/\//i.test(code.trim()) ? mobilePairingUrl(credentialInput) : mobilePairingUrl(host);
  const fields = mobilePairingFields(input);
  let address = fields.host || input;
  const credential = fields.code || (/^[a-z]+:\/\//i.test(code.trim()) ? '' : code.trim());
  if (credential && address && !address.includes('://')) address = `${isIpLiteral(address) ? 'http' : 'https'}://${address}`;
  try { return parsePairing(address, credential); }
  catch { throw new ClientError('Enter a valid T3 server address or pairing link.', 'Address'); }
}

/**
 * Shared commands identify newly authored work as web. Mobile changes only those wire payloads,
 * preserving the reducer, command IDs, replay ownership and every other native operation.
 */
export function mobileNative(native: Native): Native {
  return { available: native.available, watch: topic => native.watch(topic), later: request => {
    mobileVoiceObserveDraft(mobileClient);
    const operation = obj(request), payload = obj(operation.payload);
    if (operation.op === 'events') return bridgeReply(native, request).then(response => {
      if (response.ok) {
        mobileBrowserEvents(obj(response.value).events, mobileClient);
        mobileDevicesEvents(obj(response.value).events, mobileClient);
        mobileGitEvents(obj(response.value).events, mobileClient);
      }
      return response;
    });
    const mobileWrite = operation.op === 'request' && payload.creationSource === 'web'
      && (operation.method === 'orchestration.launchThread'
        || (operation.method === 'orchestration.dispatchCommand'
          && ['message.dispatch', 'thread.fork', 'thread.merge_back'].includes(str(payload.type))));
    return native.later(mobileWrite ? { ...operation, payload: { ...payload, creationSource: 'mobile' } } : request);
  } };
}

function answerHandles(native: Native | null | undefined, suppliedStorage: Files) {
  const handle = native?.available ? letGoAware(mobileNative(native)) : native;
  return { native: handle, storage: handle?.available ? nativeFiles(handle) : suppliedStorage };
}

/** Upstream lists every saved environment, including one environment and switched-off ones. */
export function mobileRoutingRows(sources: ReturnType<typeof environmentSources>, saved: Obj[], preferencesText: string, ready: boolean) {
  const prefs = decodePrefs(preferencesText);
  return sources.filter(source => saved.some(entry => entry.environmentId === source.environmentId)).map(source => {
    const key = gitHubRoutingConnectionKey(source);
    const label = str(saved.find(entry => entry.environmentId === source.environmentId)?.mobileLabel) || source.label;
    return { id: source.key, label, url: source.routes.map(connectionRouteAddress).find(address => address !== null) || 'T3 Connect',
      permission: key ? prefs.githubRouting[key] || 'off' : 'off', disabled: !ready || key === null };
  });
}

/** Rendering projection only. Saved credentials and access tokens never enter these rows. */
export async function mobileSnapshot(nativeInput: Native | null | undefined, suppliedStorage: Files) {
  const { native, storage } = answerHandles(nativeInput, suppliedStorage);
  await mobileClient.refresh(native, storage);
  mobileVoiceObserveDraft(mobileClient);
  let focusedStatus = {}, savedCatalog = fleet.saved, preferencesText = '{}', routingReady = false;
  if (native?.available) {
    try {
      const [status, catalog, preferences, mobilePreferences] = await Promise.all([
        bridgeReply(native, { op: 'status' }), bridgeReply(native, { op: 'environments' }),
        bridgeReply(native, { op: 'connectionPreferences' }), bridgeReply(native, { op: 'mobilePreferences' }),
      ]);
      if (mobilePreferences.ok) applyMobileComposerBehavior(mobileClient, mobilePreferences.value);
      if (status.ok) focusedStatus = obj(status.value);
      if (catalog.ok) savedCatalog = arr(obj(catalog.value).saved);
      if (preferences.ok) preferencesText = str(obj(preferences.value).text, '{}');
      routingReady = catalog.ok && preferences.ok;
    } catch (error) { if (letGo(error)) throw error; }
  }
  const sources = environmentSources(mobileClient, savedCatalog, fleet.entries, focusedStatus);
  const environments = sources.map(source => {
    const status = savedStatus(source.enabled, source.phase, source.error);
    const saved = savedCatalog.find(entry => str(entry.environmentId) === source.environmentId);
    return {
      key: source.key, environmentId: source.environmentId, origin: source.origin, label: str(saved?.mobileLabel) || source.label,
      machine: machineKind(source.config, source.machine), relayManaged: hasRelayRoute(source),
      enabled: source.enabled, active: source.focused, state: source.phase, status: status.text,
      error: source.error, traceId: source.traceId, routeCount: source.routes.length,
      routes: source.routes.map(route => ({ id: route.id, label: connectionRouteLabel(route),
        address: connectionRouteAddress(route) || '', kind: connectionRouteKind(route), active: route.id === source.activeRouteId })),
    };
  });
  return {
    nativeAvailable: native?.available === true, revision: mobileClient.revision,
    connection: mobileClient.connection, statusMessage: mobileClient.statusMessage, error: mobileClient.error,
    ready: mobileClient.ready, writable: mobileClient.writable, busy: mobileClient.busy,
    environmentId: mobileClient.environmentId, environmentLabel: str(obj(mobileClient.config.environment).label),
    origin: mobileClient.environmentId ? mobileClient.origin : '', environments,
    routing: mobileRoutingRows(sources, savedCatalog, preferencesText, routingReady),
    shellLoaded: mobileClient.shellLoaded, projectId: mobileClient.projectId, threadId: mobileClient.threadId,
    projects: mobileClient.shell.projects, threads: mobileClient.shell.threads,
    projection: mobileClient.projection, draft: mobileClient.draft,
    scopes: [...mobileClient.scopes], generation: mobileClient.generation,
    configLive: mobileClient.configLive, shellLive: mobileClient.shellLive, threadLive: mobileClient.threadLive,
  };
}

/** Root mutation arguments are [operation, id/host, value/code, numericValue]. */
export async function mobileCommand(args: unknown[], nativeInput: Native | null | undefined, suppliedStorage: Files) {
  try { return await runMobileCommand(args, nativeInput, suppliedStorage); }
  finally { mobileVoiceObserveDraft(mobileClient); }
}
async function runMobileCommand(args: unknown[], nativeInput: Native | null | undefined, suppliedStorage: Files) {
  const { native, storage } = answerHandles(nativeInput, suppliedStorage);
  if (!native?.available) return { revision: mobileClient.revision, message: 'Open T3 Code on your iPhone or iPad to connect.' };
  let op = str(args[0]), id = str(args[1]), value = str(args[2]);
  if (op === 'send' || op === 'send-alternate') return mobileSend(mobileClient, op === 'send-alternate', native, storage);
  if (op === 'draft') return mobileDraftChanged(mobileClient, value, native, storage);
  if (op === 'environment-reconnect') {
    try {
      const catalog = await bridgeReply(native, { op: 'environments' });
      if (!catalog.ok) throw new ClientError(catalog.error!.message, catalog.error!.kind);
      const saved = arr(obj(catalog.value).saved).find(entry => `${str(entry.origin)}\n${str(entry.environmentId)}` === id);
      if (!saved || saved.enabled === false) throw new ClientError('Switch on a saved environment before reconnecting.');
      if (saved.environmentId === mobileClient.environmentId)
        return mobileClient.command('reconnect', str(saved.origin), '', 0, native, storage);
      // A row's retry belongs to its fleet transport, never to the focused client.
      const response = await bridgeReply(EnvironmentFleet.native(native, id), { op: 'retry' });
      if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind);
      return { revision: ++mobileClient.revision, message: '' };
    } catch (error) {
      if (letGo(error)) throw error;
      return { revision: mobileClient.revision, message: error instanceof Error ? error.message : 'The environment could not reconnect.' };
    }
  }
  if (['connect', 'reconnect', 'environment-add', 'environment-add-route'].includes(op)) {
    try { const target = mobilePairingTarget(id || mobileClient.origin, value); id = target.origin; value = target.credential; }
    catch (error) { return { revision: mobileClient.revision, message: error instanceof Error ? error.message : 'The pairing link is invalid.' }; }
  }
  if (op === 'environment-add-route') {
    const environmentId = str(args[3]).trim();
    if (!environmentId || /\s/.test(environmentId)) return { revision: mobileClient.revision, message: 'Choose a saved environment.' };
    // Shared route pairing checks the descriptor before spending the code, then places
    // the new route without replacing the focused connection.
    return mobileClient.command('environment-route-add', `${environmentId} ${id}`, value, 0, native, storage);
  }
  if (op === 'save-environment') {
    try {
      const input = obj(JSON.parse(value)), label = str(input.label).trim(), url = str(input.url).trim();
      if (!label) throw new ClientError('Environment label cannot be empty.');
      const target = mobilePairingTarget(url, '');
      if (target.credential) throw new ClientError('Enter the environment URL without a pairing code.');
      const saved = fleet.saved.find(entry => str(entry.environmentId) === id);
      const response = await bridgeReply(native, { op: 'mobileUpdateEnvironment', environmentId: id, label, origin: target.origin });
      if (!response.ok) throw new ClientError(response.error!.message, response.error!.kind);
      // A background lease also has to reconnect with its edited profile.
      if (saved && id !== mobileClient.environmentId) {
        const key = `${str(saved.origin)}\n${id}`;
        fleet.forget(key);
        await native.later({ op: 'fleetStop', fleet: key });
      }
      return { revision: ++mobileClient.revision, message: '' };
    } catch (error) {
      if (letGo(error)) throw error;
      return { revision: mobileClient.revision, message: error instanceof SyntaxError ? 'The environment settings are invalid.' : error instanceof Error ? error.message : 'The environment could not be saved.' };
    }
  }
  // Mobile's remove-row action carries its saved origin and environment identity.
  if (op === 'forget' && id && value) op = 'environment-forget';
  // Persist the disabled state too, so fleet sync does not reopen a manually disconnected environment.
  if (op === 'disconnect' && mobileClient.environmentId) {
    const saved = fleet.saved.find(entry => str(entry.environmentId) === mobileClient.environmentId);
    if (saved) { op = 'environment-enabled'; id = `${str(saved.origin)}\n${mobileClient.environmentId}`; value = 'off'; }
  }
  // Do not expose desktop host capabilities through a mobile command source.
  if (op.startsWith('ssh') || op.startsWith('environment-ssh')) return { revision: mobileClient.revision, message: 'SSH environments are only available in the desktop app.' };
  return mobileClient.command(op, id, value, Number(args[3]) || 0, native, storage);
}
