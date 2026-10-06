// Lane composer-controls: Attach files (ChatComposer showComposerAttachAction, addComposerAttachments).
import { describe, expect, test } from 'bun:test';
import { obj } from './domain';
import { snapshot } from './presentation';
import { connected } from './composer-controls-fixture';

const id = (n: number) => `00000000-0000-4000-8000-00000000000${n}`;
describe('attach files', () => {
  test('offered when the server stages uploads; picked images join the shelf; other files become file chips', async () => {
    const { client, native, command } = await connected();
    expect(snapshot(client, 0).composer.attach).toBe(false);
    Object.assign(obj(native.config.environment).capabilities as object, { attachmentUploads: true, fileAttachments: { maxUploadBytes: 26214400 } });
    client.config.environment = native.config.environment;
    expect(snapshot(client, 0).composer.attach).toBe(true);
    native.picked = [{ kind: 'image', id: id(1), name: 'shot.png', sizeBytes: 2048 }, { kind: 'file', id: id(2), name: 'notes.txt', mimeType: 'text/plain', sizeBytes: 10 }];
    await command('cclocal:attach');
    expect(client.snapshotDrafts).toEqual([{ id: id(1), name: 'shot.png', mimeType: 'image/png', sizeBytes: 2048 }]);
    // composer-editor-attach.ts: a plain file is an inline file chip (with no editor here it joins the stored draft).
    expect(client.error).toBe('');
    expect(client.draft).toBe(`[notes.txt](t3-context://v1/file/file_${id(2)}) `);
    expect(snapshot(client, 0).snapshotDrafts.map(tile => tile.id)).toEqual([id(1)]);
    native.picked = [{ kind: 'unsupported-image', name: 'scan.tiff', sizeBytes: 10 }];
    await command('cclocal:attach');
    expect(client.error).toBe("'scan.tiff' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.");
    native.picked = [];
    await command('cclocal:attach');
    expect(client.error).toBe('');
    expect(client.snapshotDrafts.length).toBe(1);
  });
});
