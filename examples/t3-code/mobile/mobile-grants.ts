// Pinned T3 Code365aa87982 packages/contracts/src/auth.ts:
// AuthStandardClientScopes, legacyParents and sessionGrantsScope.
// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
import { obj, type Obj } from './shared/domain';
import { encodeOAuthScope } from './shared/remote-scopes';
import { bridgeReply, ClientError, reply, type Native } from './shared/protocol';
import { letGo, letGoAware } from './shared/let-go';

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

/** The compatibility scopes field is a projection of this response's actual grant. */
function sessionReply(response: unknown): unknown {
  const envelope = obj(response), session = obj(envelope.value);
  if (envelope.ok !== true || session.authenticated !== true) return response;
  const source = session.permissions !== undefined ? session.permissions : session.scopes;
  const candidates = Array.isArray(source) ? source.filter((value): value is string => typeof value === 'string') : [];
  if (session.permissions === undefined) candidates.push(...Object.keys(legacyParents));
  return { ...envelope, value: { ...session, scopes: [...new Set(candidates)].filter(scope => mobileSessionGrants(session, scope)) } };
}

export interface MobileSessionTarget {
  subject: { scopes: string[]; phase?: string };
  identity: readonly unknown[];
  current(): boolean;
  changed(): void;
  fleet?: boolean;
}
interface Authority {
  target: MobileSessionTarget;
  serial: number;
  epoch: number;
  accepted: boolean;
  scopes: string[];
}
interface Read { authority: Authority | null; serial: number; epoch: number; composition: Composition }
// Each composed call keeps weak result identities until its outer wrappers finish.
// Independent calls cannot replace one another's evidence for a reused result.
interface Composition { receipts: WeakMap<object, Read> }
const empty = Object.freeze([]) as unknown as string[];
const authorities = new WeakMap<object, Authority>();
// Only the synchronous native invocation composes wrappers. Clear this before
// awaiting so concurrent reads, even with reused inputs/results, are independent.
let invoking: Read | null = null;
const object = (value: unknown): object | null => value !== null && (typeof value === 'object' || typeof value === 'function') ? value : null;
const stale = () => new ClientError('The connection changed. Refresh before continuing.', 'stale');
const superseded = () => new ClientError('This session read was superseded.', 'superseded');

function retire(authority: Authority): void {
  const changed = authority.accepted || authority.scopes.length > 0;
  authority.epoch++; authority.serial++; authority.accepted = false; authority.scopes = empty;
  if (changed) authority.target.changed();
}
export function mobileSessionRetire(subject: object): void {
  const authority = authorities.get(subject);
  if (authority) retire(authority);
}
function bind(target: MobileSessionTarget): Authority {
  let authority = authorities.get(target.subject);
  if (authority) {
    if (!authority.target.current() || authority.target.identity.some((value, index) => value !== target.identity[index])) retire(authority);
    authority.target = target;
    return authority;
  }
  authority = { target, serial: 0, epoch: 0, accepted: false, scopes: empty };
  const owned = authority;
  authorities.set(target.subject, owned);
  // The first real session read takes ownership before the copied synchronizer
  // can assign its returned scopes. Those later assignments are projections only.
  Object.defineProperty(target.subject, 'scopes', { configurable: true, enumerable: true,
    get: () => {
      if (!owned.target.current()) { retire(owned); return empty; }
      return owned.accepted ? owned.scopes : empty;
    }, set: (_value: string[]) => {},
  });
  if (target.fleet) {
    let phase = target.subject.phase;
    Object.defineProperty(target.subject, 'phase', { configurable: true, enumerable: true,
      get: () => phase, set: (value: string) => { phase = value; if (value !== 'connected') retire(owned); },
    });
  }
  return owned;
}

/** One invocation owns decoding; one target record owns accepted session grants.
 * Cached success survives a pending read. Failure, replacement and narrower success
 * replace it atomically, before a direct caller or copied bootstrap can continue. */
export async function mobileSessionRead(nativeInput: Native, request: unknown, target: MobileSessionTarget | null): Promise<unknown> {
  const input = obj(request), fleet = input.fleet, expected = input.generation;
  const native = letGoAware(nativeInput), authority = target ? bind(target) : null;
  const serial = authority ? ++authority.serial : 0;
  const read: Read = { authority, serial, epoch: authority?.epoch ?? 0,
    composition: authority && invoking?.authority === authority ? invoking.composition : { receipts: new WeakMap() } };
  const obsolete = () => !authority ? null
    : read.epoch !== authority.epoch || !target!.current() ? stale()
    : serial !== authority.serial ? superseded() : null;
  const current = () => authority !== null && read.epoch === authority.epoch && serial === authority.serial && target!.current();
  const remembered = (value: unknown) => { const key = object(value); return key ? read.composition.receipts.get(key) : undefined; };
  const remember = (value: unknown) => { const key = object(value); if (key && authority) read.composition.receipts.set(key, read); };
  const nested = (value: unknown): boolean => {
    const receipt = remembered(value);
    if (!receipt || !authority || receipt.authority !== authority || receipt.composition !== read.composition || receipt.serial <= serial) return false;
    if (read.epoch !== authority.epoch || !target!.current()) throw stale();
    if (receipt.serial !== authority.serial) throw superseded();
    return true;
  };
  const publish = (success: boolean, scopes = empty) => {
    const invalid = obsolete();
    if (invalid) throw invalid;
    authority!.accepted = success;
    authority!.scopes = success ? Object.freeze(scopes) as unknown as string[] : empty;
    target!.changed();
  };
  try {
    const previous = invoking;
    let pending: Promise<unknown>;
    try { invoking = read; pending = native.later(request); }
    finally { invoking = previous; }
    const first = await pending;
    if (nested(first)) return first;
    const envelope = reply(first);
    // Shared call reports an error before checking its generation.
    if (typeof expected === 'number' && envelope.generation !== expected) throw stale();
    const transfer = obj(obj(envelope.value)._nativeTransfer);
    const decoded = envelope.ok && typeof transfer.id === 'string' && transfer.id
      ? { ...obj(first), ...(await bridgeReply({ available: native.available, watch: topic => native.watch(topic),
          later: value => value === request ? Promise.resolve(first) : native.later(fleet === undefined ? value : { ...obj(value), fleet }) }, request)) }
      : first;
    const result = sessionReply(decoded), response = obj(result), session = obj(response.value);
    const scopes = Array.isArray(session.scopes) ? session.scopes.filter((scope): scope is string => typeof scope === 'string') : empty;
    if (authority) publish(response.ok === true && session.authenticated === true, scopes);
    remember(result);
    return result;
  } catch (error) {
    if (nested(error)) throw error;
    if (!letGo(error)) {
      const invalid = obsolete();
      if (invalid) { remember(invalid); throw invalid; }
      if (current() && !(error instanceof ClientError && error.kind === 'stale')) publish(false);
    }
    remember(error);
    throw error;
  }
}
