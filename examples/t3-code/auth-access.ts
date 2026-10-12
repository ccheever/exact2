// Authorized clients of "This machine" (20261005-this-machine-network-access item 4), after T3 Code
// (MIT, see LICENSE-T3; reference 1e2ecbd975): packages/client-runtime/src/state/auth.ts
// (applyAuthAccessStreamEvent over `subscribeAuthAccess`), apps/web/src/environments/primary/auth.ts
// (the six `/api/auth/*` calls) and ConnectionsSettings.tsx (sorting, the summary, records).
//  - The stream: the primary's `subscribeAuthAccess` (a snapshot, then pairingLinkUpserted/Removed and
//    clientUpserted/Removed with a revision), subscribed while Settings › Connections is open, over
//    T3Client's transport while the primary is the focus and over its fleet transport otherwise
//    (the lifecycle stream's path, local-lifecycle.ts; keep-alive.ts dispatches both).
//  - The HTTP calls go to the embedded server with its in-memory bearer (`localAccess`,
//    T3Module+Local.swift): the desktop's own session holds every scope.
//  - A created credential is kept in memory by link id only (the server returns it once); it is
//    never logged or written to t3-code.json.
import { arr, num, obj, str, type Obj } from './domain';
import { bridgeReply, type Native } from './protocol';
import { focusedOnPrimary, primary } from './local-primary';
import { subscriptionSerial } from './shell-vcs';
import type { FleetEntry } from './settings-b-fleet';
import { letGo } from './let-go';

export type AuthPairingLink = { id: string; scopes: string[]; subject: string; label?: string; createdAt: string; expiresAt: string };
export type AuthClientMetadata = { label?: string; ipAddress?: string; userAgent?: string; deviceType: string; os?: string; browser?: string };
export type AuthClientSession = { sessionId: string; subject: string; scopes: string[]; method: string; client: AuthClientMetadata; issuedAt: string; expiresAt: string;
  lastConnectedAt: string | null; connected: boolean; current: boolean };
export type AuthAccessSnapshot = { pairingLinks: AuthPairingLink[]; clientSessions: AuthClientSession[] };
export type AuthAccessStreamEvent =
  | { version: 1; revision: number; type: 'snapshot'; payload: AuthAccessSnapshot }
  | { version: 1; revision: number; type: 'pairingLinkUpserted'; payload: AuthPairingLink }
  | { version: 1; revision: number; type: 'pairingLinkRemoved'; payload: { id: string } }
  | { version: 1; revision: number; type: 'clientUpserted'; payload: AuthClientSession }
  | { version: 1; revision: number; type: 'clientRemoved'; payload: { sessionId: string } };

export const EMPTY_AUTH_ACCESS_SNAPSHOT: AuthAccessSnapshot = { pairingLinks: [], clientSessions: [] };

function upsertByKey<A>(values: readonly A[], next: A, key: (value: A) => string): A[] {
  const nextKey = key(next);
  return [...values.filter(value => key(value) !== nextKey), next];
}

export function applyAuthAccessStreamEvent(current: AuthAccessSnapshot, event: AuthAccessStreamEvent): AuthAccessSnapshot {
  switch (event.type) {
    case 'snapshot': return event.payload;
    case 'pairingLinkUpserted': return { ...current, pairingLinks: upsertByKey(current.pairingLinks, event.payload, value => value.id) };
    case 'pairingLinkRemoved': return { ...current, pairingLinks: current.pairingLinks.filter(value => value.id !== event.payload.id) };
    case 'clientUpserted': return { ...current, clientSessions: upsertByKey(current.clientSessions, event.payload, value => value.sessionId) };
    case 'clientRemoved': return { ...current, clientSessions: current.clientSessions.filter(value => value.sessionId !== event.payload.sessionId) };
  }
}

// ── Decoding (DateTimeUtc travels as an ISO string) ───────────────────────
const strings = (value: unknown) => (Array.isArray(value) ? value : []).filter((item): item is string => typeof item === 'string');
const optional = (key: string, value: unknown) => (typeof value === 'string' && value.trim() ? { [key]: value } : {});
export function decodePairingLink(value: unknown): AuthPairingLink | null {
  const raw = obj(value);
  if (!str(raw.id) || !str(raw.expiresAt)) return null;
  return { id: str(raw.id), scopes: strings(raw.scopes), subject: str(raw.subject), ...optional('label', raw.label), createdAt: str(raw.createdAt), expiresAt: str(raw.expiresAt) };
}
export function decodeClientSession(value: unknown): AuthClientSession | null {
  const raw = obj(value), client = obj(raw.client);
  if (!str(raw.sessionId)) return null;
  return { sessionId: str(raw.sessionId), subject: str(raw.subject), scopes: strings(raw.scopes), method: str(raw.method),
    client: { deviceType: str(client.deviceType, 'unknown'), ...optional('label', client.label), ...optional('ipAddress', client.ipAddress), ...optional('userAgent', client.userAgent),
      ...optional('os', client.os), ...optional('browser', client.browser) },
    issuedAt: str(raw.issuedAt), expiresAt: str(raw.expiresAt), lastConnectedAt: typeof raw.lastConnectedAt === 'string' ? raw.lastConnectedAt : null,
    connected: raw.connected === true, current: raw.current === true };
}
const present = <A>(value: A | null): value is A => value !== null;
export function decodeAuthAccessEvent(value: unknown): AuthAccessStreamEvent | null {
  const raw = obj(value), payload = obj(raw.payload), revision = num(raw.revision);
  switch (raw.type) {
    case 'snapshot': return { version: 1, revision, type: 'snapshot', payload: { pairingLinks: arr(payload.pairingLinks).map(decodePairingLink).filter(present),
      clientSessions: arr(payload.clientSessions).map(decodeClientSession).filter(present) } };
    case 'pairingLinkUpserted': { const link = decodePairingLink(payload); return link ? { version: 1, revision, type: 'pairingLinkUpserted', payload: link } : null; }
    case 'pairingLinkRemoved': return str(payload.id) ? { version: 1, revision, type: 'pairingLinkRemoved', payload: { id: str(payload.id) } } : null;
    case 'clientUpserted': { const session = decodeClientSession(payload); return session ? { version: 1, revision, type: 'clientUpserted', payload: session } : null; }
    case 'clientRemoved': return str(payload.sessionId) ? { version: 1, revision, type: 'clientRemoved', payload: { sessionId: str(payload.sessionId) } } : null;
    default: return null;
  }
}

// ── ConnectionsSettings.tsx helpers ──────────────────────────────────────
const time = (iso: string) => new Date(iso).getTime();
export const sortPairingLinks = (links: readonly AuthPairingLink[]) => [...links].sort((left, right) => time(right.createdAt) - time(left.createdAt));
export const sortClientSessions = (sessions: readonly AuthClientSession[]) => [...sessions].sort((left, right) => {
  if (left.current !== right.current) return left.current ? -1 : 1;
  if (left.connected !== right.connected) return left.connected ? -1 : 1;
  return time(right.issuedAt) - time(left.issuedAt);
});
/** The Authorized clients fold's closed-header summary. */
export function summarizeAuthorizedClients(sessions: readonly unknown[], links: readonly unknown[]): string {
  return [`${sessions.length} ${sessions.length === 1 ? 'client' : 'clients'}`, links.length > 0 ? `${links.length} ${links.length === 1 ? 'pairing link' : 'pairing links'}` : null]
    .filter((part): part is string => part !== null).join(' · ');
}

// ── The stream ─────────────────────────────────────────────────────────────
export const AUTH_ACCESS_KEY = 'auth-access';
type Stream = { generation: number; id: string; tried: boolean };
const streams = new WeakMap<object, Stream>();
const streamOf = (owner: object, generation: number): Stream => {
  let value = streams.get(owner);
  if (!value || value.generation !== generation) streams.set(owner, value = { generation, id: '', tried: false });
  return value;
};

/** The primary's access state: the reduced snapshot, whether it arrived, and the stream's failure. */
export const access = {
  environmentId: '', snapshot: EMPTY_AUTH_ACCESS_SNAPSHOT, loaded: false, error: '', revision: 0,
  /** Settings › Connections is open and may manage this machine: keep the stream. */
  wanted: false,
  /** Redraws the page once an event lands (T3Client.revision). */
  changed: (): void => {},
};
function adopt(environmentId: string): void {
  if (access.environmentId === environmentId) return;
  access.environmentId = environmentId; access.snapshot = EMPTY_AUTH_ACCESS_SNAPSHOT; access.loaded = false; access.error = '';
}
function landed(): void { access.revision++; access.changed(); }

function handle(stream: Stream | undefined, environmentId: string, entry: Obj): boolean {
  if (str(entry.key) !== AUTH_ACCESS_KEY) return false;
  if (!stream || num(entry.generation, -1) !== stream.generation) return true;
  const id = str(entry.subscriptionId);
  if (stream.id && subscriptionSerial(id) < subscriptionSerial(stream.id)) return true;
  const item = obj(entry.value);
  if (item._retryDue || item._streamEnded || item._transportError) {
    stream.id = ''; stream.tried = false;
    if (item._transportError) { access.error = str(obj(item._transportError).message, 'The access stream ended.'); landed(); }
    return true;
  }
  const event = decodeAuthAccessEvent(entry.value);
  if (!event || environmentId !== primary.target?.environmentId) return true;
  adopt(environmentId);
  access.snapshot = applyAuthAccessStreamEvent(access.snapshot, event);
  access.loaded = true; access.error = '';
  landed();
  return true;
}

type Focused = { environmentId: string; origin: string; generation: number };
/** client.ts drain (through keep-alive.ts): the focused primary's access entries. */
export function accessEvent(client: Focused, entry: Obj): boolean { return handle(streams.get(client), client.environmentId, entry); }
/** settings-b-fleet.ts drain (through keep-alive.ts): the background primary's. */
export function accessFleetEvent(entry: FleetEntry, event: Obj): boolean { return handle(streams.get(entry), entry.environmentId, event); }

async function follow(stream: Stream, environmentId: string, call: (request: Obj) => Promise<Obj>): Promise<void> {
  if (!access.wanted) {
    if (!stream.id) return;
    const id = stream.id;
    stream.id = ''; stream.tried = false;
    await call({ op: 'unsubscribe', key: AUTH_ACCESS_KEY }).catch(() => undefined);
    if (id && access.environmentId === environmentId) access.loaded = false;
    return;
  }
  if (stream.id || stream.tried) return;
  adopt(environmentId);
  stream.tried = true;
  try { stream.id = str((await call({ op: 'subscribe', key: AUTH_ACCESS_KEY, method: 'subscribeAuthAccess', payload: {} })).id); }
  catch (error) {
    stream.tried = false;
    if (letGo(error)) throw error;
    access.error = error instanceof Error ? error.message : String(error);
  }
}
/** keep-alive.ts keepAlivePrepare (and the Connections page): subscribe the focused connection when it is the primary. */
export async function watchAccess(client: Focused, call: (request: Obj) => Promise<Obj>): Promise<void> {
  if (focusedOnPrimary(client) && client.environmentId) await follow(streamOf(client, client.generation), client.environmentId, call);
}
/** keep-alive.ts keepAliveFleetPass: subscribe the background primary. */
export async function accessFleetPass(call: (request: Obj) => Promise<Obj>, entry: FleetEntry): Promise<void> {
  if (entry.primary && entry.environmentId && entry.environmentId === primary.target?.environmentId) await follow(streamOf(entry, entry.generation), entry.environmentId, call);
}

// ── HTTP (environmentHttp.ts EnvironmentAuthHttpApi, through the embedded server's bearer) ──
export type AccessOperation = 'create-pairing-credential' | 'list-pairing-links' | 'revoke-pairing-link' | 'list-client-sessions' | 'revoke-client-session' | 'revoke-other-client-sessions';
/** PrimaryEnvironmentRequestError: the operation and the HTTP status, never the server's text. */
export class PrimaryEnvironmentRequestError extends Error {
  constructor(readonly operation: AccessOperation, readonly status: number) { super(`Primary environment request failed during ${operation} (HTTP ${status}).`); }
}
async function accessCall(native: Native, operation: AccessOperation, method: 'GET' | 'POST', path: string, body?: Obj): Promise<unknown> {
  const reply = await bridgeReply(native, { op: 'localAccess', method, path, ...(body ? { body } : {}) });
  // T3Module+Local.swift puts the HTTP status in `detail`; no answer at all counts as 500, as readHttpApiStatus.
  if (!reply.ok) throw new PrimaryEnvironmentRequestError(operation, Number(reply.error?.detail) || 500);
  return reply.value;
}
export type AuthPairingCredentialResult = { id: string; credential: string; label?: string; expiresAt: string };
/** createServerPairingCredential: a trimmed label when given, and the chosen scopes. */
export async function createPairingCredential(native: Native, input: { label?: string; scopes?: readonly string[] }): Promise<AuthPairingCredentialResult> {
  const label = input.label?.trim();
  const value = obj(await accessCall(native, 'create-pairing-credential', 'POST', '/api/auth/pairing-token', { ...(label ? { label } : {}), ...(input.scopes ? { scopes: [...input.scopes] } : {}) }));
  if (!str(value.id) || !str(value.credential)) throw new PrimaryEnvironmentRequestError('create-pairing-credential', 500);
  return { id: str(value.id), credential: str(value.credential), ...optional('label', value.label), expiresAt: str(value.expiresAt) };
}
export async function revokePairingLink(native: Native, id: string): Promise<void> { await accessCall(native, 'revoke-pairing-link', 'POST', '/api/auth/pairing-links/revoke', { id }); }
export async function revokeClientSession(native: Native, sessionId: string): Promise<void> { await accessCall(native, 'revoke-client-session', 'POST', '/api/auth/clients/revoke', { sessionId }); }
export async function revokeOtherClientSessions(native: Native): Promise<number> { return num(obj(await accessCall(native, 'revoke-other-client-sessions', 'POST', '/api/auth/clients/revoke-others')).revokedCount); }
export async function listPairingLinks(native: Native): Promise<AuthPairingLink[]> { return arr(await accessCall(native, 'list-pairing-links', 'GET', '/api/auth/pairing-links')).map(decodePairingLink).filter(present); }
export async function listClientSessions(native: Native): Promise<AuthClientSession[]> { return arr(await accessCall(native, 'list-client-sessions', 'GET', '/api/auth/clients')).map(decodeClientSession).filter(present); }

/** Credentials this client created, by link id (only its own creation response carries one). Memory only. */
export const createdCredentials = new Map<string, string>();
