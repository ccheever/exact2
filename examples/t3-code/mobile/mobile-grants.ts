// Pinned T3 Code365aa87982 packages/contracts/src/auth.ts:
// AuthStandardClientScopes, legacyParents and sessionGrantsScope.
// @ref llp/1107.003-pairing-and-transport.decision.md#mobile-adaptations
import { obj, type Obj } from './shared/domain';
import { encodeOAuthScope } from './shared/remote-scopes';

export const MOBILE_STANDARD_CLIENT_SCOPES = [
  'orchestration:read', 'orchestration:operate', 'settings:write', 'providers:manage',
  'environment:maintain', 'preview:operate', 'diagnostics:read', 'terminal:read',
  'terminal:operate', 'source-control:write', 'filesystem:read', 'filesystem:write', 'relay:read',
] as const;
const standardScope = encodeOAuthScope(MOBILE_STANDARD_CLIENT_SCOPES);
const legacyParents: Readonly<Record<string, string>> = {
  'filesystem:read': 'orchestration:read', 'diagnostics:read': 'orchestration:read',
  'settings:write': 'orchestration:operate', 'providers:manage': 'orchestration:operate',
  'environment:maintain': 'orchestration:operate', 'preview:operate': 'orchestration:operate',
  'source-control:write': 'orchestration:operate', 'filesystem:write': 'orchestration:operate',
  'terminal:read': 'terminal:operate',
};

/** Replace the desktop copy's older standard request only at credential exchange.
 * Saved-token reconnects do not exchange or expand their existing server grant. */
export function mobileGrantRequest<T>(request: T): T | Obj {
  const input = obj(request);
  return ['connect', 'pairEnvironment', 'fleetOutdatedPair'].includes(String(input.op))
    && typeof input.credential === 'string' && input.credential.length > 0
    ? { ...input, scope: standardScope } : request;
}

/** Explicit permissions, including [], are authoritative; only old servers use parents. */
export function mobileSessionGrants(session: Obj | null, permission: string): boolean {
  if (!session || session.authenticated !== true) return false;
  if (session.permissions !== undefined) return Array.isArray(session.permissions) && session.permissions.includes(permission);
  const scopes = Array.isArray(session.scopes) ? session.scopes : [];
  if (scopes.includes(permission)) return true;
  if (obj(session.auth).serverUpdateScope !== undefined) return false;
  const parent = legacyParents[permission];
  return parent !== undefined && scopes.includes(parent);
}
