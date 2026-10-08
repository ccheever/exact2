// The embedded T3 server's state for TypeScript (20261005-embedded-server-runtime). The native
// module runs one local backend per app (T3LocalBackend.swift); this reads its status
// (`localBackendStatus`) and watches `t3.local`; local-primary.ts builds the primary environment
// ("This machine") from it. The bearer stays native (memory only); TypeScript sees only whether it
// is ready. Once the server answers, the status also carries its descriptor's environment id,
// label and version, and `enabled` is the Local environment switch the backend started with.
// `desktopSettings` are the four keys of `<T3 home>/userdata/desktop-settings.json` this app changes
// (decision U7: the original app's file, owned by the native side as by Electron's main process;
// T3DesktopSettings.swift); `writeDesktopSettings` is the renderer's IPC setter.
import { obj, str, num, type Obj } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';

export type DesktopServerExposureMode = 'local-only' | 'network-accessible';
/** The desktop settings this app reads and writes (DesktopSettings' local and exposure keys). */
export interface DesktopSettingsFacts {
  localEnvironmentEnabled: boolean;
  serverExposureMode: DesktopServerExposureMode;
  tailscaleServeEnabled: boolean;
  tailscaleServePort: number;
}
/** DEFAULT_DESKTOP_SETTINGS' four keys. */
export const defaultDesktopSettings = (): DesktopSettingsFacts => ({ localEnvironmentEnabled: true, serverExposureMode: 'local-only', tailscaleServeEnabled: false, tailscaleServePort: 443 });
/** The native side normalized them already; anything else is the default. */
export function parseDesktopSettings(value: unknown): DesktopSettingsFacts {
  const raw = obj(value), port = raw.tailscaleServePort;
  return {
    localEnvironmentEnabled: raw.localEnvironmentEnabled !== false,
    serverExposureMode: raw.serverExposureMode === 'network-accessible' ? 'network-accessible' : 'local-only',
    tailscaleServeEnabled: raw.tailscaleServeEnabled === true,
    tailscaleServePort: typeof port === 'number' && Number.isInteger(port) && port >= 1 && port <= 65_535 ? port : 443,
  };
}

export type LocalBackendState = 'refused' | 'runtime-missing' | 'installing' | 'starting' | 'ready' | 'restarting' | 'stopped' | 'failed';
const STATES: readonly LocalBackendState[] = ['refused', 'runtime-missing', 'installing', 'starting', 'ready', 'restarting', 'stopped', 'failed'];

export interface LocalBackendStatus {
  state: LocalBackendState;
  port: number | null;
  httpBaseUrl: string;
  wsBaseUrl: string;
  bearerReady: boolean;
  restartAttempt: number;
  /** Milliseconds until a scheduled restart, when one is scheduled. */
  nextRestartMs: number | null;
  /** The last exit of a server run: `code=<n>` or `signal=<n>`. */
  lastExit: string;
  /** While installing: the phase and its fraction; when missing or failed: the reason. */
  install: { phase: string; fraction: number; reason: string } | null;
  /** Why a development build starts nothing (item 8 of the ticket). */
  refused: string;
  /** Port selection and other start failures. */
  failure: string;
  version: string;
  pid: number | null;
  /** The Local environment switch the backend runs with (false: no server runs). */
  enabled: boolean;
  /** desktop-settings.json's Local environment switch, Network access and Tailscale Serve (decision U7). */
  settings: DesktopSettingsFacts;
  /** The running server's descriptor (`/.well-known/t3/environment`), once it answered. */
  environmentId: string;
  label: string;
  serverVersion: string;
}

export const unknownLocalBackend = (): LocalBackendStatus => ({
  state: 'stopped', port: null, httpBaseUrl: '', wsBaseUrl: '', bearerReady: false, restartAttempt: 0,
  nextRestartMs: null, lastExit: '', install: null, refused: '', failure: '', version: '', pid: null,
  enabled: true, settings: defaultDesktopSettings(), environmentId: '', label: '', serverVersion: '',
});

const numberOrNull = (value: unknown): number | null => (typeof value === 'number' && Number.isFinite(value) ? value : null);

/** The native status object, as typed fields (unknown values fall back to their empty form). */
export function parseLocalBackendStatus(value: unknown): LocalBackendStatus {
  const raw = obj(value), install = raw.install === undefined || raw.install === null ? null : obj(raw.install);
  const state = STATES.includes(str(raw.state) as LocalBackendState) ? (str(raw.state) as LocalBackendState) : 'stopped';
  return {
    state,
    port: numberOrNull(raw.port),
    httpBaseUrl: str(raw.httpBaseUrl),
    wsBaseUrl: str(raw.wsBaseUrl),
    bearerReady: raw.bearerReady === true,
    restartAttempt: num(raw.restartAttempt) || 0,
    nextRestartMs: numberOrNull(raw.nextRestartMs),
    lastExit: str(raw.lastExit),
    install: install ? { phase: str(install.phase), fraction: num(install.fraction) || 0, reason: str(install.reason) } : null,
    refused: str(raw.refused),
    failure: str(raw.failure),
    version: str(raw.version),
    pid: numberOrNull(raw.pid),
    enabled: raw.enabled !== false,
    settings: parseDesktopSettings(raw.desktopSettings),
    environmentId: str(raw.environmentId),
    label: str(raw.label),
    serverVersion: str(raw.serverVersion),
  };
}

/** Reads the status and asks to be read again when `t3.local` changes. */
export async function readLocalBackend(native: Native): Promise<LocalBackendStatus> {
  native.watch('t3.local');
  const reply = await bridgeReply(native, { op: 'localBackendStatus' });
  return reply.ok ? parseLocalBackendStatus(reply.value) : unknownLocalBackend();
}

/**
 * `desktopSettingsSet`: one DesktopAppSettings setter (setLocalEnvironmentEnabled, setServerExposureMode,
 * setTailscaleServe), persisted to desktop-settings.json before it answers. The owner's status takes the
 * new keys at once (the announce that follows re-reads them). A write failure throws the reference's
 * "Desktop settings write failed during <operation> at <path>." and changes nothing.
 */
export async function writeDesktopSettings(native: Native, patch: Obj, owner?: { localBackend: LocalBackendStatus }): Promise<{ changed: boolean; settings: DesktopSettingsFacts }> {
  const reply = await bridgeReply(native, { op: 'desktopSettingsSet', ...patch });
  if (!reply.ok) throw new ClientError(reply.error?.message || 'Desktop settings write failed.', reply.error?.kind ?? 'DesktopSettings');
  const value = obj(reply.value), settings = parseDesktopSettings(value.settings);
  if (owner) owner.localBackend.settings = settings;
  return { changed: value.changed === true, settings };
}
