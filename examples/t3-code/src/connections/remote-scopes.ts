// The scopes this client asks for when it exchanges a pairing credential, and what a session's
// answer grants (T3 Code MIT, see LICENSE-T3; reference 9c161462c9: packages/contracts/src/auth.ts
// AuthStandardClientScopes, sessionGrantsScope, sessionHasLegacyPermissions,
// packages/shared/src/oauthScope.ts encodeOAuthScope, apps/web/src/connection/platform.ts
// ClientPresentation `scopes`, packages/client-runtime/src/authorization/remote.ts).
// The constants live here only: the transport sends the `scope` it is given and names none.
// Port change: a server older than the permission split refuses the new scope names with
// `invalid_scope` before it spends the credential (its parseAllowedOAuthScope allows only the
// legacy names), so the request also carries the legacy list for the transport to retry with.
import { ClientError } from '../client/protocol';

/** AuthStandardClientScopes, in the reference's order. */
export const AUTH_STANDARD_CLIENT_SCOPES = [
  'orchestration:read',
  'orchestration:operate',
  'settings:write',
  'providers:manage',
  'environment:maintain',
  'preview:operate',
  'diagnostics:read',
  'terminal:read',
  'terminal:operate',
  'source-control:write',
  'filesystem:read',
  'filesystem:write',
  'relay:read',
  'access:read',
  'access:write',
  'relay:write',
] as const;

/** What servers before the permission split grant a client by default (the 1e2ecbd975 standard scopes). */
export const LEGACY_CLIENT_SCOPES = ['orchestration:read', 'orchestration:operate', 'terminal:operate', 'review:write', 'relay:read'] as const;

/** The scopes an old server knows; a new server's `scopes` field carries only these (authScopeResponse). */
const LEGACY_SCOPES = new Set<string>(['orchestration:read', 'orchestration:operate', 'terminal:operate', 'review:write', 'access:read', 'access:write', 'relay:read', 'relay:write']);

/** legacyParents: the old scope that stood for each new permission, read only for an old server's session. */
const LEGACY_PARENTS: Record<string, string> = {
  'filesystem:read': 'orchestration:read',
  'diagnostics:read': 'orchestration:read',
  'settings:write': 'orchestration:operate',
  'providers:manage': 'orchestration:operate',
  'environment:maintain': 'orchestration:operate',
  'preview:operate': 'orchestration:operate',
  'source-control:write': 'orchestration:operate',
  'filesystem:write': 'orchestration:operate',
  'terminal:read': 'terminal:operate',
};

const OAUTH_SCOPE_TOKEN = /^[\u0021\u0023-\u005b\u005d-\u007e]+$/u;

/** encodeOAuthScope: RFC 6749 `scope`, space-separated; refuses an empty, malformed or repeated scope. */
export function encodeOAuthScope(scopes: readonly string[]): string {
  const seen = new Set<string>();
  const duplicated = scopes.some(scope => { const repeat = seen.has(scope); seen.add(scope); return repeat; });
  if (scopes.length === 0 || duplicated || scopes.some(scope => !OAUTH_SCOPE_TOKEN.test(scope))) {
    throw new ClientError('OAuth scopes must be non-empty, syntactically valid, and unique.');
  }
  return scopes.join(' ');
}

/**
 * A pairing request for the transport: a request that carries a credential asks for the
 * standard scopes, whatever the link grants (the reference never retries with fewer), and names
 * the legacy list as `legacyScope` for a server that refuses the new names.
 * A request without a credential reuses a saved token and exchanges nothing.
 */
export function withStandardScope<T extends { credential: string }>(target: T): T & { scope?: string; legacyScope?: string } {
  return target.credential
    ? { ...target, scope: encodeOAuthScope(AUTH_STANDARD_CLIENT_SCOPES), legacyScope: encodeOAuthScope(LEGACY_CLIENT_SCOPES) }
    : target;
}

const strings = (value: unknown): string[] | undefined =>
  Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === 'string') : undefined;

/**
 * Every permission a `/api/auth/session` answer grants, so that `includes(permission)` is
 * sessionGrantsScope: a new server's `permissions` as they are (an empty list grants nothing); an
 * old server's `scopes` with each new permission whose legacy parent it holds, unless the server
 * already advertises `serverUpdateScope` (then it never expands grants).
 */
export function sessionPermissions(session: Record<string, unknown>): string[] {
  if (session.authenticated !== true) return [];
  const permissions = strings(session.permissions);
  if (permissions) return permissions;
  const scopes = strings(session.scopes) ?? [];
  const auth = session.auth && typeof session.auth === 'object' ? session.auth as Record<string, unknown> : {};
  if (auth.serverUpdateScope !== undefined) return scopes;
  const expanded = Object.entries(LEGACY_PARENTS).filter(([child, parent]) => !scopes.includes(child) && scopes.includes(parent)).map(([child]) => child);
  return [...scopes, ...expanded];
}

/** sessionHasLegacyPermissions: a new server kept this session's pre-split grant (only old scope names, at least one parent). */
export function sessionHasLegacyPermissions(session: Record<string, unknown>): boolean {
  const permissions = strings(session.permissions);
  return session.authenticated === true && permissions !== undefined && permissions.every(scope => LEGACY_SCOPES.has(scope))
    && Object.values(LEGACY_PARENTS).some(parent => permissions.includes(parent));
}
