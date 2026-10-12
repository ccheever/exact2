// Version skew between this client and a connected server (T3 Code MIT, see LICENSE-T3;
// reference 1e2ecbd975: apps/web/src/versionSkew.ts, packages/shared/src/semver.ts).
// Port changes: APP_VERSION is CLIENT_VERSION (the Nightly this client mirrors) and
// every resolver takes the client version as an optional argument; the dismissal
// document lives in the client's preference file (shell-prefs.ts
// `versionMismatchDismissals`) instead of localStorage, so a store is passed in
// (a module store stands in for tests); the failed-attempt WeakSet keys by the
// attempt the native update job numbers, since its state is rebuilt on each read.
import { obj, str } from './domain';
import { manualServerUpdateCommand, type ServerInstallation } from './server-installation';

/** This client's T3 Code release: the Nightly it mirrors (f90b77d809, apps/web 0.0.46-nightly.20261004.1). */
export const CLIENT_VERSION = '0.0.46-nightly.20261004.1';

export { manualServerUpdateCommand };
export type { ServerInstallation };

export interface VersionMismatch { readonly clientVersion: string; readonly serverVersion: string; readonly hint: string }

const MISMATCH_HINT = 'Version mismatch. Try syncing the client and server to the same T3 Code version.';

// ── semver.ts ─────────────────────────────────────────────────────────────
type Semver = { core: number[]; pre: string[] };
export function parseSemver(version: string): Semver | null {
  const match = /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(version.trim());
  return match ? { core: [Number(match[1]), Number(match[2]), Number(match[3])], pre: match[4] ? match[4].split('.') : [] } : null;
}
export function compareSemver(left: string, right: string): number {
  const a = parseSemver(left), b = parseSemver(right);
  if (!a || !b) return left === right ? 0 : left < right ? -1 : 1;
  for (let i = 0; i < 3; i++) if (a.core[i] !== b.core[i]) return a.core[i]! < b.core[i]! ? -1 : 1;
  if (!a.pre.length || !b.pre.length) return a.pre.length === b.pre.length ? 0 : a.pre.length ? -1 : 1;
  for (let i = 0; i < Math.max(a.pre.length, b.pre.length); i++) {
    const x = a.pre[i], y = b.pre[i];
    if (x === undefined || y === undefined) return x === undefined ? -1 : 1;
    if (x === y) continue;
    const nx = /^\d+$/.test(x), ny = /^\d+$/.test(y);
    if (nx && ny) return Number(x) < Number(y) ? -1 : 1;
    if (nx !== ny) return nx ? -1 : 1;
    return x < y ? -1 : 1;
  }
  return 0;
}

const normalizeVersion = (version: string | null | undefined): string | null => { const trimmed = version?.trim(); return trimmed ? trimmed : null; };
/** Core `major.minor.patch`, dropping any prerelease or build suffix. */
const versionCore = (version: string) => version.replace(/[-+].*$/, '');

/**
 * resolveVersionMismatch: the connected server runs an older T3 Code than this
 * client. Two nightlies compare their full versions; other pairs compare their
 * core only; a server ahead needs nothing; non-semver falls back to inequality.
 */
export function resolveVersionMismatch(serverVersion: string | null | undefined, clientVersion: string = CLIENT_VERSION): VersionMismatch | null {
  const client = normalizeVersion(clientVersion), server = normalizeVersion(serverVersion);
  if (!client || !server) return null;
  const clientCore = versionCore(client), serverCore = versionCore(server);
  const nightly = parseSemver(client)?.pre[0] === 'nightly' && parseSemver(server)?.pre[0] === 'nightly';
  const behind = parseSemver(clientCore) && parseSemver(serverCore)
    ? compareSemver(nightly ? server : serverCore, nightly ? client : clientCore) < 0 : server !== client;
  return behind ? { clientVersion: client, serverVersion: server, hint: MISMATCH_HINT } : null;
}
/** The two versions alone (Settings › Connections rows). */
export function versionMismatch(serverVersion: string, clientVersion = CLIENT_VERSION): { serverVersion: string; clientVersion: string } | null {
  const mismatch = resolveVersionMismatch(serverVersion, clientVersion);
  return mismatch ? { serverVersion: mismatch.serverVersion, clientVersion: mismatch.clientVersion } : null;
}

const environmentOf = (config: unknown) => obj(obj(config).environment);
const capabilitiesOf = (config: unknown) => obj(environmentOf(config).capabilities);

export function resolveServerConfigVersionMismatch(config: unknown, clientVersion: string = CLIENT_VERSION): VersionMismatch | null {
  return resolveVersionMismatch(str(environmentOf(config).serverVersion) || null, clientVersion);
}
/** The update path the server offers, or null when it only supports a manual relaunch. */
export function resolveServerSelfUpdateCapability(config: unknown): string | null {
  return str(capabilitiesOf(config).serverSelfUpdate) || null;
}
/** The desktop app supervising this server can be told to update itself over RPC. */
export function supportsDesktopAppUpdate(config: unknown): boolean { return capabilitiesOf(config).desktopAppUpdate === true; }
/** The server can recover opted-in running turns after its self-update restart. */
export function supportsServerUpdateThreadContinuation(config: unknown): boolean { return capabilitiesOf(config).serverUpdateThreadContinuation === true; }

export function serverUpdateGuidance(capability: string): string {
  return capability === 'desktop-managed' ? 'Update the desktop app' : 'Update to stay in sync';
}

export function buildVersionMismatchDismissalKey(environmentId: string, mismatch: Pick<VersionMismatch, 'clientVersion' | 'serverVersion'>): string {
  return `${environmentId}:${mismatch.clientVersion}:${mismatch.serverVersion}`;
}

/** Where the dismissed keys live: the client's preference record, or the module store in tests. */
export type DismissalStore = { versionMismatchDismissals: string[] };
const moduleStore: DismissalStore = { versionMismatchDismissals: [] };
const MAX_DISMISSALS = 200;

export function isVersionMismatchDismissed(dismissalKey: string | null | undefined, store: DismissalStore = moduleStore): boolean {
  return !!dismissalKey && store.versionMismatchDismissals.includes(dismissalKey);
}
export function dismissVersionMismatch(dismissalKey: string | null | undefined, store: DismissalStore = moduleStore): void {
  if (!dismissalKey || store.versionMismatchDismissals.includes(dismissalKey)) return;
  store.versionMismatchDismissals = [...store.versionMismatchDismissals, dismissalKey].slice(-MAX_DISMISSALS);
}

/** The failed-attempt view `isServerUpdateFailureDismissed` reads: a failed state carries its attempt. */
export type UpdateAttemptState = { readonly status: string; readonly attempt?: string };
// Runtime failures keep their identity until the next attempt: dismiss only that attempt.
const dismissedServerUpdateFailures = new Set<string>();
const attemptOf = (state: UpdateAttemptState) => state.attempt ?? '';
export function isServerUpdateFailureDismissed(state: UpdateAttemptState): boolean {
  return state.status === 'failed' && !!attemptOf(state) && dismissedServerUpdateFailures.has(attemptOf(state));
}
export function dismissServerUpdateFailure(state: UpdateAttemptState): void {
  if (state.status === 'failed' && attemptOf(state)) dismissedServerUpdateFailures.add(attemptOf(state));
}

export function appendVersionMismatchHint(message: string | null | undefined, mismatch: VersionMismatch | null | undefined): string | null {
  const normalized = normalizeVersion(message);
  if (!normalized) return mismatch?.hint ?? null;
  return mismatch ? `${normalized} Hint: ${mismatch.hint}` : normalized;
}
