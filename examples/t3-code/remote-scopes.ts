// The scopes this client asks for when it exchanges a pairing credential (T3 Code MIT, see
// LICENSE-T3; reference 1e2ecbd975: packages/contracts/src/auth.ts AuthStandardClientScopes,
// packages/shared/src/oauthScope.ts encodeOAuthScope, apps/web/src/connection/platform.ts
// ClientPresentation `scopes`, packages/client-runtime/src/authorization/remote.ts).
// The constant lives here only: the Swift transport sends the `scope` it is given and names none.
import { ClientError } from './protocol';

/** AuthStandardClientScopes, in the reference's order. */
export const AUTH_STANDARD_CLIENT_SCOPES = [
  'orchestration:read',
  'orchestration:operate',
  'terminal:operate',
  'review:write',
  'relay:read',
] as const;

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
 * standard scopes, whatever the link grants (the reference never retries with fewer).
 * A request without a credential reuses a saved token and exchanges nothing.
 */
export function withStandardScope<T extends { credential: string }>(target: T): T & { scope?: string } {
  return target.credential ? { ...target, scope: encodeOAuthScope(AUTH_STANDARD_CLIENT_SCOPES) } : target;
}
