// The composer's Attach files button (lane composer-controls), adapted from
// T3 Code (MIT); see LICENSE-T3. Sources: chat/ChatComposer.tsx
// (showComposerAttachAction, addComposerAttachments), chat/composerAttachmentFiles.ts
// (fileAttachmentStagingLimit) and packages/contracts/src/chatAttachment.ts.
// Picked images join the draft's image shelf (the SnapShot drafts, uploaded as
// `image` attachments on send); other files become inline file chips
// (composer-editor-attach.ts), and what cannot be attached is named in the error.
import { arr, obj, str, num, type Obj } from './domain';
import { ClientError, type Native } from './protocol';
import type { T3Client } from './client';
import { acceptAttachedFile, attachStagingLimit, imagesGetChips, insertFileChips, releaseStagedFile, reservedAttachments } from './composer-editor-attach';
import type { DraftFile } from './composer-editor-files';
import { rememberVideoFrame } from './r4-composer-attachments';

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
  const files = arr(reply.files);
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
  await insertFileChips(client, native, chips, imageChips ? accepted : []);
  client.error = error;
  return '';
}
