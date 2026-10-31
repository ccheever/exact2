import { requestComposerField, type ComposerFieldCompletion } from './composer-field-request';
import { removeStaged, stagedName } from './composer-staged-copies';
import { stagedInUse } from './composer-copied-chips';
import { composerFieldCurrent } from './composer-field-bridge';
import { composerFieldOwner } from './auto-balance-owner';
// The composer shelf's references and video tiles, adapted from T3 Code (MIT;
// see LICENSE-T3): ChatComposer removeComposerImage, composerDraftStore
// removeImage/removeFile (lib/composerContextReferences removeInlineContextReference),
// composerAttachmentFiles isPreviewableComposerVideo and packages/shared video.ts.
import type { T3Client } from '../client/client';
import { ClientError, preferenceFiles, type Native } from '../client/protocol';
import { obj, str, type Obj } from '../client/domain';
import { draftFiles, setDraftFiles } from './composer-editor-files';
import { contextId, contextReferences } from './composer-editor-menu';
import { contextLinks, contextTarget } from './context-links';
import { videoMimeType } from './composer-video';
export { videoMimeType } from './composer-video';

/** removeInlineContextReference: every link to the context, with one adjoining space, and a trailing gap trimmed. */
export function removeInlineContextReference(prompt: string, id: string): { prompt: string; cursor: number } {
  const occurrences = contextLinks(prompt).filter(link => contextTarget(link.href)?.id === id);
  if (occurrences.length === 0) return { prompt, cursor: prompt.length };
  let result = prompt, cursor = prompt.length;
  for (const occurrence of occurrences.reverse()) {
    let { start, end } = occurrence;
    if (result[end] === ' ') end += 1;
    else if (result[start - 1] === ' ') start -= 1;
    result = `${result.slice(0, start)}${result.slice(end)}`;
    cursor = start;
  }
  if (cursor >= result.length) { result = result.trimEnd(); cursor = result.length; }
  return { prompt: result, cursor };
}

/** The shelf image ids whose chip is in the prompt (removeComposerImage's `referenced`). */
export function referencedImageIds(prompt: string, imageIds: ReadonlyArray<string>): Set<string> {
  const chips = new Set(contextReferences(prompt).filter(reference => reference.kind === 'image').map(reference => reference.id));
  return new Set(imageIds.filter(id => chips.has(contextId('image', id))));
}

/**
 * composerDraftStore removeImage / removeFile: the draft's prompt loses every
 * chip for the removed attachment. A live field persists its accepted input;
 * a background draft has no field and updates its stored source directly.
 */
export async function removeAttachmentReferences(client: T3Client, native: Native, key: string, kind: 'image' | 'file', producerId: string, effects: ComposerFieldCompletion = {}): Promise<boolean> {
  const field = key === client.draftKey ? composerFieldCurrent(client, composerFieldOwner(client)) : null;
  if (key === client.draftKey && !field) throw new ClientError('The composer is not ready to remove attachment references.');
  const prompt = field?.value ?? client.local.drafts[key] ?? '';
  const next = removeInlineContextReference(prompt, kind === 'image' ? contextId('image', producerId) : producerId).prompt;
  if (next === prompt) { await effects.applied?.(native); return false; }
  if (key !== client.draftKey) { client.local.drafts[key] = next; await effects.applied?.(native); return true; }
  // `expect`: a prompt typed past this one is never overwritten; its own input carries the chips then.
  const reply = await requestComposerField(native, { op: 'editorEdit', all: true, expect: prompt, text: next, focus: false }, {
    ...effects, applied: async current => {
      const source = composerFieldCurrent(client, composerFieldOwner(client))?.value ?? client.local.drafts[key] ?? '';
      if (removeInlineContextReference(source, kind === 'image' ? contextId('image', producerId) : producerId).prompt !== source) throw new ClientError('The attachment is still referenced by the prompt.');
      await effects.applied?.(current);
    },
  });
  if (!reply.accepted) throw new ClientError('The prompt changed before attachment references could be removed.');
  return true;
}

// ── Video tiles (composerVideos) ────────────────────────────────────────────

export type VideoTile = { id: string; name: string; src: string; contextId: string };
/** isPreviewableComposerVideo for the draft's attached files: a staged local copy, whose first frame the tile shows. */
export function videoTiles(files: ReadonlyArray<{ id: string; name: string; mimeType: string; contextId: string; source: string; status: string; draftKey: string }>, key: string): VideoTile[] {
  return files.filter(file => file.draftKey === key && file.source === 'attached' && videoMimeType(file) !== null && /^[a-f0-9-]{36}$/i.test(file.id))
    .map(file => ({ id: file.id, name: str(file.name, 'video'), src: stagedMediaSrc(file), contextId: file.contextId }));
}

// The video's oriented display size, read from its header at staging (composer-video.ts videoFrameSize). It is saved on
// the draft's file entry too (adoptComposerFiles keeps it), so a relaunched draft opens at the same aspect.
const frames = new WeakMap<object, Map<string, { width: number; height: number }>>();
export function rememberVideoFrame(client: object, file: Obj): void {
  const width = Number(file.videoWidth), height = Number(file.videoHeight), id = str(file.id);
  if (!id || !(width > 0) || !(height > 0)) return;
  let sizes = frames.get(client);
  if (!sizes) { sizes = new Map(); frames.set(client, sizes); }
  sizes.set(id, { width, height });
  const local = (client as { local?: object }).local;
  for (const entry of draftFiles(local)) if (entry.id === id) { entry.videoWidth = width; entry.videoHeight = height; }
}
function frameOf(client: T3Client, id: string): { width: number; height: number } | undefined {
  const saved = draftFiles(client.local).find(entry => entry.id === id && (entry.videoWidth ?? 0) > 0 && (entry.videoHeight ?? 0) > 0);
  return frames.get(client)?.get(id) ?? (saved ? { width: saved.videoWidth!, height: saved.videoHeight! } : undefined);
}
const previews = new WeakMap<object, string>();

export type ComposerVideoPreview = { open: boolean; id: string; name: string; width: number; height: number; src: string };
/** A staged video or audio file as an app file, which a Contract `video` or `audio` plays (composer-staged-copies.ts stagedName). */
export function stagedMediaSrc(file: { id: string; name: string; mimeType: string }): string { return `app:/data/composer-files/${stagedName(file)}`; }
/** The snapshot's shelf videos and the open preview (presentation.ts). */
export function composerVideoSnapshot(client: T3Client): { composerVideos: Array<{ id: string; name: string; src: string }>; composerVideoPreview: ComposerVideoPreview[] } {
  const tiles = videoTiles(draftFiles(client.local), client.draftKey);
  const open = previews.get(client) ?? '', tile = tiles.find(candidate => candidate.id === open);
  const size = frameOf(client, open);
  return { composerVideos: tiles.map(video => ({ id: video.id, name: video.name, src: video.src })),
    composerVideoPreview: tile ? [{ open: true, id: tile.id, name: tile.name, width: size?.width ?? 0, height: size?.height ?? 0, src: tile.src }] : [] };
}

/** `editorlocal:shelf-video-*`: Play opens the expanded preview, Remove is removeComposerFileFromDraft. */
export async function videoOp(client: T3Client, native: Native, op: string, id: string): Promise<string> {
  if (op === 'shelf-video-close') { previews.delete(client); return ''; }
  const file = draftFiles(client.local).find(candidate => candidate.id === id && candidate.draftKey === client.draftKey);
  if (!file) throw new ClientError('That video is no longer attached.');
  if (op === 'shelf-video-play') { previews.set(client, id); return ''; }
  if (op !== 'shelf-video-remove') throw new ClientError(`Unknown editor action: ${op}`);
  if (previews.get(client) === id) previews.delete(client);
  const key = client.draftKey;
  await removeAttachmentReferences(client, native, key, 'file', file.contextId, { applied: async () => {
    const previous = draftFiles(client.local);
    setDraftFiles(client.local, previous.filter(candidate => candidate !== file));
    try { await client.persist(preferenceFiles()); } catch (error) { setDraftFiles(client.local, previous); throw error; }
    if (file.status === 'staged' && !stagedInUse(client.local, file.id)) await removeStaged(file.id);
  } });
  return '';
}
