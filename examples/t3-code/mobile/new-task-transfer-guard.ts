// Captured enqueue ownership outlives a foreground route or confirmation dialog.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { T3Client } from './shared/client';
import { ClientError, type Native } from './shared/protocol';
import { mobileOutboxRead, mobileOutboxSnapshot, mobileOutboxTransferLookup } from './mobile-outbox';

export interface NewTaskTransferLease { key: string; serial: number }
const leases = new WeakMap<T3Client, Map<string, { serial: number; checked: boolean }>>();
let sequence = 0;
const valid = (key: string) => /^new-task:[\w-]{1,128}$/.test(key);
const recovery = () => new ClientError("Resolve this draft's captured task transfer before moving or discarding it.");
export function mobileNewTaskTransferGuardBusy(client: T3Client, key: string): boolean { return leases.get(client)?.has(key) ?? false; }
/** Shared by transfer preparation and destructive callers; stores only plain numbers/booleans. */
export function mobileNewTaskTransferGuardAcquire(client: T3Client, key: string): NewTaskTransferLease | null {
  if (!valid(key) || mobileNewTaskTransferGuardBusy(client, key)) return null;
  let held = leases.get(client); if (!held) { held = new Map(); leases.set(client, held); }
  const serial = ++sequence; held.set(key, { serial, checked: false }); return { key, serial };
}
export function mobileNewTaskTransferGuardCurrent(client: T3Client, lease: NewTaskTransferLease): boolean {
  return leases.get(client)?.get(lease.key)?.serial === lease.serial;
}
export function mobileNewTaskTransferGuardRelease(client: T3Client, lease: NewTaskTransferLease): void {
  if (mobileNewTaskTransferGuardCurrent(client, lease)) leases.get(client)!.delete(lease.key);
}
/** Destructive use requires a fresh complete lookup, not absence in a cached partial inventory. */
export async function mobileNewTaskTransferGuardRead(client: T3Client, native: Native, lease: NewTaskTransferLease,
  current: () => boolean = () => true): Promise<void> {
  if (!mobileNewTaskTransferGuardCurrent(client, lease) || !current()) throw new ClientError('The draft operation changed.', 'superseded');
  const stamp = JSON.stringify([client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch]);
  const held = leases.get(client)!.get(lease.key)!; held.checked = false;
  const assertCurrent = () => {
    if (!mobileNewTaskTransferGuardCurrent(client, lease) || !current()
      || stamp !== JSON.stringify([client.origin, client.generation, client.environmentId, client.projectId, client.threadId, client.threadEpoch]))
      throw new ClientError('The draft operation changed.', 'superseded');
  };
  const complete = await mobileOutboxRead(client, native); assertCurrent();
  if (!complete) throw new ClientError('Wait for complete local task ownership before moving or discarding this draft.');
  const reply = await mobileOutboxTransferLookup(client, native, lease.key);
  assertCurrent();
  if (!reply.complete) throw new ClientError('Wait for complete local task ownership before moving or discarding this draft.');
  if (reply.claims.some(claim => !['completed', 'released'].includes(claim.state))) throw recovery();
  held.checked = true; mobileNewTaskTransferGuardAssert(client, lease);
}
/** Immediately before mutation; also catches a newly adopted active claim during an alert/await. */
export function mobileNewTaskTransferGuardAssert(client: T3Client, lease: NewTaskTransferLease): void {
  if (!mobileNewTaskTransferGuardCurrent(client, lease) || !leases.get(client)!.get(lease.key)!.checked)
    throw new ClientError("Recheck this draft's local task ownership before changing it.", 'superseded');
  const snapshot = mobileOutboxSnapshot(client);
  if (!snapshot.complete) throw new ClientError('Wait for complete local task ownership before changing this draft.');
  if (snapshot.transfers.some(claim => claim.draftKey === lease.key && !['completed', 'released'].includes(claim.state))) throw recovery();
}
