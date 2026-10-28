// The composer's attachments from a drop, a paste and Attach files, adapted from
// T3 Code 74541bab17 (MIT); see LICENSE-T3. Sources: chat/ChatComposer.tsx
// (showComposerAttachAction, addComposerAttachments, onComposerPaste, addDroppedFiles,
// addComposerFolders), chat/workspaceFileDrop.ts (splitTransferredFiles),
// chat/composerAttachmentFiles.ts (fileAttachmentStagingLimit) and
// packages/contracts/src/chatAttachment.ts. All three stage their files the same way
// (composer-attach-stage.ts): images join the draft's image shelf (the SnapShot drafts,
// uploaded as `image` attachments on send), or, while the composer answers a question, that
// question's shelf (question-attachments.ts), uploaded with its answer; other files become
// inline file chips (composer-editor-attach.ts), and what cannot be attached is named in the error.
// An image's tile shows as soon as it is taken, disabled while it is prepared (preparingComposerImages.ts at
// T3 Code 7c3b3b3605ef).
import { obj, str, num, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import type { T3Client } from '../client/client';
import { folderDropTarget, primary } from '../connections/local-primary';
import { pushToast } from '../shell/toast';
import { acceptAttachedFile, appendChipLinks, attachStagingLimit, imageChipLink, imagesGetChips, insertFileChips, releaseStagedFile, reservedAttachments, reservedAttachmentsIn } from './composer-editor-attach';
import { fileChipLink, holdDraftFiles, pasteAsTextArmed, type DraftFile } from './composer-editor-files';
import { rememberVideoFrame } from './composer-attachments';
import { classifyAttachment, handleSource, previewSource, releasePreview, stageAttachment, statSource, type AttachSource } from './composer-attach-stage';
import { wakeShell } from '../client/command-waits';
import { uuid } from '../transport/transport-wire';
import { requestPresentation } from './requests';
import { composerFieldOwner } from './auto-balance-owner';
import { holdStaging } from './question-attachments';

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

// preparingComposerImages: images taken into a draft and still being prepared (a large screenshot takes a moment to
// re-encode), by attachment key. The strip shows each at once, under the id its image will take, as a 256 px thumbnail
// of its source (`source`; '' until it is made, or for none, when the tile shows the name); it holds its slot until the
// image joins the draft, and removing its tile cancels it.
type Preparing = { id: string; name: string; source: string };
const preparing = new WeakMap<T3Client, Map<string, Preparing[]>>();
function preparingIn(client: T3Client, key: string): Preparing[] { return preparing.get(client)?.get(key) ?? []; }
function setPreparing(client: T3Client, key: string, images: Preparing[]): void {
  const byKey = preparing.get(client) ?? new Map<string, Preparing[]>();
  if (images.length) byKey.set(key, images); else byKey.delete(key);
  preparing.set(client, byKey);
}
/** The composer's images still being prepared, for its strip; one that already joined the draft shows as itself. */
export function preparingImages(client: T3Client): Preparing[] {
  const ready = new Set((client.local.snapshotDrafts[client.attachmentKey] ?? []).map(image => str(image.id)));
  return preparingIn(client, client.attachmentKey).filter(image => !ready.has(image.id));
}
/** A preparing tile's Remove: its image is let go once it is ready (op cclocal:preparing-image-cancel). */
export function cancelPreparingImage(client: T3Client, id: string): void {
  setPreparing(client, client.attachmentKey, preparingImages(client).filter(image => image.id !== id));
}

/** What the window hands over (app.contract attachFiles): its `doc:` handles, or Attach files' picked copies. */
export type IncomingFiles = { files: string[]; picked: Obj[]; asText: boolean; target: string };

/** The draft files are staged for: the composer's thread draft, or a question's, which takes images beside its answer. */
export type DraftTarget = { generation: number; snapshotOwner: string; fieldOwner: string; key: string; threadKey: string; environmentId: string };
export function draftTarget(client: T3Client): DraftTarget {
  return { generation: client.generation, snapshotOwner: client.snapshotOwner, fieldOwner: composerFieldOwner(client),
    key: client.attachmentKey, threadKey: client.draftKey, environmentId: client.environmentId };
}

/** A picked file (Contract's `Picked`): its copy under app:/tmp/picked/, its name and the type the picker read. */
const pickedSource = (file: Obj): AttachSource => ({ path: str(file.path), name: str(file.name, 'file'), type: str(file.type) });

/**
 * The files a drop, a paste or Attach files brings (`origin` "drop", "paste" or "pick"). A drop's or a paste's folders
 * are addComposerFolders': such a folder is a handle with no path (LLP 1069.010 D1), so each is the reference's own
 * refusal for a folder whose path it cannot get. Paste as Text (`asText`, or armed from the menu) gives images no inline chip
 * (skipImageInlineChip). The answer's "focus:composer" focuses the prompt, as the reference's drop and Attach do.
 */
export async function attachIncoming(client: T3Client, native: Native, origin: string, incoming: IncomingFiles): Promise<string> {
  const sources = [...incoming.files.map(handleSource), ...incoming.picked.map(pickedSource)];
  if (!sources.length) return '';
  if (questionRefuses(client)) return '';
  // Edit › Paste as Text chosen with the pointer arms the paste in the module, as its key arms it in Contract.
  const asText = origin === 'paste' && (incoming.asText || await pasteAsTextArmed(native));
  const imageChips = imagesGetChips(client) && !asText;
  // Files finish staging (an image's compression) after the person may have moved to another draft; they stay
  // with the draft they were given to (insertAttachmentReferencesForDraft).
  const target = draftTarget(client);
  // Held from staging until the files join the draft, so an answer cannot leave between the two.
  const release = holdStaging(client, target.key);
  const imageIds = sources.map(source => classifyAttachment(source.name, source.type) === 'image' ? uuid() : '');
  setPreparing(client, target.key, [...preparingIn(client, target.key), ...sources.flatMap((source, at) => imageIds[at] ? [{ id: imageIds[at]!, name: source.name, source: '' }] : [])]);
  // ComposerImageThumbnail: each tile shows its source once a copy the window can draw exists.
  const previews = sources.map(async (source, at) => {
    const id = imageIds[at];
    if (!id) return '';
    const path = await previewSource(source, id);
    if (path && preparingIn(client, target.key).some(image => image.id === id)) {
      setPreparing(client, target.key, preparingIn(client, target.key).map(image => image.id === id ? { ...image, source: path } : image));
      await wakeShell(native);
    }
    return path;
  });
  let folders: Obj[] = [];
  try {
    const staged = await stageFiles(client, native, sources, target, imageIds);
    // A dropped or pasted folder is a workspace reference (addComposerFolders); a picked one is the reference's empty File.
    folders = origin === 'drop' || origin === 'paste' ? staged.filter(file => file.kind === 'folder') : [];
    const others = staged.filter(file => !folders.includes(file)).map(file => file.kind === 'folder' ? { kind: 'unreadable', name: file.name, sizeBytes: 0 } : file);
    if (others.length) await addAttachedFiles(client, native, others, imageChips, '', target);
  } finally {
    setPreparing(client, target.key, preparingIn(client, target.key).filter(image => !imageIds.includes(image.id)));
    release();
    for (const path of await Promise.all(previews)) await releasePreview(path);
  }
  if (folders.length) {
    const remote = folderDropTarget({ localEnvironmentDisabled: primary.disabled, environmentId: client.environmentId, primaryEnvironmentId: primary.target?.environmentId || null }) === 'remote';
    if (remote) pushToast(client, { kind: 'error', title: 'Folders can only be referenced in the local desktop environment' });
    else for (const folder of folders) pushToast(client, { kind: 'error', title: `Couldn't get the path of "${str(folder.name)}"`, description: 'Type the folder path with @ instead.' });
  }
  return origin === 'paste' ? '' : 'focus:composer';
}

/**
 * Each file staged as addAttachedFiles takes it; a draft that changes meanwhile lets them go. The draft's free slots
 * (countReservedAttachments, with the images other takes still prepare) are counted before each file is prepared: one
 * past them is `over-limit`, never staged. A folder is `folder`. `imageIds`: the id each image's preparing tile holds
 * ('' for none); an image whose tile was removed meanwhile is let go.
 */
export async function stageFiles(client: T3Client, native: Native, sources: AttachSource[], target?: DraftTarget, imageIds: string[] = []): Promise<Obj[]> {
  if (!client.projectId) throw new ClientError('Choose a project first.');
  const key = client.attachmentKey, generation = client.generation, limit = attachStagingLimit(client), staged: Obj[] = [];
  const others = target ? preparingIn(client, target.key).filter(image => !imageIds.includes(image.id)).length : 0;
  let room = MAX_ATTACHMENTS - others - (target ? reservedAttachmentsIn(client, target.key, target.threadKey) : reservedAttachments(client));
  const release = holdStaging(client, key);
  try {
    for (const [at, source] of sources.entries()) {
      const id = imageIds[at] ?? '';
      if (id && target && !preparingIn(client, target.key).some(image => image.id === id)) continue;
      const stat = await statSource(source);
      if (!stat) staged.push({ kind: 'unreadable', name: source.name, sizeBytes: 0 });
      else if (stat.folder) staged.push({ kind: 'folder', name: source.name, path: source.path });
      else if (room <= 0) staged.push({ kind: 'over-limit', name: source.name });
      else {
        const file = await stageAttachment(source, stat.size, limit, id);
        if (file.kind === 'image' || str(file.id)) room -= 1;
        if (id && target && !preparingIn(client, target.key).some(image => image.id === id)) { await releaseStaged(client, native, [file]); continue; }
        staged.push(file);
      }
    }
  } finally { release(); }
  // With a target, the files go to its draft wherever the composer is now; without one, only to the draft still open.
  return (target ? generation === client.generation : key === client.attachmentKey) ? staged : (await releaseStaged(client, native, staged), []);
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
 * images join the shelf (a question's has no image chips: its answer carries them beside it); other files become chips;
 * what cannot be attached is named in one error. `owner`: the composer
 * the chips go into at its caret (`editorInsert` refuses another thread's editor; then they join the end of the stored
 * draft).
 */
export async function addAttachedFiles(client: T3Client, native: Native, files: Obj[], imageChips = imagesGetChips(client), owner = '', target?: DraftTarget): Promise<void> {
  const identity = target ?? draftTarget(client), { key, threadKey } = identity, question = key !== threadKey;
  const current = () => identity.generation === client.generation && identity.snapshotOwner === client.snapshotOwner && identity.fieldOwner === composerFieldOwner(client) && key === client.attachmentKey;
  // A target's draft takes its files after the composer moved to another one; only a new client generation drops them.
  const kept = () => target ? identity.generation === client.generation : current();
  let error = '';
  const accepted: Obj[] = [], chips: DraftFile[] = [];
  let reserved = reservedAttachmentsIn(client, key, threadKey);
  const release = holdDraftFiles(client.local, threadKey); // a file chip's file is the thread draft's, in an answer too
  const staging = holdStaging(client, key);
  try {
    for (const file of files) {
      if (!kept()) { await releaseStaged(client, native, [file]); continue; }
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
        const staged = acceptAttachedFile(client, file, undefined, threadKey, identity.environmentId);
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
    const images = imageChips && !question ? accepted : [];
    if (current()) {
      const inserted = await insertFileChips(client, native, chips, images, owner || identity.snapshotOwner);
      if (current()) client.error = error || (!inserted ? 'The prompt changed before attachment references could be inserted. Attach the files again.' : '');
    } else if (kept() && !question) {
      // The editor now shows another draft: the chips join the end of the stored prompt of the draft that took the files.
      appendChipLinks(client, threadKey, [...chips.map(fileChipLink), ...images.map(imageChipLink)]);
    }
  } finally { staging(); release(); }
}
