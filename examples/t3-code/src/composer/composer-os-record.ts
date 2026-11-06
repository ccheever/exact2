import type { T3Client } from '../client/client';
import { composerFieldOwner } from './auto-balance-owner';
import { composerFieldCurrent } from './composer-field-bridge';

export type ComposerOSRecord = { id: number; snapshotOwner: string; fieldOwner: string; value: string; start: number; end: number; expect: string };
const claimed = new WeakMap<object, { generation: number; snapshotOwner: string; after: number }>();
export function composerOSRecord(client: T3Client, value: unknown): ComposerOSRecord | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  if (!('id' in value) || typeof value.id !== 'number' || !Number.isSafeInteger(value.id) || value.id <= 0
    || !('snapshotOwner' in value) || typeof value.snapshotOwner !== 'string' || value.snapshotOwner !== client.snapshotOwner
    || !('fieldOwner' in value) || typeof value.fieldOwner !== 'string' || value.fieldOwner !== composerFieldOwner(client)
    || !('value' in value) || typeof value.value !== 'string' || !('expect' in value) || typeof value.expect !== 'string'
    || !('start' in value) || typeof value.start !== 'number' || !Number.isSafeInteger(value.start)
    || !('end' in value) || typeof value.end !== 'number' || !Number.isSafeInteger(value.end)
    || value.start < 0 || value.end < value.start || value.end > value.value.length || value.value.slice(value.start, value.end) !== value.expect) return null;
  const current = composerFieldCurrent(client, value.fieldOwner);
  if (!current || current.composing || current.value !== value.value) return null;
  return { id: value.id, snapshotOwner: value.snapshotOwner, fieldOwner: value.fieldOwner, value: value.value, start: value.start, end: value.end, expect: value.expect };
}

/** Native IDs increase across folds and drops. Re-asked resources cannot
 *  consume a record twice, even if undo later restores its original source. */
export function claimComposerOSRecord(client: T3Client, value: unknown): { kind: 'accepted'; record: ComposerOSRecord } | { kind: 'duplicate' } | { kind: 'refused'; notify: boolean } {
  if (!value || typeof value !== 'object' || !('id' in value) || typeof value.id !== 'number' || !Number.isSafeInteger(value.id) || value.id <= 0
    || !('snapshotOwner' in value) || value.snapshotOwner !== client.snapshotOwner || !('fieldOwner' in value) || value.fieldOwner !== composerFieldOwner(client)) return { kind: 'refused', notify: false };
  const prior = claimed.get(client);
  if (prior?.generation === client.generation && prior.snapshotOwner === client.snapshotOwner && prior.after >= value.id) return { kind: 'duplicate' };
  claimed.set(client, { generation: client.generation, snapshotOwner: client.snapshotOwner, after: value.id });
  const record = composerOSRecord(client, value);
  return record ? { kind: 'accepted', record } : { kind: 'refused', notify: true };
}
