// An image draft's upload body (client.ts uploadSnapshots): the host sends the draft's file (`exactBodyFrom`), so its
// bytes never enter JavaScript, whose base64 copies of a 10 MB image would not fit the module's heap. A draft is
// already fitted: a capture as it is saved (`snapshotDraftSave` with `imageMaxEdge`), an attached image as it is
// staged (composer-attach-stage.ts).
import { appFiles } from '../media/save-file';
import { DRAFTS } from '../composer/composer-staged-copies';

interface UploadFiles { stat(path: string): Promise<{ size: number }> }

/** The draft `id` as an upload body; rejects when its file is gone. */
export async function draftUpload(id: string): Promise<{ bodyFrom: string; sizeBytes: number }> {
  const draft = `${DRAFTS}/${id}.png`;
  const { size } = await (appFiles() as unknown as { fs: UploadFiles }).fs.stat(draft);
  return { bodyFrom: draft, sizeBytes: size };
}
