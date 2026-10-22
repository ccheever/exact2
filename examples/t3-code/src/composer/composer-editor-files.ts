import { requestComposerField } from './composer-field-request';
// Large pastes fold into a `pasted-text.txt` file attachment (ChatComposer
// foldPastedText, client-runtime textPaste.ts): the composer's `paste` holds
// the paste back (app.contract composerPasteFold); this stages the file and
// puts its inline file chip where the paste would have gone. The send uploads it (as SnapShot
// images upload) and binds it as a `file` attachment plus its
// FileContextRecord. The upload's id persists with the device's drafts; the
// staged bytes live in memory until then.
import { readStaged, removeStaged } from './composer-staged-copies';
import type { T3Client } from '../client/client';
import { ClientError, bridgeReply, type Native } from '../client/protocol';
import { arr, obj, str, type Obj } from '../client/domain';
import { pushToast } from '../shell/toast';
import { pendingRequests } from './requests';
import { contextId, contextLink, contextReferences } from './composer-editor-menu';
import { videoMimeType } from './composer-video';
import { claimComposerOSRecord, composerOSRecord, type ComposerOSRecord } from './composer-os-record';
import { composerFieldCurrent } from './composer-field-bridge';
import { composerFieldReplacement } from './composer-field-edit';
import { composerSourceSession } from './composer-rich-state';
import { finishAttachmentEffect, prepareAttachmentEffect } from './composer-attachment-receipts';
import { composerFieldOwner } from './auto-balance-owner';

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
  /** A video's first-frame size, saved with the draft so a relaunch keeps its preview's aspect. */
  videoWidth?: number; videoHeight?: number;
};

type Holder = { composerFiles?: DraftFile[] };
export function draftFiles(local: object | undefined): DraftFile[] { return (local as Holder | undefined)?.composerFiles ?? []; }
export function setDraftFiles(local: object, files: DraftFile[]): void { (local as Holder).composerFiles = files; }
const inserting = new WeakMap<object, Map<string, number>>();
/** Staged bytes belong to the draft while its asynchronous chip insertion is pending. */
export function holdDraftFiles(local: object, key: string): () => void {
  const drafts = inserting.get(local) ?? new Map<string, number>();
  inserting.set(local, drafts);
  drafts.set(key, (drafts.get(key) ?? 0) + 1);
  let held = true;
  return () => {
    if (!held) return;
    held = false;
    const count = drafts.get(key) ?? 0;
    if (count <= 1) drafts.delete(key); else drafts.set(key, count - 1);
  };
}

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
 * `among`, when given, are the files there were when the editor's state was
 * asked for: one attached while that request was in flight is kept, since its
 * chip went into the editor after the state that reads it as empty.
 */
export function pruneFiles(local: object, drafts: Record<string, string>, liveKey: string, among?: readonly DraftFile[]): DraftFile[] {
  const files = draftFiles(local);
  const kept = files.filter(file => inserting.get(local)?.has(file.draftKey) || (among !== undefined && !among.includes(file)) || file.draftKey === liveKey || (drafts[file.draftKey] ?? '').trim() !== '');
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

/**
 * The byte limit a folded paste may have here, 0 when this paste cannot be staged; the composer's `paste` reads it
 * (composer-editor.ts ComposerEditorView.foldLimit, composer.contract composerPasteDisposition).
 */
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

/** A folded paste's text while this device holds it (a file chip's preview reads it). */
export function foldedText(client: T3Client, fileId: string): string | undefined { return mapFor(texts, client).get(fileId); }

export function settleFold(client: T3Client, id: string, applied: boolean, current = true): void {
  const file = draftFiles(client.local).find(file => file.id === id);
  if (!file) return;
  if (applied) {
    if (current) pushToast(client, { kind: 'info', title: `Large paste attached as ${file.name}`,
      description: `${formatAttachmentSize(file.sizeBytes)} · Use ⌘⇧V to keep a large paste inline.`, hideCopy: true });
    return;
  }
  const link = fileChipLink(file);
  if (Object.values(client.local.drafts).some(value => value.includes(link))) return;
  if (current) pushToast(client, { kind: 'info', title: 'Paste was not added', description: 'The prompt changed before the attachment could be inserted.', hideCopy: true });
  setDraftFiles(client.local, draftFiles(client.local).filter(entry => entry !== file));
  mapFor(texts, client).delete(file.id);
  for (const [key, staged] of mapFor(folds, client)) if (staged.id === file.id) mapFor(folds, client).delete(key);
}

/** A cross-environment thread's transcript enters the ordinary attachment upload queue. */
export function stageTextAttachment(client: T3Client, text: string, name: string, mimeType: string, id: string): DraftFile {
  const file: DraftFile = { id, contextId: contextId('file', id), draftKey: client.draftKey, environmentId: client.environmentId,
    name, mimeType, sizeBytes: utf8Length(text), source: 'thread-transcript', attachmentId: '', status: 'staged' };
  mapFor(texts, client).set(id, text);
  setDraftFiles(client.local, [...draftFiles(client.local), file]);
  return file;
}

async function nativeCall(native: Native, request: Obj): Promise<Obj> {
  const reply = await bridgeReply(native, request);
  if (!reply.ok) throw new ClientError(reply.error!.message, reply.error!.kind);
  return obj(reply.value);
}

/** A paste the composer held back (app.contract composerPasteFold): its serial, the field's owners, value and selection then, and the text. */
export type HeldPaste = { id: number; snapshotOwner: string; fieldOwner: string; value: string; start: number; end: number; text: string };

/**
 * foldPastedText for a paste the composer's `paste` prevented: `disposition` is composer.contract's
 * composerPasteDisposition. A refusal is the reference's toast; a fold stages the text and puts its chip in the
 * paste's place with one guarded shared-field edit, whose acknowledgement settles the notice and cleanup.
 */
export async function foldPaste(client: T3Client, native: Native, disposition: string, held: HeldPaste, id: string): Promise<void> {
  if (disposition === 'too-large-message' || disposition === 'too-large-attach') {
    const attach = disposition === 'too-large-attach';
    pushToast(client, { kind: 'error', title: attach ? 'Pasted text is too large to attach' : 'Pasted text is too large for this message',
      description: attach ? 'Reduce the clipboard contents or save a smaller excerpt as a file.' : 'Remove some text or an attachment, then paste again.', hideCopy: true });
    return;
  }
  if (disposition !== 'fold') return;
  const fold = { ...held, expect: typeof held.value === 'string' ? held.value.slice(Number(held.start), Number(held.end)) : '' };
  const generation = client.generation;
  const currentOwner = () => generation === client.generation && fold.snapshotOwner === client.snapshotOwner && fold.fieldOwner === composerFieldOwner(client);
  const text = str(fold.text), claimed = claimComposerOSRecord(client, fold);
  const fact = claimed.kind === 'accepted' && text ? claimed.record : null;
  if (!fact || !composerOSRecord(client, fold)) {
    if (currentOwner() && (claimed.kind === 'accepted' || claimed.kind === 'refused' && claimed.notify)) pushToast(client, { kind: 'info', title: 'Paste was not added', description: 'The prompt changed before the attachment could be inserted.', hideCopy: true });
    return;
  }
  const key = `${client.draftKey}\u0000${fold.id}`;
  // The file belongs to the draft until its chip's edit is answered, though the editor view prunes the still empty draft.
  const release = holdDraftFiles(client.local, client.draftKey);
  let queued = false;
  try { queued = await insertFold(client, native, fact, key, text, id, release); } finally { if (!queued) release(); }
}

/** Stages the fold's file and asks for its chip's edit; true when the edit waits for the field (its answer releases the file). */
async function insertFold(client: T3Client, native: Native, fact: ComposerOSRecord, key: string, text: string, id: string, release: () => void): Promise<boolean> {
  let file = mapFor(folds, client).get(key);
  if (!file) {
    file = stageFold(client.local, client.draftKey, client.environmentId, text, id);
    mapFor(folds, client).set(key, file);
    mapFor(texts, client).set(file.id, text);
  }
  const snapshot = composerFieldCurrent(client, fact.fieldOwner), session = composerSourceSession(client);
  const request = { op: 'editorEdit', snapshotOwner: fact.snapshotOwner, fieldOwner: fact.fieldOwner, value: fact.value,
    start: fact.start, end: fact.end, expect: fact.expect, text: `${fileChipLink(file)} `, pad: true };
  const edit = snapshot && composerFieldReplacement(request, { snapshot, draftOwner: client.draftKey, mode: session.mode, chips: session.chips });
  if (!snapshot || !edit || edit.kind === 'refused') { settleFold(client, file.id, false); return false; }
  const expected = edit.edit.value.slice(0, edit.edit.start) + edit.edit.text + edit.edit.value.slice(edit.edit.end);
  const completion = prepareAttachmentEffect(client, snapshot, expected, { kind: 'fold', file: file.id });
  if (!completion) { settleFold(client, file.id, false); return false; }
  const result = await requestComposerField(native, { ...request, completion }, { applied: release, refused: release }).catch(() => ({ accepted: false, queued: false, applied: false }));
  if (!result.queued) finishAttachmentEffect(client, { ...edit.edit, completion }, result.applied, composerFieldCurrent(client, fact.fieldOwner));
  return result.queued;
}

/** attachmentUploadQueue at send time: mint an upload URL, send the bytes (base64), keep the id. */
async function upload(client: T3Client, native: Native, file: DraftFile, base64: string, assertOwner?: () => void): Promise<void> {
  assertOwner?.();
  const minted = await client.rpc(native, 'attachments.createUploadUrl', { type: 'file', name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes });
  assertOwner?.();
  const attachmentId = str(minted.attachmentId);
  if (!attachmentId || !str(minted.relativeUrl).startsWith('/api/attachments/upload/')) throw new ClientError('The server returned an invalid attachment upload.');
  await nativeCall(native, { op: 'uploadAttachment', path: str(minted.relativeUrl), base64, contentType: file.source === 'pasted-text' ? PASTED_TEXT_MIME : file.mimeType, generation: client.generation });
  assertOwner?.();
  file.attachmentId = attachmentId;
  file.status = 'ready';
  // A chip pasted into another draft of this environment names the same bytes (composer-copied-chips.ts): it takes the upload too.
  for (const other of draftFiles(client.local)) if (other.id === file.id && other.environmentId === file.environmentId) { other.attachmentId = attachmentId; other.status = 'ready'; }
  // An attached file's staged copy has done its job once the server holds the bytes for every draft that names it.
  if (file.source === 'attached' && !draftFiles(client.local).some(other => other.id === file.id && other.status !== 'ready')) await removeStaged(file.id);
  assertOwner?.();
}

/** The staged bytes of a draft file, or null when they were lost (a folded paste across a restart, a staged copy removed). */
async function stagedBytes(client: T3Client, native: Native, file: DraftFile): Promise<string | null> {
  if (file.source === 'attached') return readStaged(file);
  const text = mapFor(texts, client).get(file.id);
  return text === undefined ? null : base64Utf8(text);
}

/** The send's file attachments for the prompt's file chips, uploading the staged ones (as SnapShots upload at send). */
export async function composerFileAttachments(client: T3Client, native: Native, text: string, assertOwner?: () => void): Promise<Obj[]> {
  assertOwner?.();
  const files = referencedFiles(client.local, client.draftKey, text);
  for (const file of files) {
    assertOwner?.();
    if (file.status === 'ready' && file.attachmentId) continue;
    const bytes = await stagedBytes(client, native, file);
    assertOwner?.();
    if (bytes === null) throw new ClientError(`${file.name} was not saved with this draft. Attach it again to send it.`);
    await upload(client, native, file, bytes, assertOwner);
    assertOwner?.();
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
    if (file.status === 'ready' || file.source === 'attached' || mapFor(texts, client).has(file.id)) out[`file/${file.contextId}`] = `${videoMimeType(file) ? 'video' : 'file'}\t${formatAttachmentSize(file.sizeBytes)}`; // a video's chip (FilmIcon, kind video)
  }
  return out;
}
