// @ref llp/1106.010-mobile-browser-devices.decision.md#connection-and-command-ownership
// Pinned mobile preview route identity; stream credentials remain in native code.
import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { letGoAware } from './shared/let-go';
import { mobileSessionGrants } from './environment-detail';

export function mobileStreamOwner(client: T3Client): string {
  return client.ready && client.threadId ? JSON.stringify([client.origin, client.environmentId, client.threadId, client.generation]) : '';
}
export function assertMobileStreamOwner(client: T3Client, owner: string) {
  if (!owner || owner !== mobileStreamOwner(client)) throw new ClientError('The connection or thread changed. Open this preview again.', 'superseded');
}
export function mobileStreamNative(client: T3Client, owner: string, input: Native, current: () => boolean = () => true): Native {
  const native = letGoAware(input);
  return { available: native.available, watch: topic => native.watch(topic), later: async request => {
    if (!current()) throw new ClientError('A newer preview read replaced this one.', 'superseded');
    assertMobileStreamOwner(client, owner);
    const result = await native.later(request);
    if (!current()) throw new ClientError('A newer preview read replaced this one.', 'superseded');
    assertMobileStreamOwner(client, owner);
    return result;
  } };
}
export async function mobileStreamPermission(client: T3Client, native: Native, scope: string) {
  const session = await client.http(native, '/api/auth/session');
  if (!mobileSessionGrants(session, 'orchestration:read')) throw new ClientError('This connection cannot view previews.', 'permission');
  return mobileSessionGrants(session, scope);
}
export function mobileStreamDescriptor(client: T3Client, owner: string) {
  assertMobileStreamOwner(client, owner);
  return { owner, origin: client.origin, environmentId: client.environmentId, threadId: client.threadId, generation: client.generation };
}
