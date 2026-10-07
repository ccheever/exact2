// Pinned365aa87982 keeps queued-edit content separate from thread model/runtime settings.
// @ref llp/1107.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import type { T3Client } from './shared/client';
import { arr, str, type Obj } from './shared/domain';
import type { Files, Native } from './shared/protocol';
import { ClientError } from './shared/protocol';
import { mobileQueuedEditCurrent, mobileQueuedEditLookup, mobileQueuedEditWriteText, mobileQueuedEditWriteContent, mobileQueuedEditPersist } from './queued-edit-state';

/** Content identity is independent of the real wire thread and its settings.
 * Keep the captured value through awaits; never resolve an ended edit as an ordinary draft. */
export interface MobileComposerTarget {
  kind: 'ordinary' | 'queued-edit'; owner: string; key: string; editorOwner: string; editOwner: string;
  origin: string; environmentId: string; generation: number; projectId: string; threadId: string;
}
export function mobileComposerTarget(client: T3Client): MobileComposerTarget {
  const edit = mobileQueuedEditCurrent(client);
  const base = { origin: client.origin, environmentId: client.environmentId, generation: client.generation,
    projectId: client.projectId, threadId: client.threadId };
  return edit ? { ...base, kind: 'queued-edit', owner: JSON.stringify(['queued-edit', edit.owner]), key: edit.draftKey, editorOwner: edit.owner, editOwner: edit.owner }
    : { ...base, kind: 'ordinary', owner: JSON.stringify(['ordinary', client.origin, client.environmentId, client.generation, client.draftKey]),
      key: client.draftKey, editorOwner: client.draftKey, editOwner: '' };
}
export function mobileComposerTargetCurrent(client: T3Client, target: MobileComposerTarget): boolean {
  return mobileComposerTarget(client).owner === target.owner && mobileComposerTargetExists(client, target);
}
/** Voice may finish into a captured draft after leaving its route. Connection changes
 * invalidate it; an ended queued-edit session never resolves to a same-key replacement. */
export function mobileComposerTargetExists(client: T3Client, target: MobileComposerTarget): boolean {
  if (client.origin !== target.origin || client.environmentId !== target.environmentId || client.generation !== target.generation) return false;
  return target.kind === 'ordinary' || mobileQueuedEditLookup(target.editOwner, client)?.draftKey === target.key;
}
export function mobileComposerTargetText(client: T3Client, target: MobileComposerTarget): string | null {
  if (!mobileComposerTargetExists(client, target)) return null;
  return target.kind === 'ordinary' ? client.local.drafts[target.key] ?? '' : mobileQueuedEditLookup(target.editOwner, client)?.text ?? null;
}
/** Queued edits expose a monotonic content revision, including text ABA. Ordinary
 * drafts retain the existing synchronous voice observer's revision counter. */
export function mobileComposerTargetRevision(client: T3Client, target: MobileComposerTarget): number | null {
  if (!mobileComposerTargetExists(client, target)) return null;
  return target.kind === 'queued-edit' ? mobileQueuedEditLookup(target.editOwner, client)?.revision ?? null : 0;
}
/** Synchronous writes let voice compare its captured text/revision before insertion.
 * Ordinary keyboard input still uses the shared question-aware draft reducer. */
export function mobileComposerTargetWriteText(client: T3Client, target: MobileComposerTarget, text: string): boolean {
  if (!mobileComposerTargetExists(client, target)) return false;
  if (text.length > 1_000_000) throw new ClientError('Keep a draft under 1,000,000 characters.');
  if (target.kind === 'queued-edit') return mobileQueuedEditWriteText(target.editOwner, text, client);
  client.local.drafts[target.key] = text; client.revision++; return true;
}
export async function mobileComposerTargetPersist(client: T3Client, target: MobileComposerTarget, native: Native, storage: Files): Promise<void> {
  if (target.kind === 'queued-edit') return mobileQueuedEditPersist(target.editOwner, native, client);
  await client.persist(storage);
}
export function mobileComposerTargetRequire(client: T3Client, expectedOwner = ''): MobileComposerTarget {
  const target = mobileComposerTarget(client);
  if (expectedOwner && expectedOwner !== target.owner || !mobileComposerTargetCurrent(client, target)) {
    throw new ClientError('The composer changed. Try again in the current draft.', 'superseded');
  }
  return target;
}

/** Review and terminal context must live beside the edit text, not in a shared
 * ordinary-draft cache. Callers provide the source's actual wire record. */
export async function mobileComposerEditContext(client: T3Client, target: MobileComposerTarget, text: string,
  record: Obj | null, removeId: string, native: Native): Promise<void> {
  if (target.kind !== 'queued-edit' || !mobileComposerTargetCurrent(client, target))
    throw new ClientError('The composer changed.', 'superseded');
  const edit = mobileQueuedEditLookup(target.editOwner, client);
  if (!edit) throw new ClientError('The queued edit ended.', 'superseded');
  const records = arr(edit.context?.records).filter(item => str(item.contextId) !== removeId && (!record || item.contextId !== record.contextId));
  if (record) records.push(record);
  if (records.length > 200) throw new ClientError('Remove some context from the draft and try again.');
  if (!mobileQueuedEditWriteContent(target.editOwner, { text, context: records.length ? { version: 1, records } : undefined }, client))
    throw new ClientError('The queued edit is no longer editable.', 'superseded');
  await mobileQueuedEditPersist(target.editOwner, native, client);
}
