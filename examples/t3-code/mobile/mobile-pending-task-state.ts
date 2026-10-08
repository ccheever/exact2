// Pinned365aa87982 new-task-flow-provider and pending-task-editor-writes.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { T3Client } from './shared/client';
import type { MobileOutboxWireOwner } from './mobile-outbox-wire';
import { mobileOutboxDecode, type MobileOutboxRecord } from './mobile-outbox-model';

type Client = Pick<T3Client, 'local' | 'revision'>;
type ObjectValue = Record<string, unknown>;
export interface MobilePendingTaskMarker {
  version: 1; owner: MobileOutboxWireOwner; session: string; revision: number;
  draftKey: string; contentRevision: number;
  baseline: { record: MobileOutboxRecord; token: string; revision: number };
  pending: { mutationId: string; contentRevision: number } | null;
}
export type MobilePendingTaskExpected = Pick<MobilePendingTaskMarker, 'owner' | 'session' | 'revision'>;
export interface MobilePendingTaskEditorsSnapshot {
  hydrated: boolean; ready: boolean; blocked: boolean; markers: MobilePendingTaskMarker[]; errors: string[];
}
const field = 'mobilePendingTaskEditors';
const hydrated = new WeakSet<object>();
const object = (value: unknown): value is ObjectValue => !!value && typeof value === 'object' && !Array.isArray(value);
const text = (value: unknown): value is string => typeof value === 'string' && value.length > 0 && value.trim() === value;
const integer = (value: unknown, minimum = 0): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= minimum;
const clone = <T>(value: T): T => value === undefined ? value : JSON.parse(JSON.stringify(value));
const raw = (client: Client): unknown => (client.local as unknown as ObjectValue)[field];
const ownerFields = ['origin', 'environmentId', 'threadId', 'messageId', 'commandId'];
function fields(value: ObjectValue, names: readonly string[]): boolean {
  return Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name));
}
export function mobilePendingTaskEditorKey(owner: MobileOutboxWireOwner): string {
  return JSON.stringify({ origin: owner.origin, environmentId: owner.environmentId, threadId: owner.threadId,
    messageId: owner.messageId, commandId: owner.commandId });
}
function valid(value: unknown): value is MobilePendingTaskMarker {
  if (!object(value) || !fields(value, ['version', 'owner', 'session', 'revision', 'draftKey', 'contentRevision', 'baseline', 'pending'])
    || value.version !== 1 || !object(value.owner) || !fields(value.owner, ownerFields)
    || !ownerFields.every(key => text((value.owner as ObjectValue)[key])) || !text(value.session) || !integer(value.revision, 1)
    || !integer(value.contentRevision) || value.draftKey !== `new-task:pending-${value.owner.messageId}`
    || typeof value.draftKey !== 'string' || !/^new-task:[\w-]{1,128}$/.test(value.draftKey)
    || !object(value.baseline) || !fields(value.baseline, ['record', 'token', 'revision'])
    || !text(value.baseline.token) || !integer(value.baseline.revision)) return false;
  const decoded = mobileOutboxDecode(value.baseline.record);
  if (!decoded.ok || !decoded.record.creation || ownerFields.some(key => decoded.record[key as keyof MobileOutboxRecord] !== (value.owner as ObjectValue)[key])) return false;
  return value.pending === null || object(value.pending) && fields(value.pending, ['mutationId', 'contentRevision'])
    && text(value.pending.mutationId) && integer(value.pending.contentRevision) && value.pending.contentRevision <= value.contentRevision;
}
function inspect(client: Client): { markers: MobilePendingTaskMarker[]; errors: string[] } {
  const value = raw(client), markers: MobilePendingTaskMarker[] = [], errors: string[] = [];
  if (!object(value) || !fields(value, ['version', 'markers']) || value.version !== 1 || !object(value.markers))
    return { markers, errors: ['Pending editor ownership is unreadable. Keep saved tasks until it is recovered.'] };
  const messages = new Set<string>();
  for (const [key, marker] of Object.entries(value.markers)) {
    if (!valid(marker) || key !== mobilePendingTaskEditorKey(marker.owner)) { errors.push(`Invalid pending editor marker: ${key}`); continue; }
    if (messages.has(marker.owner.messageId)) errors.push(`Conflicting pending editor message: ${marker.owner.messageId}`);
    messages.add(marker.owner.messageId); markers.push(clone(marker));
  }
  return { markers, errors };
}
/** Called only after the root has read a valid preference document ({} for a
 * confirmed missing/empty file). Shared load replaces local; readiness follows it.
 * Unknown/malformed marker data is copied intact and blocks every mutation. */
export function mobilePendingTaskEditorsHydrate(client: Client, saved: ObjectValue): void {
  if (hydrated.has(client.local)) return;
  const value = Object.hasOwn(saved, field) ? saved[field] : { version: 1, markers: {} };
  Object.assign(client.local, { [field]: clone(value) }); hydrated.add(client.local); client.revision++;
}
export function mobilePendingTaskEditorsPersisted(client: Client): unknown { return clone(raw(client)); }
export function mobilePendingTaskEditorsSnapshot(client: Client): MobilePendingTaskEditorsSnapshot {
  const loaded = hydrated.has(client.local), result = loaded ? inspect(client) : { markers: [], errors: ['Read saved pending editors before delivery.'] };
  const ready = loaded && result.errors.length === 0;
  return { hydrated: loaded, ready, blocked: !ready || result.markers.length > 0, ...result };
}
/** Ready means the document is understood; active markers still hold their rows. */
export function mobilePendingTaskEditorsReady(client: Client): boolean { return mobilePendingTaskEditorsSnapshot(client).ready; }
function write(client: Client, markers: Record<string, MobilePendingTaskMarker>): void {
  Object.assign(client.local, { [field]: { version: 1, markers: clone(markers) } }); client.revision++;
}
function current(client: Client): Record<string, MobilePendingTaskMarker> | null {
  const snapshot = mobilePendingTaskEditorsSnapshot(client);
  return snapshot.ready ? Object.fromEntries(snapshot.markers.map(marker => [mobilePendingTaskEditorKey(marker.owner), marker])) : null;
}
/** The caller must durably persist the returned marker before enabling edits.
 * No native hold, draft content, persistence or delivery authority is implied. */
export function mobilePendingTaskEditorsCreate(client: Client, marker: MobilePendingTaskMarker): MobilePendingTaskMarker | null {
  const markers = current(client);
  if (!markers || !valid(marker) || marker.revision !== 1 || marker.pending !== null
    || Object.values(markers).some(item => item.owner.messageId === marker.owner.messageId || item.draftKey === marker.draftKey)) return null;
  markers[mobilePendingTaskEditorKey(marker.owner)] = clone(marker); write(client, markers); return clone(marker);
}
function matching(markers: Record<string, MobilePendingTaskMarker>, expected: MobilePendingTaskExpected): MobilePendingTaskMarker | null {
  const marker = markers[mobilePendingTaskEditorKey(expected.owner)];
  return marker?.session === expected.session && marker.revision === expected.revision ? marker : null;
}
/** Native outcome validation belongs to the caller. This CAS prevents an old
 * editor/session from publishing over its successor or silently moving owners. */
export function mobilePendingTaskEditorsReplace(client: Client, expected: MobilePendingTaskExpected, next: MobilePendingTaskMarker): MobilePendingTaskMarker | null {
  const markers = current(client), previous = markers && matching(markers, expected);
  if (!markers || !previous || !valid(next) || next.revision !== previous.revision + 1
    || mobilePendingTaskEditorKey(next.owner) !== mobilePendingTaskEditorKey(previous.owner) || next.draftKey !== previous.draftKey
    || next.contentRevision < previous.contentRevision || next.baseline.revision < previous.baseline.revision
    || next.baseline.revision === previous.baseline.revision && JSON.stringify(next.baseline) !== JSON.stringify(previous.baseline)) return null;
  markers[mobilePendingTaskEditorKey(next.owner)] = clone(next); write(client, markers); return clone(next);
}
/** Root calls only after its exact saved update/removal and draft cleanup are
 * durably resolved. Pending mutation identity cannot be erased by this helper. */
export function mobilePendingTaskEditorsRemove(client: Client, expected: MobilePendingTaskExpected): boolean {
  const markers = current(client), previous = markers && matching(markers, expected);
  if (!markers || !previous || previous.pending !== null) return false;
  delete markers[mobilePendingTaskEditorKey(previous.owner)]; write(client, markers); return true;
}
