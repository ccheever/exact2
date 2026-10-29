// The device's copies of attached files (composer-attach-stage.ts writes them): an image's draft beside SnapShot's,
// and a plain file's copy for its chip, read once at send and removed.
import { appFiles } from '../media/save-file';
import { forgetStagedCopies } from '../media/media-frame';
import { videoMimeType } from './composer-video';

export const DRAFTS = 'app:/data/snapshots/drafts';
export const STAGED = 'app:/data/composer-files';
/** PROVIDER_SEND_TURN_MAX_FILE_BYTES. */
const MAX_STAGED_BYTES = 50 * 1024 * 1024;

interface CopyFiles {
  stat(path: string): Promise<{ size: number }>;
  mkdir(path: string, options: { recursive: boolean }): Promise<unknown>;
  writeFile(path: string, bytes: Uint8Array): Promise<unknown>;
  readdir(path: string): Promise<string[]>;
  rm(path: string): Promise<unknown>;
}
const files = (): CopyFiles => (appFiles() as unknown as { fs: CopyFiles }).fs;
const valid = (id: string) => /^[a-f0-9-]{36}$/i.test(id);

/**
 * The staged copy's file name in composer-files/: `<id>`, or `<id>.<ext>` for a video or audio file, whose extension
 * tells AVFoundation its type when a Contract `video` or `audio` plays it (letters and digits only).
 */
export function stagedName(file: { id: string; name: string; mimeType: string }): string {
  const id = file.id.toLowerCase(), dot = file.name.lastIndexOf('.');
  const ext = dot < 0 ? '' : file.name.slice(dot + 1).toLowerCase().replace(/[^a-z0-9]/g, '');
  const media = videoMimeType(file) !== null || file.mimeType.toLowerCase().startsWith('audio/');
  return media && ext ? `${id}.${ext}` : id;
}

/** A staged file for the upload (`uploadAttachment` sends it from its path), or null when the copy is gone or empty. */
export async function stagedUpload(file: { id: string; name: string; mimeType: string }): Promise<{ bodyFrom: string; sizeBytes: number } | null> {
  if (!valid(file.id)) return null;
  const path = `${STAGED}/${stagedName(file)}`;
  try {
    const { size } = await files().stat(path);
    return size > 0 && size <= MAX_STAGED_BYTES ? { bodyFrom: path, sizeBytes: size } : null;
  } catch { return null; }
}

/** Where a folded paste's text waits for its upload, removed once it is sent (`foldUpload`). */
export const FOLD_UPLOADS = 'app:/tmp/composer-stage/folds';
let foldUploads = 0;

/**
 * A folded paste's UTF-8 bytes written to an app file for the upload to send from its path: a body inline in the
 * request is refused above 4 MiB on Apple (host/apple/src/executor_core.rs MAX_REQUEST), and base64 grows it by a third.
 */
export async function foldUpload(bytes: Uint8Array): Promise<{ bodyFrom: string; sizeBytes: number }> {
  const fs = files(), path = `${FOLD_UPLOADS}/${++foldUploads}.txt`;
  await fs.mkdir(FOLD_UPLOADS, { recursive: true });
  await fs.writeFile(path, bytes);
  return { bodyFrom: path, sizeBytes: bytes.byteLength };
}

export async function removeFoldUpload(path: string): Promise<void> {
  await files().rm(path).catch(() => undefined);
}

/**
 * A staged file's copy, under either name (an older build also left a typed link and a video poster, `drafts/<id>.png`),
 * and the copies its preview made in app:/cache (media-frame.ts).
 */
export async function removeStaged(id: string): Promise<void> {
  if (!valid(id)) return;
  const name = id.toLowerCase();
  forgetStagedCopies(name);
  let fs: CopyFiles;
  try { fs = files(); } catch { return; }
  const links = await fs.readdir(STAGED).catch(() => [] as string[]);
  for (const path of [`${STAGED}/${name}`, ...links.filter(link => link.startsWith(`${name}.`)).map(link => `${STAGED}/${link}`), `${DRAFTS}/${name}.png`]) {
    await fs.rm(path).catch(() => undefined);
  }
}
