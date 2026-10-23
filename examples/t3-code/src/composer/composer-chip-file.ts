// A composer file chip's press, adapted from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): composerContextPresentation.tsx
// FileContextChip (`actions.openFile`), ChatComposer.tsx's `previewFileId` Dialog (DialogPopup
// "h-[min(85vh,52rem)] max-w-4xl overflow-hidden", no corner close, an sr-only DialogTitle of the file's name) and
// files/AttachmentFilePreview.tsx with origin "Draft", onRemove (removeComposerFileFromDraft) and onClose.
//
// The preview is the sent attachment's (attachment-surface.ts) over the draft's own bytes, as the reference passes the
// draft's Blob: a staged copy of an attached file (composer-attach-stage.ts) or a folded paste's text, which this device
// holds until the send uploads them; a file the send already uploaded is read from the server as a sent one is.
// The window draws it in composer-chip-file.contract; its controls arrive as `editorlocal:chip-file`.
import type { T3Client } from '../client/client';
import type { Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { draftFiles, foldedText, type DraftFile } from './composer-editor-files';
import { attachmentOp, attachmentView, emptyAttachment, forgetPreview, previewKind, type AttachmentMeta, type AttachmentView, type PreviewSource } from '../panel/attachment-surface';
import { stagedMediaSrc, videoOp } from './composer-attachments';
import { saveStaged, saveText, stagedText } from '../panel/attachment-files'; // a draft's staged copy, read and saved
import { stagedName } from './composer-staged-copies';

/** FILE_TEXT_PREVIEW_MAX_BYTES. */
const PREVIEW_BYTES = 1024 * 1024;
const opened = new WeakMap<T3Client, { key: string; id: string }>();

/** The draft file `previewFileId` names, while it is still in the open draft. */
function openFile(client: T3Client): DraftFile | null {
  const open = opened.get(client);
  if (!open) return null;
  // The reference's dialog state is its ChatView's, which another draft's route replaces: leaving the draft closes it.
  const file = open.key === client.draftKey ? draftFiles(client.local).find(entry => entry.id === open.id && entry.draftKey === open.key) : undefined;
  if (!file) { close(client); return null; }
  return file;
}
function close(client: T3Client): void {
  const open = opened.get(client);
  if (open) forgetPreview(client, `draft:${open.id}`);
  opened.delete(client);
}

/**
 * readFilePreviewResponse over a folded paste's text: its first megabyte, refused as binary when it holds a NUL (as the
 * native read of a staged copy refuses it, T3PanelsNative.swift).
 */
export function pastedPreview(text: string): Obj {
  const bytes = new TextEncoder().encode(text), truncated = bytes.length > PREVIEW_BYTES;
  // A cut can split one UTF-8 sequence; its replacement character goes.
  const shown = truncated ? new TextDecoder().decode(bytes.subarray(0, PREVIEW_BYTES)).replace(/�+$/, '') : text;
  if (shown.includes('\0')) return { ok: false, message: 'This file contains binary data and cannot be shown as text.' };
  return { ok: true, text: shown, truncated };
}

/** Where the preview reads `file`: the server once the send uploaded it, else this device's staged copy or paste. */
export function draftSource(client: T3Client, native: Native, file: DraftFile): { meta: AttachmentMeta; source: PreviewSource } {
  const key = `draft:${file.id}`;
  const meta = { id: file.status === 'ready' && file.attachmentId ? file.attachmentId : file.id, name: file.name, mimeType: file.mimeType, sizeBytes: file.sizeBytes };
  if (file.status === 'ready' && file.attachmentId) return { meta, source: { id: file.id, key, origin: 'Draft', dialog: true, local: null } };
  const pasted = file.source !== 'attached';
  return { meta, source: { id: file.id, key, origin: 'Draft', dialog: true, local: {
    // AudioPreview plays the staged copy (a Contract `audio`); a PDF or a page is the media body's, by the copy's name
    // (T3MediaPreview.swift reads it from composer-files/).
    url: previewKind(file) === 'audio' && !pasted ? stagedMediaSrc(file) : `t3-staged:${stagedName(file)}`,
    text: async () => {
      if (!pasted) return stagedText(stagedName(file));
      const text = foldedText(client, file.id);
      return text === undefined ? { ok: false, message: 'Could not load this file.' } : pastedPreview(text);
    },
    save: async () => {
      if (!pasted) return saveStaged(client, stagedName(file), file.name);
      const text = foldedText(client, file.id);
      if (text === undefined) throw new ClientError('The file could not be loaded. Try again.');
      await saveText(client, text, file.name);
    },
  } } };
}

/** FileContextChip's `openFile`: the chip's context id names the draft file. */
export function openComposerFile(client: T3Client, context: string): void {
  const file = draftFiles(client.local).find(entry => entry.contextId === context && entry.draftKey === client.draftKey);
  if (!file) throw new ClientError('That file is no longer attached.');
  forgetPreview(client, `draft:${file.id}`);
  opened.set(client, { key: client.draftKey, id: file.id });
}

export type ComposerFilePreview = { open: boolean; file: AttachmentView };
/** The shell's `composerFile`: the open draft file's preview, or none. */
export async function composerFileView(client: T3Client, native: Native | null | undefined, now: number): Promise<ComposerFilePreview> {
  const file = openFile(client);
  if (!file || !native?.available) return { open: false, file: emptyAttachment() };
  const { meta, source } = draftSource(client, native, file);
  return { open: true, file: await attachmentView(client, native, meta, now, source) };
}

/**
 * `editorlocal:chip-file`: `id` the control (the header's `surface-linked-att-<op>`, or `close`), `value` the draft file.
 * Remove from draft is removeComposerFileFromDraft (the chip and the staged copy go), then the dialog closes.
 */
export async function composerFileLocal(client: T3Client, native: Native, control: string, value: string): Promise<string> {
  const op = control.replace(/^surface-linked-att-/, ''), file = openFile(client);
  // A control of a preview already gone (a late media error) leaves the open one alone.
  if (!file || file.id !== value) return '';
  if (op === 'close') { close(client); return ''; }
  if (op === 'remove') {
    close(client);
    return videoOp(client, native, 'shelf-video-remove', file.id);
  }
  const { meta, source } = draftSource(client, native, file);
  return attachmentOp(client, native, meta, source, op, 0);
}
