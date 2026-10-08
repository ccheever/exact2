// Network access for the embedded server (20261005-this-machine-network-access item 1), after T3
// Code (MIT, see LICENSE-T3; reference 1e2ecbd975): apps/desktop/src/backend/DesktopServerExposure.ts.
// Local-only binds 127.0.0.1; network access binds 0.0.0.0 and advertises the first usable LAN IPv4
// (or an explicit host). A network request with neither a LAN nor a Tailscale IPv4 is unavailable:
// `setMode` refuses it, and at launch it falls back to local-only without losing the preference.
// The settings are the clone's own `t3-code.json` (decision U7): `serverExposureMode`,
// `tailscaleServeEnabled`, `tailscaleServePort`. Facts come from the native module (interfaces,
// the MagicDNS name, the HTTPS probe); the restart is `applyLocalSetting` (this-machine.ts).
import { createAdvertisedEndpoint, type AdvertisedEndpoint, type AdvertisedEndpointProvider, type CreateAdvertisedEndpointInput } from './advertised-endpoint';
import { DEFAULT_TAILSCALE_SERVE_PORT, isTailscaleIpv4Address, resolveTailscaleAdvertisedEndpoints, type NetworkInterfaces } from './tailscale';

export type DesktopServerExposureMode = 'local-only' | 'network-accessible';
export interface DesktopServerExposureState {
  mode: DesktopServerExposureMode; endpointUrl: string | null; advertisedHost: string | null; tailscaleServeEnabled: boolean; tailscaleServePort: number;
}
export interface ExposureSettings { serverExposureMode: DesktopServerExposureMode; tailscaleServeEnabled: boolean; tailscaleServePort: number }
export const DEFAULT_EXPOSURE_SETTINGS: ExposureSettings = { serverExposureMode: 'local-only', tailscaleServeEnabled: false, tailscaleServePort: DEFAULT_TAILSCALE_SERVE_PORT };

const DESKTOP_LOOPBACK_HOST = '127.0.0.1';
const DESKTOP_LAN_BIND_HOST = '0.0.0.0';

interface ResolvedDesktopServerExposure {
  mode: DesktopServerExposureMode; bindHost: string; localHttpUrl: string; localWsUrl: string; endpointUrl: string | null; advertisedHost: string | null;
}

const DESKTOP_CORE_ENDPOINT_PROVIDER: AdvertisedEndpointProvider = { id: 'desktop-core', label: 'Desktop', kind: 'core', isAddon: false };
const DESKTOP_MANUAL_ENDPOINT_PROVIDER: AdvertisedEndpointProvider = { id: 'manual', label: 'Manual', kind: 'manual', isAddon: false };

const normalizeOptionalHost = (value: string | undefined): string | undefined => { const normalized = value?.trim(); return normalized && normalized.length > 0 ? normalized : undefined; };
const isUsableLanIpv4Address = (address: string): boolean => !address.startsWith('127.') && !address.startsWith('169.254.') && !isTailscaleIpv4Address(address);
const isHttpsEndpointUrl = (value: string): boolean => { try { return new URL(value).protocol === 'https:'; } catch { return false; } };

/** resolveLanAdvertisedHost: an explicit host wins, else the first non-internal IPv4 that is not loopback, link-local or Tailscale. */
export function resolveLanAdvertisedHost(networkInterfaces: NetworkInterfaces, explicitHost: string | undefined): string | null {
  const normalizedExplicitHost = normalizeOptionalHost(explicitHost);
  if (normalizedExplicitHost) return normalizedExplicitHost;
  for (const interfaceAddresses of Object.values(networkInterfaces)) {
    if (!interfaceAddresses) continue;
    for (const address of interfaceAddresses) {
      if (address.internal || address.family !== 'IPv4' || !isUsableLanIpv4Address(address.address)) continue;
      return address.address;
    }
  }
  return null;
}

export function resolveDesktopServerExposure(input: { mode: DesktopServerExposureMode; port: number; networkInterfaces: NetworkInterfaces; advertisedHostOverride?: string }): ResolvedDesktopServerExposure {
  const localHttpUrl = `http://${DESKTOP_LOOPBACK_HOST}:${input.port}`;
  const localWsUrl = `ws://${DESKTOP_LOOPBACK_HOST}:${input.port}`;
  if (input.mode === 'local-only') return { mode: input.mode, bindHost: DESKTOP_LOOPBACK_HOST, localHttpUrl, localWsUrl, endpointUrl: null, advertisedHost: null };
  const advertisedHost = resolveLanAdvertisedHost(input.networkInterfaces, input.advertisedHostOverride);
  return { mode: input.mode, bindHost: DESKTOP_LAN_BIND_HOST, localHttpUrl, localWsUrl, endpointUrl: advertisedHost ? `http://${advertisedHost}:${input.port}` : null, advertisedHost };
}

const createDesktopEndpoint = (input: Omit<CreateAdvertisedEndpointInput, 'provider' | 'source'>) => createAdvertisedEndpoint({ ...input, provider: DESKTOP_CORE_ENDPOINT_PROVIDER, source: 'desktop-core' });
const createManualEndpoint = (input: Omit<CreateAdvertisedEndpointInput, 'provider' | 'source'>) => createAdvertisedEndpoint({ ...input, provider: DESKTOP_MANUAL_ENDPOINT_PROVIDER, source: 'user' });

/** resolveDesktopCoreAdvertisedEndpoints: "This machine" (loopback), "Local network" (the default), then each configured URL. */
export function resolveDesktopCoreAdvertisedEndpoints(input: { port: number; exposure: ResolvedDesktopServerExposure; customHttpsEndpointUrls?: readonly string[] }): AdvertisedEndpoint[] {
  const endpoints: AdvertisedEndpoint[] = [createDesktopEndpoint({ id: `desktop-loopback:${input.port}`, label: 'This machine', httpBaseUrl: input.exposure.localHttpUrl,
    reachability: 'loopback', status: 'available', description: 'Loopback endpoint for this desktop app.' })];
  if (input.exposure.endpointUrl) {
    endpoints.push(createDesktopEndpoint({ id: `desktop-lan:${input.exposure.endpointUrl}`, label: 'Local network', httpBaseUrl: input.exposure.endpointUrl,
      reachability: 'lan', status: 'available', isDefault: true, description: 'Reachable from devices on the same network.' }));
  }
  for (const customEndpointUrl of input.customHttpsEndpointUrls ?? []) {
    try {
      const isHttpsEndpoint = isHttpsEndpointUrl(customEndpointUrl);
      endpoints.push(createManualEndpoint({ id: `manual:${customEndpointUrl}`, label: isHttpsEndpoint ? 'Custom HTTPS' : 'Custom endpoint', httpBaseUrl: customEndpointUrl,
        reachability: 'public', ...(isHttpsEndpoint ? { hostedHttpsCompatibility: 'compatible' as const } : {}), status: 'unknown',
        description: isHttpsEndpoint ? 'User-configured HTTPS endpoint for this desktop backend.' : 'User-configured endpoint for this desktop backend.' }));
    } catch {
      // Ignore malformed user-configured endpoints without dropping valid endpoints.
    }
  }
  return endpoints;
}

export class DesktopServerExposureNoNetworkAddressError extends Error {
  readonly _tag = 'DesktopServerExposureNoNetworkAddressError';
  constructor(readonly port: number) { super(`No reachable network address is available for desktop network access on port ${port}.`); }
}
export class DesktopServerExposureModePersistenceError extends Error {
  readonly _tag = 'DesktopServerExposureModePersistenceError';
  constructor(readonly mode: DesktopServerExposureMode, readonly cause: unknown) { super(`Failed to persist desktop server exposure mode ${mode}.`); }
}
export class DesktopTailscaleServePersistenceError extends Error {
  readonly _tag = 'DesktopTailscaleServePersistenceError';
  constructor(readonly enabled: boolean, readonly port: number | null, readonly cause: unknown) {
    super(`Failed to persist desktop Tailscale Serve settings (enabled: ${enabled}, port: ${port ?? 'unchanged'}).`);
  }
}

interface RuntimeState {
  requestedMode: DesktopServerExposureMode; mode: DesktopServerExposureMode; port: number; bindHost: string; localHttpUrl: string; localWsUrl: string;
  httpBaseUrl: URL; endpointUrl: string | null; advertisedHost: string | null; tailscaleServeEnabled: boolean; tailscaleServePort: number;
}

function runtimeStateFromResolvedExposure(input: { requestedMode: DesktopServerExposureMode; settings: ExposureSettings; exposure: ResolvedDesktopServerExposure; port: number }): RuntimeState {
  return { requestedMode: input.requestedMode, mode: input.exposure.mode, port: input.port, bindHost: input.exposure.bindHost, localHttpUrl: input.exposure.localHttpUrl,
    localWsUrl: input.exposure.localWsUrl, httpBaseUrl: new URL(input.exposure.localHttpUrl), endpointUrl: input.exposure.endpointUrl, advertisedHost: input.exposure.advertisedHost,
    tailscaleServeEnabled: input.settings.tailscaleServeEnabled, tailscaleServePort: input.settings.tailscaleServePort };
}

/** resolveRuntimeState: a network request with no LAN and no Tailscale IPv4 resolves to local-only and says so. */
export function resolveRuntimeState(input: { requestedMode: DesktopServerExposureMode; settings: ExposureSettings; port: number; networkInterfaces: NetworkInterfaces; advertisedHostOverride?: string }): { state: RuntimeState; unavailable: boolean } {
  const override = input.advertisedHostOverride ? { advertisedHostOverride: input.advertisedHostOverride } : {};
  const requestedExposure = resolveDesktopServerExposure({ mode: input.requestedMode, port: input.port, networkInterfaces: input.networkInterfaces, ...override });
  const unavailable = input.requestedMode === 'network-accessible' && requestedExposure.endpointUrl === null
    && !Object.values(input.networkInterfaces).some(addresses => addresses?.some(address => !address.internal && address.family === 'IPv4' && isTailscaleIpv4Address(address.address)));
  const exposure = unavailable ? resolveDesktopServerExposure({ mode: 'local-only', port: input.port, networkInterfaces: input.networkInterfaces, ...override }) : requestedExposure;
  return { state: runtimeStateFromResolvedExposure({ requestedMode: input.requestedMode, settings: input.settings, exposure, port: input.port }), unavailable };
}

/** requiresBackendRelaunch: the port, the bind host or the local URL changed. */
export const requiresBackendRelaunch = (previous: { port: number; bindHost: string; localHttpUrl: string }, next: { port: number; bindHost: string; localHttpUrl: string }): boolean =>
  previous.port !== next.port || previous.bindHost !== next.bindHost || previous.localHttpUrl !== next.localHttpUrl;

const toContractState = (state: RuntimeState): DesktopServerExposureState => ({ mode: state.mode, endpointUrl: state.endpointUrl, advertisedHost: state.advertisedHost,
  tailscaleServeEnabled: state.tailscaleServeEnabled, tailscaleServePort: state.tailscaleServePort });
const toResolvedExposure = (state: RuntimeState): ResolvedDesktopServerExposure => ({ mode: state.mode, bindHost: state.bindHost, localHttpUrl: state.localHttpUrl,
  localWsUrl: state.localWsUrl, endpointUrl: state.endpointUrl, advertisedHost: state.advertisedHost });

export interface DesktopServerExposureBackendConfig { port: number; bindHost: string; httpBaseUrl: URL; tailscaleServeEnabled: boolean; tailscaleServePort: number }
export interface DesktopServerExposureChange { state: DesktopServerExposureState; requiresRelaunch: boolean }

/** DesktopAppSettings' exposure part: get, and the two setters that report whether they changed anything. */
export interface ExposureSettingsStore {
  get(): ExposureSettings;
  setServerExposureMode(mode: DesktopServerExposureMode): Promise<{ changed: boolean }>;
  setTailscaleServe(input: { enabled: boolean; port?: number }): Promise<{ settings: ExposureSettings; changed: boolean }>;
}

export function normalizeTailscaleServePort(value: unknown): number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 1 && value <= 65_535 ? value : DEFAULT_TAILSCALE_SERVE_PORT;
}
/** DesktopAppSettings' decoding of the three keys (unknown values fall back to the defaults). */
export function decodeExposureSettings(saved: Record<string, unknown>): ExposureSettings {
  return { serverExposureMode: saved.serverExposureMode === 'network-accessible' ? 'network-accessible' : 'local-only',
    tailscaleServeEnabled: saved.tailscaleServeEnabled === true, tailscaleServePort: normalizeTailscaleServePort(saved.tailscaleServePort) };
}
/** setServerExposureMode / setTailscaleServe on a settings value. */
export const withServerExposureMode = (settings: ExposureSettings, mode: DesktopServerExposureMode): ExposureSettings => settings.serverExposureMode === mode ? settings : { ...settings, serverExposureMode: mode };
export function withTailscaleServe(settings: ExposureSettings, input: { enabled: boolean; port?: number }): ExposureSettings {
  const port = input.port === undefined ? settings.tailscaleServePort : normalizeTailscaleServePort(input.port);
  return settings.tailscaleServeEnabled === input.enabled && settings.tailscaleServePort === port ? settings : { ...settings, tailscaleServeEnabled: input.enabled, tailscaleServePort: port };
}

/** The settings store over an in-memory holder (tests, and the client's `local` preferences). */
export function memoryExposureSettings(holder: { settings: ExposureSettings }, write: () => Promise<void> = async () => {}): ExposureSettingsStore {
  return {
    get: () => holder.settings,
    setServerExposureMode: async mode => { const next = withServerExposureMode(holder.settings, mode); const changed = next !== holder.settings; if (changed) { holder.settings = next; await write(); } return { changed }; },
    setTailscaleServe: async input => { const next = withTailscaleServe(holder.settings, input); const changed = next !== holder.settings; if (changed) { holder.settings = next; await write(); } return { settings: holder.settings, changed }; },
  };
}

export interface DesktopServerExposureDeps {
  settings: ExposureSettingsStore;
  /** DesktopNetworkInterfaces.read. */
  readNetworkInterfaces: () => Promise<NetworkInterfaces>;
  /** The cached `tailscale status` MagicDNS name (null when none or the CLI failed). */
  readMagicDnsName: () => Promise<string | null>;
  /** probeTailscaleHttpsEndpoint. */
  probe: (baseUrl: string) => Promise<boolean>;
  /** DesktopConfig: T3CODE_DESKTOP_LAN_HOST and T3CODE_DESKTOP_HTTPS_ENDPOINTS. */
  config?: { desktopLanHostOverride?: string; desktopHttpsEndpointUrls?: readonly string[] };
}

const initialRuntimeState = (): RuntimeState => runtimeStateFromResolvedExposure({ requestedMode: DEFAULT_EXPOSURE_SETTINGS.serverExposureMode, settings: DEFAULT_EXPOSURE_SETTINGS,
  exposure: resolveDesktopServerExposure({ mode: DEFAULT_EXPOSURE_SETTINGS.serverExposureMode, port: 0, networkInterfaces: {} }), port: 0 });

/** The DesktopServerExposure service. */
export class DesktopServerExposure {
  private state: RuntimeState = initialRuntimeState();
  constructor(private readonly deps: DesktopServerExposureDeps) {}
  private get override() { return this.deps.config?.desktopLanHostOverride; }

  getState(): DesktopServerExposureState { return toContractState(this.state); }
  backendConfig(): DesktopServerExposureBackendConfig {
    return { port: this.state.port, bindHost: this.state.bindHost, httpBaseUrl: this.state.httpBaseUrl, tailscaleServeEnabled: this.state.tailscaleServeEnabled, tailscaleServePort: this.state.tailscaleServePort };
  }

  async configureFromSettings({ port }: { port: number }): Promise<DesktopServerExposureState> {
    const settings = this.deps.settings.get();
    const resolved = resolveRuntimeState({ requestedMode: settings.serverExposureMode, settings, port, networkInterfaces: await this.deps.readNetworkInterfaces(),
      ...(this.override ? { advertisedHostOverride: this.override } : {}) });
    this.state = resolved.state;
    return toContractState(resolved.state);
  }

  async setMode(mode: DesktopServerExposureMode): Promise<DesktopServerExposureChange> {
    const previous = this.state;
    const nextSettings = withServerExposureMode(this.deps.settings.get(), mode);
    const resolved = resolveRuntimeState({ requestedMode: mode, settings: nextSettings, port: previous.port, networkInterfaces: await this.deps.readNetworkInterfaces(),
      ...(this.override ? { advertisedHostOverride: this.override } : {}) });
    if (resolved.unavailable) throw new DesktopServerExposureNoNetworkAddressError(previous.port);
    let change: { changed: boolean };
    try { change = await this.deps.settings.setServerExposureMode(mode); } catch (cause) { throw new DesktopServerExposureModePersistenceError(mode, cause); }
    this.state = resolved.state;
    return { state: toContractState(resolved.state), requiresRelaunch: change.changed || requiresBackendRelaunch(previous, resolved.state) };
  }

  async setTailscaleServeEnabled(input: { enabled: boolean; port?: number }): Promise<DesktopServerExposureChange> {
    let result: { settings: ExposureSettings; changed: boolean };
    try { result = await this.deps.settings.setTailscaleServe(input); } catch (cause) { throw new DesktopTailscaleServePersistenceError(input.enabled, input.port ?? null, cause); }
    this.state = { ...this.state, tailscaleServeEnabled: result.settings.tailscaleServeEnabled, tailscaleServePort: result.settings.tailscaleServePort };
    return { state: toContractState(this.state), requiresRelaunch: result.changed };
  }

  /** The core endpoints, then Tailscale's, which are read only while network access or Serve is on (the spawn can raise macOS's "Other apps" prompt). */
  async getAdvertisedEndpoints(): Promise<AdvertisedEndpoint[]> {
    const state = this.state;
    const networkInterfaces = await this.deps.readNetworkInterfaces();
    const coreEndpoints = resolveDesktopCoreAdvertisedEndpoints({ port: state.port, exposure: toResolvedExposure(state), customHttpsEndpointUrls: this.deps.config?.desktopHttpsEndpointUrls ?? [] });
    if (state.mode !== 'network-accessible' && !state.tailscaleServeEnabled) return coreEndpoints;
    const tailscaleEndpoints = await resolveTailscaleAdvertisedEndpoints({ port: state.port, serveEnabled: state.tailscaleServeEnabled, servePort: state.tailscaleServePort,
      networkInterfaces, readMagicDnsName: this.deps.readMagicDnsName, probe: this.deps.probe });
    return [...coreEndpoints, ...tailscaleEndpoints];
  }
}

// ── t3-code.json (decision U7: the clone's own file; top-level keys as desktop-settings.json) ──
type LocalHolder = { local: object };
const prefsOf = (owner: LocalHolder) => owner.local as Record<string, unknown>;
export function exposureSettings(owner: LocalHolder): ExposureSettings { return decodeExposureSettings(prefsOf(owner)); }
export function writeExposureSettings(owner: LocalHolder, settings: ExposureSettings): void { Object.assign(prefsOf(owner), settings); }
/** The chosen default endpoint (ConnectionsSettings `defaultAdvertisedEndpointKey`, endpointDefaultPreferenceKey). */
export const defaultEndpointKey = (owner: LocalHolder): string | null => { const key = prefsOf(owner).defaultAdvertisedEndpointKey; return typeof key === 'string' && key ? key : null; };
export function setDefaultEndpointKey(owner: LocalHolder, key: string): void { prefsOf(owner).defaultAdvertisedEndpointKey = key; }
/** load(): carry the exposure keys and the default endpoint from the saved file. */
export function adoptNetworkPrefs(next: object, saved: Record<string, unknown>): void {
  writeExposureSettings({ local: next }, decodeExposureSettings(saved));
  if (typeof saved.defaultAdvertisedEndpointKey === 'string' && saved.defaultAdvertisedEndpointKey) prefsOf({ local: next }).defaultAdvertisedEndpointKey = saved.defaultAdvertisedEndpointKey;
}
