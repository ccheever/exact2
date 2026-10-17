// The composer's Attach files button, adapted from
// T3 Code (MIT); see LICENSE-T3. Sources: chat/ChatComposer.tsx
// (showComposerAttachAction, addComposerAttachments), chat/composerAttachmentFiles.ts
// (fileAttachmentStagingLimit) and packages/contracts/src/chatAttachment.ts.
// Picked images join the draft's image shelf (the SnapShot drafts, uploaded as
// `image` attachments on send); other files become inline file chips
// (composer-editor-attach.ts), and what cannot be attached is named in the error.
import { arr, obj, str, num, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { acceptAttachedFile, attachStagingLimit, imagesGetChips, insertFileChips, releaseStagedFile, reservedAttachments } from './composer-editor-attach';
import type { DraftFile } from './composer-editor-files';
import { rememberVideoFrame } from './composer-attachments';

export const MAX_ATTACHMENTS = 100; // PROVIDER_SEND_TURN_MAX_ATTACHMENTS

/** showComposerAttachAction: the server stages uploads, and a pending question (if any) takes attachments with a custom answer. */
export function attachOffered(client: T3Client, question: { customAllowed: boolean } | undefined): boolean {
  const capabilities = obj(obj(client.config.environment).capabilities);
  const known = Object.keys(obj(client.config.environment)).length > 0;
  const staging = !known || (capabilities.attachmentUploads === true && obj(capabilities.fileAttachments).maxUploadBytes != null);
  if (!staging || !client.projectId) return false;
  return !question || (capabilities.questionAttachments === true && question.customAllowed);
}

/** addComposerAttachments for the picked files: images join the shelf; the rest are named in one error, as the reference does. */
export async function attachFiles(client: T3Client, native: Native): Promise<string> {
  if (!client.projectId) throw new ClientError('Choose a project first.');
  const key = client.draftKey, imageChips = imagesGetChips(client);
  const reply = await client.restAccess(native).call({ op: 'composerAttachPick', fileLimit: attachStagingLimit(client) });
  if (key !== client.draftKey) throw new ClientError('The draft changed while files were being chosen.');
  await addAttachedFiles(client, native, arr(reply.files), imageChips);
  return '';
}

/**
 * onComposerPaste's addComposerAttachments: the files a paste took from the pasteboard (T3ComposerEditor.swift's
 * `pastedFiles`), staged as Attach stages picked ones and added the same way. `key`: the draft the editor reported
 * them for; once another draft is open they are let go. Files Paste as Text took (`asText`) give their images no
 * inline chip (`skipImageInlineChip`); a take holding both kinds of paste follows Paste as Text.
 */
export async function takePastedFiles(client: T3Client, native: Native, key = client.draftKey): Promise<void> {
  const prose = imagesGetChips(client);
  const files = arr(obj(await client.restAccess(native).call({ op: 'composerPasteTake', fileLimit: attachStagingLimit(client) })).files);
  const imageChips = prose && !files.some(file => file.asText === true);
  if (!files.length) return;
  if (key !== client.draftKey || !client.projectId) {
    for (const file of files) {
      if (file.kind === 'image') client.local.snapshotReleases.push(str(file.id));
      await releaseStagedFile(native, file);
    }
    // Attach files says the same; the reference claims no paste without a thread.
    if (key === client.draftKey) client.error = 'Choose a project first.';
    return;
  }
  await addAttachedFiles(client, native, files, imageChips);
}

/**
 * addComposerFiles over staged files (the picker's, or the sidebar row drop's, legacy-sidebar-drop.ts): images join the
 * shelf; other files become chips; what cannot be attached is named in one error. `owner`: the composer the chips go into
 * at its caret (`editorInsert` refuses another thread's editor; then they join the end of the stored draft).
 */
export async function addAttachedFiles(client: T3Client, native: Native, files: Obj[], imageChips = imagesGetChips(client), owner = ''): Promise<void> {
  const key = client.draftKey;
  let error = '';
  const accepted: Obj[] = [], chips: DraftFile[] = [];
  let reserved = reservedAttachments(client);
  for (const file of files) {
    const name = str(file.name, 'file');
    if (reserved >= MAX_ATTACHMENTS) {
      if (file.kind === 'image') client.local.snapshotReleases.push(str(file.id)); // staged bytes nothing will reference
      await releaseStagedFile(native, file);
      error = `You can attach up to ${MAX_ATTACHMENTS} files per message.`; continue;
    }
    if (file.kind === 'unsupported-image') { error = `'${name}' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.`; continue; }
    if (file.kind === 'unreadable') { error = `'${name}' is empty or could not be read.`; continue; }
    if (file.kind === 'too-large') { error = `'${name}' is too large to attach. Images can be up to 10 MB.`; continue; }
    if (file.kind === 'file') { const staged = acceptAttachedFile(client, file); if (staged.file) { chips.push(staged.file); reserved += 1; rememberVideoFrame(client, file); } else error = staged.error; continue; }
    if (file.kind !== 'image' || !/^[a-f0-9-]{36}$/i.test(str(file.id))) { error = `'${name}' can't be attached here: this app attaches images only.`; continue; }
    accepted.push({ id: str(file.id), name, mimeType: 'image/png', sizeBytes: num(file.sizeBytes) });
    reserved += 1;
  }
  if (accepted.length) client.local.snapshotDrafts[key] = [...(client.local.snapshotDrafts[key] ?? []), ...accepted];
  await insertFileChips(client, native, chips, imageChips ? accepted : [], owner);
  client.error = error;
}
