// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
// Mobile presentation and permission rules: T3 Code 365aa87982
// SettingsEnvironmentDetailRouteScreen.tsx, environment-maintenance.ts, auth.ts.
import { mobileClient, mobileNative } from './client';
import { environmentSources } from './shared/connections';
import { connectionRouteAddress, connectionRouteKind, connectionRouteLabel, isLearned } from './shared/connection-routes';
import { moveSavedRoute, removeSavedRoute, savedList } from './shared/connection-routes-ops';
import { arr, obj, str, type Obj } from './shared/domain';
import { letGo, letGoAware } from './shared/let-go';
import { bridgeReply, ClientError, type Native, type Files } from './shared/protocol';
import { environmentRequest } from './shared/settings-scope-sources';
import { EnvironmentFleet, fleet } from './shared/settings-b-fleet';
import { serverUpdateStateFor, updateTargetFromConfig, updateEnvironment, nativeUpdateDeps, type ServerUpdateState } from './shared/server-update';
import { compareSemver, resolveServerSelfUpdateCapability, supportsDesktopAppUpdate } from './shared/version-skew';

type Source = ReturnType<typeof environmentSources>[number];
type DetailState = { error: string; notice: string; fromVersion: string; targetVersion: string | null; checked: boolean };
const states = new Map<string, DetailState>();
const emptyState = (): DetailState => ({ error: '', notice: '', fromVersion: '', targetVersion: null, checked: false });
const stateFor = (key: string) => states.get(key) ?? emptyState();

import { mobileSessionGrants } from './mobile-grants';
export { mobileSessionGrants } from './mobile-grants';

/** The mobile update button has narrower eligibility than desktop's install/repair action. */
export function mobileCanUpdateProvider(provider: Obj): boolean {
  const advisory = obj(provider.versionAdvisory), compatibility = obj(provider.compatibilityAdvisory);
  return provider.installed === true && provider.availability !== 'unavailable'
    && advisory.status === 'behind_latest' && advisory.canUpdate === true && typeof advisory.latestVersion === 'string'
    && !['broken', 'unsupported'].includes(str(compatibility.latestVersionStatus))
    && !['running', 'queued'].includes(str(obj(provider.updateState).status));
}

const routeIcons: Record<string, string> = { relay: 'cloud', loopback: 'desktopcomputer', lan: 'wifi',
  tailnet: 'point.3.connected.trianglepath.dotted', public: 'globe', ssh: 'terminal' };

export function environmentDetailProjection(source: Source | null, session: Obj | null,
  options: { sessionFailed?: boolean; state?: DetailState; update?: ServerUpdateState } = {}) {
  const config = source?.config ?? {}, descriptor = obj(config.environment), version = str(descriptor.serverVersion);
  const key = source?.key ?? '', state = options.state ?? stateFor(key);
  const connected = source?.enabled === true && source.phase === 'connected';
  const allowed = connected && mobileSessionGrants(session, 'environment:maintain');
  const providers = arr(config.providers), update = options.update ?? serverUpdateStateFor(source?.environmentId ?? '', version || null);
  const running = update.status === 'running';
  const busy = !connected || running || providers.some(provider => ['running', 'queued'].includes(str(obj(provider.updateState).status)));
  const capability = resolveServerSelfUpdateCapability(config);
  const supportsUpdate = capability !== null && (capability !== 'desktop-managed' || supportsDesktopAppUpdate(config));
  const checked = state.checked && state.fromVersion === version;
  return {
    loaded: true, found: source !== null, key, environmentId: source?.environmentId ?? '', title: source?.label ?? 'Environment',
    connected, configPresent: Object.keys(config).length > 0, version, busy,
    disabled: !allowed || busy, refreshDisabled: busy || !mobileSessionGrants(session, 'orchestration:read'),
    permissionNote: !connected ? 'Connect this environment to manage it.' : allowed ? ''
      : options.sessionFailed ? 'Could not verify your permissions. Reconnect to try again.'
      : session === null ? 'Checking permissions…' : 'This connection does not have permission to manage the environment.',
    error: state.error, notice: state.notice,
    updateMessage: running ? update.stage === 'resuming' ? 'Restarting and reconnecting…' : 'Downloading update…'
      : update.status === 'failed' ? update.message : '', updateFailed: update.status === 'failed',
    releaseMessage: checked ? state.targetVersion ? `Version ${state.targetVersion} is available.` : 'You are up to date.' : '',
    updateConfirmation: `Install T3 Code ${checked ? state.targetVersion ?? '' : ''}. ${capability === 'desktop-managed' ? 'The desktop app will close and relaunch.' : 'The server will restart and reconnect.'} Running threads may be interrupted.`,
    targetVersion: checked ? state.targetVersion ?? '' : '', showUpdate: checked && state.targetVersion !== null && supportsUpdate,
    manualUpdate: supportsUpdate ? '' : capability === 'desktop-managed' ? 'Update the desktop app on this machine.' : 'Update and restart T3 Code on this machine.',
    routesHeight: (source?.routes ?? []).reduce((height, route) => height + (connectionRouteAddress(route) ? 71.75 : 51), 0),
    routes: (source?.routes ?? []).map((route, index, routes) => ({ id: route.id, label: connectionRouteLabel(route),
      address: connectionRouteAddress(route) ?? '', icon: routeIcons[connectionRouteKind(route)] ?? 'globe',
      active: connected && route.id === source?.activeRouteId, removable: routes.length > 1 && !isLearned(route),
      learned: isLearned(route), position: index + 1, count: routes.length,
      moveUp: index > 0 ? `${route.id}>${routes[index - 1]!.id}` : '',
      moveDown: index < routes.length - 1 ? `${route.id}>${routes[index + 2]?.id ?? ''}` : '' })),
    providers: providers.filter(provider => provider.enabled === true).map(provider => {
      const advisory = obj(provider.versionAdvisory), progress = obj(provider.updateState), compatibility = obj(provider.compatibilityAdvisory);
      return { id: str(provider.instanceId), driver: str(provider.driver), label: str(provider.displayName) || str(provider.driver),
        version: `${provider.installed === true ? str(provider.version) || 'Version unknown' : 'Not installed'}${advisory.latestVersion ? ` · Latest ${str(advisory.latestVersion)}` : ''}`,
        updateMessage: progress.status && progress.status !== 'idle' ? str(progress.message) || `Update ${str(progress.status)}` : '',
        updateFailed: progress.status === 'failed', compatibility: str(compatibility.message),
        message: str(provider.unavailableReason) || str(provider.message),
        manualUpdate: advisory.status === 'behind_latest' && advisory.canUpdate !== true,
        canUpdate: mobileCanUpdateProvider(provider), disabled: busy || !mobileSessionGrants(session, 'providers:manage') };
    }),
  };
}

/** Public links name a server ID; app actions retain the exact route key. Refuse ambiguous IDs. */
export function mobileEnvironmentSource<T extends { key: string; environmentId: string }>(sources: readonly T[], key: string): T | null {
  const exact = sources.find(source => source.key === key);
  if (exact) return exact;
  const matches = sources.filter(source => source.environmentId === key);
  return matches.length === 1 ? matches[0]! : null;
}
async function selectedEnvironment(key: string, native: Native) {
  const saved = await savedList(native);
  const status = await bridgeReply(native, { op: 'status' });
  const source = mobileEnvironmentSource(environmentSources(mobileClient, saved, fleet.entries, status.ok ? obj(status.value) : {}), key);
  if (source) source.label = str(saved.find(entry => entry.environmentId === source.environmentId)?.mobileLabel) || source.label;
  const remote = source?.focused ? native : EnvironmentFleet.native(native, source?.key ?? key);
  const generation = source?.focused ? mobileClient.generation : fleet.entries.get(source?.key ?? key)?.generation ?? -1;
  return { source, remote, generation };
}
async function call(remote: Native, generation: number, request: Obj): Promise<Obj> {
  const reply = await bridgeReply(remote, { ...request, generation });
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  if (reply.generation !== generation) throw new ClientError('The connection changed. Reconnect to try again.', 'stale');
  return obj(reply.value);
}

async function selectedRPC(source: Source, native: Native, generation: number, method: string, payload: Obj) {
  // settingsCoreRequest intentionally refuses provider methods; use the focused client's
  // ordinary RPC and the shared scoped fleet dispatcher for background environments.
  if ((source.focused ? mobileClient.generation : fleet.entries.get(source.key)?.generation) !== generation)
    throw new ClientError('The connection changed. Reconnect to try again.', 'stale');
  return source.focused ? mobileClient.request(native, method, payload, generation, true)
    : environmentRequest(mobileClient, native, { fleetKey: source.key }, method, payload, true);
}

/** Root resource depends on the main snapshot revision. No second client or refresh/reducer. */
export async function mobileEnvironmentDetail(key: string, nativeInput: Native | null | undefined, _storage?: Files) {
  if (!key || !nativeInput?.available) return environmentDetailProjection(null, null);
  const native = letGoAware(mobileNative(nativeInput));
  native.watch('t3.status'); native.watch('t3.fleet');
  const { source, remote, generation } = await selectedEnvironment(key, native);
  let session: Obj | null = null, sessionFailed = false;
  if (source?.enabled && source.phase === 'connected') {
    try { session = await call(remote, generation, { op: 'http', path: '/api/auth/session' }); }
    catch (error) { if (letGo(error)) throw error; sessionFailed = true; }
  }
  return environmentDetailProjection(source, session, { sessionFailed });
}

/** Pinned cliRelease channel selection; shared semver comparison prevents downgrades. */
export function mobileReleaseChannel(version: string): string {
  return /^[^-+]+-(nightly|preview)\.\d{8}\.\d+$/.exec(version)?.[1] ?? 'stable';
}
export function newestMobileRelease(releases: unknown, channel: string): string | undefined {
  if (!Array.isArray(releases) || releases.some(value => typeof obj(value).tag_name !== 'string'
    || (obj(value).draft !== undefined && typeof obj(value).draft !== 'boolean'))) throw new ClientError('The release response is invalid.');
  for (const release of releases) {
    if (release.draft) continue;
    const version = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/.exec(release.tag_name)?.[1];
    if (version && mobileReleaseChannel(version) === channel) return version;
  }
}
async function checkRelease(native: Native, current: string): Promise<string | null> {
  const channel = mobileReleaseChannel(current);
  let deadlineMs = 0;
  for (let page = 1; page <= 100; page++) {
    const reply = await bridgeReply(native, { op: 'mobileReleasePage', page, ...(page > 1 ? { deadlineMs } : {}) });
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
    const value = obj(reply.value);
    if (typeof value.deadlineMs !== 'number' || !Number.isFinite(value.deadlineMs) || value.deadlineMs <= 0)
      throw new ClientError('The release deadline is invalid.');
    deadlineMs = value.deadlineMs;
    const releases = value.releases;
    const version = newestMobileRelease(releases, channel);
    if (version) return compareSemver(version, current) > 0 ? version : null;
    if ((releases as unknown[]).length < 100) throw new ClientError(`No ${channel} release was found.`);
  }
  throw new ClientError('The release check exceeded 100 pages. Try again.');
}

/** Parent confirms destructive update/removal before calling. Scope is rechecked on every server write. */
export async function mobileEnvironmentDetailCommand(kind: string, key: string, value: string,
  nativeInput: Native | null | undefined, _storage?: Files) {
  if (!nativeInput?.available) return { revision: mobileClient.revision, message: 'Open T3 Code on your iPhone or iPad to manage environments.' };
  const native = letGoAware(mobileNative(nativeInput)), state = { ...stateFor(key), error: '', notice: '' };
  try {
    const { source, remote, generation } = await selectedEnvironment(key, native);
    if (!source) throw new ClientError('This environment is no longer saved on this device.');
    if (kind === 'route-remove') await removeSavedRoute(native, source.environmentId, value);
    else if (kind === 'route-move') await moveSavedRoute(native, source.environmentId, value);
    else {
      if (!source.enabled || source.phase !== 'connected') throw new ClientError('Connect this environment to manage it.');
      const session = await call(remote, generation, { op: 'http', path: '/api/auth/session' });
      const data = environmentDetailProjection(source, session, { state });
      if (kind === 'refresh-providers') {
        if (data.refreshDisabled) throw new ClientError('This connection does not have permission to refresh providers.');
        await selectedRPC(source, native, generation, 'server.refreshProviders', {});
        state.notice = 'Provider status refreshed.';
      } else if (kind === 'update-provider') {
        const provider = arr(source.config.providers).find(entry => entry.instanceId === value);
        if (data.busy || !mobileSessionGrants(session, 'providers:manage') || !provider || !mobileCanUpdateProvider(provider))
          throw new ClientError('This provider cannot be updated from this connection.');
        await selectedRPC(source, native, generation, 'server.updateProvider', { provider: str(provider.driver), instanceId: value });
      } else if (kind === 'check-updates') {
        if (data.disabled) throw new ClientError('This connection does not have permission to manage the environment.');
        state.targetVersion = await checkRelease(native, data.version); state.fromVersion = data.version; state.checked = true;
      } else if (kind === 'update-server') {
        if (data.disabled || !data.showUpdate || !data.targetVersion) throw new ClientError('Check for an available update before updating this environment.');
        const target = { ...updateTargetFromConfig(source.config, key, source.environmentId, source.label), targetVersion: data.targetVersion };
        const result = await updateEnvironment(target, nativeUpdateDeps(native, mobileClient));
        if (result !== 'started') throw new ClientError(result === 'busy' ? 'An update is already running.' : 'The environment update could not be started.');
        state.checked = false;
      } else throw new ClientError(`Unknown environment action: ${kind}`);
    }
    states.set(key, state);
    return { revision: ++mobileClient.revision, message: '' };
  } catch (error) {
    if (letGo(error)) throw error;
    state.error = error instanceof Error ? error.message : 'The action could not be completed. Try again.';
    states.set(key, state);
    return { revision: ++mobileClient.revision, message: state.error };
  }
}

/** Same origin policy as pinned resolveOfficialAcpRegistryIconUrl. */
export function mobileProviderIconURL(input: unknown): string {
  if (typeof input !== 'string') return '';
  try {
    const url = new URL(input);
    return url.protocol === 'https:' && url.hostname === 'cdn.agentclientprotocol.com'
      && !url.port && !url.username && !url.password ? url.href : '';
  } catch { return ''; }
}
