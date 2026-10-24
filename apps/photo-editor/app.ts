// The photo editor's data (LLP 1133.019 §11). An edit is saved to Photos as
// a new photo: the photo transformed by the edit into app:/tmp/edits/, then
// added to the library. The recent photos are the library's newest, read
// only once the person asks to see them or the read is granted, since the
// first read while `prompt` asks (D7).
import type { Answer, Storage } from './app.contract.d.ts';
import { addToPhotoLibrary, photoAssets } from '@exact/photos';

export const appId = 'com.exact.photoeditor';
const EDITS = 'app:/tmp/edits';
export const grants = `device.camera cameraPurpose\ndevice.photos-add savePurpose\ndevice.photos recentsPurpose\nfs.read app:/tmp\nfs.write app:/tmp`;

/** The edit in the photo's terms, as `<photo-editor>` reports it. */
type Edit = { rotate?: number; crop?: { x: number; y: number; width: number; height: number } };

const failed = (code: string, message: string) => ({ saved: false, asset: '', error: code, message, metadata: '' });

const sources: Record<string, (args: any, store: unknown, storage: Storage) => Promise<unknown>> = {
  savePhoto: async ([photo, edit]: [string, string], _store, storage) => {
    let edits: string[] = [];
    try {
      edits = await storage.fs.readdir(EDITS);
    } catch {
      // No edit saved yet: the first makes the directory.
    }
    await storage.fs.mkdir(EDITS);
    let e: Edit = {};
    try {
      e = JSON.parse(edit || '{}') as Edit;
    } catch {
      // An edit the editor has not reported yet is the photo as it is.
    }
    let made;
    try {
      made = await storage.fs.transformImage(photo, `${EDITS}/${edits.length + 1}`, { rotate: e.rotate ?? 0, crop: e.crop });
    } catch (err: any) {
      return failed(err?.code ?? 'UnknownError', `Couldn’t make the edited photo (${err?.code ?? err?.message ?? 'error'}).`);
    }
    try {
      const added = await addToPhotoLibrary({ path: made.path });
      return { saved: true, asset: added.asset ?? '', error: '', message: 'Saved to Photos', metadata: made.metadata };
    } catch (err: any) {
      const code = err?.code ?? err?.name ?? 'UnknownError';
      return failed(code, code === 'NotAllowedError' ? 'Photo Editor can’t add to your photos. Turn it on in Settings.' : `Couldn’t save to Photos (${code}).`);
    }
  },
  // A read refused (denied, or a selection that left nothing) shows no strip, not the last one.
  recents: async ([show]: [boolean]) => {
    if (!show) return { assets: [] };
    try {
      return await photoAssets({ limit: 24 });
    } catch {
      return { assets: [] };
    }
  },
};

export const answer = ((source: string, args: unknown, store: unknown, storage: Storage) => sources[source](args, store, storage)) as unknown as Answer;
