// The device's copies of attached files (composer-attach-stage.ts writes them): an image's draft beside SnapShot's,
// and a plain file's copy for its chip, read once at send and removed.
import { appFiles } from '../media/save-file';
import { toBase64 } from '../browser/browser-import-io';
import { videoMimeType } from './composer-video';

export const DRAFTS = 'app:/data/snapshots/drafts';
export const STAGED = 'app:/data/composer-files';
/** PROVIDER_SEND_TURN_MAX_FILE_BYTES. */
const MAX_STAGED_BYTES = 50 * 1024 * 1024;

interface CopyFiles {
  readFile(path: string): Promise<ArrayBuffer>;
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

/** A staged file's bytes as base64 for the upload (`uploadAttachment`), or null when the copy is gone or empty. */
export async function readStaged(file: { id: string; name: string; mimeType: string }): Promise<string | null> {
  if (!valid(file.id)) return null;
  try {
    const bytes = new Uint8Array(await files().readFile(`${STAGED}/${stagedName(file)}`));
    return bytes.byteLength > 0 && bytes.byteLength <= MAX_STAGED_BYTES ? toBase64(bytes) : null;
  } catch { return null; }
}

/** A staged file's copy, under either name (an older build also left a typed link and a video poster, `drafts/<id>.png`). */
export async function removeStaged(id: string): Promise<void> {
  if (!valid(id)) return;
  const name = id.toLowerCase();
  let fs: CopyFiles;
  try { fs = files(); } catch { return; }
  const links = await fs.readdir(STAGED).catch(() => [] as string[]);
  for (const path of [`${STAGED}/${name}`, ...links.filter(link => link.startsWith(`${name}.`)).map(link => `${STAGED}/${link}`), `${DRAFTS}/${name}.png`]) {
    await fs.rm(path).catch(() => undefined);
  }
}
