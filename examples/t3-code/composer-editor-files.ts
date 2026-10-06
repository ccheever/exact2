// Large pastes fold into a `pasted-text.txt` file attachment (ChatComposer
// foldPastedText, client-runtime textPaste.ts): the native editor holds the
// paste back and reports it; this stages the file and puts its inline file
// chip where the paste would have gone. The send uploads it (as SnapShot
// images upload) and binds it as a `file` attachment plus its
// FileContextRecord. The upload's id persists with the device's drafts; the
// staged bytes live in memory until then.
import type { T3Client } from './client';
import { bridgeReply, ClientError, type Native } from './protocol';
import { arr, obj, str, type Obj } from './domain';
import { pushToast } from './toast';
import { pendingRequests } from './requests';
import { contextId, contextLink, contextReferences } from './composer-editor-menu';
import { videoMimeType } from './r4-composer-video';

export const PASTED_TEXT_THRESHOLD_BYTES = 32 * 1024;
export const MAX_FILE_BYTES = 50 * 1024 * 1024;
export const MAX_ATTACHMENTS = 100;
export const PASTED_TEXT_MIME = 'text/plain;charset=utf-8';

export type DraftFile = {
  id: string; contextId: string; draftKey: string; environmentId: string;
  name: string; mimeType: string; sizeBytes: number; source: string;
  /** The server's upload id once the bytes are there (uploaded at send). */
  attachmentId: string;
  status: 'staged' | 'ready';
  /** r5-composer: a video's first-frame size, saved with the draft so a relaunch keeps its preview's aspect. */
  videoWidth?: number; videoHeight?: number;
};

type Holder = { composerFiles?: DraftFile[] };
export function draftFiles(local: object | undefined): DraftFile[] { return (local as Holder | undefined)?.composerFiles ?? []; }
export function setDraftFiles(local: object, files: DraftFile[]): void { (local as Holder).composerFiles = files; }

/**
 * Decode the saved list: uploaded files survive a restart (their bytes are on
 * the server), and so do attached files staged on this device
 * (composer-editor-attach.ts); a folded paste's bytes lived in memory only.
 */
export function adoptComposerFiles(next: object, saved: Obj): void {
  setDraftFiles(next, arr(saved.composerFiles).filter(file => (str(file.status) === 'ready' ? !!str(file.attachmentId) : str(file.source) === 'attached') && str(file.id) && str(file.name))
    .slice(0, 400).map(file => ({ id: str(file.id), contextId: str(file.contextId), draftKey: str(file.draftKey), environmentId: str(file.environmentId),
      name: str(file.name), mimeType: str(file.mimeType, PASTED_TEXT_MIME), sizeBytes: Number(file.sizeBytes) || 0, source: str(file.source),
      attachmentId: str(file.attachmentId), status: str(file.status) === 'ready' ? 'ready' as const : 'staged' as const,
      ...(Number(file.videoWidth) > 0 && Number(file.videoHeight) > 0 ? { videoWidth: Number(file.videoWidth), videoHeight: Number(file.videoHeight) } : {}) })));
}

/** formatAttachmentSize: "3.2 MB" / "48 KB", never "0 KB". */
export function formatAttachmentSize(sizeBytes: number): string {
  return sizeBytes >= 1024 * 1024 ? `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB` : `${Math.max(1, Math.ceil(sizeBytes / 1024))} KB`;
}

/** nextPastedTextFileName: stable names when a draft holds several folded pastes. */
export function nextPastedTextFileName(existing: string[]): string {
  const names = new Set(existing.map(name => name.toLowerCase()));
  if (!names.has('pasted-text.txt')) return 'pasted-text.txt';
  for (let sequence = 2; ; sequence += 1) if (!names.has(`pasted-text-${sequence}.txt`)) return `pasted-text-${sequence}.txt`;
}

/**
 * fileAttachmentStagingLimit: the byte limit a folded paste may have, or 0
 * when this server takes no file uploads (the paste then stays inline, or is
 * refused when it would pass the prompt limit).
 */
export function fileStagingLimit(capabilities: Obj): number {
  if (capabilities.attachmentUploads === undefined && capabilities.fileAttachments === undefined) return 0;
  if (capabilities.attachmentUploads !== true) return 0;
  const advertised = Number(obj(capabilities.fileAttachments).maxUploadBytes);
  return Number.isFinite(advertised) && advertised >= 1 ? Math.min(advertised, MAX_FILE_BYTES) : 0;
}

export function utf8Length(text: string): number { return new TextEncoder().encode(text).byteLength; }

/** A staged file for one folded paste in the draft `draftKey`. */
export function stageFold(local: object, draftKey: string, environmentId: string, text: string, id: string): DraftFile {
  const names = draftFiles(local).filter(file => file.draftKey === draftKey).map(file => file.name);
  const file: DraftFile = { id, contextId: contextId('file', id), draftKey, environmentId, name: nextPastedTextFileName(names),
    mimeType: PASTED_TEXT_MIME, sizeBytes: utf8Length(text), source: 'pasted-text', attachmentId: '', status: 'staged' };
  setDraftFiles(local, [...draftFiles(local), file]);
  return file;
}

/** The chip a staged file shows in the prompt. */
export function fileChipLink(file: DraftFile): string { return contextLink('file', file.contextId, file.name); }

/** The draft's files its prompt references, in prompt order. */
export function referencedFiles(local: object, draftKey: string, text: string): DraftFile[] {
  const files = draftFiles(local).filter(file => file.draftKey === draftKey);
  return contextReferences(text).flatMap(reference => {
    if (reference.kind !== 'file') return [];
    const file = files.find(candidate => candidate.contextId === reference.id);
    return file ? [file] : [];
  });
}

/** Why the prompt's files cannot send yet ('' when they can). */
export function fileSendBlock(files: DraftFile[]): string {
  return files.some(file => file.status !== 'ready' || !file.attachmentId) ? 'Wait for attachments to finish uploading, or remove failed uploads.' : '';
}

/** ChatFileAttachment entries for the send. */
export function fileAttachments(files: DraftFile[]): Obj[] {
  return files.map(file => ({ type: 'file', id: file.attachmentId, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes,
    ...(file.source === 'pasted-text' ? { source: { _tag: 'pasted-text' } } : {}) }));
}

/** attachmentContextRecord for each referenced file. */
export function fileContextRecords(files: DraftFile[]): Obj[] {
  return files.map(file => ({ version: 1, kind: 'file', contextId: file.contextId, label: file.name, attachmentId: file.attachmentId,
    name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes }));
}

/**
 * Drop the files of drafts that are empty (sent or cleared). `liveKey` names
 * the draft whose editor still holds text the stored draft may not have yet.
 */
export function pruneFiles(local: object, drafts: Record<string, string>, liveKey: string): DraftFile[] {
  const files = draftFiles(local);
  const kept = files.filter(file => file.draftKey === liveKey || (drafts[file.draftKey] ?? '').trim() !== '');
  if (kept.length !== files.length) setDraftFiles(local, kept);
  return files.filter(file => !kept.includes(file));
}

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
/** base64 of UTF-8 text, for the native upload op (no btoa in every runtime). */
export function base64Utf8(text: string): string {
  const bytes = new TextEncoder().encode(text);
  const out: string[] = [];
  for (let index = 0; index < bytes.length; index += 3) {
    const a = bytes[index]!, b = bytes[index + 1], c = bytes[index + 2];
    const triple = (a << 16) | ((b ?? 0) << 8) | (c ?? 0);
    out.push(ALPHABET[(triple >> 18) & 63]! + ALPHABET[(triple >> 12) & 63]! + (b === undefined ? '=' : ALPHABET[(triple >> 6) & 63]!) + (c === undefined ? '=' : ALPHABET[triple & 63]!));
  }
  return out.join('');
}

// ── Staging, upload and send (ChatComposer addComposerAttachments) ─────────

/** The native editor's limit for folding a paste here (0: keep it inline). */
export function foldLimit(client: T3Client): number {
  const capabilities = obj(obj(client.config.environment).capabilities);
  // Questions take no files here, and a message holds 100 attachments at most.
  if (pendingRequests(client.projection).inputs.length > 0) return 0;
  if (client.snapshotDrafts.length + referencedFiles(client.local, client.draftKey, client.draft).length >= MAX_ATTACHMENTS) return 0;
  return fileStagingLimit(capabilities);
}

/** The staged pastes' text, by file id (the bytes live only in memory until sent). */
const texts = new WeakMap<object, Map<string, string>>();
/** Files staged per editor fold, so a re-asked view never stages one twice. */
const folds = new WeakMap<object, Map<string, DraftFile>>();
function mapFor<T>(store: WeakMap<object, Map<string, T>>, client: T3Client): Map<string, T> {
  let map = store.get(client);
  if (!map) { map = new Map(); store.set(client, map); }
  return map;
}

async function nativeCall(native: Native, request: Obj): Promise<Obj> {
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  return obj(reply.value);
}

/**
 * One paste the native editor holds back: stage pasted-text.txt, announce it,
 * then put its chip where the paste was. The edit comes last: its own input
 * re-asks the view, after which nothing in this answer is certain to run. The
 * editor keeps the fold until an edit names it, so a lost answer retries.
 */
export async function takeFold(client: T3Client, native: Native, fold: Obj, id: string): Promise<void> {
  const text = str(fold.text);
  if (!text) return;
  const key = `${client.draftKey}\u0000${Number(fold.id)}`;
  let file = mapFor(folds, client).get(key);
  if (!file) {
    file = stageFold(client.local, client.draftKey, client.environmentId, text, id);
    mapFor(folds, client).set(key, file);
    mapFor(texts, client).set(file.id, text);
    pushToast(client, { kind: 'info', title: `Large paste attached as ${file.name}`,
      description: `${formatAttachmentSize(file.sizeBytes)} · Use ⌘⇧V to keep a large paste inline.`, hideCopy: true });
  }
  await nativeCall(native, { op: 'editorEdit', start: Number(fold.start) || 0, end: Number(fold.end) || 0, expect: str(fold.expect),
    text: `${fileChipLink(file)} `, pad: true, fold: Number(fold.id) });
}

/** attachmentUploadQueue at send time: mint an upload URL, send the bytes (base64), keep the id. */
async function upload(client: T3Client, native: Native, file: DraftFile, base64: string): Promise<void> {
  const minted = await client.rpc(native, 'attachments.createUploadUrl', { type: 'file', name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes });
  const attachmentId = str(minted.attachmentId);
  if (!attachmentId || !str(minted.relativeUrl).startsWith('/api/attachments/upload/')) throw new ClientError('The server returned an invalid attachment upload.');
  await nativeCall(native, { op: 'uploadAttachment', path: str(minted.relativeUrl), base64, contentType: file.source === 'pasted-text' ? PASTED_TEXT_MIME : file.mimeType, generation: client.generation });
  file.attachmentId = attachmentId;
  file.status = 'ready';
  // An attached file's staged copy has done its job once the server holds the bytes.
  if (file.source === 'attached') await nativeCall(native, { op: 'composerAttachRemove', id: file.id }).catch(() => undefined);
}

/** The staged bytes of a draft file, or null when they were lost (a folded paste across a restart, a staged copy removed). */
async function stagedBytes(client: T3Client, native: Native, file: DraftFile): Promise<string | null> {
  if (file.source === 'attached') {
    const read = await nativeCall(native, { op: 'composerAttachRead', id: file.id }).catch(() => null);
    return read && str(read.base64) ? str(read.base64) : null;
  }
  const text = mapFor(texts, client).get(file.id);
  return text === undefined ? null : base64Utf8(text);
}

/** The send's file attachments for the prompt's file chips, uploading the staged ones (as SnapShots upload at send). */
export async function composerFileAttachments(client: T3Client, native: Native, text: string): Promise<Obj[]> {
  const files = referencedFiles(client.local, client.draftKey, text);
  for (const file of files) {
    if (file.status === 'ready' && file.attachmentId) continue;
    const bytes = await stagedBytes(client, native, file);
    if (bytes === null) throw new ClientError(`${file.name} was not saved with this draft. Attach it again to send it.`);
    await upload(client, native, file, bytes);
  }
  const block = fileSendBlock(files);
  if (block) throw new ClientError(block);
  return fileAttachments(files);
}

/** FileContextRecords for the prompt's uploaded file chips. */
export function composerFileRecords(client: T3Client, text: string): Obj[] {
  return fileContextRecords(referencedFiles(client.local, client.draftKey, text).filter(file => file.status === 'ready' && file.attachmentId));
}

/**
 * Chip kinds for the native editor: `file/<id>` → "file\t<size>". A file
 * whose bytes were lost with a restart (staged, never uploaded) is left out,
 * so its chip draws unresolved (composerFileNeedsReattach).
 */
export function fileChipContexts(client: T3Client, text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const file of referencedFiles(client.local, client.draftKey, text)) {
    if (file.status === 'ready' || file.source === 'attached' || mapFor(texts, client).has(file.id)) out[`file/${file.contextId}`] = `${videoMimeType(file) ? 'video' : 'file'}\t${formatAttachmentSize(file.sizeBytes)}`; // r4-composer: a video's chip (FilmIcon, kind video)
  }
  return out;
}
