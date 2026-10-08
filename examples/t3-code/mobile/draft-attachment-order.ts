// Source365aa87982 use-composer-drafts appendComposerDraftAttachments keeps one ordered list.
// @ref llp/1109.005-composer-and-transcript.decision.md#new-task-ownership
import type { T3Client } from './shared/client';
import { obj, str, type Obj } from './shared/domain';
import { draftFiles, referencedFiles } from './shared/composer-editor-files';

type Orders = Record<string, string[]>;
function orders(client: T3Client): Orders { return obj(obj(client.local).mobileAttachmentOrder) as Orders; }
function ownedIds(client: T3Client, key: string): string[] {
  return [...new Set([...(client.local.snapshotDrafts[key] ?? []).map(image => str(image.id)),
    ...draftFiles(client.local).filter(file => file.draftKey === key).map(file => file.id)].filter(Boolean))];
}
/** Reads never prune the live order: shared removal may restore content after a failed save. */
export function mobileDraftAttachmentIds(client: T3Client, key: string): string[] {
  const live = ownedIds(client, key), set = new Set(live);
  return [...new Set([...(orders(client)[key] ?? []), ...live])].filter(id => set.has(id));
}
/** The caller supplies the pre-pick order followed by this pick's original order.
 * Repeating this as acceptance progresses cannot freeze a partially accepted batch out of order. */
export function mobileDraftAttachmentRecord(client: T3Client, key: string, preferred: string[]): void {
  const live = ownedIds(client, key), set = new Set(live);
  const ids = [...new Set([...preferred, ...live])].filter(id => set.has(id));
  Object.assign(client.local, { mobileAttachmentOrder: { ...orders(client), [key]: ids } });
}
export function mobileDraftAttachmentForget(client: T3Client, key: string): void {
  const next = { ...orders(client) }; delete next[key]; Object.assign(client.local, { mobileAttachmentOrder: next });
}
export function mobileDraftAttachmentsOrdered<T extends { id?: unknown }>(client: T3Client, key: string, items: T[]): T[] {
  const rank = new Map(mobileDraftAttachmentIds(client, key).map((id, index) => [id, index]));
  return [...items].sort((a, b) => (rank.get(str(a.id)) ?? Infinity) - (rank.get(str(b.id)) ?? Infinity));
}
/** Serialize actual owners without changing rollback state in memory. */
export function mobileDraftAttachmentOrdersPersisted(client: T3Client): Orders {
  const keys = new Set([...Object.keys(orders(client)), ...Object.keys(client.local.snapshotDrafts),
    ...draftFiles(client.local).map(file => file.draftKey)]);
  return Object.fromEntries([...keys].map(key => [key, mobileDraftAttachmentIds(client, key)] as const).filter(([, ids]) => ids.length));
}
export function mobileDraftAttachmentOrdersHydrate(client: T3Client, saved: Obj): void {
  const decoded: Orders = {};
  for (const [key, raw] of Object.entries(obj(saved.mobileAttachmentOrder))) {
    if (Array.isArray(raw) && raw.every((id): id is string => typeof id === 'string' && !!id)) decoded[key] = [...new Set(raw)];
  }
  Object.assign(client.local, { mobileAttachmentOrder: decoded });
  Object.assign(client.local, { mobileAttachmentOrder: mobileDraftAttachmentOrdersPersisted(client) });
}
/** Reorder only attachments already in a fresh shared payload, using their actual upload bindings. */
export function mobileDraftAttachmentsForSend(client: T3Client, key: string, attachments: Obj[], text = client.local.drafts[key] ?? ''): Obj[] {
  // The shared sender emits the snapshot array, then files in the captured prompt's reference order.
  // Assign payload occurrences in that source order before restoring mobile chronology. Names and
  // remote IDs can repeat; matching only one remote-ID rank would swap distinct payload objects.
  const sources = [...(client.local.snapshotDrafts[key] ?? []).map(image =>
    ({ id: str(image.id), remote: str(image.uploadId), type: 'image' })),
    ...referencedFiles(client.local, key, text).map(file => ({ id: file.id, remote: file.attachmentId, type: 'file' }))];
  const remaining = [...attachments], bound = new Map<string, Obj[]>();
  for (const source of sources) {
    if (!source.remote) continue;
    const index = remaining.findIndex(item => item.id === source.remote && (!item.type || item.type === source.type));
    if (index >= 0) bound.set(source.id, [...(bound.get(source.id) ?? []), ...remaining.splice(index, 1)]);
  }
  return [...mobileDraftAttachmentIds(client, key).flatMap(id => bound.get(id) ?? []), ...remaining];
}
