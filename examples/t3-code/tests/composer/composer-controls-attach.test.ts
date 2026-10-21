// Lane composer-controls: Attach files (ChatComposer showComposerAttachAction, the hidden file input → addComposerAttachments).
import { describe, expect, test } from 'bun:test';
import { obj } from '../../src/client/domain';
import { snapshot } from '../../src/client/presentation';
import { observeComposer, acceptComposerEdit } from '../support/composer-field-fixture';
import { connected, opened } from '../support/composer-controls-fixture';
import { toasts } from '../../src/shell/toast';
import { memoryFiles, png, text } from '../support/app-files-fixture';
import { STAGED } from '../../src/composer/composer-attach-stage';

/** app.ts's `composerAttach` answer for Attach files' picks (app.contract attachFiles). */
const picks = (files: { path: string; name: string; type: string; size: number }[]) => JSON.stringify({ files: [], picked: files, asText: false, target: '' });

describe('attach files', () => {
  test('offered when the server stages uploads; picked images join the shelf; other files become file chips', async () => {
    const { client, native, command } = await connected();
    expect(snapshot(client, 0).composer.attach).toBe(false);
    Object.assign(obj(native.config.environment).capabilities as object, { attachmentUploads: true, fileAttachments: { maxUploadBytes: 26214400 } });
    client.config.environment = native.config.environment;
    expect(snapshot(client, 0).composer.attach).toBe(true);
    const { store } = memoryFiles({ 'app:/tmp/picked/1/shot.png': png(4, 3), 'app:/tmp/picked/2/notes.txt': text('ten bytes!'), 'app:/tmp/picked/3/scan.tiff': text('II*') });
    observeComposer(client, client.draft);
    const later = native.later.bind(native);
    native.later = async request => {
      if (obj(request).op === 'editorInsert') return { ok: true, generation: client.generation, value: { applied: acceptComposerEdit(client, request) } };
      return later(request);
    };
    const changed = await command('cclocal:attach-files', 'pick', picks([{ path: 'app:/tmp/picked/1/shot.png', name: 'shot.png', type: 'image/png', size: 33 },
      { path: 'app:/tmp/picked/2/notes.txt', name: 'notes.txt', type: 'text/plain', size: 10 }]));
    // The reference's Attach focuses the prompt after the files land (app.contract filesAddedDone).
    expect(changed.message).toBe('focus:composer');
    expect(client.snapshotDrafts).toMatchObject([{ name: 'shot.png', mimeType: 'image/png', sizeBytes: 33 }]);
    // composer-editor-attach.ts: a plain file is inserted through the accepted shared field, its copy staged.
    const [copy] = [...store.keys()].filter(path => path.startsWith(`${STAGED}/`));
    expect(client.error).toBe('');
    expect(client.draft).toBe(`[notes.txt](t3-context://v1/file/file_${copy!.slice(STAGED.length + 1)}) `);
    expect(snapshot(client, 0).snapshotDrafts.map(tile => tile.id)).toEqual([String(client.snapshotDrafts[0]!.id)]);
    await command('cclocal:attach-files', 'pick', picks([{ path: 'app:/tmp/picked/3/scan.tiff', name: 'scan.tiff', type: 'image/tiff', size: 3 }]));
    expect(client.error).toBe("'scan.tiff' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.");
    await command('cclocal:attach-files', 'pick', picks([]));
    expect(client.snapshotDrafts.length).toBe(1);
  });
  test('a pending question that takes no attachments refuses them (addComposerAttachments)', async () => {
    const { client, command } = await opened();
    client.projection.runtimeRequests = [{ id: 'q', kind: 'user_input', status: 'pending', responseCapability: { type: 'live' } }];
    client.projection.turnItems = [{ id: 'q-item', type: 'user_input_request', requestId: 'q', questions: [{ id: 'one', question: 'Pick one', allowCustomAnswer: false, options: [{ label: 'A' }] }] }];
    const { store } = memoryFiles({ 'app:/tmp/picked/1/notes.txt': text('notes') });
    await command('cclocal:attach-files', 'pick', picks([{ path: 'app:/tmp/picked/1/notes.txt', name: 'notes.txt', type: 'text/plain', size: 5 }]));
    expect(toasts(client).map(toast => toast.title)).toEqual(['This question cannot accept attachments.']);
    expect([...store.keys()].filter(path => path.startsWith(`${STAGED}/`))).toEqual([]);
  });
});
