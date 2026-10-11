// The primary environment, "This machine" (20261005-local-primary-environment items 1-3): the
// embedded server (local-backend.ts, T3LocalBackend.swift) as the environment this app runs on.
// After T3 Code (MIT, see LICENSE-T3; reference 1e2ecbd975): apps/web/src/environments/primary/
// target.ts (resolveDesktopPrimaryTarget, readPrimaryEnvironmentTarget), localEnvironment.ts,
// state/primaryEnvironment.ts, connection/platform.ts loadPrimaryConnectionRegistration (the label
// is the descriptor's), components/chat/folderDrop.ts, and ConnectionsSettings.tsx (the desktop
// session counts as AuthAdministrativeScopes; canManageLocalBackend).
//
// It connects through T3Transport (the focus) or T3Fleet (in the background) with `primary: true`:
// the bearer is the embedded server's, in memory, never Keychain; it is never saved, has no row
// under Environments, and leads Load balancing and GitHub sharing. Its port can change every
// launch, so its origin is never remembered: the transport keeps the focus token `primary` beside
// `t3.server.origin`. A saved entry with the primary's environment id (the same T3 home, paired
// before) is a duplicate: as the reference's `installPlatformRegistration` (registry.ts), its GitHub
// sharing trust is forgotten, then the entry and its credential are removed without a word, and a
// window focused on it moves to the primary (the reference replaces the entry in place). Pairing this
// machine's own server saves nothing (`register` is a no-op for a platform id), decision U6.
import { obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import { readLocalBackend, unknownLocalBackend, type LocalBackendStatus } from './local-backend';
import { routingKeyEnvironment } from './connection-routes';

/** PRIMARY_LOCAL_ENVIRONMENT_ID: the bootstrap's id and the fleet's name for the primary. */
export const PRIMARY_LOCAL_ENVIRONMENT_ID = 'primary';
/** AuthAdministrativeScopes: the desktop treats its own session as administrative. */
export const AUTH_ADMINISTRATIVE_SCOPES = ['orchestration:read', 'orchestration:operate', 'terminal:operate', 'review:write', 'relay:read',
  'access:read', 'access:write', 'relay:write'] as const;

/** DesktopEnvironmentBootstrapIncompleteError: one base URL without the other. */
export class DesktopEnvironmentBootstrapIncompleteError extends Error {
  constructor(readonly hasHttpBaseUrl: boolean, readonly hasWsBaseUrl: boolean) {
    super(`Desktop bootstrap is missing ${[...(hasHttpBaseUrl ? [] : ['httpBaseUrl']), ...(hasWsBaseUrl ? [] : ['wsBaseUrl'])].join(' and ')} for the local environment.`);
  }
}
/** PrimaryEnvironmentDisabledError. */
export class PrimaryEnvironmentDisabledError extends Error {
  constructor() { super('The local environment is disabled.'); }
}

export type PrimaryTarget = {
  id: typeof PRIMARY_LOCAL_ENVIRONMENT_ID; source: 'desktop-managed';
  httpBaseUrl: string; wsBaseUrl: string;
  /** From the server's descriptor; '' until it answered. */
  environmentId: string; label: string;
};

const trim = (origin: string) => origin.trim().replace(/\/+$/, '');
function normalizeBaseUrl(raw: string): string {
  try { return new URL(raw).toString(); } catch { throw new Error(`Could not parse the desktop-managed primary environment target ${raw}.`); }
}

/**
 * readPrimaryEnvironmentTarget for this desktop app: null when the local environment is switched
 * off (isLocalEnvironmentDisabled), or the backend has no bases (a refused development build, a
 * runtime still unpacking); one base without the other is an error (resolveDesktopPrimaryTarget).
 */
export function readPrimaryTarget(status: LocalBackendStatus, enabled: boolean): PrimaryTarget | null {
  if (!enabled || !status.enabled || status.state === 'refused') return null;
  if (!status.httpBaseUrl && !status.wsBaseUrl) return null;
  if (!status.httpBaseUrl || !status.wsBaseUrl) throw new DesktopEnvironmentBootstrapIncompleteError(!!status.httpBaseUrl, !!status.wsBaseUrl);
  return { id: PRIMARY_LOCAL_ENVIRONMENT_ID, source: 'desktop-managed', httpBaseUrl: normalizeBaseUrl(status.httpBaseUrl),
    wsBaseUrl: normalizeBaseUrl(status.wsBaseUrl), environmentId: status.environmentId, label: status.label };
}

/** resolvePrimaryEnvironmentHttpUrl: a path on the primary's HTTP base; none while it is switched off. */
export function resolvePrimaryEnvironmentHttpUrl(pathname: string, source: LocalPrimary = primary, searchParams?: Record<string, string>): string {
  if (!source.target) throw new PrimaryEnvironmentDisabledError();
  const url = new URL(source.target.httpBaseUrl);
  url.pathname = pathname;
  if (searchParams) url.search = new URLSearchParams(searchParams).toString();
  return url.toString();
}

export type PrimaryPhase = 'available' | 'connecting' | 'reconnecting' | 'connected' | 'error' | 'unsupported';
/**
 * The primary's connection phase, from the backend's state and its socket's: `installing` and
 * `starting` connect, `restarting` reconnects with the last exit, `failed` (and a missing runtime)
 * failed with the reason, `ready` follows the socket.
 */
export function primaryPhase(status: LocalBackendStatus, socket: { phase: PrimaryPhase; message: string }): { phase: PrimaryPhase; message: string } {
  if (status.state === 'installing' || status.state === 'starting') return { phase: 'connecting', message: '' };
  if (status.state === 'restarting') return { phase: 'reconnecting', message: status.lastExit ? `The local server exited (${status.lastExit}).` : '' };
  if (status.state === 'failed' || status.state === 'runtime-missing') return { phase: 'error', message: status.failure || status.install?.reason || '' };
  if (status.state === 'ready') return socket.phase === 'available' ? { phase: 'connecting', message: '' } : socket;
  return { phase: 'available', message: '' };
}

/** The app's one primary: what the backend reports and the switch this client keeps. */
export class LocalPrimary {
  status: LocalBackendStatus = unknownLocalBackend();
  /** The Local environment switch as saved (desktop-settings.json `localEnvironmentEnabled`, decision U7). */
  setting = true;
  target: PrimaryTarget | null = null;
  /** A bootstrap the primary cannot be built from (one base URL only). */
  problem = '';
  update(status: LocalBackendStatus, setting: boolean): void {
    this.status = status; this.setting = setting;
    try { this.target = readPrimaryTarget(status, setting); this.problem = ''; }
    catch (error) { this.target = null; this.problem = error instanceof Error ? error.message : String(error); }
  }
  /** Connected-able: ready, with its bearer and descriptor. */
  get connectable(): boolean { return !!this.target?.environmentId && this.status.state === 'ready' && this.status.bearerReady; }
  /** No primary will come without a change: switched off, refused, or no runtime. */
  get unavailable(): boolean {
    return !this.setting || !this.status.enabled || ['refused', 'runtime-missing', 'failed'].includes(this.status.state) || !!this.problem;
  }
  /** isLocalEnvironmentDisabled: the switch is off. */
  get disabled(): boolean { return !this.setting; }
}
export const primary = new LocalPrimary();

/** Whether this environment id is the primary's. */
export const isPrimaryEnvironment = (environmentId: string, source: LocalPrimary = primary) =>
  !!environmentId && !!source.target?.environmentId && source.target.environmentId === environmentId;
/** Whether this origin is the primary's address (the focus before its descriptor answered). */
export const isPrimaryOrigin = (origin: string, source: LocalPrimary = primary) =>
  !!source.target && !!origin && trim(origin) === trim(source.target.httpBaseUrl);
/** The focused connection is the primary. */
export const focusedOnPrimary = (client: { environmentId: string; origin: string }, source: LocalPrimary = primary) =>
  client.environmentId ? isPrimaryEnvironment(client.environmentId, source) : isPrimaryOrigin(client.origin, source);

/** A session's effective scopes: the desktop's own session is administrative (ConnectionsSettings.tsx). */
export const sessionScopes = (environmentId: string, scopes: string[], source: LocalPrimary = primary): string[] =>
  isPrimaryEnvironment(environmentId, source) ? [...AUTH_ADMINISTRATIVE_SCOPES] : scopes;
/**
 * canManageLocalBackend: the local environment is on and the session can write access, which the
 * desktop's own (administrative) session always can. A refused development build has no server to manage.
 */
export const canManageLocalBackend = (source: LocalPrimary = primary) =>
  !source.disabled && source.status.state !== 'refused' && (AUTH_ADMINISTRATIVE_SCOPES as readonly string[]).includes('access:write');

// ── The switch in desktop-settings.json (decision U7, 2026-10-08: the original app's file) ──
type Holder = { localBackend: LocalBackendStatus };
/** The saved switch (default on, as DesktopAppSettings), as the native side read or last wrote it. */
export function localEnvironmentEnabled(owner: Holder): boolean { return owner.localBackend.settings.localEnvironmentEnabled; }
/** client.ts refresh: read the embedded server's status into the client and the primary; true when it changed (it redraws "This machine"). */
export async function refreshLocal(client: Holder, native: Native, source: LocalPrimary = primary): Promise<boolean> {
  const status = await readLocalBackend(native);
  const changed = JSON.stringify(status) !== JSON.stringify(client.localBackend);
  if (changed) client.localBackend = status;
  source.update(client.localBackend, localEnvironmentEnabled(client));
  return changed;
}
/** The keys the clone kept in t3-code.json before decision U7; the native side carried them over once, so a load drops them. */
export const LEGACY_DESKTOP_KEYS = ['localEnvironmentEnabled', 'serverExposureMode', 'tailscaleServeEnabled', 'tailscaleServePort'] as const;
export const hasLegacyDesktopKeys = (saved: Obj) => LEGACY_DESKTOP_KEYS.some(key => saved[key] !== undefined);

/** folderDropTarget: a dropped Finder folder reaches only the primary, and only while it is on. */
export function folderDropTarget(input: { localEnvironmentDisabled: boolean; environmentId: string; primaryEnvironmentId: string | null }): 'local' | 'remote' {
  if (input.localEnvironmentDisabled || input.primaryEnvironmentId === null || input.environmentId !== input.primaryEnvironmentId) return 'remote';
  return 'local';
}

// ── The primary as a source (connections.ts environmentSources, fleet) ─────
/** The saved-list shape of the primary, for the fleet and the source lists. */
export function primaryEntry(source: LocalPrimary = primary): Obj | null {
  const target = source.target;
  if (!target?.environmentId) return null;
  return { origin: trim(target.httpBaseUrl), environmentId: target.environmentId, label: target.label, enabled: true, primary: true };
}

/**
 * githubRoutingPermissions.forget(environmentId): every stored sharing permission of that environment.
 * False when the preferences could not be written (the reference then leaves the saved entry alone).
 */
export async function forgetGitHubRouting(native: Native, environmentId: string): Promise<boolean> {
  const read = await bridgeReply(native, { op: 'connectionPreferences' }).catch(() => null);
  if (!read?.ok) return false;
  let raw: Obj;
  try { raw = obj(JSON.parse(str(obj(read.value).text, '{}') || '{}')); } catch { raw = {}; }
  const routing = obj(raw.githubRouting), kept = Object.fromEntries(Object.entries(routing).filter(([key]) => routingKeyEnvironment(key) !== environmentId));
  if (Object.keys(kept).length === Object.keys(routing).length) return true;
  const written = await bridgeReply(native, { op: 'setConnectionPreferences', text: JSON.stringify({ ...raw, githubRouting: kept }) }).catch(() => null);
  return written?.ok === true;
}

/**
 * Decision U6 (2026-10-08: as the reference, `installPlatformRegistration`): a saved environment with
 * the primary's id is the same machine paired before. Its GitHub sharing trust goes first (a failure
 * keeps the entry), then the entry and every route's Keychain item (`forgetEnvironment`, which leaves
 * the primary connected), without a word. `focusDropped`: the window was focused on it, so the
 * primary takes the focus (client.ts, r8-pointer-reconnect.ts `primaryTakesFocus`).
 */
export async function dropPrimaryDuplicates(native: Native, saved: Obj[], source: LocalPrimary = primary): Promise<{ origins: string[]; focusDropped: boolean }> {
  const id = source.target?.environmentId;
  if (!id) return { origins: [], focusDropped: false };
  const duplicates = saved.filter(entry => str(entry.environmentId) === id && entry.primary !== true);
  if (!duplicates.length || !(await forgetGitHubRouting(native, id))) return { origins: [], focusDropped: false };
  let focusDropped = false;
  for (const entry of duplicates) {
    await native.later({ op: 'fleetStop', fleet: `${trim(str(entry.origin))}\n${id}` }).catch(() => undefined);
    const reply = await bridgeReply(native, { op: 'forgetEnvironment', origin: str(entry.origin), environmentId: id }).catch(() => null);
    if (reply?.ok && obj(reply.value).forgotFocus === true) focusDropped = true;
  }
  return { origins: duplicates.map(entry => str(entry.origin)), focusDropped };
}

/** The saved list without the primary's duplicates (what every list reads). */
export const withoutPrimaryDuplicates = (saved: Obj[], source: LocalPrimary = primary): Obj[] => {
  const id = source.target?.environmentId;
  return id ? saved.filter(entry => str(entry.environmentId) !== id) : saved;
};
