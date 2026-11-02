// Image and file chips copied from one draft and pasted into another, after ChatComposer's context clipboard at
// T3 Code 74541bab17 (buildContextClipboardFragment, importContextRecords, copiedComposerAttachments.ts). The reference
// carries each chip's record in a clipboard type of its own and keeps the latest copy's bytes in memory. Exact's
// clipboard carries text only, but the pasted link names its image or file by id, so a paste adopts the record from
// whichever draft holds it, or from the latest copy, which keeps its bytes from being released until another copy.
import type { T3Client } from '../client/client';
import { str, type Obj } from '../client/domain';
import { contextId, contextReferences } from './composer-editor-menu';
import { draftFiles, setDraftFiles, type DraftFile } from './composer-editor-files';

type Copy = { environmentId: string; images: Obj[]; files: DraftFile[] };
const copies = new WeakMap<object, Copy>();

function imageRecord(client: T3Client, reference: string): Obj | undefined {
  for (const shelf of Object.values(client.local.snapshotDrafts)) {
    const image = shelf.find(entry => contextId('image', str(entry.id)) === reference);
    if (image) return image;
  }
  return undefined;
}

/** A copy or cut in the composer: its image and file chips, held as the latest copy (rememberCopiedComposerAttachments). */
export function rememberCopiedChips(client: T3Client, text: string): void {
  const images: Obj[] = [], files: DraftFile[] = [];
  for (const reference of contextReferences(text)) {
    if (reference.kind === 'image') { const image = imageRecord(client, reference.id); if (image) images.push(image); }
    if (reference.kind === 'file') { const file = draftFiles(client.local).find(entry => entry.contextId === reference.id); if (file) files.push(file); }
  }
  if (images.length || files.length) copies.set(client.local, { environmentId: client.environmentId, images, files });
  else copies.delete(client.local);
}

/** Image ids the latest copy holds: their draft bytes stay until another copy replaces it. */
export function heldImageIds(local: object): Set<string> {
  return new Set((copies.get(local)?.images ?? []).map(image => str(image.id)));
}

/** Whether a staged file copy is still needed: a draft file or the latest copy names it before its upload. */
export function stagedInUse(local: object, id: string): boolean {
  return [...draftFiles(local), ...copies.get(local)?.files ?? []].some(file => file.id === id && file.status !== 'ready');
}

/**
 * A paste's image and file chips that the open draft does not hold join it: an image the shelf of the draft it
 * came from, or the latest copy, still has (to be uploaded again from its bytes); a file the same way, its server
 * upload kept when it was made in this environment. Returns the chips adopted. `limit`: the attachments the draft
 * may still take.
 */
export function adoptPastedChips(client: T3Client, text: string, limit: number): number {
  const copy = copies.get(client.local), key = client.attachmentKey, threadKey = client.draftKey;
  let adopted = 0;
  for (const reference of contextReferences(text)) {
    if (adopted >= limit) break;
    if (reference.kind === 'image') {
      const shelf = client.local.snapshotDrafts[key] ?? [];
      if (shelf.some(image => contextId('image', str(image.id)) === reference.id)) continue;
      const image = imageRecord(client, reference.id) ?? copy?.images.find(entry => contextId('image', str(entry.id)) === reference.id);
      if (!image) continue;
      const { uploadId: _uploaded, ...record } = image;
      client.local.snapshotDrafts[key] = [...shelf, record];
      adopted++;
    } else if (reference.kind === 'file') {
      const files = draftFiles(client.local);
      if (files.some(file => file.contextId === reference.id && file.draftKey === threadKey)) continue;
      const file = files.find(entry => entry.contextId === reference.id) ?? copy?.files.find(entry => entry.contextId === reference.id);
      // An upload belongs to its environment's server; elsewhere only staged bytes can go again.
      if (!file || file.environmentId !== client.environmentId && file.status === 'ready') continue;
      setDraftFiles(client.local, [...files, { ...file, draftKey: threadKey }]);
      adopted++;
    }
  }
  return adopted;
}
