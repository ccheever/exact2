// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// A connection view, never a second client or preference/outbox owner.
import type { T3Client } from './shared/client';
import { fleet, environmentKey } from './shared/settings-b-fleet';
import { obj, str, type Obj } from './shared/domain';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileQueuedEditOrigin } from './queued-edit-origin';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';

export type MobileOutboxConnection = Pick<T3Client, 'origin' | 'environmentId' | 'generation' | 'connection'
  | 'config' | 'configLive' | 'shell' | 'shellLive' | 'scopes' | 'call' | 'request' | 'http'>;
export function mobileOutboxBackgroundSaved(owner: Pick<MobileOutboxWireOwner, 'origin' | 'environmentId'>): boolean {
  return fleet.saved.some(saved => saved.enabled !== false && saved.environmentId === owner.environmentId
    && saved.origin === owner.origin);
}
/** Local draft restoration uses saved home ownership, never a connection focus. */
export function mobileOutboxHomeAvailable(client: T3Client, owner: Pick<MobileOutboxWireOwner, 'origin' | 'environmentId'>): boolean {
  return client.environmentId === owner.environmentId ? mobileQueuedEditOrigin(client) === owner.origin : mobileOutboxBackgroundSaved(owner);
}
/** Local journals, attachments and IDs stay on the global module. Only wire
 * operations borrow the captured fleet transport. Native rechecks its home. */
export async function mobileOutboxConnection(client: T3Client, native: Native,
  owner: MobileOutboxWireOwner): Promise<MobileOutboxConnection> {
  if (client.environmentId === owner.environmentId) return client;
  const key = environmentKey(owner.origin, owner.environmentId), entry = fleet.entries.get(key);
  const generation = entry?.generation ?? -1;
  const valid = () => !!entry && fleet.entries.get(key) === entry && mobileOutboxBackgroundSaved(owner)
    && client.environmentId !== owner.environmentId && entry.environmentId === owner.environmentId
    && entry.origin === owner.origin && entry.generation === generation && entry.phase === 'connected';
  const stale = (write = false) => new ClientError('The pending task connection changed. Wait for its saved environment.', 'stale', write);
  async function call(handle: Native, request: unknown, expected = generation, write = false): Promise<Obj> {
    if (!valid() || expected !== generation) throw stale();
    const value = obj(request), op = str(value.op);
    const remote = ['status', 'http', 'request', 'uploadAttachment'].includes(op)
      || op === 'mobileOutboxDelivery' && ['reserve', 'reserveInline', 'send'].includes(str(value.action))
      || op === 'mobileOutboxInline' && ['reserve', 'send'].includes(str(value.action));
    let reply;
    try { reply = await bridgeReply(handle, { ...value, generation, ...(remote ? { fleet: key } : {}) }); }
    catch (error) {
      if (letGo(error) || error instanceof ClientError) throw error;
      throw new ClientError(error instanceof Error ? error.message : String(error), 'transport', write);
    }
    if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind, reply.error!.uncertain, reply.error!);
    if (!valid() || reply.generation !== generation) throw stale(write);
    return obj(reply.value);
  }
  const status = await call(native, { op: 'status' });
  const origin = str(status.origin);
  if (!origin || status.state !== 'connected' || status.environmentId !== owner.environmentId
    || (str(status.homeOrigin) || origin) !== owner.origin) throw stale();
  return {
    origin, environmentId: owner.environmentId, generation,
    get connection() { return valid() ? 'connected' : 'disconnected'; },
    get config() { return entry!.config; }, get scopes() { return entry!.scopes; },
    get shell() { return entry!.shell; },
    get configLive() { return valid() && entry!.synchronized === generation; },
    get shellLive() { return valid() && entry!.synchronized === generation; },
    call,
    http: (handle, path, expected) => call(handle, { op: 'http', path }, expected),
    request: (handle, method, payload, expected, write) => call(handle, { op: 'request', method, payload }, expected, write),
  };
}
