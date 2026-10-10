// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r4-composer-attachments.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// The composer shelf's references and video tiles, adapted from T3 Code (MIT;
// see LICENSE-T3): ChatComposer removeComposerImage, composerDraftStore
// removeImage/removeFile (lib/composerContextReferences removeInlineContextReference),
// composerAttachmentFiles isPreviewableComposerVideo and packages/shared video.ts.
import type { T3Client } from './client';
import { bridgeReply, ClientError, type Native } from './protocol';
import { str, type Obj } from './domain';
import { draftFiles, setDraftFiles } from './composer-editor-files';
import { contextId, contextReferences } from './composer-editor-menu';
import { videoMimeType } from './r4-composer-video';
export { videoMimeType } from './r4-composer-video';

const CONTEXT_LINK = /(!?)\[([^\]\n]{0,512})\]\((t3-context:\/\/v1\/([a-z][a-z0-9-]{0,39})\/([a-z0-9_-]{1,128}))\)/gi;

/** removeInlineContextReference: every link to the context, with one adjoining space, and a trailing gap trimmed. */
export function removeInlineContextReference(prompt: string, id: string): { prompt: string; cursor: number } {
  const occurrences = [...prompt.matchAll(CONTEXT_LINK)].filter(match => match[5] === id)
    .map(match => ({ start: match.index!, end: match.index! + match[0].length }));
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
 * chip for the removed attachment. The stored draft changes first; the open
 * editor then takes the same text as one edit, whose input supersedes it.
 */
export async function removeAttachmentReferences(client: T3Client, native: Native, key: string, kind: 'image' | 'file', producerId: string): Promise<boolean> {
  const prompt = client.local.drafts[key] ?? '';
  const next = removeInlineContextReference(prompt, kind === 'image' ? contextId('image', producerId) : producerId).prompt;
  if (next === prompt) return false;
  client.local.drafts[key] = next;
  if (key !== client.draftKey) return true;
  // `expect`: a prompt typed past this one is never overwritten; its own input carries the chips then.
  await bridgeReply(native, { op: 'editorEdit', all: true, expect: prompt, text: next, focus: false }).catch(() => null);
  return true;
}

// ── Video tiles (composerVideos) ────────────────────────────────────────────

export type VideoTile = { id: string; name: string; poster: string; contextId: string };
/** isPreviewableComposerVideo for the draft's attached files: a staged local copy (and its poster frame) to show. */
export function videoTiles(files: ReadonlyArray<{ id: string; name: string; mimeType: string; contextId: string; source: string; status: string; draftKey: string }>, key: string): VideoTile[] {
  return files.filter(file => file.draftKey === key && file.source === 'attached' && videoMimeType(file) !== null && /^[a-f0-9-]{36}$/i.test(file.id))
    .map(file => ({ id: file.id, name: str(file.name, 'video'), poster: file.id, contextId: file.contextId }));
}

// The first frame's natural size from the pick (T3ComposerAttach.swift). r5-composer: it is saved on the
// draft's file entry too (adoptComposerFiles keeps it), so a relaunched draft opens at the same aspect.
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

export type ComposerVideoPreview = { open: boolean; id: string; name: string; width: number; height: number };
/** The snapshot's shelf videos and the open preview (presentation.ts). */
export function composerVideoSnapshot(client: T3Client): { composerVideos: Array<{ id: string; name: string }>; composerVideoPreview: ComposerVideoPreview[] } {
  const tiles = videoTiles(draftFiles(client.local), client.draftKey);
  const open = previews.get(client) ?? '', tile = tiles.find(candidate => candidate.id === open);
  const size = frameOf(client, open);
  return { composerVideos: tiles.map(video => ({ id: video.id, name: video.name })),
    composerVideoPreview: tile ? [{ open: true, id: tile.id, name: tile.name, width: size?.width ?? 0, height: size?.height ?? 0 }] : [] };
}

/** `editorlocal:r4c-video-*`: Play opens the expanded preview, Remove is removeComposerFileFromDraft. */
export async function videoOp(client: T3Client, native: Native, op: string, id: string): Promise<string> {
  if (op === 'r4c-video-close') { previews.delete(client); return ''; }
  const file = draftFiles(client.local).find(candidate => candidate.id === id && candidate.draftKey === client.draftKey);
  if (!file) throw new ClientError('That video is no longer attached.');
  if (op === 'r4c-video-play') { previews.set(client, id); return ''; }
  if (op !== 'r4c-video-remove') throw new ClientError(`Unknown editor action: ${op}`);
  if (previews.get(client) === id) previews.delete(client);
  setDraftFiles(client.local, draftFiles(client.local).filter(candidate => candidate !== file));
  await removeAttachmentReferences(client, native, client.draftKey, 'file', file.contextId);
  if (file.status === 'staged') await bridgeReply(native, { op: 'composerAttachRemove', id: file.id }).catch(() => undefined);
  return '';
}
