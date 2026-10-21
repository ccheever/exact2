// The composer's attachments from a drop, a paste and Attach files, adapted from
// T3 Code 9c161462c9 (MIT); see LICENSE-T3. Sources: chat/ChatComposer.tsx
// (showComposerAttachAction, addComposerAttachments, onComposerPaste, addDroppedFiles,
// addDroppedFolders), chat/workspaceFileDrop.ts (splitDroppedItems),
// chat/composerAttachmentFiles.ts (fileAttachmentStagingLimit) and
// packages/contracts/src/chatAttachment.ts. All three stage their files the same way
// (composer-attach-stage.ts): images join the draft's image shelf (the SnapShot drafts,
// uploaded as `image` attachments on send); other files become inline file chips
// (composer-editor-attach.ts), and what cannot be attached is named in the error.
import { obj, str, num, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { folderDropTarget, primary } from '../connections/local-primary';
import { pushToast } from '../shell/toast';
import { acceptAttachedFile, attachStagingLimit, imagesGetChips, insertFileChips, releaseStagedFile, reservedAttachments } from './composer-editor-attach';
import { holdDraftFiles, type DraftFile } from './composer-editor-files';
import { rememberVideoFrame } from './composer-attachments';
import { handleSource, stageAttachment, statSource, type AttachSource } from './composer-attach-stage';
import { requestPresentation } from './requests';
import { composerFieldOwner } from './auto-balance-owner';

export const MAX_ATTACHMENTS = 100; // PROVIDER_SEND_TURN_MAX_ATTACHMENTS

/** showComposerAttachAction: the server stages uploads, and a pending question (if any) takes attachments with a custom answer. */
export function attachOffered(client: T3Client, question: { customAllowed: boolean } | undefined): boolean {
  const capabilities = obj(obj(client.config.environment).capabilities);
  const known = Object.keys(obj(client.config.environment)).length > 0;
  const staging = !known || (capabilities.attachmentUploads === true && obj(capabilities.fileAttachments).maxUploadBytes != null);
  if (!staging || !client.projectId) return false;
  return !question || (capabilities.questionAttachments === true && question.customAllowed);
}

/**
 * addComposerAttachments' first check: a pending question takes attachments only with a custom answer the server lets
 * carry them. True, with the reference's error, when it refuses them.
 */
export function questionRefuses(client: T3Client): boolean {
  const question = requestPresentation(client).questions[0];
  if (!question || (obj(obj(client.config.environment).capabilities).questionAttachments === true && question.customAllowed && !question.responding)) return false;
  pushToast(client, { kind: 'error', title: 'This question cannot accept attachments.' });
  return true;
}

/** What the window hands over (app.contract attachFiles): its `doc:` handles, or Attach files' picked copies. */
export type IncomingFiles = { files: string[]; picked: Obj[]; asText: boolean; target: string };

/** A picked file (Contract's `Picked`): its copy under app:/tmp/picked/, its name and the type the picker read. */
const pickedSource = (file: Obj): AttachSource => ({ path: str(file.path), name: str(file.name, 'file'), type: str(file.type) });

/**
 * The files a drop, a paste or Attach files brings (`origin` "drop", "paste" or "pick"). A drop's folders are
 * addDroppedFolders': a dropped folder is a handle with no path (LLP 1069.010 D1), so each is the reference's own
 * refusal for a folder whose path it cannot get. Paste as Text (`asText`) gives images no inline chip
 * (skipImageInlineChip). The answer's "focus:composer" focuses the prompt, as the reference's drop and Attach do.
 */
export async function attachIncoming(client: T3Client, native: Native, origin: string, incoming: IncomingFiles): Promise<string> {
  const sources = [...incoming.files.map(handleSource), ...incoming.picked.map(pickedSource)];
  if (!sources.length) return '';
  if (questionRefuses(client)) return '';
  const imageChips = imagesGetChips(client) && !(origin === 'paste' && incoming.asText);
  const staged = await stageFiles(client, native, sources);
  // A pasted or picked folder is the reference's empty File.
  const folders = origin === 'drop' ? staged.filter(file => file.kind === 'folder') : [];
  const others = staged.filter(file => !folders.includes(file)).map(file => file.kind === 'folder' ? { kind: 'unreadable', name: file.name, sizeBytes: 0 } : file);
  if (others.length) await addAttachedFiles(client, native, others, imageChips);
  if (folders.length) {
    const remote = folderDropTarget({ localEnvironmentDisabled: primary.disabled, environmentId: client.environmentId, primaryEnvironmentId: primary.target?.environmentId || null }) === 'remote';
    if (remote) pushToast(client, { kind: 'error', title: "Folders can't be dropped into remote environments" });
    else for (const folder of folders) pushToast(client, { kind: 'error', title: `Couldn't get the path of "${str(folder.name)}"`, description: 'Type the folder path with @ instead.' });
  }
  return origin === 'paste' ? '' : 'focus:composer';
}

/**
 * Each file staged as addAttachedFiles takes it; a draft that changes meanwhile lets them go. The draft's free slots
 * (countReservedAttachments) are counted before each file is prepared: one past them is `over-limit`, never staged.
 * A folder is `folder`.
 */
export async function stageFiles(client: T3Client, native: Native, sources: AttachSource[]): Promise<Obj[]> {
  if (!client.projectId) throw new ClientError('Choose a project first.');
  const key = client.draftKey, limit = attachStagingLimit(client), staged: Obj[] = [];
  let room = MAX_ATTACHMENTS - reservedAttachments(client);
  for (const source of sources) {
    const stat = await statSource(source);
    if (!stat) staged.push({ kind: 'unreadable', name: source.name, sizeBytes: 0 });
    else if (stat.folder) staged.push({ kind: 'folder', name: source.name, path: source.path });
    else if (room <= 0) staged.push({ kind: 'over-limit', name: source.name });
    else {
      const file = await stageAttachment(native, source, stat.size, limit);
      if (file.kind === 'image' || str(file.id)) room -= 1;
      staged.push(file);
    }
  }
  return key === client.draftKey ? staged : (await releaseStaged(client, native, staged), []);
}

/** Staged files nothing will reference: a shelf image's draft and a file's copy go. */
export async function releaseStaged(client: T3Client, native: Native, files: Obj[]): Promise<void> {
  for (const file of files) {
    if (file.kind === 'image') client.local.snapshotReleases.push(str(file.id));
    await releaseStagedFile(native, file);
  }
}

/**
 * addComposerFiles over staged files (a drop, a paste, Attach files, or the sidebar row drop's, legacy-sidebar-drop.ts):
 * images join the shelf; other files become chips; what cannot be attached is named in one error. `owner`: the composer
 * the chips go into at its caret (`editorInsert` refuses another thread's editor; then they join the end of the stored
 * draft).
 */
export async function addAttachedFiles(client: T3Client, native: Native, files: Obj[], imageChips = imagesGetChips(client), owner = ''): Promise<void> {
  const key = client.draftKey;
  const identity = { generation: client.generation, snapshotOwner: client.snapshotOwner, fieldOwner: composerFieldOwner(client) };
  const current = () => identity.generation === client.generation && identity.snapshotOwner === client.snapshotOwner && identity.fieldOwner === composerFieldOwner(client) && key === client.draftKey;
  let error = '';
  const accepted: Obj[] = [], chips: DraftFile[] = [];
  let reserved = reservedAttachments(client);
  const release = holdDraftFiles(client.local, key);
  try {
    for (const file of files) {
      if (!current()) { await releaseStaged(client, native, [file]); continue; }
      const name = str(file.name, 'file');
      if (reserved >= MAX_ATTACHMENTS || file.kind === 'over-limit') {
        await releaseStaged(client, native, [file]); // staged bytes nothing will reference
        error = `You can attach up to ${MAX_ATTACHMENTS} files per message.`; continue;
      }
      if (file.kind === 'unsupported-image') { error = `'${name}' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.`; continue; }
      if (file.kind === 'unreadable') { error = `'${name}' is empty or could not be read.`; continue; }
      if (file.kind === 'image-unreadable') { error = `'${name}' could not be read as an image.`; continue; }
      if (file.kind === 'image-too-large') { error = `'${name}' is too large to attach, even after compression.`; continue; }
      if (file.kind === 'file') {
        const staged = acceptAttachedFile(client, file);
        if (staged.file) { chips.push(staged.file); reserved += 1; rememberVideoFrame(client, file); }
        else { error = staged.error; await releaseStaged(client, native, [file]); }
        continue;
      }
      if (file.kind !== 'image' || !/^[a-f0-9-]{36}$/i.test(str(file.id))) { error = `'${name}' can't be attached here: this app attaches images only.`; continue; }
      accepted.push({ id: str(file.id), name, mimeType: str(file.mimeType, 'image/png'), sizeBytes: num(file.sizeBytes),
        ...(file.downscalable === false ? { downscalable: false } : {}) });
      reserved += 1;
    }
    if (accepted.length) client.local.snapshotDrafts[key] = [...(client.local.snapshotDrafts[key] ?? []), ...accepted];
    const inserted = await insertFileChips(client, native, chips, imageChips ? accepted : [], owner || identity.snapshotOwner);
    if (current()) client.error = error || (!inserted ? 'The prompt changed before attachment references could be inserted. Attach the files again.' : '');
  } finally { release(); }
}
